(** * Accounting.v -- counting the cells a plan uses, epoch by epoch.

    The region core of increment B2 records every allocation in [regs] as a
    pair of its site and its epoch, and the allocation's index in that list
    is its region number.  Under a plan the allocation writes the cell
    [cell_of P site r], a slot for a site the plan reuses and the fresh bump
    cell [Bump r] otherwise.  This module counts those cells, with no
    reference to the machine, so its results hold of any allocation record.

    The quantity is the per-epoch footprint, the number of distinct cells the
    allocations of one epoch use.  A cycle of the shipping virtual machine
    reclaims its bump storage at the reset that ends it, so the per-epoch
    footprint is the quantity a worst-case memory bound must cover.  The
    region core itself never reclaims a cell, which [Footprint.v] states. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core.
Import ListNotations.

(** An allocation with its index, [(r, (site, epoch))]. *)
Definition alloc : Type := (nat * (nat * nat))%type.

Fixpoint indexed_from (k : nat) (rs : list (nat * nat)) : list alloc :=
  match rs with
  | [] => []
  | x :: r => (k, x) :: indexed_from (S k) r
  end.

Definition indexed (rs : list (nat * nat)) : list alloc := indexed_from 0 rs.

Definition site_of (a : alloc) : nat := fst (snd a).
Definition epoch_of (a : alloc) : nat := snd (snd a).

(** The cell an allocation writes under a plan. *)
Definition cell_at (P : plan) (a : alloc) : cell := cell_of P (site_of a) (fst a).

Definition allocs_at (rs : list (nat * nat)) (e : nat) : list alloc :=
  filter (fun a => Nat.eqb (epoch_of a) e) (indexed rs).

(** The per-epoch footprint, the number of distinct cells used. *)
Definition footprint_at (P : plan) (rs : list (nat * nat)) (e : nat) : nat :=
  length (nodup cell_eq_dec (map (cell_at P) (allocs_at rs e))).

(** The allocations of an epoch at sites the plan does not reuse. *)
Definition unplanned_at (P : plan) (rs : list (nat * nat)) (e : nat) : list alloc :=
  filter (fun a => match P (site_of a) with Some _ => false | None => true end) (allocs_at rs e).

(** ** Bookkeeping. *)

Lemma indexed_from_fst : forall rs k, map fst (indexed_from k rs) = seq k (length rs).
Proof. induction rs as [|x r IH]; intros k; simpl; [reflexivity|]. rewrite IH. reflexivity. Qed.

Lemma indexed_from_app : forall rs k x,
  indexed_from k (rs ++ [x]) = indexed_from k rs ++ [(k + length rs, x)].
Proof.
  induction rs as [|y r IH]; intros k x; simpl.
  - rewrite Nat.add_0_r. reflexivity.
  - rewrite IH, Nat.add_succ_r. reflexivity.
Qed.

Lemma indexed_app : forall rs x, indexed (rs ++ [x]) = indexed rs ++ [(length rs, x)].
Proof. intros rs x. unfold indexed. rewrite indexed_from_app. reflexivity. Qed.

Lemma nodup_length_le : forall (A : Type) (d : forall a b : A, {a = b} + {a <> b}) (l : list A),
  length (nodup d l) <= length l.
Proof.
  intros A d l. apply NoDup_incl_length; [apply NoDup_nodup|].
  intros x H. rewrite nodup_In in H. exact H.
Qed.

Lemma NoDup_fst_filter : forall (f : alloc -> bool) (l : list alloc),
  NoDup (map fst l) -> NoDup (map fst (filter f l)).
Proof.
  intros f l. induction l as [|a l IH]; intros H; simpl; [constructor|].
  inversion H as [|x y Hn Hd]; subst. destruct (f a); simpl; [|exact (IH Hd)].
  constructor; [|exact (IH Hd)].
  intros Hin. apply Hn. apply in_map_iff in Hin. destruct Hin as [b [Hb Hin]].
  apply filter_In in Hin. apply in_map_iff. exists b. split; [exact Hb|exact (proj1 Hin)].
Qed.

Lemma NoDup_allocs_at : forall rs e, NoDup (map fst (allocs_at rs e)).
Proof.
  intros rs e. unfold allocs_at. apply NoDup_fst_filter.
  unfold indexed. rewrite indexed_from_fst. apply seq_NoDup.
Qed.

Lemma NoDup_map_bump : forall l : list nat, NoDup l -> NoDup (map Bump l).
Proof.
  induction l as [|x l IH]; intros H; simpl; [constructor|].
  inversion H as [|y z Hn Hd]; subst. constructor; [|exact (IH Hd)].
  intros Hin. apply Hn. apply in_map_iff in Hin. destruct Hin as [y [Hy Hin]].
  injection Hy as ->. exact Hin.
Qed.

(** ** The baseline footprint is the number of allocations. *)
Theorem baseline_footprint : forall rs e, footprint_at baseline rs e = length (allocs_at rs e).
Proof.
  intros rs e. unfold footprint_at.
  assert (Hm : map (cell_at baseline) (allocs_at rs e) = map Bump (map fst (allocs_at rs e))).
  { rewrite map_map. reflexivity. }
  rewrite Hm, nodup_fixed_point.
  - rewrite !length_map. reflexivity.
  - apply NoDup_map_bump. apply NoDup_allocs_at.
Qed.

(** ** A plan never uses more cells than the baseline. *)
Theorem footprint_le_baseline : forall P rs e, footprint_at P rs e <= footprint_at baseline rs e.
Proof.
  intros P rs e. rewrite baseline_footprint. unfold footprint_at.
  eapply Nat.le_trans; [apply nodup_length_le|]. rewrite length_map. lia.
Qed.

(** ** Theorem C at the core level.  When the plan's slots are below [K],
    the allocations of one epoch use at most [K] slots and one bump cell for
    each allocation at a site the plan does not reuse, however many times a
    reused site allocates in that epoch. *)
Lemma NoDup_below : forall (l : list nat) K, NoDup l -> (forall k, In k l -> k < K) -> length l <= K.
Proof.
  intros l K Hd Hb. rewrite <- (length_seq K 0). apply NoDup_incl_length; [exact Hd|].
  intros k Hk. apply in_seq. specialize (Hb k Hk). lia.
Qed.

Definition slots_at (P : plan) (rs : list (nat * nat)) (e : nat) : list nat :=
  flat_map (fun a => match P (site_of a) with Some k => [k] | None => [] end) (allocs_at rs e).

Theorem footprint_slot_bound : forall P K, (forall s k, P s = Some k -> k < K) ->
  forall rs e, footprint_at P rs e <= K + length (unplanned_at P rs e).
Proof.
  intros P K HK rs e. unfold footprint_at.
  set (S := nodup Nat.eq_dec (slots_at P rs e)).
  assert (HS : length S <= K).
  { apply NoDup_below; [apply NoDup_nodup|]. intros k Hk. unfold S in Hk. rewrite nodup_In in Hk.
    unfold slots_at in Hk. apply in_flat_map in Hk. destruct Hk as [a [_ Ha]].
    destruct (P (site_of a)) as [k'|] eqn:E; [|contradiction].
    destruct Ha as [<-|[]]. exact (HK _ _ E). }
  eapply Nat.le_trans.
  - apply (NoDup_incl_length (l' := map Slot S ++ map (cell_at P) (unplanned_at P rs e))).
    + apply NoDup_nodup.
    + intros c Hc. rewrite nodup_In in Hc. apply in_map_iff in Hc.
      destruct Hc as [a [<- Ha]]. apply in_or_app. unfold cell_at, cell_of.
      destruct (P (site_of a)) as [k|] eqn:E.
      * left. apply in_map. unfold S. apply nodup_In. unfold slots_at. apply in_flat_map.
        exists a. split; [exact Ha|]. rewrite E. left. reflexivity.
      * right. replace (Bump (fst a)) with (cell_at P a) by (unfold cell_at, cell_of; rewrite E; reflexivity).
        apply in_map. apply filter_In. split; [exact Ha|]. rewrite E. reflexivity.
  - rewrite length_app, !length_map. lia.
Qed.
