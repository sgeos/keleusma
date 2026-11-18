(** * Termination.v -- every module B2's checker accepts finishes.

    Increment B2's [reuse_sound] says the reuse run and the baseline run
    either both stop for lack of fuel or both finish with the same log.  It
    leaves open whether an accepted module finishes at all, so for all it
    shows, both runs might always stop.  This module closes that.  The checker
    checks each function against the result shapes of earlier functions only,
    so a call names an earlier function and no call chain recurses.  From the
    block structure alone a fuel [bound m] is computed, and at that fuel every
    run of an accepted module finishes, for every plan, host, starting state
    and number of cycles.  With [reuse_sound], the reuse run and the baseline
    run then both finish with the same log. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Sim Reuse.
Import ListNotations.

(** ** Calls name functions below a limit. *)
Fixpoint calls_b (K : nat) (b : block) : Prop :=
  match b with BNil => True | BCons i r => calls_i K i /\ calls_b K r end
with calls_i (K : nat) (i : instr) : Prop :=
  match i with
  | If t e => calls_b K t /\ calls_b K e
  | LoopN _ body => calls_b K body
  | Call f => f < K
  | _ => True
  end.

Lemma shape_calls : forall ft b reg S S', shape_b ft reg S b = Some S' -> calls_b (length ft) b.
Proof.
  intros ft b.
  apply (block_mut (fun i => forall reg S S', shape_i ft reg S i = Some S' -> calls_i (length ft) i)
                   (fun b => forall reg S S', shape_b ft reg S b = Some S' -> calls_b (length ft) b));
    try (intros; exact I).
  - intros t Ht e He reg S S' H. simpl in H. destruct (tpop S) as [[|q] r]; [|discriminate].
    destruct (shape_b ft reg r t) as [T|] eqn:ET; [|discriminate].
    destruct (shape_b ft reg r e) as [E|] eqn:EE; [|discriminate].
    split; [exact (Ht _ _ _ ET)|exact (He _ _ _ EE)].
  - intros k body Hb reg S S' H. simpl in H.
    destruct (shape_b ft reg S body) as [S2|] eqn:E; [|discriminate]. exact (Hb _ _ _ E).
  - intros f reg S S' H. simpl in H. destruct (nth_error ft f) eqn:E; [|discriminate].
    simpl. apply nth_error_Some. congruence.
  - intros i Hi r Hr reg S S' H. simpl in H.
    destruct (shape_i ft reg S i) as [S1|] eqn:E; [|discriminate].
    split; [exact (Hi _ _ _ E)|exact (Hr _ _ _ H)].
Qed.

Lemma calls_mono : forall K K' b, K <= K' -> calls_b K b -> calls_b K' b.
Proof.
  intros K K' b HK.
  apply (block_mut (fun i => calls_i K i -> calls_i K' i) (fun b => calls_b K b -> calls_b K' b));
    simpl; try tauto; try lia.
Qed.

(** The functions of a module, each calling only functions before it. *)
Fixpoint calls_from (k : nat) (gs : list block) : Prop :=
  match gs with [] => True | g :: gs' => calls_b k g /\ calls_from (S k) gs' end.

Lemma calls_from_nth : forall gs k i g, calls_from k gs -> nth_error gs i = Some g -> calls_b (k + i) g.
Proof.
  induction gs as [|g' gs IH]; intros k i g Hc Hn; [destruct i; discriminate|].
  destruct Hc as [Hg Hr]. destruct i as [|i]; simpl in Hn.
  - injection Hn as <-. rewrite Nat.add_0_r. exact Hg.
  - replace (k + S i) with (S k + i) by lia. exact (IH _ _ _ Hr Hn).
Qed.

Lemma check_funcs_calls : forall gs ft0 ft, check_funcs ft0 gs = Some ft ->
  calls_from (length ft0) gs /\ length ft = length ft0 + length gs.
Proof.
  induction gs as [|g gs IH]; intros ft0 ft H; simpl in H.
  - injection H as <-. split; [exact I|simpl; lia].
  - destruct (shape_b ft0 false [] g) as [R|] eqn:E; [|discriminate].
    destruct (forallb is_ts R); [|discriminate].
    destruct (IH _ _ H) as [Hc Hl]. rewrite length_app in Hc, Hl. simpl in Hc, Hl.
    split; [split|simpl; lia].
    + exact (shape_calls _ _ _ _ _ E).
    + rewrite Nat.add_1_r in Hc. exact Hc.
Qed.

(** ** The fuel a block needs.  [F] gives each function's fuel. *)
Fixpoint fuel_b (F : list nat) (b : block) : nat :=
  match b with BNil => 1 | BCons i r => S (Nat.max (fuel_i F i) (fuel_b F r)) end
with fuel_i (F : list nat) (i : instr) : nat :=
  match i with
  | If t e => S (Nat.max (fuel_b F t) (fuel_b F e))
  | LoopN k body => S (k + S (fuel_b F body))
  | Call f => S (nth f F 0)
  | _ => 1
  end.

Lemma fuel_agree : forall K F1 F2 b, (forall f, f < K -> nth f F1 0 = nth f F2 0) ->
  calls_b K b -> fuel_b F1 b = fuel_b F2 b.
Proof.
  intros K F1 F2 b HF.
  apply (block_mut (fun i => calls_i K i -> fuel_i F1 i = fuel_i F2 i)
                   (fun b => calls_b K b -> fuel_b F1 b = fuel_b F2 b)); simpl; intros; try reflexivity.
  - destruct H1. rewrite H, H0 by assumption. reflexivity.
  - rewrite H by assumption. reflexivity.
  - rewrite HF by assumption. reflexivity.
  - destruct H1. rewrite H, H0 by assumption. reflexivity.
Qed.

(** The fuel table, built in function order. *)
Fixpoint ftab (F : list nat) (gs : list block) : list nat :=
  match gs with [] => F | g :: gs' => ftab (F ++ [fuel_b F g]) gs' end.

Lemma ftab_prefix : forall gs F, exists rest, ftab F gs = F ++ rest.
Proof.
  induction gs as [|g gs IH]; intros F; simpl; [exists []; rewrite app_nil_r; reflexivity|].
  destruct (IH (F ++ [fuel_b F g])) as [rest E]. rewrite E, <- app_assoc. eexists. reflexivity.
Qed.

Lemma nth_firstn_lt : forall (l : list nat) i f, f < i -> nth f (firstn i l) 0 = nth f l 0.
Proof.
  induction l as [|x l IH]; intros i f H; destruct i as [|i]; try lia; simpl; [destruct f; reflexivity|].
  destruct f as [|f]; [reflexivity|]. apply IH. lia.
Qed.

Lemma ftab_nth : forall gs F i g, nth_error gs i = Some g ->
  nth (length F + i) (ftab F gs) 0 = fuel_b (firstn (length F + i) (ftab F gs)) g.
Proof.
  induction gs as [|g' gs IH]; intros F i g Hn; [destruct i; discriminate|].
  destruct i as [|i]; simpl in Hn |- *.
  - injection Hn as ->. rewrite Nat.add_0_r.
    destruct (ftab_prefix gs (F ++ [fuel_b F g])) as [rest E]. rewrite E, <- app_assoc.
    rewrite app_nth2 by lia. rewrite Nat.sub_diag. simpl.
    rewrite firstn_app, Nat.sub_diag, firstn_all. simpl. rewrite app_nil_r. reflexivity.
  - pose proof (IH (F ++ [fuel_b F g']) i g Hn) as H. rewrite length_app in H. simpl in H.
    replace (length F + S i) with (length F + 1 + i) by lia. exact H.
Qed.

(** ** Every run within the fuel finishes. *)
Section Run.

Variable P : plan.
Variable host : list ev -> nat.
Variable fs : list block.
Variable F : list nat.
Hypothesis good : forall f g, nth_error fs f = Some g ->
  calls_b (length fs) g /\ fuel_b F g <= nth f F 0.

Lemma fuel_b_pos : forall b, 1 <= fuel_b F b.
Proof. destruct b; simpl; lia. Qed.

Lemma runs : forall n,
  (forall b s, calls_b (length fs) b -> fuel_b F b <= n -> exists s', run_b P host fs n b s = Some s') /\
  (forall i s, calls_i (length fs) i -> fuel_i F i <= n -> exists s', run_i P host fs n i s = Some s') /\
  (forall k b s, calls_b (length fs) b -> k + S (fuel_b F b) <= n ->
     exists s', run_loop P host fs n k b s = Some s').
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { split; [|split].
    - intros b s _ H. pose proof (fuel_b_pos b). lia.
    - intros i s _ H. destruct i; simpl in H; lia.
    - intros k b s _ H. lia. }
  split; [|split].
  - intros b s Hc Hf. destruct b as [|i r]; simpl; [exists s; reflexivity|].
    simpl in Hc, Hf. destruct Hc as [Hci Hcr].
    destruct (IHi i s Hci ltac:(lia)) as [s1 E]. rewrite E. simpl. apply IHb; [exact Hcr|lia].
  - intros i s Hc Hf. destruct i; simpl in Hc, Hf |- *;
      try (repeat match goal with |- context [pop ?x] => destruct (pop x) end; eexists; reflexivity).
    + destruct (pop s) as [c s1]. destruct Hc as [Ht He].
      destruct (Nat.eqb (num c) 0); apply IHb; try assumption; lia.
    + apply IHl; [exact Hc|lia].
    + destruct (nth_error fs f) as [g|] eqn:E.
      * destruct (good f g E) as [Hcg Hfg].
        destruct (IHb g (with_stk s []) Hcg ltac:(lia)) as [s2 E2]. rewrite E2. simpl.
        eexists; reflexivity.
      * apply nth_error_None in E. lia.
  - intros k b s Hc Hf. destruct k as [|k]; simpl; [exists s; reflexivity|].
    destruct (IHb b s Hc ltac:(lia)) as [s1 E]. rewrite E. simpl. apply IHl; [exact Hc|lia].
Qed.

End Run.

(** ** The headline. *)
Definition bound (m : module) : nat := fuel_b (ftab [] (mfuncs m)) (mmain m).

Lemma accepted_good : forall m ft, region_check m = Some ft ->
  calls_b (length (mfuncs m)) (mmain m) /\
  forall f g, nth_error (mfuncs m) f = Some g ->
    calls_b (length (mfuncs m)) g /\ fuel_b (ftab [] (mfuncs m)) g <= nth f (ftab [] (mfuncs m)) 0.
Proof.
  intros m ft H. unfold region_check in H.
  destruct (check_funcs [] (mfuncs m)) as [ft1|] eqn:E; [|discriminate].
  destruct (shape_b ft1 true [] (mmain m)) as [S'|] eqn:Em; [|discriminate].
  destruct (check_funcs_calls _ _ _ E) as [Hc Hl]. simpl in Hl.
  split; [rewrite <- Hl; exact (shape_calls _ _ _ _ _ Em)|].
  intros f g Hn. pose proof (calls_from_nth _ _ _ _ Hc Hn) as Hcg. simpl in Hcg.
  split.
  - apply (calls_mono f); [|exact Hcg]. apply Nat.lt_le_incl. apply nth_error_Some. congruence.
  - pose proof (ftab_nth (mfuncs m) [] f g Hn) as Ht. simpl in Ht. rewrite Ht.
    erewrite (fuel_agree f (firstn f (ftab [] (mfuncs m))) (ftab [] (mfuncs m))); [lia| |exact Hcg].
    intros f' Hf'. apply nth_firstn_lt. exact Hf'.
Qed.

Theorem accepted_terminates : forall m ft, region_check m = Some ft ->
  forall P host n s, exists s', cycles P host m (bound m) n s = Some s'.
Proof.
  intros m ft H P host n. destruct (accepted_good m ft H) as [Hmain Hgood].
  induction n as [|n IH]; intros s; simpl; [exists s; reflexivity|].
  destruct (proj1 (runs P host (mfuncs m) (ftab [] (mfuncs m)) Hgood (bound m)) (mmain m) s Hmain (le_n _))
    as [s1 E].
  rewrite E. simpl. apply IH.
Qed.

(** With [reuse_sound], both runs of an accepted module finish at [bound m]
    and agree, so B2's theorem is never satisfied by both runs stopping. *)
Corollary accepted_runs_agree : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host n,
  exists sp sb, cycles P host m (bound m) n init = Some sp /\
                cycles baseline host m (bound m) n init = Some sb /\ log sp = log sb.
Proof.
  intros m ft H P HP host n.
  destruct (accepted_terminates m ft H P host n init) as [sp Ep].
  destruct (accepted_terminates m ft H baseline host n init) as [sb Eb].
  pose proof (reuse_sound m ft H P HP host (bound m) n) as R. rewrite Ep, Eb in R.
  exists sp, sb. split; [exact Ep|]. split; [exact Eb|exact R].
Qed.
