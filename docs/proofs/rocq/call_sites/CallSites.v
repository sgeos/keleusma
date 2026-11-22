(** * CallSites.v -- reuse is sound when functions allocate.

    [reuse_sound_calls] is the headline.  For every module [region_checkC]
    accepts, every plan giving distinct sites distinct slots, every host,
    every fuel and every number of cycles, the plan run and the baseline run
    either both stop or both finish with the same observation log, although
    functions may allocate and a reused site may be executed many times in
    one scope through repeated calls. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
From CallSites Require Import ShapeC Frame SimC.
Import ListNotations.

Lemma region_checkC_wf : forall m ft, region_checkC m = Some ft ->
  (forall f R A g, nth_error ft f = Some (R, A) -> nth_error (mfuncs m) f = Some g ->
     shapeC_b ft true [] g = Some R /\ forallb is_ts R = true /\ A = sites_b ft g) /\
  (forall f g, nth_error (mfuncs m) f = Some g ->
     exists R A, nth_error ft f = Some (R, A) /\ incl (sites_b ft g) A) /\
  exists S', shapeC_b ft true [] (mmain m) = Some S'.
Proof.
  intros m ft H. unfold region_checkC in H.
  destruct (check_funcsC [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shapeC_b ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate]. injection H as <-.
  destruct (check_funcsC_inv _ _ _ E) as [_ [Hl Hall]]. simpl in Hl.
  assert (WF : forall f R A g, nth_error ft1 f = Some (R, A) -> nth_error (mfuncs m) f = Some g ->
     shapeC_b ft1 true [] g = Some R /\ forallb is_ts R = true /\ A = sites_b ft1 g).
  { intros f R A g HR Hg. exact (Hall f g R A Hg HR). }
  split; [exact WF|]. split; [|exists S'; exact Em].
  intros f g Hg. destruct (nth_error ft1 f) as [[R A]|] eqn:Ef.
  - exists R, A. split; [reflexivity|]. destruct (WF f R A g Ef Hg) as [_ [_ ->]]. intros x Hx; exact Hx.
  - exfalso. apply nth_error_None in Ef. assert (f < length (mfuncs m)) by (apply nth_error_Some; congruence). lia.
Qed.

Theorem reuse_sound_calls : forall m ft, region_checkC m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros m ft Hc P Pinj host fuel n.
  destruct (region_checkC_wf m ft Hc) as [WF [Hfs [S' Hmain]]].
  assert (Hgen : forall n sp sb, rel P sp sb -> stk sb = [] -> locs_ok sb ->
    match cycles P host m fuel n sp, cycles baseline host m fuel n sb with
    | Some sp', Some sb' => rel P sp' sb'
    | None, None => True
    | _, _ => False
    end).
  { induction n0 as [|n0 IH]; intros sp sb Hrel Hs Hl; simpl; [exact Hrel|].
    assert (Hm : matches sb []) by (unfold matches; rewrite Hs; constructor).
    pose proof (proj1 (simulationC P Pinj host (mfuncs m) ft WF Hfs fuel) (mmain m) true [] S' sp sb
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
