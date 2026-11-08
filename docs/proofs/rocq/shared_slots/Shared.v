(** * Shared.v -- reuse is sound for every plan the plan-aware checker accepts.

    [reuse_sound_shared] removes increment B2's premise that a plan gives
    distinct sites distinct slots.  For every module and plan that
    [region_checkP] accepts together, the run that reuses the plan's slots
    and the baseline run either both stop or both finish with the same
    observation log, for every host, fuel and number of cycles.

    [shared_generalises] shows the new checker loses nothing.  For a plan that
    gives distinct sites distinct slots, it accepts exactly the modules B2's
    checker accepts, so [reuse_sound_shared] contains [Reuse.reuse_sound]. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim Reuse.
From SharedSlots Require Import SharedShape SharedSim.
Import ListNotations.

Lemma region_checkP_wf : forall P m ft, region_checkP P m = Some ft ->
  (forall f R g, nth_error ft f = Some R -> nth_error (mfuncs m) f = Some g ->
     shape_b ft false [] g = Some R /\ forallb is_ts R = true) /\
  exists S', shapeP_b P ft true [] (mmain m) = Some S'.
Proof.
  intros P m ft H. unfold region_checkP in H.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shapeP_b P ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate]. injection H as <-.
  split; [|exists S'; exact Em].
  intros f R g HR Hg. exact (proj2 (check_funcs_inv _ _ _ E) f g R Hg HR).
Qed.

Theorem reuse_sound_shared : forall P m ft, region_checkP P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros P m ft Hc host fuel n.
  destruct (region_checkP_wf P m ft Hc) as [WF [S' Hmain]].
  assert (Hgen : forall n sp sb, rel P sp sb -> stk sb = [] -> locs_ok sb ->
    match cycles P host m fuel n sp, cycles baseline host m fuel n sb with
    | Some sp', Some sb' => rel P sp' sb'
    | None, None => True
    | _, _ => False
    end).
  { induction n0 as [|n0 IH]; intros sp sb Hrel Hs Hl; simpl; [exact Hrel|].
    assert (Hm : matches sb []) by (unfold matches; rewrite Hs; constructor).
    pose proof (proj1 (simulation_shared P host (mfuncs m) ft WF fuel) (mmain m) true [] S' sp sb
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
  destruct Hgen as [_ [_ [Hg _]]]. exact Hg.
Qed.

(** ** The new checker contains the old one. *)
Lemma conflict_injective : forall P s t, injective P -> conflict P s t = true -> t = TR s.
Proof.
  intros P s [|s'] HP H; simpl in H; [discriminate|].
  apply orb_true_iff in H as [H|H]; [apply Nat.eqb_eq in H; subst; reflexivity|].
  destruct (P s) as [k|] eqn:E; [|discriminate]. destruct (P s') as [k'|] eqn:E'; [|discriminate].
  apply Nat.eqb_eq in H. subst k'. rewrite (HP s' s k E' E). reflexivity.
Qed.

Lemma existsb_conflict : forall P s r, injective P ->
  existsb (conflict P s) r = (if in_dec tag_eq_dec (TR s) r then true else false).
Proof.
  intros P s r HP. induction r as [|t r IH]; simpl; [reflexivity|].
  destruct (conflict P s t) eqn:Ec.
  - apply (conflict_injective P s t HP) in Ec as ->.
    destruct (tag_eq_dec (TR s) (TR s)); [reflexivity|contradiction].
  - rewrite IH. destruct (tag_eq_dec t (TR s)) as [->|Hne].
    + exfalso. simpl in Ec. rewrite Nat.eqb_refl in Ec. discriminate.
    + destruct (in_dec tag_eq_dec (TR s) r) as [Hin|Hnin].
      * destruct (in_dec tag_eq_dec (TR s) (t :: r)) as [_|C]; [reflexivity|exfalso; apply C; right; exact Hin].
      * destruct (in_dec tag_eq_dec (TR s) (t :: r)) as [[C|C]|_]; [congruence|contradiction|reflexivity].
Qed.

Lemma shape_injective_eq : forall P ft, injective P ->
  forall b reg S, shapeP_b P ft reg S b = shape_b ft reg S b.
Proof.
  intros P ft HP b.
  apply (block_mut (fun i => forall reg S, shapeP_i P ft reg S i = shape_i ft reg S i)
                   (fun b => forall reg S, shapeP_b P ft reg S b = shape_b ft reg S b));
    intros; simpl; try reflexivity.
  - destruct (tpop S) as [[|q] r]; [|reflexivity]. rewrite H, H0. reflexivity.
  - rewrite H. reflexivity.
  - destruct reg; [|reflexivity]. destruct (tpop S) as [[|q] r]; [|reflexivity].
    rewrite existsb_conflict by exact HP. destruct (in_dec tag_eq_dec (TR s) r); reflexivity.
  - rewrite H. destruct (shape_i ft reg S i); [apply H0|reflexivity].
Qed.

Theorem shared_generalises : forall P m, injective P -> region_checkP P m = region_check m.
Proof.
  intros P m HP. unfold region_checkP, region_check.
  destruct (check_funcs [] (mfuncs m)); [|reflexivity].
  rewrite shape_injective_eq by exact HP. reflexivity.
Qed.
