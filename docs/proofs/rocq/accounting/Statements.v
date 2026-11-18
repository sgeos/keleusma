(** * Statements.v -- the statements of the accounting results, pinned.

    Each pin is a definition whose stated type must match the result's type
    up to conversion, so a statement weakened by an added premise or a
    changed conclusion fails to compile.  The definitions the conclusions
    rest on are fixed below by proved equations, so a change to what a
    footprint counts also fails here.  The script requires the number of
    pins below. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape.
From RegionCore Require Controls.
From SharedSlots Require Import ArmShape.
From SharedSlots Require Controls.
From Accounting Require Import Accounting Footprint ArmFootprint.
From Accounting Require Controls ArmControls.
Import ListNotations.

Definition pin_baseline_footprint : forall rs e,
  footprint_at baseline rs e = length (allocs_at rs e)
  := baseline_footprint.

Definition pin_footprint_le_baseline : forall P rs e,
  footprint_at P rs e <= footprint_at baseline rs e
  := footprint_le_baseline.

Definition pin_footprint_slot_bound : forall (P : plan) K, (forall s k, P s = Some k -> k < K) ->
  forall rs e, footprint_at P rs e <= K + length (unplanned_at P rs e)
  := footprint_slot_bound.

Definition pin_mem_within : forall P host m fuel n s,
  cycles P host m fuel n init = Some s -> within P s
  := mem_within.

Definition pin_same_allocations : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => regs sp = regs sb
  | None, None => True
  | _, _ => False
  end
  := same_allocations.

Definition pin_reuse_footprint_bounded : forall m ft, region_check m = Some ft ->
  forall P K, injective P -> (forall s k, P s = Some k -> k < K) ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb =>
      regs sp = regs sb /\ within P sp /\
      forall e,
        footprint_at P (regs sp) e <= K + length (unplanned_at P (regs sp) e) /\
        footprint_at P (regs sp) e <= footprint_at baseline (regs sb) e /\
        footprint_at baseline (regs sb) e = length (allocs_at (regs sb) e)
  | None, None => True
  | _, _ => False
  end
  := reuse_footprint_bounded.

Definition pin_loop_ok_footprints :
  (Accounting.Controls.fp RegionCore.Controls.P0 0, Accounting.Controls.fp RegionCore.Controls.P0 1,
   Accounting.Controls.fp baseline 0, Accounting.Controls.fp baseline 1) = (1, 1, 3, 3)
  := Accounting.Controls.loop_ok_footprints.

Definition pin_slots_alone_do_not_bound :
  ~ (forall e, Accounting.Controls.fp baseline e <= 0)
  := Accounting.Controls.slots_alone_do_not_bound.

Definition pin_same_allocations_arm : forall P m ft, region_checkA P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => regs sp = regs sb
  | None, None => True
  | _, _ => False
  end
  := same_allocations_arm.

Definition pin_reuse_footprint_bounded_arm : forall P m ft, region_checkA P m = Some ft ->
  forall K, (forall s k, P s = Some k -> k < K) ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb =>
      regs sp = regs sb /\ within P sp /\
      forall e,
        footprint_at P (regs sp) e <= K + length (unplanned_at P (regs sp) e) /\
        footprint_at P (regs sp) e <= footprint_at baseline (regs sb) e /\
        footprint_at baseline (regs sb) e = length (allocs_at (regs sb) e)
  | None, None => True
  | _, _ => False
  end
  := reuse_footprint_bounded_arm.

Definition pin_sequential_shares_one_cell :
  match ArmControls.seq_run SharedSlots.Controls.Pshare, ArmControls.seq_run baseline with
  | Some sp, Some sb =>
      (footprint_at SharedSlots.Controls.Pshare (regs sp) 0, footprint_at baseline (regs sb) 0,
       footprint_at SharedSlots.Controls.Pshare (regs sp) 1, footprint_at baseline (regs sb) 1)
  | _, _ => (0, 0, 0, 0)
  end = (1, 2, 1, 2)
  := ArmControls.sequential_shares_one_cell.

(** The definitions the conclusions rest on, pinned by proved equations. *)
Example footprint_at_pinned : forall P rs e,
  footprint_at P rs e = length (nodup cell_eq_dec (map (cell_at P) (allocs_at rs e))).
Proof. reflexivity. Qed.

Example cell_at_pinned : forall P a, cell_at P a = cell_of P (fst (snd a)) (fst a).
Proof. reflexivity. Qed.

Example allocs_at_pinned : forall rs e,
  allocs_at rs e = filter (fun a => Nat.eqb (snd (snd a)) e) (indexed rs).
Proof. reflexivity. Qed.

Example indexed_pinned : forall rs, map fst (indexed rs) = seq 0 (length rs) /\ map snd (indexed rs) = rs.
Proof.
  intros rs. unfold indexed. split; [apply indexed_from_fst|].
  generalize 0. induction rs as [|x r IH]; intros k; simpl; [reflexivity|]. rewrite IH. reflexivity.
Qed.

Example unplanned_at_pinned : forall P rs e,
  unplanned_at P rs e =
  filter (fun a => match P (fst (snd a)) with Some _ => false | None => true end) (allocs_at rs e).
Proof. reflexivity. Qed.

Example within_pinned : forall P s,
  within P s = forall c x, mem s c = Some x -> In c (map (cell_at P) (indexed (regs s))).
Proof. reflexivity. Qed.
