import Lake.CLI.Main

open Lean Lake

unsafe def main (args : List String) : IO UInt32 := do
  let some root ← IO.getEnv "LEAN_SYSROOT" | throw <| IO.userError "LEAN_SYSROOT is required"
  let lean ← LeanInstall.get root
  let opts : LakeOptions := {
    rootDir := "."
    updateToolchain := false
    noCache := true
    leanInstall? := some lean
    lakeInstall? := some (LakeInstall.ofLean lean) }
  let config ← (opts.mkLoadConfig).toIO (fun e => IO.userError (toString e))
  let some ws ← (loadWorkspace config).toBaseIO
    | throw <| IO.userError "cannot load Lake workspace"
  let mut rows := #[]
  for path in args do
    let path ← IO.FS.realPath path
    let row ← if let some mod := ws.findModuleBySrc? path then
        pure <| Json.mkObj [
          ("Path", toJson path.toString), ("Module", toJson mod.name.toString),
          ("Registered", toJson true),
          ("Arguments", toJson (mod.weakLeanArgs ++ mod.leanArgs)),
          ("Options", toJson mod.leanOptions)]
      else do
        let name ← moduleNameOfFileName path (some ws.root.dir)
        pure <| Json.mkObj [
          ("Path", toJson path.toString), ("Module", toJson name.toString),
          ("Registered", toJson false),
          ("Arguments", toJson (ws.root.weakLeanArgs ++ ws.root.moreLeanArgs)),
          ("Options", toJson ws.root.leanOptions)]
    rows := rows.push row
  let mut environment := #[]
  for name in #["LEAN_PATH", "LEAN_SRC_PATH"] do
    let some value ← IO.getEnv name | throw <| IO.userError s!"{name} is required"
    environment := environment.push (name, toJson value)
  IO.println (Json.mkObj [("Files", Json.arr rows),
    ("Environment", Json.mkObj environment.toList)]).compress
  return 0
