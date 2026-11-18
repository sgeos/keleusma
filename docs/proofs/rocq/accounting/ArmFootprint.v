(** * ArmFootprint.v -- memory accounting for plans that share slots.

    [Footprint.reuse_footprint_bounded] needs an injective plan only because
    [same_allocations] rests on increment B2's simulation.  The join checker
    of [../shared_slots/] has its own simulation, [ArmSim.simulation_arm],
    with no injectivity premise, and its relation also equates the two runs'
    allocation records.  Read that way, it gives the same accounting for
    every plan the join checker accepts, among them plans that put several
    sites in one slot, which is where the saving of Theorem A2 and Corollary
    A3 comes from.  The counting of [Accounting.v] and the containment of
    [Footprint.mem_within] need no change, since neither mentions the
    checker. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Sim Reuse.
From SharedSlots Require Import SharedShape SharedSim Shared ArmShape ArmSim Arm.
From Accounting Require Import Accounting Footprint.
Import ListNotations.

(** The join checker's simulation, read for the allocation record. *)
Theorem same_allocations_arm : forall P m ft, region_checkA P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => regs sp = regs sb
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros P m ft Hc host fuel n.
  destruct (region_checkA_wf P m ft Hc) as [WF [S' Hmain]].
  assert (Hgen : forall n sp sb, rel P sp sb -> stk sb = [] -> locs_ok sb ->
    match cycles P host m fuel n sp, cycles baseline host m fuel n sb with
    | Some sp', Some sb' => rel P sp' sb'
    | None, None => True
    | _, _ => False
    end).
  { induction n0 as [|n0 IH]; intros sp sb Hrel Hs Hl; simpl; [exact Hrel|].
    assert (Hm : matchesA sb []) by (unfold matchesA; rewrite Hs; constructor).
    pose proof (proj1 (simulation_arm P host (mfuncs m) ft WF fuel) (mmain m) true [] S' sp sb
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

(** ** The headline for shared slots.  For every module and plan the join
    checker accepts together, with the plan's slots below [K] and no
    injectivity premise, the accounting of
    [Footprint.reuse_footprint_bounded] holds. *)
Theorem reuse_footprint_bounded_arm : forall P m ft, region_checkA P m = Some ft ->
  forall K, (forall s k, P s = Some k -> k < K) ->
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
  intros P m ft Hc K HK host fuel n.
  pose proof (same_allocations_arm P m ft Hc host fuel n) as Hs.
  destruct (cycles P host m fuel n init) as [sp|] eqn:Ep;
    destruct (cycles baseline host m fuel n init) as [sb|]; try contradiction; [|exact I].
  split; [exact Hs|]. split; [exact (mem_within P host m fuel n sp Ep)|].
  intros e. split; [exact (footprint_slot_bound P K HK (regs sp) e)|]. split.
  - rewrite <- Hs. apply footprint_le_baseline.
  - apply baseline_footprint.
Qed.
