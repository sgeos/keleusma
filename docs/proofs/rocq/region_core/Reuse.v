(** * Reuse.v -- reuse is sound for every module the checker accepts.

    [reuse_sound] is the headline result of increment B2.  For every module
    [region_check] accepts, every plan giving distinct sites distinct slots,
    every host, every fuel and every number of stream cycles, the plan run and
    the baseline run either both stop or both finish with the same
    observation log.  The plan may reuse every site at once. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim.
Import ListNotations.

(** A shape computed against a table stays the same against an extension of
    it, because only earlier entries are consulted. *)
Lemma shape_extend : forall ft u,
  (forall b reg S S', shape_b ft reg S b = Some S' -> shape_b (ft ++ u) reg S b = Some S').
Proof.
  intros ft u.
  apply (block_mut (fun i => forall reg S S', shape_i ft reg S i = Some S' -> shape_i (ft ++ u) reg S i = Some S')
                   (fun b => forall reg S S', shape_b ft reg S b = Some S' -> shape_b (ft ++ u) reg S b = Some S')).
  1-6: intros; simpl in *; assumption.
  - intros te Ht e He reg S S' H. simpl in *.
    destruct (tpop S) as [[|q] r]; [|discriminate].
    destruct (shape_b ft reg r te) as [T|] eqn:E1; [|discriminate].
    destruct (shape_b ft reg r e) as [E|] eqn:E2; [|discriminate].
    rewrite (Ht _ _ _ E1), (He _ _ _ E2). exact H.
  - intros k body Hb reg S S' H. simpl in *.
    destruct (shape_b ft reg S body) as [S2|] eqn:E1; [|discriminate]. rewrite (Hb _ _ _ E1). exact H.
  - intros f reg S S' H. simpl in *. destruct (nth_error ft f) eqn:E1; [|discriminate].
    rewrite nth_error_app1 by (apply nth_error_Some; rewrite E1; discriminate). rewrite E1. exact H.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros i Hi r Hr reg S S' H. simpl in *.
    destruct (shape_i ft reg S i) as [S1|] eqn:E1; [|discriminate]. rewrite (Hi _ _ _ E1). exact (Hr _ _ _ H).
Qed.

Lemma check_funcs_inv : forall gs ft0 ft, check_funcs ft0 gs = Some ft ->
  (exists u, ft = ft0 ++ u) /\
  (forall i g R, nth_error gs i = Some g -> nth_error ft (length ft0 + i) = Some R ->
     shape_b ft false [] g = Some R /\ forallb is_ts R = true).
Proof.
  induction gs as [|g gs IH]; intros ft0 ft H; simpl in H.
  - injection H as <-. split; [exists []; rewrite app_nil_r; reflexivity|]. intros [|i] g R Hg; discriminate.
  - destruct (shape_b ft0 false [] g) as [R0|] eqn:E; [|discriminate].
    destruct (forallb is_ts R0) eqn:K; [|discriminate].
    destruct (IH _ _ H) as [[u Hu] Hall].
    split; [exists ([R0] ++ u); rewrite Hu, app_assoc; reflexivity|].
    intros [|i] g' R Hg HR; simpl in Hg.
    + injection Hg as <-. rewrite Nat.add_0_r, Hu, <- app_assoc in HR.
      rewrite nth_error_app2, Nat.sub_diag in HR by lia. simpl in HR. injection HR as <-.
      split; [|exact K]. rewrite Hu, <- app_assoc. apply shape_extend. exact E.
    + replace (length ft0 + S i) with (length (ft0 ++ [R0]) + i) in HR by (rewrite length_app; simpl; lia).
      exact (Hall i g' R Hg HR).
Qed.

Lemma region_check_wf : forall m ft, region_check m = Some ft ->
  (forall f R g, nth_error ft f = Some R -> nth_error (mfuncs m) f = Some g ->
     shape_b ft false [] g = Some R /\ forallb is_ts R = true) /\
  exists S', shape_b ft true [] (mmain m) = Some S'.
Proof.
  intros m ft H. unfold region_check in H.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shape_b ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate]. injection H as <-.
  split; [|exists S'; exact Em].
  intros f R g HR Hg. exact (proj2 (check_funcs_inv _ _ _ E) f g R Hg HR).
Qed.

Theorem reuse_sound : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
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
  destruct Hgen as [_ [_ [Hg _]]]. exact Hg.
Qed.
