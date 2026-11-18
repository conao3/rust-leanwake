import Lean

theorem repeated (n : Nat) : (n + 0 = n) ∧ (n + 0 = n) := by
  constructor
  · simp only [Nat.add_zero]
  · simp only [Nat.add_zero]

theorem heavy : (List.range 100).length = 100 := by
  decide +kernel

set_option trace.profiler.useHeartbeats true in
 theorem configured (n : Nat) : n + 0 = n := by
  simp only [Nat.add_zero]

set_option trace.profiler false

theorem afterOption : (List.range 80).length = 80 := by
  decide +kernel

open Lean Elab Tactic

elab "localClose" : tactic => do evalTactic (← `(tactic| rfl))

theorem locallyDefined : 1 = 1 := by
  localClose

theorem simplifyMany (xs : List Nat) :
    ((xs ++ []).map (fun x => x + 0)).length = xs.length := by
  simp

def wrap (n : Nat) := n

theorem wrap_eq (n : Nat) (_h : n = n) : wrap n = n := rfl

theorem simplifyConditional (n : Nat) : wrap n = n := by
  simp only [wrap_eq]
