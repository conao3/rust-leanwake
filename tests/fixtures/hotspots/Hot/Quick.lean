def double (n : Nat) : Nat := 2 * n

theorem double_zero : double 0 = 0 := rfl

theorem add_zero' (n : Nat) : n + 0 = n := by
  simp
