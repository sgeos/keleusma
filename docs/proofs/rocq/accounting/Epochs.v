(** * Epochs.v -- a dereference reads only the current epoch's cells.

    [Footprint.v] counts the cells each epoch's allocations use.  Reading
    that count as the memory one cycle needs assumes that during an epoch
    the run reads only cells of that epoch's allocations, so that earlier
    epochs' storage could be reclaimed at the reset.  This module proves the
    two facts that assumption rests on.

    - [run_ok] and [cycles_ok]: every handle on the operand stack or in a
      local names a recorded allocation whose epoch is the handle's own, and
      the allocation record only grows.  Every instruction, call and reset
      preserves this, for every plan and host.
    - [deref_current]: from a state where that holds, a dereference that
      succeeds reads the cell of an allocation of the current epoch, that is
      a cell counted by [footprint_at] for the current epoch.

    The invariant is proved of the final state of every run, and the proof
    carries it through every instruction.  The statements here do not
    expose the intermediate states of a run, so that the dereference at each
    [Get] of a run reads a current cell follows from the structure of the
    proof together with [deref_current], not from one theorem statement. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core.
From Accounting Require Import Accounting.
Import ListNotations.

Definition hok (s : state) (v : val) : Prop :=
  match v with VH r e => exists site, nth_error (regs s) r = Some (site, e) | VN _ => True end.

Definition handles_ok (s : state) : Prop :=
  (forall v, In v (stk s) -> hok s v) /\ (forall j, hok s (locs s j)).

Definition grows (s s' : state) : Prop := exists l, regs s' = regs s ++ l.

Lemma grows_refl : forall s s', regs s' = regs s -> grows s s'.
Proof. intros s s' H. exists []. rewrite app_nil_r. exact H. Qed.

Lemma grows_trans : forall a b c, grows a b -> grows b c -> grows a c.
Proof. intros a b c [l1 H1] [l2 H2]. exists (l1 ++ l2). rewrite H2, H1, app_assoc. reflexivity. Qed.

Lemma hok_grows : forall s s' v, grows s s' -> hok s v -> hok s' v.
Proof.
  intros s s' [|r e] [l Hl] H; [exact I|]. destruct H as [site Hn]. exists site.
  rewrite Hl. rewrite nth_error_app1; [exact Hn|]. apply nth_error_Some. congruence.
Qed.

Lemma ok_grows : forall s s', grows s s' -> stk s' = stk s -> locs s' = locs s ->
  handles_ok s -> handles_ok s'.
Proof.
  intros s s' G Hs Hl [Hk Hlc]. split.
  - intros v Hv. rewrite Hs in Hv. exact (hok_grows _ _ _ G (Hk v Hv)).
  - intros j. rewrite Hl. exact (hok_grows _ _ _ G (Hlc j)).
Qed.

Lemma pop_ok : forall s v s1, pop s = (v, s1) -> handles_ok s ->
  hok s v /\ handles_ok s1 /\ regs s1 = regs s.
Proof.
  intros s v s1 H [Hk Hl]. unfold pop in H. destruct (stk s) as [|x r] eqn:E.
  - injection H as <- <-. split; [exact I|]. split; [split; [intros w Hw; rewrite E in Hw; destruct Hw|exact Hl]|reflexivity].
  - injection H as <- <-. split; [apply Hk; left; reflexivity|].
    split; [|reflexivity]. split.
    + intros v Hv. apply Hk. right. exact Hv.
    + exact Hl.
Qed.

Lemma push_ok : forall s v, handles_ok s -> hok s v -> handles_ok (push v s).
Proof.
  intros s v [Hk Hl] Hv. split; [|exact Hl].
  intros w [<-|Hw]; [exact Hv|exact (Hk w Hw)].
Qed.

Lemma note_ok : forall s e, handles_ok s -> handles_ok (note e s).
Proof. intros s e [Hk Hl]. split; [exact Hk|exact Hl]. Qed.

Lemma store_ok : forall s j v, handles_ok s -> hok s v -> handles_ok (store j v s).
Proof.
  intros s j v [Hk Hl] Hv. split; [exact Hk|].
  intros j'. simpl. destruct (Nat.eqb j' j); [exact Hv|exact (Hl j')].
Qed.

Lemma new_ok : forall P site s, handles_ok s ->
  handles_ok (new_step P site s) /\ grows s (new_step P site s).
Proof.
  intros P site s Hs. unfold new_step. destruct (pop s) as [v s1] eqn:E.
  destruct (pop_ok _ _ _ E Hs) as [_ [[Hk Hl] Hr]].
  assert (G : grows s1 (mkstate (VH (length (regs s1)) (epoch s1) :: stk s1) (locs s1) (log s1)
             (regs s1 ++ [(site, epoch s1)])
             (fun c' => if cell_eq_dec c' (cell_of P site (length (regs s1))) then Some (num v) else mem s1 c')
             (epoch s1))) by (exists [(site, epoch s1)]; reflexivity).
  split.
  - split.
    + intros w [<-|Hw].
      * exists site. simpl. rewrite nth_error_app2 by lia. rewrite Nat.sub_diag. reflexivity.
      * exact (hok_grows _ _ _ G (Hk w Hw)).
    + intros j. exact (hok_grows _ _ _ G (Hl j)).
  - destruct G as [l Hg]. exists l. rewrite Hg, Hr. reflexivity.
Qed.

Ltac pop_all Hs :=
  repeat match goal with
  | H : context [pop ?x] |- _ =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E;
      let Hv := fresh "Hv" in let Ht := fresh "Ht" in let Hr := fresh "Hr" in
      destruct (pop_ok _ _ _ E ltac:(assumption)) as [Hv [Ht Hr]]; clear E
  | |- context [pop ?x] =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E;
      let Hv := fresh "Hv" in let Ht := fresh "Ht" in let Hr := fresh "Hr" in
      destruct (pop_ok _ _ _ E ltac:(assumption)) as [Hv [Ht Hr]]; clear E
  end.

Lemma run_ok : forall P host fs n,
  (forall b s s', run_b P host fs n b s = Some s' -> handles_ok s -> handles_ok s' /\ grows s s') /\
  (forall i s s', run_i P host fs n i s = Some s' -> handles_ok s -> handles_ok s' /\ grows s s') /\
  (forall k b s s', run_loop P host fs n k b s = Some s' -> handles_ok s -> handles_ok s' /\ grows s s').
Proof.
  intros P host fs n. induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; discriminate. }
  split; [|split].
  - intros b s s' H Hs. destruct b as [|i r]; simpl in H.
    + injection H as <-. split; [exact Hs|apply grows_refl; reflexivity].
    + unfold bind in H. destruct (run_i P host fs n i s) as [s1|] eqn:E; [|discriminate].
      destruct (IHi _ _ _ E Hs) as [H1 G1]. destruct (IHb _ _ _ H H1) as [H2 G2].
      split; [exact H2|exact (grows_trans _ _ _ G1 G2)].
  - intros i s s' H Hs. destruct i; simpl in H.
    + injection H as <-. split; [apply push_ok; [exact Hs|exact I]|apply grows_refl; reflexivity].
    + injection H as <-. split; [apply push_ok; [exact Hs|exact (proj2 Hs i)]|apply grows_refl; reflexivity].
    + pop_all Hs. injection H as <-. split.
      * apply store_ok; [exact Ht|]. eapply hok_grows; [apply grows_refl; exact Hr|exact Hv].
      * apply grows_refl. simpl. exact Hr.
    + pop_all Hs. injection H as <-. split; [apply push_ok; [assumption|exact I]|apply grows_refl; simpl; congruence].
    + pop_all Hs. injection H as <-. split; [apply push_ok; [assumption|exact I]|apply grows_refl; simpl; congruence].
    + pop_all Hs. injection H as <-. split; [apply push_ok; [assumption|exact I]|apply grows_refl; simpl; congruence].
    + pop_all Hs. destruct (IHb _ _ _ H Ht) as [H2 G2].
      split; [exact H2|exact (grows_trans _ _ _ (grows_refl _ _ Hr) G2)].
    + exact (IHl _ _ _ _ H Hs).
    + destruct (nth_error fs f) as [g|]; [|discriminate]. unfold bind in H.
      destruct (run_b P host fs n g (with_stk s [])) as [s2|] eqn:E; [|discriminate].
      injection H as <-.
      assert (H0 : handles_ok (with_stk s [])) by (split; [intros v []|exact (proj2 Hs)]).
      destruct (IHb _ _ _ E H0) as [[Hk2 Hl2] G2].
      assert (G : grows s s2) by (destruct G2 as [l Hl]; exists l; exact Hl).
      split; [|exact G]. split; [|exact Hl2].
      intros v Hv. simpl in Hv. apply in_app_or in Hv. destruct Hv as [Hv|Hv].
      * exact (Hk2 v Hv).
      * exact (hok_grows _ _ _ G (proj1 Hs v Hv)).
    + injection H as <-. unfold yield_step. pop_all Hs.
      split; [apply push_ok; [apply note_ok; assumption|exact I]|apply grows_refl; simpl; congruence].
    + injection H as <-. exact (new_ok P _ s Hs).
    + injection H as <-. unfold get_step. pop_all Hs.
      split; [apply push_ok; [apply note_ok; assumption|exact I]|apply grows_refl; simpl; congruence].
    + pop_all Hs. injection H as <-.
      assert (Hv' : hok t v) by (eapply hok_grows; [apply grows_refl; exact Hr|exact Hv]).
      split; [apply push_ok; [apply push_ok; assumption|exact Hv']|apply grows_refl; simpl; congruence].
    + pop_all Hs. injection H as <-. split; [assumption|apply grows_refl; congruence].
  - intros k b s s' H Hs. destruct k as [|k]; simpl in H.
    + injection H as <-. split; [exact Hs|apply grows_refl; reflexivity].
    + unfold bind in H. destruct (run_b P host fs n b s) as [s1|] eqn:E; [|discriminate].
      destruct (IHb _ _ _ E Hs) as [H1 G1]. destruct (IHl _ _ _ _ H H1) as [H2 G2].
      split; [exact H2|exact (grows_trans _ _ _ G1 G2)].
Qed.

Lemma reset_ok : forall s, handles_ok s -> handles_ok (reset s).
Proof. intros s [_ Hl]. split; [intros v []|exact Hl]. Qed.

Theorem cycles_ok : forall P host m fuel n s, cycles P host m fuel n init = Some s -> handles_ok s.
Proof.
  intros P host m fuel n.
  assert (G : forall n s0 s, cycles P host m fuel n s0 = Some s -> handles_ok s0 -> handles_ok s).
  { induction n0 as [|n0 IH]; intros s0 s H Hs; simpl in H.
    - injection H as <-. exact Hs.
    - unfold bind in H. destruct (run_b P host (mfuncs m) fuel (mmain m) s0) as [s1|] eqn:E; [|discriminate].
      apply (IH _ _ H). apply reset_ok.
      exact (proj1 (proj1 (run_ok P host (mfuncs m) fuel) _ _ _ E Hs)). }
  intros s H. apply (G n init s H). split; [intros v []|intros j; exact I].
Qed.

(** ** A dereference reads the current epoch's cells. *)
Lemma indexed_from_nth : forall rs k r x, nth_error rs r = Some x -> In (k + r, x) (indexed_from k rs).
Proof.
  induction rs as [|y rs IH]; intros k r x H; [destruct r; discriminate|].
  destruct r as [|r]; simpl in H |- *.
  - injection H as ->. left. rewrite Nat.add_0_r. reflexivity.
  - right. replace (k + S r) with (S k + r) by lia. exact (IH _ _ _ H).
Qed.

Theorem deref_current : forall P s v x, hok s v -> deref P s v = Some x ->
  exists r site, nth_error (regs s) r = Some (site, epoch s) /\
    mem s (cell_of P site r) = Some x /\
    In (cell_of P site r) (map (cell_at P) (allocs_at (regs s) (epoch s))).
Proof.
  intros P s [n|r e] x Hv H; [discriminate|]. destruct Hv as [site Hn].
  unfold deref in H. destruct (Nat.eqb e (epoch s)) eqn:Ee; [|discriminate].
  apply Nat.eqb_eq in Ee. subst e. rewrite Hn in H.
  exists r, site. split; [exact Hn|]. split; [exact H|].
  change (cell_of P site r) with (cell_at P (r, (site, epoch s))). apply in_map.
  apply filter_In. split.
  - exact (indexed_from_nth (regs s) 0 r _ Hn).
  - unfold epoch_of. simpl. apply Nat.eqb_refl.
Qed.
