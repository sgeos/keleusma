(** * Controls.v -- evidence that the theorems are not vacuous.

    A theorem whose premises no model meets, or whose conclusion every model
    meets, would check and say nothing.  Two controls exclude each failure.

    - [reuse_without_separation_is_unsound] exhibits a machine and a plan for
      which soundness fails.  Two allocations into one slot in one epoch
      followed by a read of the first is observably different from the
      baseline, so [sound] is not trivially true, and the separation premise
      is doing work.
    - [unsound_plan_fails_the_premises] shows that the same plan cannot meet
      the premises of Lemma 4, so the premises exclude the divergence rather
      than being met by every plan.
    - [premises_of_B1_are_satisfiable] exhibits a run with reuse, two
      allocations into one slot and a read of each region, meets every premise
      of Theorem B1 for every prefix of it, and so derives its soundness from
      the theorem.  The premises are therefore satisfiable by a run that is not
      degenerate.  This control was built by the adversarial audit of
      2026-11-07 and adopted here.

    What the controls do not show is recorded in [README.md].  The flow model
    is a certificate the theorem's caller supplies, and the positive control's
    certificate has no [Escapes] execution, so it exercises the separation and
    the read order, not a nontrivial confinement argument. *)

From Stdlib Require Import List Arith Lia.
From RegionReuse Require Import Trace Clearance Theorems.
Import ListNotations.

(** The instance.  One site, construction bytes are numbers, one view that
    selects everything, no scalars. *)
Definition C_event := Trace.event unit nat unit unit.
Definition C_sel (_ : unit) (b : nat) : nat := b.

(** The machine allocates the site with bytes 1, then with bytes 2, then
    dereferences the first region, then halts. *)
Definition C_next (h : list (Trace.obs nat unit)) : option C_event :=
  match length h with
  | 0 => Some (Trace.EAlloc unit nat unit unit tt 1)
  | 1 => Some (Trace.EAlloc unit nat unit unit tt 2)
  | 2 => Some (Trace.EDeref unit nat unit unit (Trace.SReg 0) tt)
  | _ => None
  end.

(** The plan reuses the site in slot 0. *)
Definition C_plan : Trace.plan unit := fun _ => Some 0.

Theorem reuse_without_separation_is_unsound :
  ~ Trace.sound unit nat unit unit C_sel C_next C_plan.
Proof.
  intros H. specialize (H 3). vm_compute in H. discriminate H.
Qed.

