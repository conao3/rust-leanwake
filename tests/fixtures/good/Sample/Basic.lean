import Lean

namespace Sample

def answer : Nat := 42

theorem answer_eq : answer = 42 := rfl

structure Box where
  value : Nat

instance : Inhabited Box := ⟨⟨0⟩⟩

mutual
  def even : Nat → Bool
    | 0 => true
    | n + 1 => odd n
  def odd : Nat → Bool
    | 0 => false
    | n + 1 => even n
end

example : answer = 42 := rfl
example : answer = 42 := rfl

private def hidden : Nat := 1

def café : Nat := hidden

open Nat

#eval IO.println "fixture output"

end Sample
