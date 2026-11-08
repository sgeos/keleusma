(** * Bound.v -- the structural bound results of Part I.

    Mechanises Theorem A1 for one conditional instance, Corollary A1s, the
    bound arithmetic of Theorem C and of Corollary B2a, and the remark that
    neither the footprint nor the all-bump bound dominates the other, from
    [docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md].

    Modelling decisions, each recorded in [README.md]:
    - A program unit is a structure tree.  Calls are represented by inlining the
      callee's tree, which Hypothesis H's acyclic call graph licenses, so
      [B(call) = B(callee)] holds by construction rather than as a rule.
    - A leaf is a weighted label.  A label stands for an allocation site or, for
      Corollary B2a, an [Escapes] occurrence.
    - [exec t l] says that one complete execution of [t] executes the leaves [l]
      in order.  A conditional executes exactly one arm, and a loop executes its
      body at most [cap] times, which are the Setting's and Hypothesis H's
      structural facts. *)

From Stdlib Require Import List Arith Lia.
Import ListNotations.

(** [tree] nests [list], and registering the [All] scheme for [list] keeps the
    nested-inductive definition warning-free. *)
Scheme All for list.

Section Bound.

Variable L : Type.

Inductive tree : Type :=
| Nil : tree
| Lf : L -> tree
| Seq : tree -> tree -> tree
| Cond : list tree -> tree
| Loop : nat -> tree -> tree.

(** One complete execution, as the sequence of leaves it executes. *)
Inductive exec : tree -> list L -> Prop :=
| ExNil : exec Nil []
| ExLf : forall l, exec (Lf l) [l]
| ExSeq : forall a b la lb, exec a la -> exec b lb -> exec (Seq a b) (la ++ lb)
| ExCond : forall arms arm l, In arm arms -> exec arm l -> exec (Cond arms) l
| ExLoop : forall cap body n l, n <= cap -> iter body n l -> exec (Loop cap body) l
with iter : tree -> nat -> list L -> Prop :=
| It0 : forall body, iter body 0 []
| ItS : forall body n l1 l2, exec body l1 -> iter body n l2 -> iter body (S n) (l1 ++ l2).

Scheme exec_ind2 := Induction for exec Sort Prop
with iter_ind2 := Induction for iter Sort Prop.

Definition sum (w : L -> nat) (l : list L) : nat := fold_right (fun x acc => w x + acc) 0 l.

Lemma sum_app : forall w l1 l2, sum w (l1 ++ l2) = sum w l1 + sum w l2.
Proof. intros w l1 l2; induction l1 as [|x l1 IH]; simpl; lia. Qed.

Definition maxl (l : list nat) : nat := fold_right Nat.max 0 l.

Lemma le_maxl : forall x l, In x l -> x <= maxl l.
Proof.
  intros x l; induction l as [|y l IH]; simpl; [tauto|].
  intros [<-|H]; [lia|]. specialize (IH H). lia.
Qed.

(** The static bound of Corollary A1s, parametric in the leaf weights. *)
Fixpoint B (w : L -> nat) (t : tree) : nat :=
  match t with
  | Nil => 0
  | Lf l => w l
  | Seq a b => B w a + B w b
  | Cond arms => maxl (map (B w) arms)
  | Loop cap body => cap * B w body
  end.

(** ** Theorem A1, the per-instance branch bound.

    Any per-arm bound that holds on every execution of the arm bounds a
    conditional instance by its maximum over the arms. *)
Theorem A1_branch_bound :
  forall (w : L -> nat) (barm : tree -> nat) (arms : list tree),
    (forall arm l, In arm arms -> exec arm l -> sum w l <= barm arm) ->
    forall l, exec (Cond arms) l -> sum w l <= maxl (map barm arms).
Proof.
  intros w barm arms Harm l Hex. inversion Hex; subst.
  eapply Nat.le_trans; [apply (Harm arm); assumption|].
  apply le_maxl. apply in_map. assumption.
Qed.

(** ** Corollary A1s, the static bound. *)
Theorem A1s_static_bound :
  forall (w : L -> nat) t l, exec t l -> sum w l <= B w t.
Proof.
  intros w.
  apply (exec_ind2
           (fun t l _ => sum w l <= B w t)
           (fun body n l _ => sum w l <= n * B w body)); simpl; intros.
  - lia.
  - lia.
  - rewrite sum_app. lia.
  - eapply Nat.le_trans; [eassumption|]. apply le_maxl. apply in_map. assumption.
  - eapply Nat.le_trans; [eassumption|]. apply Nat.mul_le_mono_r. assumption.
  - lia.
  - rewrite sum_app. lia.
Qed.

