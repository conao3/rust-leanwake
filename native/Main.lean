import Lean
import Sha256

open Lean Elab Frontend

structure Request where
  path : String
  setup : ModuleSetup
  arguments : Array String
  deriving FromJson

def positionJson (p : Position) : Json :=
  Json.mkObj [("Line", toJson p.line), ("Column", toJson p.column)]

def forceSynchronous (env : Environment) : Environment :=
  Command.commandElabAttribute.ext.modifyState env fun s =>
    { s with table := s.table.fold (init := {}) fun table kind entries =>
      table.insert kind (entries.map fun entry =>
        if kind == ``Parser.Command.set_option then
          { entry with value := fun stx => do
            entry.value stx
            Command.modifyScope fun scope =>
              { scope with opts := scope.opts.setBool `Elab.async false } }
        else entry) }

def collectNames (before after : Environment) : Array String := Id.run do
  let mut names : Array (Name × DeclarationRanges) := #[]
  for (name, _) in after.constants.map₂.toList do
    if !before.contains name then
      if let some range := declRangeExt.find? (level := .exported) after name then
        names := names.push (name, range)
  names := names.qsort fun a b =>
    if a.2.range.pos == b.2.range.pos then a.1.toString < b.1.toString
    else a.2.range.pos.line < b.2.range.pos.line || (a.2.range.pos.line == b.2.range.pos.line && a.2.range.pos.column < b.2.range.pos.column)
  return names.map (·.1.toString)

def measureCommands (path input : String) : FrontendM (Array Json × Bool) := do
  let mut rows := #[]
  let mut failed := false
  let mut occurrences : Std.HashMap (String × String) Nat := {}
  repeat
    modify fun s => { s with commandState := { s.commandState with
      messages := {}, snapshotTasks := #[], traceState := {},
      scopes := s.commandState.scopes.map fun scope =>
        { scope with opts := scope.opts.setBool `Elab.async false } } }
    updateCmdPos
    let cs ← getCommandState
    let ictx ← getInputContext
    let ps ← getParserState
    let scope := cs.scopes.head!
    let pmctx : Parser.ParserModuleContext := { env := cs.env, options := scope.opts, currNamespace := scope.currNamespace, openDecls := scope.openDecls }
    let (cmd, ps, messages) := Parser.parseCommand ictx pmctx ps cs.messages
    setParserState ps
    setMessages messages
    let h0 ← IO.getNumHeartbeats
    let t0 ← IO.monoNanosNow
    elabCommandAtFrontend cmd
    let t1 ← IO.monoNanosNow
    let h1 ← IO.getNumHeartbeats
    let state ← getCommandState
    let mut diagnostics := messages ++ state.messages
    for task in state.snapshotTasks do
      for snapshot in task.get.getAll do
        diagnostics := diagnostics ++ snapshot.diagnostics.msgLog
    let hasErrors := diagnostics.hasErrors
    failed := failed || hasErrors
    for message in diagnostics.toList do IO.eprintln (← message.toString)
    if cmd.isOfKind ``Parser.Command.eoi then break
    let some start := cmd.getPos? | throw <| IO.userError "command has no start position"
    let some stop := cmd.getTailPos? | throw <| IO.userError "command has no end position"
    let source := String.Pos.Raw.extract input start stop
    let names := collectNames cs.env state.env
    let kind := cmd.getKind.toString
    let occurrence := occurrences.getD (kind, source) 0 + 1
    occurrences := occurrences.insert (kind, source) occurrence
    let key := if let some name := names[0]? then name else
      s!"{path}::{kind.splitOn "." |>.getLast!}:{(Leanwake.sha256 source).take 12}#{occurrence}"
    rows := rows.push <| Json.mkObj [
      ("Key", toJson key), ("Kind", toJson kind), ("Names", toJson names),
      ("Range", Json.mkObj [("Start", positionJson (ictx.fileMap.toPosition start)),
        ("End", positionJson (ictx.fileMap.toPosition stop))]),
      ("Heartbeats", toJson (h1 - h0)), ("ElapsedNs", toJson (t1 - t0)),
      ("HasErrors", toJson hasErrors)]
    if Parser.isTerminalCommand cmd then break
  return (rows, failed)

def setConfigOption (opts : Options) (arg : String) : IO Options := do
  let parts := arg.splitOn "="
  let name := parts.head!.toName
  if parts.length < 2 then throw <| IO.userError s!"invalid Lean option: {arg}"
  let value := String.intercalate "=" parts.tail!
  if let some decl := (← getOptionDecls).find? name then
    Language.Lean.setOption opts decl name value
  else
    pure (opts.set name value)

unsafe def profile (req : Request) : IO Json := do
  let some sysroot ← IO.getEnv "LEAN_SYSROOT" | throw <| IO.userError "LEAN_SYSROOT is required"
  initSearchPath sysroot
  enableInitializersExecution
  let input ← IO.FS.readFile req.path
  let ictx := Parser.mkInputContext input req.path
  let mut opts := Options.empty
  for arg in req.arguments do
    if arg.startsWith "-D" then
      opts ← setConfigOption opts (arg.drop 2).copy
    else
      throw <| IO.userError s!"unsupported Lake Lean argument: {arg}"
  opts := opts.mergeBy (fun _ _ value => value) req.setup.options.toOptions
  opts := opts.setBool `Elab.async false
  let (header, ps, messages) ← Parser.parseHeader ictx
  let header : HeaderSyntax := header
  let start ← IO.monoNanosNow
  req.setup.dynlibs.forM Lean.loadDynlib
  let (env, messages) ← processHeaderCore header.startPos
    (req.setup.imports?.getD header.imports) (req.setup.isModule || header.isModule)
    opts messages ictx (plugins := req.setup.plugins)
    (mainModule := req.setup.name) (package? := req.setup.package?)
    (arts := req.setup.importArts) (headerStx? := header)
  let stop ← IO.monoNanosNow
  let (commands, failed) ← if messages.hasErrors then do
      for message in messages.toList do IO.eprintln (← message.toString)
      pure (#[], true)
    else do
      let (result, _) ← (measureCommands req.path input).run { inputCtx := ictx } |>.run {
        commandState := Command.mkState (forceSynchronous env) messages opts,
        parserState := ps, cmdPos := ps.pos }
      pure result
  return Json.mkObj [("Path", toJson req.path), ("Module", toJson req.setup.name.toString),
    ("SourceSha256", toJson (Leanwake.sha256 input)), ("HasErrors", toJson failed),
    ("ImportElapsedNs", toJson (stop - start)), ("Commands", toJson commands)]

unsafe def main : IO UInt32 := do
  let stdout ← IO.getStdout
  let line ← (← IO.getStdin).getLine
  let req ← IO.ofExcept (Json.parse line >>= fromJson? (α := Request))
  let result ← IO.withStdout (← IO.getStderr) (profile req)
  stdout.putStrLn result.compress
  return 0
