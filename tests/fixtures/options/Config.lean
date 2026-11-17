import Lean
register_option probe.enabled : Bool := { defValue := false }
register_option probe.count : Nat := { defValue := 0 }
register_option probe.label : String := { defValue := "" }
