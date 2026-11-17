leanfran::lean! {
    #[lean(imports = ["Std"])]
    pub mod core;
    r#"
    inductive Status where
      | regressed | improved | unchanged | added | removed
      deriving DecidableEq, Repr

    def grows (base head absolute relative : Nat) : Bool :=
      decide (absolute < head - base ∧ base * relative < (head - base) * 100)

    def classify (base head absolute relative : Nat) : Status :=
      if grows base head absolute relative then .regressed
      else if grows head base absolute relative then .improved
      else .unchanged

    def reverseStatus : Status → Status
      | .regressed => .improved
      | .improved => .regressed
      | .unchanged => .unchanged
      | .added => .removed
      | .removed => .added

    theorem grows_lt {b h a r : Nat} (hyp : grows b h a r = true) : b < h := by
      simp only [grows, decide_eq_true_eq] at hyp
      omega

    theorem grows_asymmetric {b h a r : Nat} (hyp : grows b h a r = true) :
        grows h b a r = false := by
      have lt := grows_lt hyp
      simp [grows, Nat.sub_eq_zero_of_le (Nat.le_of_lt lt)]

    theorem classify_regressed (b h a r : Nat) :
        classify b h a r = .regressed ↔ grows b h a r = true := by
      unfold classify
      split <;> simp_all
      split <;> simp_all

    theorem classify_improved (b h a r : Nat) :
        classify b h a r = .improved ↔ grows h b a r = true := by
      unfold classify
      split
      · have := grows_asymmetric ‹grows b h a r = true›
        simp_all
      · split <;> simp_all

    theorem equal_unchanged (b a r : Nat) : classify b b a r = .unchanged := by
      simp [classify, grows]

    theorem swap_classification (b h a r : Nat) :
        classify h b a r = reverseStatus (classify b h a r) := by
      by_cases forward : grows b h a r = true
      · have backward := grows_asymmetric forward
        simp [classify, forward, backward, reverseStatus]
      · by_cases backward : grows h b a r = true
        · simp [classify, forward, backward, reverseStatus]
        · simp [classify, forward, backward, reverseStatus]

    theorem regression_monotone {b h h' a r : Nat}
        (hyp : classify b h a r = .regressed) (increase : h ≤ h') :
        classify b h' a r = .regressed := by
      rw [classify_regressed] at hyp ⊢
      simp only [grows, decide_eq_true_eq] at hyp ⊢
      have mul := Nat.mul_le_mul_right 100 (Nat.sub_le_sub_right increase b)
      omega

    theorem grows_thresholds {b h a a' r r' : Nat}
        (ha : a ≤ a') (hr : r ≤ r') (hyp : grows b h a' r' = true) :
        grows b h a r = true := by
      simp only [grows, decide_eq_true_eq] at hyp ⊢
      have mul := Nat.mul_le_mul_left b hr
      omega

    theorem regression_thresholds {b h a a' r r' : Nat}
        (ha : a ≤ a') (hr : r ≤ r') (hyp : classify b h a' r' = .regressed) :
        classify b h a r = .regressed := by
      rw [classify_regressed] at hyp ⊢
      exact grows_thresholds ha hr hyp

    theorem improvement_thresholds {b h a a' r r' : Nat}
        (ha : a ≤ a') (hr : r ≤ r') (hyp : classify b h a' r' = .improved) :
        classify b h a r = .improved := by
      rw [classify_improved] at hyp ⊢
      exact grows_thresholds ha hr hyp

    structure Entry where
      path : String
      key : String
      heartbeats : Nat
      deriving DecidableEq, Repr

    def sameKey (a b : Entry) : Bool := a.path == b.path && a.key == b.key

    def takeMatch (base : Entry) : List Entry → Option Entry × List Entry
      | [] => (none, [])
      | h :: hs =>
        if sameKey base h then (some h, hs)
        else let (found, rest) := takeMatch base hs
             (found, h :: rest)

    structure Row where
      base : Option Entry
      head : Option Entry
      status : Status
      deriving DecidableEq, Repr

    def matchRow (base : Entry) (head : Option Entry) (a r : Nat) : Row :=
      ⟨some base, head, match head with
        | some h => classify base.heartbeats h.heartbeats a r
        | none => .removed⟩

    def matchEntries : List Entry → List Entry → Nat → Nat → List Row
      | [], hs, _, _ => hs.map (fun h => ⟨none, some h, .added⟩)
      | b :: bs, hs, a, r =>
        let (found, rest) := takeMatch b hs
        matchRow b found a r :: matchEntries bs rest a r

    theorem takeMatch_conservation (b : Entry) (hs : List Entry) :
        ((takeMatch b hs).1.toList ++ (takeMatch b hs).2).Perm hs := by
      induction hs with
      | nil => simp [takeMatch]
      | cons h hs ih =>
        simp only [takeMatch]
        split
        · simp
        · cases result : takeMatch b hs with
          | mk found rest =>
            simp only [result] at ih ⊢
            exact (List.perm_middle).trans (ih.cons h)

    theorem matching_base_once (bs hs : List Entry) (a r : Nat) :
        (matchEntries bs hs a r).filterMap Row.base = bs := by
      induction bs generalizing hs with
      | nil => simp [matchEntries, List.filterMap_map, Function.comp_def]
      | cons b bs ih =>
        simp [matchEntries, matchRow, ih]

    theorem matching_head_once (bs hs : List Entry) (a r : Nat) :
        ((matchEntries bs hs a r).filterMap Row.head).Perm hs := by
      induction bs generalizing hs with
      | nil => simp [matchEntries, List.filterMap_map, Function.comp_def]
      | cons b bs ih =>
        have conservation := takeMatch_conservation b hs
        cases result : takeMatch b hs with
        | mk found rest =>
          simp only [result] at conservation
          simp only [matchEntries, result, matchRow, List.filterMap_cons]
          cases found with
          | none => exact (ih rest).trans conservation
          | some h => exact ((ih rest).cons h).trans conservation

    structure Summary where
      regressed : Nat := 0
      improved : Nat := 0
      unchanged : Nat := 0
      added : Nat := 0
      removed : Nat := 0
      baseHeartbeats : Nat := 0
      headHeartbeats : Nat := 0
      deriving DecidableEq, Repr

    def entryHeartbeats (e : Option Entry) : Nat :=
      match e with
      | none => 0
      | some e => e.heartbeats

    def summarize : List Row → Summary
      | [] => {}
      | row :: rows =>
        let s := summarize rows
        { regressed := s.regressed + if row.status == .regressed then 1 else 0
          improved := s.improved + if row.status == .improved then 1 else 0
          unchanged := s.unchanged + if row.status == .unchanged then 1 else 0
          added := s.added + if row.status == .added then 1 else 0
          removed := s.removed + if row.status == .removed then 1 else 0
          baseHeartbeats := s.baseHeartbeats + entryHeartbeats row.base
          headHeartbeats := s.headHeartbeats + entryHeartbeats row.head }

    def failed (s : Summary) : Bool := 0 < s.regressed

    theorem summary_regressed (rows : List Row) :
        (summarize rows).regressed = (rows.filter (·.status == .regressed)).length := by
      induction rows with
      | nil => rfl
      | cons row rows ih =>
        simp only [summarize, List.filter_cons]
        split <;> simp_all <;> omega

    theorem failure_iff_regression (rows : List Row) :
        failed (summarize rows) = true ↔ ∃ row ∈ rows, row.status = .regressed := by
      simp [failed, summary_regressed, List.length_pos_iff, List.filter_eq_nil_iff]
    "#
}
