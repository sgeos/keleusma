(** * ArmControls.v -- evidence that slot sharing saves memory.

    The module [sequential] of [../shared_slots/Controls.v] allocates at site
    0, reads and drops the handle, then allocates at site 1.  The plan
    [Pshare] puts both sites in slot 0, so it is not injective and
    [Footprint.reuse_footprint_bounded] cannot apply.  The join checker
    accepts the pair, and in its first epoch the reuse run uses one cell
    where the baseline uses two, so sharing really saves memory and
    [reuse_footprint_bounded_arm] applies to it. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape.
From RegionCore Require Controls.
From SharedSlots Require Import ArmShape Arm.
From SharedSlots Require Controls.
From Accounting Require Import Accounting Footprint ArmFootprint.
Import ListNotations.

Definition seq_run (P : plan) : option state :=
  cycles P RegionCore.Controls.host_len SharedSlots.Controls.sequential 100 2 init.

Example sequential_accepted_by_join :
  region_checkA SharedSlots.Controls.Pshare SharedSlots.Controls.sequential = Some [].
Proof. vm_compute. reflexivity. Qed.

Example sequential_shares_one_cell :
  match seq_run SharedSlots.Controls.Pshare, seq_run baseline with
  | Some sp, Some sb =>
      (footprint_at SharedSlots.Controls.Pshare (regs sp) 0, footprint_at baseline (regs sb) 0,
       footprint_at SharedSlots.Controls.Pshare (regs sp) 1, footprint_at baseline (regs sb) 1)
  | _, _ => (0, 0, 0, 0)
  end = (1, 2, 1, 2).
Proof. vm_compute. reflexivity. Qed.

Theorem sequential_footprint_bounded : forall host fuel n,
  match cycles SharedSlots.Controls.Pshare host SharedSlots.Controls.sequential fuel n init,
        cycles baseline host SharedSlots.Controls.sequential fuel n init with
  | Some sp, Some sb =>
      regs sp = regs sb /\ within SharedSlots.Controls.Pshare sp /\
      forall e,
        footprint_at SharedSlots.Controls.Pshare (regs sp) e <=
          1 + length (unplanned_at SharedSlots.Controls.Pshare (regs sp) e) /\
        footprint_at SharedSlots.Controls.Pshare (regs sp) e <= footprint_at baseline (regs sb) e /\
        footprint_at baseline (regs sb) e = length (allocs_at (regs sb) e)
  | None, None => True
  | _, _ => False
  end.
Proof.
  apply (reuse_footprint_bounded_arm SharedSlots.Controls.Pshare SharedSlots.Controls.sequential []
           sequential_accepted_by_join 1).
  intros s k H. unfold SharedSlots.Controls.Pshare in H.
  destruct s as [|[|s]]; [injection H as <-; lia|injection H as <-; lia|discriminate].
Qed.
