(** * Statements.v -- the statements of B1's named results, pinned.

    Each pin is a definition whose stated type must match the result's type
    up to conversion, so a statement weakened by an added premise or a
    changed conclusion fails to compile.  Increment A's audit of 2026-11-08
    showed by execution that the other checks of [check.sh] cannot detect such
    a weakening.  Each stated type is the closed type that Rocq printed for the
    result on 2026-11-18, with the section premises explicit, so this file
    pins the statements as they were proved and does not restate them.  The
    script requires the number of pins below.

    The pins fix the statements.  They do not fix the definitions those
    statements mention, apart from the two pinned by proved equations at the
    end, [Trace.sound] and [Trace.baseline], which every reuse theorem
    concludes or compares against. *)

From Stdlib Require Import List.
From RegionReuse Require Bound Trace Clearance Theorems Controls.

Definition pin_A1_branch_bound :
  forall (L : Type) (w : L -> nat) (barm : Bound.tree L -> nat) (arms : list (Bound.tree L)), (forall (arm : Bound.tree L) (l : list L), List.In arm arms -> Bound.exec L arm l -> Bound.sum L w l <= barm arm) -> forall l : list L, Bound.exec L (Bound.Cond L arms) l -> Bound.sum L w l <= Bound.maxl (List.map barm arms)
  := Bound.A1_branch_bound.

Definition pin_A1s_static_bound :
  forall (L : Type) (w : L -> nat) (t : Bound.tree L) (l : list L), Bound.exec L t l -> Bound.sum L w l <= Bound.B L w t
  := Bound.A1s_static_bound.

Definition pin_cond_one_arm :
  forall (L : Type) (arms : list (Bound.tree L)) (l : list L), Bound.exec L (Bound.Cond L arms) l -> exists arm : Bound.tree L, List.In arm arms /\ (forall x : L, List.In x l -> List.In x (Bound.leaves L arm))
  := Bound.cond_one_arm.

Definition pin_C_plan_bound :
  forall (L : Type) (sz : L -> nat) (reused : L -> bool) (slots : nat) (unit : Bound.tree L) (l p q : list L), Bound.exec L unit l -> l = (p ++ q)%list -> Bound.occupancy L sz reused slots p <= Bound.footprint L sz reused slots unit
  := Bound.C_plan_bound.

Definition pin_B2a_accounting :
  forall (L : Type) (reused_w static actual : L -> nat) (slots : nat) (unit : Bound.tree L) (l p q : list L), (forall x : L, actual x <= static x) -> Bound.exec L unit l -> l = (p ++ q)%list -> slots + Bound.sum L reused_w p + Bound.sum L actual p <= slots + Bound.B L reused_w unit + Bound.B L static unit
  := Bound.B2a_accounting.

Definition pin_footprint_below_bump :
  let unit := Bound.Loop bool 4 (Bound.Lf bool true) in Bound.footprint bool Bound.sz5 (fun b : bool => b) 5 unit < Bound.B bool (Bound.bump_weight bool Bound.sz5 Bound.none_reused) unit
  := Bound.footprint_below_bump.

Definition pin_footprint_above_bump :
  let unit := Bound.Cond bool (Bound.Lf bool true :: Bound.Lf bool false :: nil) in Bound.footprint bool Bound.sz5 (fun b : bool => b) 5 unit > Bound.B bool (Bound.bump_weight bool Bound.sz5 Bound.none_reused) unit
  := Bound.footprint_above_bump.

Definition pin_lemma4_composition :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n k : nat, Trace.slot_safe Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Trace.lemma4_composition.

Definition pin_plan_follows_baseline :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n k : nat, Trace.slot_safe Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> forall n : nat, Trace.gen Site Bytes View Scalar sel next P n = Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n
  := Trace.plan_follows_baseline.

Definition pin_slot_theorem :
  forall (Site Bytes View Scalar : Type) (P : Trace.plan Site) (k : nat) (send : nat -> nat) (evs : list (Trace.event Site Bytes View Scalar)), Trace.separated Site Bytes View Scalar P k send evs -> Trace.slot_safe Site Bytes View Scalar P k evs
  := Trace.slot_theorem.

