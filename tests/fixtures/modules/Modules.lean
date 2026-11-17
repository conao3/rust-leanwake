module

public import Modules.Base

public section

theorem yearStart_decomp (c q r : Int) (hq : 0 ≤ q ∧ q ≤ 24) (hr : 0 ≤ r ∧ r ≤ 3) :
    yearStart (100 * c + 4 * q + r) = 36524 * c + 1461 * q + 365 * r := by
  have h4 : (100 * c + 4 * q + r) / 4 = 25 * c + q := by omega
  have h100 : (100 * c + 4 * q + r) / 100 = c := by omega
  simp only [yearStart, h4, h100]
  omega
