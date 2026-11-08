(** * Arm.v -- reuse is sound for every plan the join checker accepts.

    [reuse_sound_arm] is the headline result for arm groups.  For every
    module and plan that [region_checkA] accepts together, the reuse run and
    the baseline run either both stop or both finish with the same
    observation log, for every host, fuel and number of cycles.

    [arm_generalises] shows the join checker accepts everything [SharedShape]
    accepts, so with [Shared.shared_generalises] it also accepts everything
    increment B2's checker accepts under an injective plan. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim Reuse.
From SharedSlots Require Import SharedShape SharedSim Shared ArmShape ArmSim.
Import ListNotations.

Lemma region_checkA_wf : forall P m ft, region_checkA P m = Some ft ->
  (forall f R g, nth_error ft f = Some R -> nth_error (mfuncs m) f = Some g ->
     shape_b ft false [] g = Some R /\ forallb is_ts R = true) /\
  exists S', shapeA_b P ft true [] (mmain m) = Some S'.
Proof.
  intros P m ft H. unfold region_checkA in H.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shapeA_b P ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate]. injection H as <-.
  split; [|exists S'; exact Em].
  intros f R g HR Hg. exact (proj2 (check_funcs_inv _ _ _ E) f g R Hg HR).
Qed.

Theorem reuse_sound_arm : forall P m ft, region_checkA P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
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
  destruct Hgen as [_ [_ [Hg _]]]. exact Hg.
Qed.

(** ** The join checker contains the shared checker. *)
Lemma apop_embed : forall S t r, tpop S = (t, r) -> apop (map embed S) = (embed t, map embed r).
Proof. intros [|x S] t r H; simpl in H; injection H as <- <-; reflexivity. Qed.

Lemma ajoin_embed_refl : forall T, ajoin (map embed T) (map embed T) = Some (map embed T).
Proof.
  induction T as [|[|s] T IH]; [reflexivity|simpl; rewrite IH; reflexivity|].
  change (option_map (cons (AR (if list_eq_dec Nat.eq_dec [s] [s] then [s] else [s] ++ [s])))
            (ajoin (map embed T) (map embed T)) = Some (AR [s] :: map embed T)).
  rewrite IH. destruct (list_eq_dec Nat.eq_dec [s] [s]) as [_|C]; [reflexivity|contradiction].
Qed.

Lemma aconflict_embed : forall P s t, aconflict P s (embed t) = conflict P s t.
Proof. intros P s [|s']; simpl; [reflexivity|apply orb_false_r]. Qed.

Lemma existsb_embed : forall P s r, existsb (aconflict P s) (map embed r) = existsb (conflict P s) r.
Proof. intros P s r. induction r as [|t r IH]; simpl; [reflexivity|]. rewrite aconflict_embed, IH. reflexivity. Qed.

Lemma shape_embed : forall P ft b reg S S', shapeP_b P ft reg S b = Some S' ->
  shapeA_b P ft reg (map embed S) b = Some (map embed S').
Proof.
  intros P ft b.
  apply (block_mut (fun i => forall reg S S', shapeP_i P ft reg S i = Some S' ->
                               shapeA_i P ft reg (map embed S) i = Some (map embed S'))
                   (fun b => forall reg S S', shapeP_b P ft reg S b = Some S' ->
                               shapeA_b P ft reg (map embed S) b = Some (map embed S'))).
  - intros n reg S S' H. simpl in *. injection H as <-. reflexivity.
  - intros j reg S S' H. simpl in *. injection H as <-. reflexivity.
  - intros j reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    injection H as <-. rewrite (apop_embed S TS r E). reflexivity.
  - intros reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    destruct (tpop r) as [[|q] r2] eqn:E2; [|discriminate]. injection H as <-.
    rewrite (apop_embed S TS r E). simpl. rewrite (apop_embed r TS r2 E2). reflexivity.
  - intros reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    destruct (tpop r) as [[|q] r2] eqn:E2; [|discriminate]. injection H as <-.
    rewrite (apop_embed S TS r E). simpl. rewrite (apop_embed r TS r2 E2). reflexivity.
  - intros reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    destruct (tpop r) as [[|q] r2] eqn:E2; [|discriminate]. injection H as <-.
    rewrite (apop_embed S TS r E). simpl. rewrite (apop_embed r TS r2 E2). reflexivity.
  - intros te Ht e He reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    destruct (shapeP_b P ft reg r te) as [T|] eqn:ET; [|discriminate].
    destruct (shapeP_b P ft reg r e) as [E2|] eqn:EE; [|discriminate].
    destruct (list_eq_dec tag_eq_dec T E2) as [<-|]; [|discriminate]. injection H as <-.
    rewrite (apop_embed S TS r E). simpl. rewrite (Ht _ _ _ ET), (He _ _ _ EE). apply ajoin_embed_refl.
  - intros k body Hb reg S S' H. simpl in *.
    destruct (shapeP_b P ft reg S body) as [S2|] eqn:EB; [|discriminate].
    destruct (list_eq_dec tag_eq_dec S2 S) as [->|]; [|discriminate]. injection H as <-.
    rewrite (Hb _ _ _ EB). destruct (list_eq_dec atag_eq_dec (map embed S) (map embed S)) as [_|C];
      [reflexivity|contradiction].
  - intros f reg S S' H. simpl in *. destruct (nth_error ft f) as [R|]; [|discriminate].
    injection H as <-. rewrite map_app. reflexivity.
  - intros reg S S' H. simpl in *. destruct (tpop S) as [[|q] r] eqn:E; [|discriminate].
    injection H as <-. rewrite (apop_embed S TS r E). reflexivity.
  - intros s reg S S' H. simpl in *. destruct reg; [|discriminate].
    destruct (tpop S) as [[|q] r] eqn:E; [|discriminate]. rewrite (apop_embed S TS r E). simpl.
    rewrite existsb_embed. destruct (existsb (conflict P s) r); [discriminate|]. injection H as <-. reflexivity.
  - intros reg S S' H. simpl in *. destruct reg; [|discriminate].
    destruct (tpop S) as [[|q] r] eqn:E; [discriminate|]. injection H as <-.
    rewrite (apop_embed S (TR q) r E). reflexivity.
  - intros reg S S' H. simpl in *. destruct (tpop S) as [t r] eqn:E. injection H as <-.
    rewrite (apop_embed S t r E). reflexivity.
  - intros reg S S' H. simpl in *. injection H as <-. destruct (tpop S) as [t r] eqn:E.
    rewrite (apop_embed S t r E). reflexivity.
  - intros reg S S' H. simpl in *. injection H as <-. reflexivity.
  - intros i Hi r Hr reg S S' H. simpl in *.
    destruct (shapeP_i P ft reg S i) as [S1|] eqn:E1; [|discriminate].
    rewrite (Hi _ _ _ E1). exact (Hr _ _ _ H).
Qed.

Theorem arm_generalises : forall P m ft, region_checkP P m = Some ft -> region_checkA P m = Some ft.
Proof.
  intros P m ft H. unfold region_checkP, region_checkA in *.
  destruct (check_funcs [] (mfuncs m)) as [ft1|]; [|discriminate].
  destruct (shapeP_b P ft1 true [] (mmain m)) as [S'|] eqn:E; [|discriminate].
  pose proof (shape_embed P ft1 (mmain m) true [] S' E) as HE. simpl in HE. rewrite HE. exact H.
Qed.

Corollary arm_contains_b2 : forall P m ft, injective P -> region_check m = Some ft ->
  region_checkA P m = Some ft.
Proof.
  intros P m ft HP H. apply arm_generalises. rewrite shared_generalises by exact HP. exact H.
Qed.
