import Lean
import Sha256

open Lean Elab Frontend

structure Request where
  path : String
  setup : ModuleSetup
  arguments : Array String
  detail : String
  stepThresholdNs : Nat
  captureThresholdMs : Nat
  traceStarts : Option (Array (Nat × Nat))
  deriving FromJson

def positionJson (p : Position) : Json :=
  Json.mkObj [("Line", toJson p.line), ("Column", toJson p.column)]

structure Tracing where
  traced : Bool
  selective : Bool
  captureMs : Nat
  deriving Inhabited

def measurementOptions (tracing : Tracing) (opts : Options) : Options :=
  let opts := opts.setBool `Elab.async false
  if tracing.traced then
    opts.setBool `trace.profiler true |>.set `trace.profiler.threshold tracing.captureMs
      |>.setBool `trace.profiler.useHeartbeats false |>.set `trace.profiler.output ""
      |>.setBool `trace.Leanwake.tactic true
  else if tracing.selective then
    opts.setBool `trace.profiler false |>.setBool `trace.Leanwake.tactic false
  else opts

initialize currentTracing : IO.Ref Tracing ←
  IO.mkRef { traced := false, selective := false, captureMs := 0 }

initialize wrappedCommands : SimpleScopedEnvExtension Name (PHashSet Name) ←
  registerSimpleScopedEnvExtension { initial := {}, addEntry := (·.insert ·) }

def forceSynchronous (env : Environment) : Environment := Id.run do
  let wrapped := wrappedCommands.getState env
  let mut names := wrapped
  let state := Command.commandElabAttribute.ext.getState env
  for entry in state.table.fold (init := []) (fun entries _ es => es ++ entries) do
    names := names.insert entry.declName
  let env := Command.commandElabAttribute.ext.modifyState env fun s =>
    { s with table := s.table.fold (init := {}) fun table kind entries =>
      table.insert kind (entries.map fun entry =>
        if wrapped.contains entry.declName then entry else
          { entry with value := fun stx => do
            let tracing ← currentTracing.get
            Command.modifyScope fun scope =>
              { scope with opts := measurementOptions tracing scope.opts }
            try entry.value stx
            finally
              Command.modifyScope fun scope =>
                { scope with opts := measurementOptions tracing scope.opts } }) }
  return wrappedCommands.modifyState env fun _ => names

def settleSnapshots (tasks : Array (Language.SnapshotTask Language.SnapshotTree)) : IO Unit := do
  let deadline := (← IO.monoNanosNow) + 1000000000
  let mut pending := tasks
  while !pending.isEmpty do
    let task := pending.back!
    pending := pending.pop
    while !(← IO.hasFinished task.task) do
      if (← IO.monoNanosNow) ≥ deadline then return
      IO.sleep 1
    pending := pending ++ task.get.children

def finishedSnapshots (tasks : Array (Language.SnapshotTask Language.SnapshotTree)) :
    IO (Array Language.Snapshot × Bool) := do
  let mut pending := tasks.reverse
  let mut snapshots := #[]
  let mut unfinished := false
  while !pending.isEmpty do
    let task := pending.back!
    pending := pending.pop
    if ← IO.hasFinished task.task then
      let tree := task.get
      snapshots := snapshots.push tree.element
      pending := pending ++ tree.children.reverse
    else
      unfinished := true
      if let some token := task.cancelTk? then token.set
  return (snapshots, unfinished)

