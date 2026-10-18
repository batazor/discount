import Discount.Apply.Step

/-!
# List lemmas used by the proofs

Small facts about `List.sum` over `Int`, `getD`, `set` and `subLists` that core does not
provide in the shape we need. All proofs are by induction; no Mathlib.
-/
namespace Discount

theorem sum_map_mul_left (c : Int) (l : List Int) (f : Int → Int) :
    (l.map (fun x => c * f x)).sum = c * (l.map f).sum := by
  induction l with
  | nil => simp
  | cons x xs ih => simp [List.sum_cons, ih, Int.mul_add]

theorem sum_map_le_sum_map (l : List Int) (f g : Int → Int) (h : ∀ x ∈ l, f x ≤ g x) :
    (l.map f).sum ≤ (l.map g).sum := by
  induction l with
  | nil => simp
  | cons x xs ih =>
    simp only [List.map_cons, List.sum_cons]
    have h1 := h x (by simp)
    have h2 := ih (fun y hy => h y (by simp [hy]))
    omega

theorem sum_map_add (l : List Int) (f g : Int → Int) :
    (l.map (fun x => f x + g x)).sum = (l.map f).sum + (l.map g).sum := by
  induction l with
  | nil => simp
  | cons x xs ih => simp only [List.map_cons, List.sum_cons, ih]; omega

theorem sum_map_sub (l : List Int) (f g : Int → Int) :
    (l.map (fun x => f x - g x)).sum = (l.map f).sum - (l.map g).sum := by
  induction l with
  | nil => simp
  | cons x xs ih => simp only [List.map_cons, List.sum_cons, ih]; omega

theorem getD_map_lt (l : List Int) (f : Int → Int) (i : Nat) (hi : i < l.length) :
    (l.map f).getD i 0 = f (l.getD i 0) := by
  induction l generalizing i with
  | nil => simp at hi
  | cons x xs ih =>
    cases i with
    | zero => simp
    | succ i => simp only [List.map_cons, List.getD_cons_succ]; exact ih i (by simpa using hi)

theorem sum_le_sum_of_getD (a b : List Int) (h : a.length = b.length)
    (hle : ∀ i, i < a.length → a.getD i 0 ≤ b.getD i 0) : a.sum ≤ b.sum := by
  induction a generalizing b with
  | nil => cases b <;> simp_all
  | cons x xs ih =>
    cases b with
    | nil => simp at h
    | cons y ys =>
      simp only [List.sum_cons]
      have h0 := hle 0 (by simp)
      simp only [List.getD_cons_zero] at h0
      have := ih ys (by simpa using h) (fun i hi => by
        have := hle (i + 1) (by simpa using Nat.succ_lt_succ hi)
        simpa using this)
      omega

theorem sum_map_zero {α : Type} (l : List α) : (l.map (fun _ => (0 : Int))).sum = 0 := by
  induction l with
  | nil => simp
  | cons x xs ih => simp [ih]

theorem getD_map_zero {α : Type} (l : List α) (i : Nat) : (l.map (fun _ => (0 : Int))).getD i 0 = 0 := by
  induction l generalizing i with
  | nil => simp
  | cons x xs ih =>
    cases i with
    | zero => simp
    | succ i => simp only [List.map_cons, List.getD_cons_succ]; exact ih i

theorem sum_nonneg_of_forall (l : List Int) (h : ∀ x ∈ l, 0 ≤ x) : 0 ≤ l.sum := by
  induction l with
  | nil => simp
  | cons x xs ih =>
    simp only [List.sum_cons]
    have := h x (by simp)
    have := ih (fun y hy => h y (by simp [hy]))
    omega

/-- Sum of an indicator-weighted list: each element contributes `a` when `p` holds. -/
theorem sum_map_ite {α : Type} (l : List α) (p : α → Bool) (a : Int) :
    (l.map (fun i => if p i then a else 0)).sum = a * (l.countP p : Int) := by
  induction l with
  | nil => simp
  | cons x xs ih =>
    simp only [List.map_cons, List.sum_cons, List.countP_cons, ih]
    split <;> simp_all [Int.mul_add] <;> omega

