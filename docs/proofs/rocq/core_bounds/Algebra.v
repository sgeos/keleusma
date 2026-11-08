(** * Algebra.v -- summaries of segment lists, and the bounds that dominate them.

    [summary_join] shows that summarising a joined run is sequencing the
    summaries, so the checker's algebra describes the interpreter's real
    segments.  [le_seq], [le_join_l] and [le_join_r] show that the abstract
    operations of [Check.v] dominate the summaries they bound.
    [summary_elements] recovers a bound on every segment from a summary. *)

From Stdlib Require Import List Arith Lia.
From CoreBounds Require Import Core Check.
Import ListNotations.

(** ** Segment lists. *)
Lemma join_nonempty : forall a b, a <> [] -> b <> [] -> join a b <> [].
Proof.
  intros a b Ha Hb. destruct a as [|x [|y a']]; [contradiction| |discriminate].
  destruct b; [contradiction|discriminate].
Qed.

(** Prefixing one segment to a nonempty list, as a summary operation. *)
Definition cons_s (x : nat) (S : summ) : summ :=
  match S with NoY c => Y x 0 c | Y f m l => Y x (Nat.max f m) l end.

Lemma summary_cons : forall x t, t <> [] -> summary (x :: t) = cons_s x (summary t).
Proof.
  intros x t Ht. destruct t as [|c t']; [contradiction|].
  destruct t' as [|d t'']; [reflexivity|]. simpl. reflexivity.
Qed.

Lemma sseq_cons : forall x S T, sseq (cons_s x S) T = cons_s x (sseq S T).
Proof.
  intros x [c|f m l] [d|f2 m2 l2]; simpl; f_equal; lia.
Qed.

Lemma sseq_noy_cons : forall x y T, sseq (NoY x) (cons_s y T) = cons_s (x + y) T.
Proof. intros x y [c|f m l]; reflexivity. Qed.

Theorem summary_join : forall a b, a <> [] -> b <> [] ->
  summary (join a b) = sseq (summary a) (summary b).
Proof.
  induction a as [|x a IH]; intros b Ha Hb; [contradiction|].
  destruct a as [|y a'].
  - destruct b as [|z b']; [contradiction|]. simpl join.
    destruct b' as [|w b''].
    + reflexivity.
    + rewrite (summary_cons (x + z) (w :: b'')) by discriminate.
      rewrite (summary_cons z (w :: b'')) by discriminate.
      change (summary [x]) with (NoY x). rewrite sseq_noy_cons. reflexivity.
  - change (join (x :: y :: a') b) with (x :: join (y :: a') b).
    rewrite summary_cons by (apply join_nonempty; [discriminate|assumption]).
    rewrite (summary_cons x (y :: a')) by discriminate.
    rewrite IH by (discriminate || assumption). rewrite sseq_cons. reflexivity.
Qed.

(** ** Domination of summaries by abstract bounds. *)
Definition le3 (a b : nat * nat * nat) : Prop :=
  let '(f1, m1, l1) := a in let '(f2, m2, l2) := b in f1 <= f2 /\ m1 <= m2 /\ l1 <= l2.

Definition le_summ (S : summ) (A : abs) : Prop :=
  match S with
  | NoY c => exists c', an A = Some c' /\ c <= c'
  | Y f m l => exists t, ay A = Some t /\ le3 (f, m, l) t
  end.

Lemma le3_trans : forall a b c, le3 a b -> le3 b c -> le3 a c.
Proof.
  intros [[f1 m1] l1] [[f2 m2] l2] [[f3 m3] l3]; simpl; lia.
Qed.

Lemma omax3_l : forall x y t, x = Some t -> exists u, omax3 x y = Some u /\ le3 t u.
Proof.
  intros x y [[f m] l] ->. destruct y as [[[f2 m2] l2]|]; simpl; eexists; split; try reflexivity; simpl; lia.
Qed.

Lemma omax3_r : forall x y t, y = Some t -> exists u, omax3 x y = Some u /\ le3 t u.
Proof.
  intros x y [[f m] l] ->. destruct x as [[[f2 m2] l2]|]; simpl; eexists; split; try reflexivity; simpl; lia.
Qed.

Lemma omax_l : forall x y c, x = Some c -> exists c', omax x y = Some c' /\ c <= c'.
Proof. intros x y c ->. destruct y; simpl; eexists; split; try reflexivity; lia. Qed.

Lemma omax_r : forall x y c, y = Some c -> exists c', omax x y = Some c' /\ c <= c'.
Proof. intros x y c ->. destruct x; simpl; eexists; split; try reflexivity; lia. Qed.

Lemma le_join_l : forall S A B d, le_summ S A -> le_summ S (ajoin A B d).
Proof.
  intros [c|f m l] A B d.
  - intros [c' [H1 H2]]. destruct (omax_l (an A) (an B) c' H1) as [c'' [H3 H4]].
    exists c''. simpl. split; [exact H3|lia].
  - intros [t [H1 H2]]. destruct (omax3_l (ay A) (ay B) t H1) as [u [H3 H4]].
    exists u. split; [exact H3|]. exact (le3_trans _ _ _ H2 H4).
Qed.

Lemma le_join_r : forall S A B d, le_summ S B -> le_summ S (ajoin A B d).
Proof.
  intros [c|f m l] A B d.
  - intros [c' [H1 H2]]. destruct (omax_r (an A) (an B) c' H1) as [c'' [H3 H4]].
    exists c''. simpl. split; [exact H3|lia].
  - intros [t [H1 H2]]. destruct (omax3_r (ay A) (ay B) t H1) as [u [H3 H4]].
    exists u. split; [exact H3|]. exact (le3_trans _ _ _ H2 H4).
Qed.

(** The three ways a sequenced run can yield, each dominated by [seq_y]. *)
Lemma seq_y_1 : forall an1 ay1 an2 ay2 a f m l,
  an1 = Some a -> ay2 = Some (f, m, l) ->
  exists u, seq_y an1 ay1 an2 ay2 = Some u /\ le3 (a + f, m, l) u.
Proof. intros. unfold seq_y. subst. eapply omax3_l. reflexivity. Qed.

Lemma seq_y_2 : forall an1 ay1 an2 ay2 f m l b,
  ay1 = Some (f, m, l) -> an2 = Some b ->
  exists u, seq_y an1 ay1 an2 ay2 = Some u /\ le3 (f, m, l + b) u.
Proof.
  intros. unfold seq_y. subst.
  match goal with |- exists u, omax3 ?X (omax3 ?Y ?Z) = Some u /\ _ =>
    destruct (omax3_l Y Z (f, m, l + b)) as [u1 [E1 L1]]; [reflexivity|];
    destruct (omax3_r X (omax3 Y Z) u1 E1) as [u [E L]];
    exists u; split; [exact E|eapply le3_trans; eassumption]
  end.
Qed.

Lemma seq_y_3 : forall an1 ay1 an2 ay2 f1 m1 l1 f2 m2 l2,
  ay1 = Some (f1, m1, l1) -> ay2 = Some (f2, m2, l2) ->
  exists u, seq_y an1 ay1 an2 ay2 = Some u /\
            le3 (f1, Nat.max m1 (Nat.max (l1 + f2) m2), l2) u.
Proof.
  intros. unfold seq_y. subst.
  match goal with |- exists u, omax3 ?X (omax3 ?Y ?Z) = Some u /\ _ =>
    destruct (omax3_r Y Z (f1, Nat.max m1 (Nat.max (l1 + f2) m2), l2)) as [u1 [E1 L1]]; [reflexivity|];
    destruct (omax3_r X (omax3 Y Z) u1 E1) as [u [E L]];
    exists u; split; [exact E|eapply le3_trans; eassumption]
  end.
Qed.

Lemma le3_mono : forall f m l f' m' l' t,
  f <= f' -> m <= m' -> l <= l' -> le3 (f', m', l') t -> le3 (f, m, l) t.
Proof. intros f m l f' m' l' [[f2 m2] l2]; simpl; lia. Qed.

Lemma le_seq : forall S T A B d, le_summ S A -> le_summ T B -> le_summ (sseq S T) (aseq A B d).
Proof.
  intros [a|f m l] [b|f2 m2 l2] A B d HA HB; simpl in HA, HB |- *.
  - destruct HA as [a' [Ha Ha']]. destruct HB as [b' [Hb Hb']].
    exists (a' + b'). unfold seq_n. rewrite Ha, Hb. split; [reflexivity|lia].
  - destruct HA as [a' [Ha Ha']]. destruct HB as [[[f' m'] l'] [Hb Hb']]. simpl in Hb'.
    destruct (seq_y_1 (an A) (ay A) (an B) (ay B) _ _ _ _ Ha Hb) as [u [E L]].
    exists u. split; [exact E|]. eapply le3_mono; [| | |exact L]; lia.
  - destruct HA as [[[f' m'] l'] [Ha Ha']]. destruct HB as [b' [Hb Hb']]. simpl in Ha'.
    destruct (seq_y_2 (an A) (ay A) (an B) (ay B) _ _ _ _ Ha Hb) as [u [E L]].
    exists u. split; [exact E|]. eapply le3_mono; [| | |exact L]; lia.
  - destruct HA as [[[f' m'] l'] [Ha Ha']]. destruct HB as [[[f3 m3] l3] [Hb Hb']].
    simpl in Ha', Hb'.
    destruct (seq_y_3 (an A) (ay A) (an B) (ay B) _ _ _ _ _ _ Ha Hb) as [u [E L]].
    exists u. split; [exact E|]. eapply le3_mono; [| | |exact L]; lia.
Qed.

(** ** Every segment from its summary. *)
Lemma maxl_ge : forall x l, In x l -> x <= maxl l.
Proof. intros x l; induction l as [|y l IH]; simpl; [tauto|]. intros [->|H]; [lia|]. specialize (IH H). lia. Qed.

Lemma in_removelast_or_last : forall (x : nat) l, In x l -> In x (removelast l) \/ x = last l 0.
Proof.
  intros x l. induction l as [|y l IH]; simpl; [tauto|].
  destruct l as [|z l'].
  - intros [->|[]]. right. reflexivity.
  - intros [->|H].
    + left. left. reflexivity.
    + destruct (IH H) as [H1|H1]; [left; right; exact H1|right; exact H1].
Qed.

Theorem summary_elements : forall p, p <> [] ->
  match summary p with
  | NoY c => forall x, In x p -> x <= c
  | Y f m l => forall x, In x p -> x <= Nat.max f (Nat.max m l)
  end.
Proof.
  intros p Hp. destruct p as [|c [|d t]]; [contradiction| |].
  - simpl. intros x [->|[]]. lia.
  - change (summary (c :: d :: t)) with (Y c (maxl (removelast (d :: t))) (last (d :: t) 0)).
    intros x [->|H]; [lia|].
    destruct (in_removelast_or_last x (d :: t) H) as [H1|H1].
    + pose proof (maxl_ge x _ H1). lia.
    + subst x. lia.
Qed.