Definition pin_A2_arm_overlap :
  forall (Site Bytes View Scalar : Type) (P : Trace.plan Site) (k : nat) (evs : list (Trace.event Site Bytes View Scalar)), Trace.once_per_epoch Site Bytes View Scalar P k evs -> Trace.slot_safe Site Bytes View Scalar P k evs
  := Trace.A2_arm_overlap.

Definition pin_memory_agrees :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (P : Trace.plan Site) (evs : list (Trace.event Site Bytes View Scalar)) (sp sb : Trace.state Site Bytes), Trace.corr Site Bytes sp sb -> Trace.ok Site Bytes View Scalar sel P sp sb evs -> fst (Trace.run Site Bytes View Scalar sel P sp evs) = fst (Trace.run Site Bytes View Scalar sel (Trace.baseline Site) sb evs)
  := Trace.memory_agrees.

Definition pin_lemma1_provenance :
  forall f : Clearance.flow, Clearance.flow_ok f -> forall (t : nat) (l : Clearance.Loc f) (r : nat), Clearance.holds f t l r -> exists j : nat, j < t /\ Clearance.alloc_of f j = Some r
  := Clearance.lemma1_provenance.

Definition pin_lemma2_confinement_clearance :
  forall f : Clearance.flow, Clearance.flow_ok f -> forall j r : nat, Clearance.alloc_of f j = Some r -> Clearance.confined f j r -> (forall l : Clearance.Loc f, ~ Clearance.holds f (Clearance.send f j) l r) /\ (forall t : nat, Clearance.send f j <= t -> ~ Clearance.reads f t r)
  := Clearance.lemma2_confinement_clearance.

Definition pin_lemma3_refined_clearance :
  forall f : Clearance.flow, Clearance.flow_ok f -> forall (j r : nat) (D : Clearance.Loc f -> Prop), Clearance.alloc_of f j = Some r -> Clearance.refined_confined f j r D -> Clearance.dead_at f j D -> forall t : nat, Clearance.send f j <= t -> ~ Clearance.reads f t r
  := Clearance.lemma3_refined_clearance.

Definition pin_lemma5_unconditional_clearance :
  forall f : Clearance.flow, Clearance.flow_ok f -> forall j r : nat, Clearance.alloc_of f j = Some r -> Clearance.discipline f r -> (forall l : Clearance.Loc f, ~ Clearance.holds f (Clearance.send f j) l r) /\ (forall t : nat, Clearance.send f j <= t -> ~ Clearance.reads f t r)
  := Clearance.lemma5_unconditional_clearance.