(** The same machine under the baseline reads the first region's bytes, and
    under the plan the second's, which is the divergence the theorems exclude. *)
Example baseline_reads_first :
  Trace.obs_of unit nat unit unit C_sel (Trace.baseline unit)
    (Trace.gen unit nat unit unit C_sel C_next (Trace.baseline unit) 3)
  = [Trace.ONone nat unit; Trace.ONone nat unit; Trace.OBytes nat unit 1].
Proof. vm_compute. reflexivity. Qed.

Example plan_reads_second :
  Trace.obs_of unit nat unit unit C_sel C_plan
    (Trace.gen unit nat unit unit C_sel C_next C_plan 3)
  = [Trace.ONone nat unit; Trace.ONone nat unit; Trace.OBytes nat unit 2].
Proof. vm_compute. reflexivity. Qed.

(** The premises of Lemma 4 exclude the unsound plan. *)
Theorem unsound_plan_fails_the_premises :
  ~ (forall n k, element_ok unit nat unit unit C_plan k
                   (Trace.gen unit nat unit unit C_sel C_next (Trace.baseline unit) n)).
Proof.
  intros H. apply reuse_without_separation_is_unsound.
  exact (lemma4_plan_sound unit nat unit unit C_sel C_next C_plan H).
Qed.

(** ** A run that meets every premise of Theorem B1.

    The machine allocates the site with bytes 1, dereferences that region,
    allocates the site again with bytes 2 into the same slot, and dereferences
    the second region.  Each region's scope ends before the next allocation.
    Location [false] holds the first reference at time 1, and [true] the second
    at time 3. *)
Definition I_evs : list C_event :=
  [Trace.EAlloc unit nat unit unit tt 1; Trace.EDeref unit nat unit unit (Trace.SReg 0) tt;
   Trace.EAlloc unit nat unit unit tt 2; Trace.EDeref unit nat unit unit (Trace.SReg 1) tt].

Definition I_next (h : list (Trace.obs nat unit)) : option C_event := nth_error I_evs (length h).

Definition I_flow (es : list C_event) : flow := {|
  Loc := bool;
  holds := fun t l r => (l = false /\ t = 1 /\ r = 0 /\ 1 <= length es) \/
                        (l = true /\ t = 3 /\ r = 1 /\ 3 <= length es);
  ops := fun t l => (t = 1 /\ l = false) \/ (t = 3 /\ l = true);
  writes := fun t l => (t = 0 /\ l = false) \/ (t = 2 /\ l = true);
  escapes := fun _ => False;
  death := fun l => if l then Some 4 else Some 2;
  send := fun t => if t <? 2 then 2 else Nat.max 4 (S t);
  alloc_of := alloc_at unit nat unit unit es;
  reads := read_at unit nat unit unit es
|}.

Lemma alloc_in_range : forall es t, alloc_at unit nat unit unit es t <> None -> t < length es.
Proof.
  intros es t H. unfold alloc_at in H. destruct (nth_error es t) eqn:E; [|congruence].
  apply nth_error_Some. congruence.
Qed.

Ltac tcase t := destruct t as [|[|[|[|[|t]]]]].

Lemma I_flow_ok : forall n, n <= 4 -> flow_ok (I_flow (firstn n I_evs)).
Proof.
  intros n Hn. destruct n as [|[|[|[|[|n]]]]]; try lia; clear Hn;
  constructor; cbn [Clearance.holds Clearance.ops Clearance.writes Clearance.escapes Clearance.death
     Clearance.send Clearance.alloc_of Clearance.reads I_flow]; unfold outlives; cbn [Clearance.death I_flow];
  simpl length.
  all: try (intros l r [H|H]; lia).
  all: try (intros t l r [[-> [-> _]]|[-> [-> _]]]; lia).
  all: try (intros t l r [[-> [Ht [-> H1]]]|[-> [Ht [-> H1]]]];
            [try lia; replace t with 0 by lia|try lia; replace t with 2 by lia];
            right; (split; [auto|right; reflexivity]); fail).
  all: try (exact (alloc_at_inj unit nat unit unit _)).
  all: try (intros t l r [[-> ->]|[-> ->]] _ Ho; simpl in Ho; lia).
  all: try (intros t l r _ [[-> ->]|[-> ->]] _ Ho; simpl in Ho; lia).
  all: try (intros t r Hal t' l Hle Hh; tcase t; vm_compute in Hal; try discriminate;
            try (pose proof (alloc_in_range _ _ ltac:(rewrite Hal; discriminate)) as X; simpl in X; lia);
            injection Hal as <-; destruct Hh as [[_ [? [? _]]]|[_ [? [? _]]]]; lia).
  all: try (intros t t' r H1 H2; tcase t; tcase t'; vm_compute in H1, H2; try discriminate;
            try reflexivity; try (rewrite H1 in H2; discriminate);
            try (pose proof (alloc_in_range _ _ ltac:(rewrite H1; discriminate)) as X; simpl in X; lia);
            try (pose proof (alloc_in_range _ _ ltac:(rewrite H2; discriminate)) as X; simpl in X; lia); fail).
  all: try (intros j t Hj Hjt Hts; tcase j; vm_compute in Hj; try (exfalso; apply Hj; reflexivity);
            try (exfalso; apply Hj; pose proof (alloc_in_range _ _ Hj) as X; simpl in X; lia);
            tcase t; simpl in Hts |- *; lia).
  all: try (intros t; destruct (t <? 2) eqn:T; [apply Nat.ltb_lt in T|apply Nat.ltb_ge in T]; lia).
  all: try (intros t r [e [He Hre]]; tcase t; simpl in He; try discriminate;
            try (destruct t; discriminate);
            injection He as <-; simpl in Hre; try contradiction; subst r;
            first [exists false; split; [left; auto|left; repeat split; lia]
                  |exists true; split; [right; auto|right; repeat split; lia]]).
Qed.

Lemma run_length : forall (Q : Trace.plan unit) st (l : list C_event),
  length (fst (Trace.run unit nat unit unit C_sel Q st l)) = length l.
Proof.
  intros Q st l. revert st. induction l as [|e l IH]; intros st; [reflexivity|].
  simpl. destruct (Trace.exec unit nat unit unit C_sel Q e st) as [o s1] eqn:X.
  specialize (IH s1). destruct (Trace.run unit nat unit unit C_sel Q s1 l) eqn:Y.
  simpl in *. f_equal. exact IH.
Qed.

Lemma I_gen : forall n, Trace.gen unit nat unit unit C_sel I_next (Trace.baseline unit) n = firstn n I_evs.
Proof.
  assert (Hs : forall k, Trace.gen unit nat unit unit C_sel I_next (Trace.baseline unit) (4 + k) = I_evs).
  { induction k as [|k IH]; [vm_compute; reflexivity|].
    rewrite Nat.add_succ_r.
    change (Trace.gen unit nat unit unit C_sel I_next (Trace.baseline unit) (S (4 + k))) with
      (let e := Trace.gen unit nat unit unit C_sel I_next (Trace.baseline unit) (4 + k) in
       match I_next (Trace.obs_of unit nat unit unit C_sel (Trace.baseline unit) e) with
       | Some ev => e ++ [ev] | None => e end).
    cbv zeta. rewrite IH. unfold I_next at 1, Trace.obs_of.
    rewrite run_length. reflexivity. }
  intros n. destruct (le_lt_dec 4 n) as [H|H].
  - replace n with (4 + (n - 4)) by lia. rewrite Hs. rewrite firstn_all2; simpl; [reflexivity|lia].
  - destruct n as [|[|[|[|n]]]]; try lia; vm_compute; reflexivity.
Qed.

Ltac split_list :=
  repeat match goal with
  | H : ?a ++ _ = _ |- _ => is_var a; let x := fresh "x" in destruct a as [|x a]; simpl in H
  | H : _ :: _ = _ :: _ |- _ => injection H; clear H; intros; subst
  | H : [] = _ :: _ |- _ => discriminate H
  | H : _ :: _ = [] |- _ => discriminate H
  | H : Trace.EDeref _ _ _ _ _ _ = Trace.EAlloc _ _ _ _ _ _ |- _ => discriminate H
  | H : Trace.EAlloc _ _ _ _ _ _ = Trace.EDeref _ _ _ _ _ _ |- _ => discriminate H
  | H : Trace.EAlloc _ _ _ _ _ _ = Trace.EAlloc _ _ _ _ _ _ |- _ => injection H; clear H; intros; subst
  | H : Trace.EDeref _ _ _ _ _ _ = Trace.EDeref _ _ _ _ _ _ |- _ => injection H; clear H; intros; subst
  end.

Lemma I_separated : forall n, n <= 4 ->
  separated_scopes unit nat unit unit C_plan 0 (send (I_flow (firstn n I_evs))) (firstn n I_evs).
Proof.
  intros n Hn p1 s1 b1 m s2 b2 q Heq _ _ _.
  destruct n as [|[|[|[|[|n]]]]]; try lia; simpl in Heq; symmetry in Heq; split_list;
  right; simpl; lia.
Qed.

Theorem premises_of_B1_are_satisfiable : Trace.sound unit nat unit unit C_sel I_next C_plan.
Proof.
  apply (theorem_B1 unit nat unit unit C_sel I_next C_plan 0).
  - intros s k' H. injection H; auto.
  - intros n. rewrite I_gen.
    assert (Hm : firstn n I_evs = firstn (Nat.min n 4) I_evs).
    { destruct (le_lt_dec 4 n); [rewrite Nat.min_r by lia; rewrite !firstn_all2; simpl; auto; lia
                                |rewrite Nat.min_l by lia; reflexivity]. }
    rewrite Hm. exists (I_flow (firstn (Nat.min n 4) I_evs)).
    split; [apply I_flow_ok; lia|]. split; [split; [intros t; reflexivity|intros t r H; exact H]|].
    split; [apply I_separated; lia|]. intros p1 s b rest _ _ t _ [].
Qed.

(** The run reuses the slot and reads each region's own bytes. *)
Example instance_reads_each_region :
  Trace.obs_of unit nat unit unit C_sel C_plan I_evs =
  [Trace.ONone nat unit; Trace.OBytes nat unit 1; Trace.ONone nat unit; Trace.OBytes nat unit 2].
Proof. vm_compute. reflexivity. Qed.
