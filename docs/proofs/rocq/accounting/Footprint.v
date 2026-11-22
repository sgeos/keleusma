(** * Footprint.v -- the memory a reuse run uses, related to the machine.

    Three results connect [Accounting.v]'s counting to the region core.

    - [mem_within] holds for every plan, module, host and fuel, accepted or
      not.  Every cell that holds a value after a run is the cell of some
      recorded allocation, so the counting of [Accounting.v] covers the
      memory the run actually uses.
    - [same_allocations] holds for every module the checker accepts and
      every injective plan.  The reuse run and the baseline run record the
      same allocations, in the same order and epochs.  It is B2's simulation,
      read for its allocation record instead of its log.
    - [reuse_footprint_bounded], the headline, combines them.  In every
      epoch the reuse run uses no more cells than the baseline, which uses one
      per allocation, and at most the plan's slots plus one cell for each
      allocation at a site the plan does not reuse. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Sim Reuse.
From Accounting Require Import Accounting.
Import ListNotations.

(** Every cell holding a value is the cell of a recorded allocation. *)
Definition within (P : plan) (s : state) : Prop :=
  forall c x, mem s c = Some x -> In c (map (cell_at P) (indexed (regs s))).

Lemma within_same : forall P s s', regs s' = regs s -> mem s' = mem s -> within P s -> within P s'.
Proof. intros P s s' Hr Hm H c x Hc. rewrite Hr. rewrite Hm in Hc. exact (H c x Hc). Qed.

Lemma pop_keeps : forall s v s1, pop s = (v, s1) -> regs s1 = regs s /\ mem s1 = mem s.
Proof. intros s v s1. unfold pop. destruct (stk s); intros H; injection H as <- <-; split; reflexivity. Qed.

Lemma new_within : forall P site s, within P s -> within P (new_step P site s).
Proof.
  intros P site s Hw. unfold new_step. destruct (pop s) as [v s1] eqn:E.
  destruct (pop_keeps _ _ _ E) as [Hr Hm]. intros c x Hc. cbn [mem regs] in Hc |- *.
  rewrite indexed_app, map_app. apply in_or_app.
  destruct (cell_eq_dec c (cell_of P site (length (regs s1)))) as [->|Hne].
  - right. left. reflexivity.
  - left. rewrite Hr. rewrite Hm in Hc. exact (Hw c x Hc).
Qed.

(** Pops are named so their bookkeeping lemma can be applied. *)
Ltac pops :=
  repeat match goal with
  | H : context [pop ?x] |- _ =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E; apply pop_keeps in E; destruct E
  | |- context [pop ?x] =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E; apply pop_keeps in E; destruct E
  end.

Lemma run_within : forall P host fs n,
  (forall b s s', run_b P host fs n b s = Some s' -> within P s -> within P s') /\
  (forall i s s', run_i P host fs n i s = Some s' -> within P s -> within P s') /\
  (forall k b s s', run_loop P host fs n k b s = Some s' -> within P s -> within P s').
Proof.
  intros P host fs n. induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; discriminate. }
  split; [|split].
  - intros b s s' H Hw. destruct b as [|i r]; simpl in H.
    + injection H as <-. exact Hw.
    + unfold bind in H. destruct (run_i P host fs n i s) as [s1|] eqn:E; [|discriminate].
      exact (IHb _ _ _ H (IHi _ _ _ E Hw)).
  - intros i s s' H Hw. destruct i; simpl in H.
    + injection H as <-. exact (within_same P s _ eq_refl eq_refl Hw).
    + injection H as <-. exact (within_same P s _ eq_refl eq_refl Hw).
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. apply (IHb _ _ _ H). eapply within_same; [| |exact Hw]; congruence.
    + exact (IHl _ _ _ _ H Hw).
    + destruct (nth_error fs f) as [g|]; [|discriminate]. unfold bind in H.
      destruct (run_b P host fs n g (with_stk s [])) as [s2|] eqn:E; [|discriminate].
      injection H as <-. apply (within_same P s2); [reflexivity|reflexivity|].
      apply (IHb _ _ _ E). exact (within_same P s _ eq_refl eq_refl Hw).
    + injection H as <-. unfold yield_step. pops.
      eapply within_same; [| |exact Hw]; cbn; congruence.
    + injection H as <-. exact (new_within P _ s Hw).
    + injection H as <-. unfold get_step. pops.
      eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
    + pops. injection H as <-. eapply within_same; [| |exact Hw]; cbn; congruence.
  - intros k b s s' H Hw. destruct k as [|k]; simpl in H.
    + injection H as <-. exact Hw.
    + unfold bind in H. destruct (run_b P host fs n b s) as [s1|] eqn:E; [|discriminate].
      exact (IHl _ _ _ _ H (IHb _ _ _ E Hw)).
Qed.

Theorem mem_within : forall P host m fuel n s,
  cycles P host m fuel n init = Some s -> within P s.
Proof.
  intros P host m fuel n.
  assert (G : forall n s0 s, cycles P host m fuel n s0 = Some s -> within P s0 -> within P s).
  { induction n0 as [|n0 IH]; intros s0 s H Hw; simpl in H.
    - injection H as <-. exact Hw.
    - unfold bind in H. destruct (run_b P host (mfuncs m) fuel (mmain m) s0) as [s1|] eqn:E; [|discriminate].
      apply (IH _ _ H). apply (within_same P s1); [reflexivity|reflexivity|].
      exact (proj1 (run_within P host (mfuncs m) fuel) _ _ _ E Hw). }
  intros s H. apply (G n init s H). intros c x Hc. discriminate.
Qed.

(** B2's simulation, read for the allocation record. *)
Theorem same_allocations : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => regs sp = regs sb
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros m ft Hc P Pinj host fuel n.
  destruct (region_check_wf m ft Hc) as [WF [S' Hmain]].
  assert (Hgen : forall n sp sb, rel P sp sb -> stk sb = [] -> locs_ok sb ->
    match cycles P host m fuel n sp, cycles baseline host m fuel n sb with
    | Some sp', Some sb' => rel P sp' sb'
    | None, None => True
    | _, _ => False
    end).
  { induction n0 as [|n0 IH]; intros sp sb Hrel Hs Hl; simpl; [exact Hrel|].
    assert (Hm : matches sb []) by (unfold matches; rewrite Hs; constructor).
    pose proof (proj1 (simulation P Pinj host (mfuncs m) ft WF fuel) (mmain m) true [] S' sp sb
                  Hmain Hrel Hm Hl) as H1.
    destruct (run_b P host (mfuncs m) fuel (mmain m) sp) as [sp1|];
      destruct (run_b baseline host (mfuncs m) fuel (mmain m) sb) as [sb1|];
      simpl in H1 |- *; try contradiction; [|exact I].
    destruct H1 as [[Hs1 [Hlc1 [Hg1 [Hr1 [He1 _]]]]] [_ Hl1]].
    apply IH.
    - unfold reset. repeat split; simpl; try assumption; try congruence. intros r e s [].
    - reflexivity.
    - intros j. apply Hl1. }
  assert (Hinit : rel P init init) by (repeat split; intros r e s []).
  specialize (Hgen n init init Hinit eq_refl (fun j => ex_intro _ 0 eq_refl)).
  destruct (cycles P host m fuel n init) as [sp|]; destruct (cycles baseline host m fuel n init) as [sb|];
    try contradiction; [|exact I].
  destruct Hgen as [_ [_ [_ [Hr _]]]]. exact Hr.
Qed.

(** ** The headline.  For a module the checker accepts and an injective plan
    whose slots are below [K], whenever both runs finish, every cell the
    reuse run holds is the cell of a recorded allocation, and in every epoch
    the reuse run uses at most [K] slots plus one cell per allocation at a
    site the plan does not reuse, and no more cells than the baseline, which
    uses one per allocation. *)
Theorem reuse_footprint_bounded : forall m ft, region_check m = Some ft ->
  forall P K, injective P -> (forall s k, P s = Some k -> k < K) ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb =>
      regs sp = regs sb /\ within P sp /\
      forall e,
        footprint_at P (regs sp) e <= K + length (unplanned_at P (regs sp) e) /\
        footprint_at P (regs sp) e <= footprint_at baseline (regs sb) e /\
        footprint_at baseline (regs sb) e = length (allocs_at (regs sb) e)
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros m ft Hc P K Pinj HK host fuel n.
  pose proof (same_allocations m ft Hc P Pinj host fuel n) as Hs.
  destruct (cycles P host m fuel n init) as [sp|] eqn:Ep;
    destruct (cycles baseline host m fuel n init) as [sb|]; try contradiction; [|exact I].
  split; [exact Hs|]. split; [exact (mem_within P host m fuel n sp Ep)|].
  intros e. split; [exact (footprint_slot_bound P K HK (regs sp) e)|]. split.
  - rewrite <- Hs. apply footprint_le_baseline.
  - apply baseline_footprint.
Qed.