Definition pin_theorem_A2 :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site) (k : nat), Theorems.sole Site P k -> (forall n : nat, Trace.once_per_epoch Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.theorem_A2.

Definition pin_theorem_B1 :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site) (k : nat), Theorems.sole Site P k -> (forall n : nat, exists f : Clearance.flow, Clearance.flow_ok f /\ Theorems.describes Site Bytes View Scalar f (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ Theorems.separated_scopes Site Bytes View Scalar P k (Clearance.send f) (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ Theorems.all_confined Site Bytes View Scalar f P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.theorem_B1.

Definition pin_theorem_B1r :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site) (k : nat), Theorems.sole Site P k -> (forall n : nat, exists f : Clearance.flow, Clearance.flow_ok f /\ Theorems.describes Site Bytes View Scalar f (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ Theorems.separated_scopes Site Bytes View Scalar P k (Clearance.send f) (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ Theorems.all_refined Site Bytes View Scalar f P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.theorem_B1r.

Definition pin_corollary_A3 :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site) (k : nat), Theorems.sole Site P k -> (forall n : nat, exists f : Clearance.flow, Clearance.flow_ok f /\ Theorems.describes Site Bytes View Scalar f (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ Theorems.separated_scopes Site Bytes View Scalar P k (Clearance.send f) (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ (forall (p1 : list (Trace.event Site Bytes View Scalar)) (s : Site) (b : Bytes) (rest : list (Trace.event Site Bytes View Scalar)), Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n = (p1 ++ Trace.EAlloc Site Bytes View Scalar s b :: rest)%list -> P s = Some k -> Clearance.confined f (length p1) (Trace.nallocs Site Bytes View Scalar p1) \/ exists D : Clearance.Loc f -> Prop, Clearance.refined_confined f (length p1) (Trace.nallocs Site Bytes View Scalar p1) D /\ Clearance.dead_at f (length p1) D)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.corollary_A3.

Definition pin_theorem_B2 :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n : nat, exists f : Clearance.flow, Clearance.flow_ok f /\ Theorems.describes Site Bytes View Scalar f (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n) /\ (forall k : nat, Theorems.separated_scopes Site Bytes View Scalar P k (Clearance.send f) (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) /\ (forall r : nat, Clearance.discipline f r)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.theorem_B2.

Definition pin_corollary_B2b_hybrid :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n k : nat, Theorems.element_ok Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.corollary_B2b_hybrid.

Definition pin_lemma4_plan_sound :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n k : nat, Theorems.element_ok Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> Trace.sound Site Bytes View Scalar sel next P
  := Theorems.lemma4_plan_sound.

Definition pin_plan_runs_baseline_events :
  forall (Site Bytes View Scalar : Type) (sel : View -> Bytes -> Bytes) (next : list (Trace.obs Bytes Scalar) -> option (Trace.event Site Bytes View Scalar)) (P : Trace.plan Site), (forall n k : nat, Theorems.element_ok Site Bytes View Scalar P k (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)) -> forall n : nat, Trace.gen Site Bytes View Scalar sel next P n = Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n
  := Theorems.plan_runs_baseline_events.

Definition pin_element_safe :
  forall (Site Bytes View Scalar : Type) (P : Trace.plan Site) (k : nat) (evs : list (Trace.event Site Bytes View Scalar)) (f : Clearance.flow), Clearance.flow_ok f -> Theorems.describes Site Bytes View Scalar f evs -> Theorems.separated_scopes Site Bytes View Scalar P k (Clearance.send f) evs -> Theorems.cleared Site Bytes View Scalar f P k evs -> Trace.slot_safe Site Bytes View Scalar P k evs
  := Theorems.element_safe.

Definition pin_reuse_without_separation_is_unsound :
  ~ Trace.sound unit nat unit unit Controls.C_sel Controls.C_next Controls.C_plan
  := Controls.reuse_without_separation_is_unsound.

Definition pin_unsound_plan_fails_the_premises :
  ~ (forall n k : nat, Theorems.element_ok unit nat unit unit Controls.C_plan k (Trace.gen unit nat unit unit Controls.C_sel Controls.C_next (Trace.baseline unit) n))
  := Controls.unsound_plan_fails_the_premises.

Definition pin_B1_premises_hold :
  Theorems.sole unit Controls.C_plan 0 /\ (forall n : nat, exists f : Clearance.flow, Clearance.flow_ok f /\ Theorems.describes unit nat unit unit f (Trace.gen unit nat unit unit Controls.C_sel Controls.I_next (Trace.baseline unit) n) /\ Theorems.separated_scopes unit nat unit unit Controls.C_plan 0 (Clearance.send f) (Trace.gen unit nat unit unit Controls.C_sel Controls.I_next (Trace.baseline unit) n) /\ Theorems.all_confined unit nat unit unit f Controls.C_plan 0 (Trace.gen unit nat unit unit Controls.C_sel Controls.I_next (Trace.baseline unit) n))
  := Controls.B1_premises_hold.

Definition pin_premises_of_B1_are_satisfiable :
  Trace.sound unit nat unit unit Controls.C_sel Controls.I_next Controls.C_plan
  := Controls.premises_of_B1_are_satisfiable.

(** The two definitions every reuse theorem rests on, pinned by proved
    equations.  [sound] says the run under a plan and the baseline run make
    the same observations at every length, and [baseline] places every site
    in fresh bump storage. *)
Example sound_pinned : forall Site Bytes View Scalar sel next P,
  Trace.sound Site Bytes View Scalar sel next P =
  (forall n : nat,
     Trace.obs_of Site Bytes View Scalar sel P (Trace.gen Site Bytes View Scalar sel next P n) =
     Trace.obs_of Site Bytes View Scalar sel (Trace.baseline Site)
       (Trace.gen Site Bytes View Scalar sel next (Trace.baseline Site) n)).
Proof. reflexivity. Qed.

Example baseline_pinned : forall Site, Trace.baseline Site = fun _ => None.
Proof. reflexivity. Qed.