theorem sum_set_succ (l : List Int) (j : Nat) (hj : j < l.length) :
    (l.set j (l.getD j 0 + 1)).sum = l.sum + 1 := by
  induction l generalizing j with
  | nil => simp at hj
  | cons x xs ih =>
    cases j with
    | zero => simp [List.sum_cons]; omega
    | succ j =>
      simp only [List.length_cons] at hj
      simp only [List.getD_cons_succ, List.set_cons_succ, List.sum_cons]
      rw [ih j (by omega)]; omega

theorem getD_set_succ (l : List Int) (j i : Nat) :
    (l.set j (l.getD j 0 + 1)).getD i 0 = l.getD i 0 + (if j = i ∧ i < l.length then 1 else 0) := by
  induction l generalizing i j with
  | nil => simp
  | cons x xs ih =>
    cases j with
    | zero =>
      cases i with
      | zero => simp
      | succ i => simp
    | succ j =>
      cases i with
      | zero => simp
      | succ i =>
        simp only [List.set_cons_succ, List.getD_cons_succ, List.length_cons, ih j i]
        have : (j + 1 = i + 1 ∧ i + 1 < xs.length + 1) ↔ (j = i ∧ i < xs.length) := by omega
        simp only [this]

/-- Elements of `set j (getD j + 1)` are elements of `l` or one more than an element. -/
theorem mem_set_succ (l : List Int) (j : Nat) (x : Int) (hx : x ∈ l.set j (l.getD j 0 + 1)) :
    x ∈ l ∨ ∃ y ∈ l, x = y + 1 := by
  rcases List.mem_or_eq_of_mem_set hx with h | h
  · exact Or.inl h
  · by_cases hj : j < l.length
    · right
      refine ⟨l.getD j 0, ?_, h⟩
      rw [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem hj]
      simp [List.getElem_mem]
    · left
      rw [List.set_eq_of_length_le (by omega)] at hx
      exact hx

theorem subLists_length (a b : List Int) (h : a.length = b.length) : (subLists a b).length = a.length := by
  induction a generalizing b with
  | nil => cases b <;> simp [subLists]
  | cons x xs ih =>
    cases b with
    | nil => simp at h
    | cons y ys => simp [subLists, ih ys (by simpa using h)]

theorem subLists_sum (a b : List Int) (h : a.length = b.length) : (subLists a b).sum = a.sum - b.sum := by
  induction a generalizing b with
  | nil => cases b <;> simp_all [subLists]
  | cons x xs ih =>
    cases b with
    | nil => simp at h
    | cons y ys => simp only [subLists, List.sum_cons, ih ys (by simpa using h)]; omega

theorem subLists_getD (a b : List Int) (h : a.length = b.length) (i : Nat) (hi : i < a.length) :
    (subLists a b).getD i 0 = a.getD i 0 - b.getD i 0 := by
  induction a generalizing b i with
  | nil => simp at hi
  | cons x xs ih =>
    cases b with
    | nil => simp at h
    | cons y ys =>
      cases i with
      | zero => simp [subLists]
      | succ i =>
        simp only [subLists, List.getD_cons_succ]
        exact ih ys (by simpa using h) i (by simpa using hi)

/-- `getD` bounds from a `∀ x ∈ l` fact. -/
theorem getD_mem_of_lt (l : List Int) (i : Nat) (hi : i < l.length) : l.getD i 0 ∈ l := by
  rw [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem hi]
  simp [List.getElem_mem]

theorem forall_mem_of_forall_getD (l : List Int) (P : Int → Prop)
    (h : ∀ i, i < l.length → P (l.getD i 0)) : ∀ x ∈ l, P x := by
  intro x hx
  obtain ⟨i, hi, rfl⟩ := List.mem_iff_getElem.mp hx
  have := h i hi
  rwa [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem hi] at this

end Discount
