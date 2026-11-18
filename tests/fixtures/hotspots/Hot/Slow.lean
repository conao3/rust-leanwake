theorem small (n : Nat) : 0 + n = n := by
  simp

theorem slow : (List.range 8000).length = 8000 ∧ 2 + 2 = 4 := by
  constructor
  · decide +kernel
  · simp

theorem termMode : (List.range 60).length = 60 := of_decide_eq_true rfl