def wrapSourceTactic (original : Tactic.Tactic) : Tactic.Tactic := fun stx =>
  withTraceNode `Leanwake.tactic
    (fun _ => pure (.ofOriginatingSyntax stx (MessageData.ofSyntax stx)))
    (original stx) (tag := stx.getKind.toString)

initialize wrappedTactics : SimpleScopedEnvExtension Name (PHashSet Name) ←
  registerSimpleScopedEnvExtension { initial := {}, addEntry := (·.insert ·) }

def instrumentTactics (env : Environment) : Environment := Id.run do
  let wrapped := wrappedTactics.getState env
  let mut names := wrapped
  let state := Tactic.tacticElabAttribute.ext.getState env
  for entry in state.table.fold (init := []) (fun entries _ es => es ++ entries) do
    names := names.insert entry.declName
  let env := Tactic.tacticElabAttribute.ext.modifyState env fun s =>
    { s with table := s.table.fold (init := {}) fun table kind entries =>
      table.insert kind (entries.map fun entry =>
        if wrapped.contains entry.declName then entry else
          { entry with value := wrapSourceTactic entry.value }) }
  return wrappedTactics.modifyState env fun _ => names

def syntaxRange (fm : FileMap) (stx : Syntax) : Json :=
  match stx.getPos?, stx.getTailPos? with
  | some start, some stop => Json.mkObj [
      ("Start", positionJson (fm.toPosition start)),
      ("End", positionJson (fm.toPosition stop))]
  | _, _ => Json.null

partial def originatingSyntax : MessageData → Option Syntax
  | .ofOriginatingSyntax stx _ => some stx
  | .withContext _ msg | .withNamingContext _ msg | .nest _ msg | .group msg
    | .tagged _ msg | .ofWidget _ msg => originatingSyntax msg
  | _ => none

partial def traceJson (fm : FileMap) (threshold : Nat) (origin : Option Syntax) :
    MessageData → Array Json
  | .trace data msg children => Id.run do
    let children := children.flatMap (traceJson fm threshold none)
    let elapsed := ((data.stopTime - data.startTime) * 1000000000).round.toUInt64.toNat
    if elapsed < threshold && data.cls != `Leanwake.tactic then return children
    let stx := (originatingSyntax msg).or origin
    let fields := [
      ("Class", toJson data.cls.toString), ("ElapsedNs", toJson elapsed),
      ("Range", stx.map (syntaxRange fm) |>.getD Json.null),
      ("Children", toJson children)]
    let fields := if data.cls == `Leanwake.tactic then
      fields ++ [("Kind", toJson data.tag)] else fields
    return #[Json.mkObj fields]
  | .ofOriginatingSyntax stx msg => traceJson fm threshold (some stx) msg
  | .withContext _ msg | .withNamingContext _ msg | .nest _ msg | .group msg
    | .tagged _ msg | .ofWidget _ msg => traceJson fm threshold origin msg
  | .compose a b => traceJson fm threshold origin a ++ traceJson fm threshold origin b
  | _ => #[]

def stateSteps (fm : FileMap) (threshold : Nat) (state : TraceState) : Array Json :=
  state.traces.toArray.flatMap fun trace => traceJson fm threshold (some trace.ref) trace.msg

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

def measureCommands (path input : String) (steps : Bool) (threshold : Nat) (captureMs : Nat)
    (targets : Option (Array (Nat × Nat))) : FrontendM (Array Json × Bool) := do
  let mut rows := #[]
  let mut failed := false
  let mut remaining := targets.map (·.size) |>.getD 0
  let mut occurrences : Std.HashMap (String × String) Nat := {}
  repeat
    modify fun s => { s with commandState := { s.commandState with
      env := forceSynchronous s.commandState.env } }
    if steps then
      modify fun s => { s with commandState := { s.commandState with
        env := instrumentTactics s.commandState.env } }
    modify fun s => { s with commandState := { s.commandState with
      messages := {}, snapshotTasks := #[], traceState := {} } }
    updateCmdPos
    let cs ← getCommandState
    let ictx ← getInputContext
    let ps ← getParserState
    let scope := cs.scopes.head!
    let pmctx : Parser.ParserModuleContext := { env := cs.env, options := scope.opts, currNamespace := scope.currNamespace, openDecls := scope.openDecls }
    let (cmd, ps, messages) := Parser.parseCommand ictx pmctx ps cs.messages
    setParserState ps
    setMessages messages
    let position := ictx.fileMap.toPosition (cmd.getPos?.getD ps.pos)
    let traced := steps && targets.all (·.contains (position.line, position.column))
    let tracing : Tracing := { traced, selective := targets.isSome, captureMs }
    currentTracing.set tracing
    modify fun s => { s with commandState := { s.commandState with
      scopes := s.commandState.scopes.map fun scope =>
        { scope with opts := measurementOptions tracing scope.opts } } }
    let previousCommands := (← get).commands
    modify fun s => { s with commands := s.commands.push cmd }
    let h0 ← IO.getNumHeartbeats
    let t0 ← IO.monoNanosNow
    runCommandElabM <| Command.elabCommandTopLevel cmd previousCommands
    let state ← getCommandState
    discard <| IO.wait state.env.checked
    settleSnapshots state.snapshotTasks
    let t1 ← IO.monoNanosNow
    let h1 ← IO.getNumHeartbeats
    let (snapshots, unfinished) ← finishedSnapshots state.snapshotTasks
    let mut diagnostics := messages ++ state.messages
    let mut traces := if traced then stateSteps ictx.fileMap threshold state.traceState else #[]
    for snapshot in snapshots do
      diagnostics := diagnostics ++ snapshot.diagnostics.msgLog
      if traced then traces := traces ++ stateSteps ictx.fileMap threshold snapshot.traces
    if unfinished then
      IO.eprintln s!"{path}:{position}: unfinished elaboration task; synchronous measurement cannot be guaranteed"
    let hasErrors := diagnostics.hasErrors || unfinished
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
    let fields := [
      ("Key", toJson key), ("Kind", toJson kind), ("Names", toJson names),
      ("Range", Json.mkObj [("Start", positionJson (ictx.fileMap.toPosition start)),
        ("End", positionJson (ictx.fileMap.toPosition stop))]),
      ("Heartbeats", toJson (h1 - h0)), ("ElapsedNs", toJson (t1 - t0)),
      ("HasErrors", toJson hasErrors)]
    if traced then
      rows := rows.push <| Json.mkObj (fields ++ [("Steps", toJson traces)])
      remaining := remaining - 1
    else if targets.isNone then
      rows := rows.push <| Json.mkObj fields
    if unfinished || Parser.isTerminalCommand cmd || (targets.isSome && remaining == 0) then break
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
  unless req.detail == "none" || req.detail == "steps" do
    throw <| IO.userError s!"unsupported detail: {req.detail}"
  let steps := req.detail == "steps"
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
  opts ← Language.Lean.reparseOptions opts
  if req.traceStarts.isSome && !steps then
    throw <| IO.userError "traceStarts requires detail steps"
  let tracing : Tracing := {
    traced := steps && req.traceStarts.isNone
    selective := req.traceStarts.isSome
    captureMs := req.captureThresholdMs }
  opts := measurementOptions tracing opts
  let (commands, failed) ← if messages.hasErrors then do
      for message in messages.toList do IO.eprintln (← message.toString)
      pure (#[], true)
    else do
      let (result, _) ← (measureCommands req.path input steps req.stepThresholdNs
        req.captureThresholdMs req.traceStarts).run
        { inputCtx := ictx } |>.run {
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
  stdout.flush
  IO.Process.exit 0
