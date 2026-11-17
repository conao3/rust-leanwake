import Lean
open Lean Elab Command
elab "force_async" c:command : command =>
  withScope (fun s => { s with opts := s.opts.setBool `Elab.async true }) do
    elabCommand c
force_async theorem escaped : (List.range 500).length = 500 := by decide +kernel
