import Config
open Lean Elab Command
run_cmd do
  let opts ← getOptions
  unless maxRecDepth.get opts == 1234 do throwError "wrong maxRecDepth"
  unless probe.enabled.get opts do throwError "wrong Bool option"
  unless probe.count.get opts == 42 do throwError "wrong Nat option"
  unless probe.label.get opts == "hello=world" do throwError "wrong String option"
theorem configured : (List.range 500).length = 500 := by decide +kernel