(** Every executed leaf lies lexically in the tree.  For a conditional, every
    leaf of one execution lies in a single arm, which is the lexical-containment
    step of Theorem A2's proof. *)
Fixpoint leaves (t : tree) : list L :=
  match t with
  | Nil => []
  | Lf l => [l]
  | Seq a b => leaves a ++ leaves b
  | Cond arms => flat_map leaves arms
  | Loop _ body => leaves body
  end.

Lemma exec_leaves : forall t l, exec t l -> forall x, In x l -> In x (leaves t).
Proof.
  apply (exec_ind2
           (fun t l _ => forall x, In x l -> In x (leaves t))
           (fun body n l _ => forall x, In x l -> In x (leaves body))); simpl; intros.
  - contradiction.
  - assumption.
  - apply in_app_or in H1. apply in_or_app. destruct H1; [left|right]; auto.
  - apply in_flat_map. exists arm. auto.
  - auto.
  - contradiction.
  - apply in_app_or in H1. destruct H1; auto.
Qed.

Theorem cond_one_arm :
  forall arms l, exec (Cond arms) l ->
    exists arm, In arm arms /\ forall x, In x l -> In x (leaves arm).
Proof.
  intros arms l Hex. inversion Hex; subst. exists arm. split; [assumption|].
  apply exec_leaves. assumption.
Qed.

(** ** Theorem C, the plan bound.

    [reused] marks the leaves whose site the plan reuses.  The plan's slots
    contribute the static total [slots], and the bump term is Corollary A1s
    instantiated with reused leaves at zero.  Occupancy at a moment of a cycle
    is the slots plus the bump allocations made so far in the cycle, and by
    Hypothesis H those allocations are a prefix of one execution of the unit. *)
Definition bump_weight (sz : L -> nat) (reused : L -> bool) (x : L) : nat :=
  if reused x then 0 else sz x.

Definition occupancy (sz : L -> nat) (reused : L -> bool) (slots : nat) (sofar : list L) : nat :=
  slots + sum (bump_weight sz reused) sofar.

Definition footprint (sz : L -> nat) (reused : L -> bool) (slots : nat) (unit : tree) : nat :=
  slots + B (bump_weight sz reused) unit.

Lemma sum_prefix : forall w p q, sum w p <= sum w (p ++ q).
Proof. intros. rewrite sum_app. lia. Qed.

Theorem C_plan_bound :
  forall sz reused slots unit l p q,
    exec unit l -> l = p ++ q ->
    occupancy sz reused slots p <= footprint sz reused slots unit.
Proof.
  intros sz reused slots unit l p q Hex ->. unfold occupancy, footprint.
  pose proof (A1s_static_bound (bump_weight sz reused) _ _ Hex) as H.
  pose proof (sum_prefix (bump_weight sz reused) p q). lia.
Qed.

(** ** Corollary B2a, the accounting part.

    Leaves now also stand for [Escapes] occurrences.  [static] is the analysis'
    per-occurrence bound and [actual] the extent a copy really takes, bounded by
    [static] on every occurrence, which is the premise the proof says an
    analysis must supply.  Occupancy adds the cycle's live copies, and the copy
    store is reclaimed at the boundary, so the copies are those of the prefix. *)
Lemma B_mono : forall w1 w2, (forall x, w1 x <= w2 x) -> forall t, B w1 t <= B w2 t.
Proof.
  intros w1 w2 Hw. fix IH 1. intros t. destruct t as [| x | a b | arms | cap body]; simpl.
  - lia.
  - apply Hw.
  - pose proof (IH a). pose proof (IH b). lia.
  - induction arms as [|arm arms IHa]; simpl; [lia|].
    pose proof (IH arm). lia.
  - apply Nat.mul_le_mono_l. apply IH.
Qed.

Lemma maxl_add :
  forall (f g : tree -> nat) arms,
    maxl (map (fun t => f t + g t) arms) <= maxl (map f arms) + maxl (map g arms).
Proof. intros f g arms. induction arms as [|a arms IH]; simpl; lia. Qed.

Lemma B_subadditive :
  forall w1 w2 t, B (fun x => w1 x + w2 x) t <= B w1 t + B w2 t.
Proof.
  intros w1 w2. fix IH 1. intros t. destruct t as [| x | a b | arms | cap body]; simpl.
  - lia.
  - lia.
  - pose proof (IH a). pose proof (IH b). lia.
  - eapply Nat.le_trans; [|apply (maxl_add (B w1) (B w2))].
    induction arms as [|arm arms IHa]; simpl; [lia|].
    pose proof (IH arm). lia.
  - pose proof (IH body). nia.
Qed.

Theorem B2a_accounting :
  forall (reused_w static actual : L -> nat) slots unit l p q,
    (forall x, actual x <= static x) ->
    exec unit l -> l = p ++ q ->
    slots + sum reused_w p + sum actual p
      <= slots + B reused_w unit + B static unit.
Proof.
  intros reused_w static actual slots unit l p q Hact Hex ->.
  assert (Hsum : forall r, sum reused_w r + sum actual r = sum (fun x => reused_w x + actual x) r).
  { induction r as [|x r IH]; simpl; lia. }
  rewrite <- Nat.add_assoc, Hsum.
  pose proof (sum_prefix (fun x => reused_w x + actual x) p q) as Hp.
  pose proof (A1s_static_bound (fun x => reused_w x + actual x) _ _ Hex) as Hb.
  pose proof (B_mono (fun x => reused_w x + actual x) (fun x => reused_w x + static x)) as Hm.
  pose proof (B_subadditive reused_w static unit) as Hs.
  assert (Hm' : B (fun x => reused_w x + actual x) unit
                <= B (fun x => reused_w x + static x) unit)
    by (apply Hm; intros x; specialize (Hact x); lia).
  lia.
Qed.

End Bound.

(** ** The remark, neither bound dominates.

    Two concrete units over [bool] labels, each label one site of size five.
    In the first, a loop of cap four over one reused site gives footprint five
    against an all-bump bound of twenty.  In the second, a conditional whose
    arms are two sites, one of them reused, gives footprint ten against an
    all-bump bound of five, the slot being provisioned outside the arm maximum. *)
Section Remark.

Definition sz5 (_ : bool) : nat := 5.
Definition none_reused (_ : bool) : bool := false.

Example footprint_below_bump :
  let unit := Loop bool 4 (Lf bool true) in
  footprint bool sz5 (fun b => b) 5 unit < B bool (bump_weight bool sz5 none_reused) unit.
Proof. simpl. unfold footprint, bump_weight. simpl. lia. Qed.

Example footprint_above_bump :
  let unit := Cond bool [Lf bool true; Lf bool false] in
  footprint bool sz5 (fun b => b) 5 unit > B bool (bump_weight bool sz5 none_reused) unit.
Proof. simpl. unfold footprint, bump_weight. simpl. lia. Qed.

End Remark.
