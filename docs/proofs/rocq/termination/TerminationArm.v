(** * TerminationArm.v -- the join checker's accepted modules finish too.

    The join checker of [../shared_slots/] checks functions exactly as B2's
    checker does, through [check_funcs], and only the main block by its own
    rules, [shapeA_b].  So the termination argument of [Termination.v] needs
    one more fact, that a main block the join checker accepts calls only
    functions of the module.  With it, every module and plan the join checker
    accepts together finishes at [bound m], for every plan, host, starting
    state and number of cycles, and with [reuse_sound_arm] both runs finish
    and agree, with no injectivity premise. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Sim Reuse.
From SharedSlots Require Import SharedShape ArmShape Arm.
From Termination Require Import Termination.
Import ListNotations.

Lemma shapeA_calls : forall P ft b reg S S', shapeA_b P ft reg S b = Some S' -> calls_b (length ft) b.
Proof.
  intros P ft b.
  apply (block_mut (fun i => forall reg S S', shapeA_i P ft reg S i = Some S' -> calls_i (length ft) i)
                   (fun b => forall reg S S', shapeA_b P ft reg S b = Some S' -> calls_b (length ft) b));
    try (intros; exact I).
  - intros t Ht e He reg S S' H. simpl in H. destruct (apop S) as [[|q] r]; [|discriminate].
    destruct (shapeA_b P ft reg r t) as [T|] eqn:ET; [|discriminate].
    destruct (shapeA_b P ft reg r e) as [E|] eqn:EE; [|discriminate].
    split; [exact (Ht _ _ _ ET)|exact (He _ _ _ EE)].
  - intros k body Hb reg S S' H. simpl in H.
    destruct (shapeA_b P ft reg S body) as [S2|] eqn:E; [|discriminate]. exact (Hb _ _ _ E).
  - intros f reg S S' H. simpl in H. destruct (nth_error ft f) eqn:E; [|discriminate].
    simpl. apply nth_error_Some. congruence.
  - intros i Hi r Hr reg S S' H. simpl in H.
    destruct (shapeA_i P ft reg S i) as [S1|] eqn:E; [|discriminate].
    split; [exact (Hi _ _ _ E)|exact (Hr _ _ _ H)].
Qed.

Lemma accepted_goodA : forall P m ft, region_checkA P m = Some ft ->
  calls_b (length (mfuncs m)) (mmain m) /\
  forall f g, nth_error (mfuncs m) f = Some g ->
    calls_b (length (mfuncs m)) g /\ fuel_b (ftab [] (mfuncs m)) g <= nth f (ftab [] (mfuncs m)) 0.
Proof.
  intros P m ft H. unfold region_checkA in H.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shapeA_b P ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate].
  destruct (check_funcs_calls _ _ _ E) as [Hc Hl]. simpl in Hl.
  split; [rewrite <- Hl; exact (shapeA_calls _ _ _ _ _ _ Em)|].
  intros f g Hn. pose proof (calls_from_nth _ _ _ _ Hc Hn) as Hcg. simpl in Hcg.
  split.
  - apply (calls_mono f); [|exact Hcg]. apply Nat.lt_le_incl. apply nth_error_Some. congruence.
  - pose proof (ftab_nth (mfuncs m) [] f g Hn) as Ht. simpl in Ht. rewrite Ht.
    erewrite (fuel_agree f (firstn f (ftab [] (mfuncs m))) (ftab [] (mfuncs m))); [lia| |exact Hcg].
    intros f' Hf'. apply nth_firstn_lt. exact Hf'.
Qed.

Theorem accepted_terminates_arm : forall P m ft, region_checkA P m = Some ft ->
  forall Q host n s, exists s', cycles Q host m (bound m) n s = Some s'.
Proof.
  intros P m ft H Q host n. destruct (accepted_goodA P m ft H) as [Hmain Hgood].
  induction n as [|n IH]; intros s; simpl; [exists s; reflexivity|].
  destruct (proj1 (runs Q host (mfuncs m) (ftab [] (mfuncs m)) Hgood (bound m)) (mmain m) s Hmain (le_n _))
    as [s1 E].
  rewrite E. simpl. apply IH.
Qed.

(** With [reuse_sound_arm], both runs finish at [bound m] and agree, for
    every module and plan the join checker accepts together. *)
Corollary accepted_runs_agree_arm : forall P m ft, region_checkA P m = Some ft ->
  forall host n,
  exists sp sb, cycles P host m (bound m) n init = Some sp /\
                cycles baseline host m (bound m) n init = Some sb /\ log sp = log sb.
Proof.
  intros P m ft H host n.
  destruct (accepted_terminates_arm P m ft H P host n init) as [sp Ep].
  destruct (accepted_terminates_arm P m ft H baseline host n init) as [sb Eb].
  pose proof (reuse_sound_arm P m ft H host (bound m) n) as R. rewrite Ep, Eb in R.
  exists sp, sb. split; [exact Ep|]. split; [exact Eb|exact R].
Qed.
