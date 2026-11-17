import Sample.Basic

open Lean Elab Command

elab "check_configuration" : command => do
  let opts ← getOptions
  unless opts.getBool `autoImplicit true == false do
    throwError "autoImplicit was not forwarded"
  unless maxRecDepth.get opts == 1234 do
    throwError "weakLeanArgs was not forwarded"
  unless opts.getBool `linter.unusedVariables true == false do
    throwError "moreLeanArgs was not forwarded"

set_option Elab.async true

check_configuration

namespace Sample

theorem heavy : (List.range 500).length = 500 := by decide +kernel

set_option Elab.async true in
 theorem nested : (List.range 700).length = 700 := by decide +kernel

end Sample
