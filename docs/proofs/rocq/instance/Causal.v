(** * Causal.v -- an event depends only on the dereference outcomes before it.

    Two runs of the oracle interpreter from one state, whose oracles agree on
    every read before the read numbered [k + 1] of the first run, either end
    in the same state, or both pass that read and agree on every event up to
    and including it.  This is what makes the machine of [Machine.v] a
    strategy in increment B1's sense, a function of the observation history,
    since the history holds exactly the outcomes of the reads before the next
    event. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core.
From RegionReuse Require Trace.
From Instance Require Import Encode.
Import ListNotations.

Section Causal.

Variables A1 A2 : oracle.
Variable host : list ev -> nat.
Variable fs : list block.
(** [FIN] is the final event sequence of the first run, and [k] the number
    of reads on which the oracles must agree. *)
Variable FIN : list tevent.
Variable k : nat.

Hypothesis agree : forall tr x rest, FIN = tr ++ TDeref x tt :: rest -> nder tr < k ->
  A1 tr x = A2 tr x.

Definition prefix (l : list tevent) : Prop := exists d, FIN = l ++ d.

Definition Rel2 (o1 o2 : ostate) : Prop :=
  k < nder (otr o1) /\ k < nder (otr o2) /\ upto k (otr o1) = upto k (otr o2).

Definition Rel (o1 o2 : ostate) : Prop := o1 = o2 \/ Rel2 o1 o2.

Lemma rel2_ext : forall o1 o2 o1' o2', Rel2 o1 o2 -> extends o1 o1' -> extends o2 o2' -> Rel2 o1' o2'.
Proof.
  intros o1 o2 o1' o2' [H1 [H2 H3]] [d1 E1] [d2 E2]. unfold Rel2. rewrite E1, E2, !nder_app.
  rewrite !upto_app by assumption. repeat split; [lia|lia|exact H3].
Qed.

Lemma prefix_ext : forall o o', extends o o' -> prefix (otr o') -> prefix (otr o).
Proof. intros o o' [d E] [d' F]. exists (d ++ d'). rewrite F, E, app_assoc. reflexivity. Qed.

Lemma seq_rel : forall (f1 f2 : ostate -> ostate) x1 x2,
  (forall y, extends y (f1 y)) -> (forall y, extends y (f2 y)) ->
  Rel x1 x2 -> (ohalt x1 = false -> Rel (f1 x1) (f2 x1)) ->
  Rel (if ohalt x1 then x1 else f1 x1) (if ohalt x2 then x2 else f2 x2).
Proof.
  intros f1 f2 x1 x2 E1 E2 [<-|H] Hin.
  - destruct (ohalt x1) eqn:Eh; [left; reflexivity|exact (Hin eq_refl)].
  - right. destruct (ohalt x1), (ohalt x2);
      eapply rel2_ext; try exact H; try apply extends_refl; try apply E1; try apply E2.
Qed.

Lemma ext_if : forall (f : ostate -> ostate) x, (forall y, extends y (f y)) ->
  extends x (if ohalt x then x else f x).
Proof. intros f x E. destruct (ohalt x); [apply extends_refl|apply E]. Qed.

Lemma causal : forall n,
  (forall b o, prefix (otr (orun_b A1 host fs n b o)) ->
     Rel (orun_b A1 host fs n b o) (orun_b A2 host fs n b o)) /\
  (forall i o, prefix (otr (orun_i A1 host fs n i o)) ->
     Rel (orun_i A1 host fs n i o) (orun_i A2 host fs n i o)) /\
  (forall kk body o, prefix (otr (orun_loop A1 host fs n kk body o)) ->
     Rel (orun_loop A1 host fs n kk body o) (orun_loop A2 host fs n kk body o)).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; left; reflexivity. }
  pose proof (fun A => proj1 (orun_extends A host fs n)) as Eb.
  pose proof (fun A => proj1 (proj2 (orun_extends A host fs n))) as Ei.
  pose proof (fun A => proj2 (proj2 (orun_extends A host fs n))) as El.
  split; [|split].
  - intros [|i r] o Hp; simpl in Hp |- *; [left; reflexivity|].
    apply seq_rel; [intros y; apply Eb|intros y; apply Eb| |].
    + apply IHi. eapply prefix_ext; [|exact Hp]. apply ext_if. intros y; apply Eb.
    + intros Hh. rewrite Hh in Hp. apply IHb. exact Hp.
  - intros i o Hp. destruct i as [kc|j|j| | | |t e|kc body|f| |site| | |]; simpl in Hp |- *;
      try (left; reflexivity).
    + destruct (pop (ost o)) as [c s1]. apply IHb. exact Hp.
    + apply IHl. exact Hp.
    + destruct (nth_error fs f) as [g|]; [|left; reflexivity].
      set (wrap := fun o2 : ostate => mko (with_stk (ost o2) (stk (ost o2) ++ stk (ost o))) (otr o2) (ohalt o2)).
      change (Rel (if ohalt (orun_b A1 host fs n g (ostep o (with_stk (ost o) []) TStep))
                   then orun_b A1 host fs n g (ostep o (with_stk (ost o) []) TStep)
                   else wrap (orun_b A1 host fs n g (ostep o (with_stk (ost o) []) TStep)))
                  (if ohalt (orun_b A2 host fs n g (ostep o (with_stk (ost o) []) TStep))
                   then orun_b A2 host fs n g (ostep o (with_stk (ost o) []) TStep)
                   else wrap (orun_b A2 host fs n g (ostep o (with_stk (ost o) []) TStep)))).
      assert (Ew : forall y, extends y (wrap y)) by (intros y; exists []; simpl; rewrite app_nil_r; reflexivity).
      apply seq_rel; [exact Ew|exact Ew| |intros _; left; reflexivity].
      apply IHb. eapply prefix_ext; [|exact Hp]. exact (ext_if wrap _ Ew).
    + unfold oget in *. destruct (pop (ost o)) as [v s1].
      destruct (Nat.lt_ge_cases (nder (otr o)) k) as [Hk|Hk].
      * destruct Hp as [d Hd]. simpl in Hd. rewrite <- app_assoc in Hd. simpl in Hd.
        rewrite (agree (otr o) (src_of v) d Hd Hk). left; reflexivity.
      * right. unfold Rel2, ostep. simpl. rewrite nder_app. simpl. split; [lia|split; [lia|reflexivity]].
  - intros [|kk] body o Hp; simpl in Hp |- *; [left; reflexivity|].
    apply seq_rel; [intros y; apply El|intros y; apply El| |].
    + apply IHb. eapply prefix_ext; [|exact Hp]. apply ext_if. intros y; apply El.
    + intros Hh. rewrite Hh in Hp. apply IHl. exact Hp.
Qed.

Lemma causal_cycles : forall m fuel nc o, fs = mfuncs m ->
  prefix (otr (ocycles A1 host m fuel nc o)) ->
  Rel (ocycles A1 host m fuel nc o) (ocycles A2 host m fuel nc o).
Proof.
  intros m fuel nc. induction nc as [|nc IH]; intros o Hfs Hp; simpl in Hp |- *; [left; reflexivity|].
  rewrite <- Hfs in Hp |- *.
  assert (Ec : forall A y, extends y (ocycles A host m fuel nc (oreset y))).
  { intros A y. eapply extends_trans; [apply extends_step|apply ocycles_extends]. }
  apply (seq_rel (fun y => ocycles A1 host m fuel nc (oreset y)) (fun y => ocycles A2 host m fuel nc (oreset y)));
    [intros y; apply Ec|intros y; apply Ec| |].
  - apply (proj1 (causal fuel)). eapply prefix_ext; [|exact Hp].
    exact (ext_if (fun y => ocycles A1 host m fuel nc (oreset y)) _ (Ec A1)).
  - intros Hh. rewrite Hh in Hp. apply IH; [exact Hfs|exact Hp].
Qed.

End Causal.
