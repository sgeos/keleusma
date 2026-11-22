(** * Contains.v -- the call-site checker accepts everything B2's accepts.

    [lift] gives each entry of B2's function table no allocation sites.  On
    a lifted table the call rule is vacuous, so [shapeC_b] computes exactly
    what [shape_b] computes.  B2 checks functions with allocation forbidden,
    so they allocate nothing and their sites are empty, and a block accepted
    with allocation forbidden is accepted with it allowed.  Hence every
    module [region_check] accepts, [region_checkC] accepts, with the lifted
    table, and [reuse_sound_calls] covers everything [reuse_sound] covers. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
From CallSites Require Import ShapeC.
Import ListNotations.

Definition lift (ft : list (list tag)) : list fentry := map (fun R => (R, [])) ft.

Lemma nth_lift : forall ft f, nth_error (lift ft) f = option_map (fun R => (R, [])) (nth_error ft f).
Proof. intros ft f. unfold lift. apply nth_error_map. Qed.

Lemma existsb_live_nil : forall S, existsb (live_in []) S = false.
Proof. induction S as [|t S IH]; simpl; [reflexivity|]. rewrite IH. destruct t; reflexivity. Qed.

Lemma shapeC_lift : forall ft b reg S, shapeC_b (lift ft) reg S b = shape_b ft reg S b.
Proof.
  intros ft b.
  apply (block_mut (fun i => forall reg S, shapeC_i (lift ft) reg S i = shape_i ft reg S i)
                   (fun b => forall reg S, shapeC_b (lift ft) reg S b = shape_b ft reg S b));
    try (intros; reflexivity).
  - intros t Ht e He reg S. simpl. destruct (tpop S) as [[|q] r]; [|reflexivity]. rewrite Ht, He. reflexivity.
  - intros k body Hb reg S. simpl. rewrite Hb. reflexivity.
  - intros f reg S. simpl. rewrite nth_lift. destruct (nth_error ft f); simpl; [|reflexivity].
    rewrite existsb_live_nil. reflexivity.
  - intros i Hi r Hr reg S. simpl. rewrite Hi. destruct (shape_i ft reg S i); [apply Hr|reflexivity].
Qed.

(** A block accepted with allocation forbidden allocates nothing, so on a
    lifted table it has no sites, and it is accepted with allocation allowed. *)
Lemma no_alloc : forall ft b S S', shape_b ft false S b = Some S' ->
  sites_b (lift ft) b = [] /\ shape_b ft true S b = Some S'.
Proof.
  intros ft b.
  apply (block_mut (fun i => forall S S', shape_i ft false S i = Some S' ->
                               sites_i (lift ft) i = [] /\ shape_i ft true S i = Some S')
                   (fun b => forall S S', shape_b ft false S b = Some S' ->
                               sites_b (lift ft) b = [] /\ shape_b ft true S b = Some S')).
  1-6: intros; simpl in *; split; [reflexivity|assumption].
  - intros t Ht e He S S' H. simpl in *. destruct (tpop S) as [[|q] r]; [|discriminate].
    destruct (shape_b ft false r t) as [T|] eqn:ET; [|discriminate].
    destruct (shape_b ft false r e) as [E|] eqn:EE; [|discriminate].
    destruct (Ht _ _ ET) as [A1 B1]. destruct (He _ _ EE) as [A2 B2]. rewrite A1, A2, B1, B2.
    split; [reflexivity|exact H].
  - intros k body Hb S S' H. simpl in *. destruct (shape_b ft false S body) as [S2|] eqn:E; [|discriminate].
    destruct (Hb _ _ E) as [A1 B1]. rewrite A1, B1. split; [reflexivity|exact H].
  - intros f S S' H. simpl in *. rewrite nth_lift. destruct (nth_error ft f); simpl; [|discriminate].
    split; [reflexivity|exact H].
  - intros; simpl in *; split; [reflexivity|assumption].
  - intros s S S' H. simpl in H. discriminate.
  - intros S S' H. simpl in H. discriminate.
  - intros; simpl in *; split; [reflexivity|assumption].
  - intros; simpl in *; split; [reflexivity|assumption].
  - intros; simpl in *; split; [reflexivity|assumption].
  - intros i Hi r Hr S S' H. simpl in *. destruct (shape_i ft false S i) as [S1|] eqn:E; [|discriminate].
    destruct (Hi _ _ E) as [A1 B1]. destruct (Hr _ _ H) as [A2 B2]. rewrite A1, A2, B1, B2.
    split; reflexivity.
Qed.

Lemma check_funcs_lift : forall gs ft0 ft, check_funcs ft0 gs = Some ft ->
  check_funcsC (lift ft0) gs = Some (lift ft).
Proof.
  induction gs as [|g gs IH]; intros ft0 ft H; simpl in H |- *.
  - injection H as <-. reflexivity.
  - destruct (shape_b ft0 false [] g) as [R|] eqn:E; [|discriminate].
    destruct (forallb is_ts R) eqn:K; [|discriminate].
    destruct (no_alloc _ _ _ _ E) as [Hs Ht]. rewrite shapeC_lift, Ht, K, Hs.
    replace (lift ft0 ++ [(R, [])]) with (lift (ft0 ++ [R])) by (unfold lift; rewrite map_app; reflexivity).
    exact (IH _ _ H).
Qed.

Theorem calls_contains_b2 : forall m ft, region_check m = Some ft -> region_checkC m = Some (lift ft).
Proof.
  intros m ft H. unfold region_check in H. unfold region_checkC.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  pose proof (check_funcs_lift _ _ _ E) as L. change (lift []) with (@nil fentry) in L.
  rewrite L, shapeC_lift.
  destruct (shape_b ft1 true [] (mmain m)); [|discriminate]. injection H as <-. reflexivity.
Qed.
