(** * Frame.v -- a run writes only cells of the sites it may allocate.

    For every plan, a run of a block writes memory only at cells of the form
    [cell_of Q s r] with [s] among the block's allocation sites, [sites_b],
    and [r] an allocation index at least the length of the allocation record
    when the run began.  The record only grows and the epoch does not
    change.  The function table must have an entry for every function, whose
    sites cover the function body's sites, which [check_funcsC] establishes.
    This is what lets a callee allocate without disturbing a caller's live
    handle of a site the callee does not allocate. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape.
From CallSites Require Import ShapeC.
Import ListNotations.

Section Frame.

Variable Q : plan.
Variable host : list ev -> nat.
Variable fs : list block.
Variable ft : list fentry.
Hypothesis Hfs : forall f g, nth_error fs f = Some g ->
  exists R A, nth_error ft f = Some (R, A) /\ incl (sites_b ft g) A.

(** What a run from [s] to [s'] may change, given the sites [A]. *)
Definition framed (A : list nat) (s s' : state) : Prop :=
  (exists u, regs s' = regs s ++ u) /\ epoch s' = epoch s /\
  forall c, (forall s0 r, In s0 A -> length (regs s) <= r -> c <> cell_of Q s0 r) -> mem s' c = mem s c.

Lemma framed_same : forall A s s', regs s' = regs s -> epoch s' = epoch s -> mem s' = mem s -> framed A s s'.
Proof.
  intros A s s' Hr He Hm. split; [exists []; rewrite app_nil_r; exact Hr|]. split; [exact He|].
  intros c _. rewrite Hm. reflexivity.
Qed.

Lemma framed_mono : forall A A' s s', incl A A' -> framed A s s' -> framed A' s s'.
Proof.
  intros A A' s s' Hi [Hr [He Hm]]. split; [exact Hr|]. split; [exact He|].
  intros c Hc. apply Hm. intros s0 r Hs0 Hr0. apply Hc; [apply Hi; exact Hs0|exact Hr0].
Qed.

Lemma framed_trans : forall A1 A2 s s1 s', framed A1 s s1 -> framed A2 s1 s' -> framed (A1 ++ A2) s s'.
Proof.
  intros A1 A2 s s1 s' [[u1 Hr1] [He1 Hm1]] [[u2 Hr2] [He2 Hm2]].
  split; [exists (u1 ++ u2); rewrite Hr2, Hr1, app_assoc; reflexivity|]. split; [congruence|].
  intros c Hc. rewrite Hm2, Hm1.
  - reflexivity.
  - intros s0 r Hs0 Hr0. apply Hc; [apply in_or_app; left; exact Hs0|exact Hr0].
  - intros s0 r Hs0 Hr0. apply Hc; [apply in_or_app; right; exact Hs0|].
    rewrite Hr1, length_app in Hr0. lia.
Qed.

Lemma pop_keepsF : forall s v s1, pop s = (v, s1) -> regs s1 = regs s /\ mem s1 = mem s /\ epoch s1 = epoch s.
Proof. intros s v s1 H. unfold pop in H. destruct (stk s); injection H as <- <-; repeat split. Qed.

Ltac popsF :=
  repeat match goal with
  | H : context [pop ?x] |- _ =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E; apply pop_keepsF in E; destruct E as [? [? ?]]
  | |- context [pop ?x] =>
      let v := fresh "v" in let t := fresh "t" in let E := fresh "E" in
      destruct (pop x) as [v t] eqn:E; apply pop_keepsF in E; destruct E as [? [? ?]]
  end.

Lemma frame : forall n,
  (forall b s s', run_b Q host fs n b s = Some s' -> framed (sites_b ft b) s s') /\
  (forall i s s', run_i Q host fs n i s = Some s' -> framed (sites_i ft i) s s') /\
  (forall k b s s', run_loop Q host fs n k b s = Some s' -> framed (sites_b ft b) s s').
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; discriminate. }
  split; [|split].
  - intros b s s' H. destruct b as [|i r]; simpl in H.
    + injection H as <-. apply framed_same; reflexivity.
    + unfold bind in H. destruct (run_i Q host fs n i s) as [s1|] eqn:E; [|discriminate].
      exact (framed_trans _ _ _ _ _ (IHi _ _ _ E) (IHb _ _ _ H)).
  - intros i s s' H. destruct i as [k|j|j| | | |t e|k body|f| |site| | |]; simpl in H |- *.
    + injection H as <-. apply framed_same; reflexivity.
    + injection H as <-. apply framed_same; reflexivity.
    + popsF. injection H as <-. apply framed_same; simpl; congruence.
    + popsF. injection H as <-. apply framed_same; simpl; congruence.
    + popsF. injection H as <-. apply framed_same; simpl; congruence.
    + popsF. injection H as <-. apply framed_same; simpl; congruence.
    + destruct (pop s) as [c s1] eqn:E. destruct (pop_keepsF _ _ _ E) as [R1 [M1 E1]].
      assert (F0 : framed [] s s1) by (apply framed_same; assumption).
      destruct (Nat.eqb (num c) 0).
      * apply (framed_mono ([] ++ sites_b ft e)); [intros x Hx; apply in_or_app; right; exact Hx|].
        exact (framed_trans _ _ _ _ _ F0 (IHb _ _ _ H)).
      * apply (framed_mono ([] ++ sites_b ft t)); [intros x Hx; apply in_or_app; left; exact Hx|].
        exact (framed_trans _ _ _ _ _ F0 (IHb _ _ _ H)).
    + exact (IHl _ _ _ _ H).
    + destruct (nth_error fs f) as [g|] eqn:Eg; [|discriminate]. unfold bind in H.
      destruct (run_b Q host fs n g (with_stk s [])) as [s2|] eqn:E; [|discriminate].
      injection H as <-. destruct (Hfs f g Eg) as [R [A [Ef Hi]]]. rewrite Ef.
      destruct (IHb _ _ _ E) as [[u Hr] [He Hm]].
      split; [exists u; exact Hr|]. split; [exact He|].
      intros c Hc. simpl. apply Hm. intros s0 r Hs0 Hr0. apply Hc; [apply Hi; exact Hs0|exact Hr0].
    + injection H as <-. unfold yield_step. popsF. apply framed_same; simpl; congruence.
    + injection H as <-. unfold new_step. destruct (pop s) as [v s1] eqn:E.
      destruct (pop_keepsF _ _ _ E) as [R1 [M1 E1]].
      split; [exists [(site, epoch s1)]; simpl; rewrite R1; reflexivity|]. split; [simpl; exact E1|].
      intros c Hc. simpl. destruct (cell_eq_dec c (cell_of Q site (length (regs s1)))) as [->|_].
      * exfalso. apply (Hc site (length (regs s1))); [left; reflexivity|rewrite R1; lia|reflexivity].
      * rewrite M1. reflexivity.
    + injection H as <-. unfold get_step. popsF. apply framed_same; simpl; congruence.
    + popsF. injection H as <-. apply framed_same; simpl; congruence.
    + popsF. injection H as <-. apply framed_same; congruence.
  - intros k b s s' H. destruct k as [|k]; simpl in H.
    + injection H as <-. apply framed_same; reflexivity.
    + unfold bind in H. destruct (run_b Q host fs n b s) as [s1|] eqn:E; [|discriminate].
      pose proof (framed_trans _ _ _ _ _ (IHb _ _ _ E) (IHl _ _ _ _ H)) as F.
      apply (framed_mono _ _ _ _ (fun x Hx => match in_app_or _ _ _ Hx with or_introl h => h | or_intror h => h end) F).
Qed.

End Frame.
