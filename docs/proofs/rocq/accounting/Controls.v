(** * Controls.v -- evidence that the accounting is not vacuous.

    - Increment B2's accepted loop [loop_ok] allocates at site 0 three times
      in each of two cycles.  Under the plan [P0], which reuses site 0 in slot
      0, each epoch uses one cell, while the baseline uses three, so the
      saving is strict and the bound of [footprint_slot_bound] is met with
      equality.
    - Under the baseline the same run uses three cells in an epoch with no
      slot at all, so the term counting allocations at sites the plan does
      not reuse is needed.  A bound by slots alone would be false.
    - The headline applies to [loop_ok] under [P0], which is proved injective
      here, so its premises are satisfiable. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape.
From RegionCore Require Controls.
From Accounting Require Import Accounting Footprint.
Import ListNotations.

Definition run_of (P : plan) : option state :=
  cycles P RegionCore.Controls.host_len RegionCore.Controls.loop_ok 100 2 init.

Definition fp (P : plan) (e : nat) : nat :=
  match run_of P with Some s => footprint_at P (regs s) e | None => 0 end.

Example loop_ok_footprints :
  (fp RegionCore.Controls.P0 0, fp RegionCore.Controls.P0 1, fp baseline 0, fp baseline 1) = (1, 1, 3, 3).
Proof. vm_compute. reflexivity. Qed.

Example loop_ok_bound_is_tight :
  match run_of RegionCore.Controls.P0 with
  | Some s => (footprint_at RegionCore.Controls.P0 (regs s) 0,
               1 + length (unplanned_at RegionCore.Controls.P0 (regs s) 0))
  | None => (0, 0)
  end = (1, 1).
Proof. vm_compute. reflexivity. Qed.

Example slots_alone_do_not_bound :
  ~ (forall e, fp baseline e <= 0).
Proof. intros H. specialize (H 0). vm_compute in H. lia. Qed.

Lemma P0_injective : injective RegionCore.Controls.P0.
Proof.
  intros s1 s2 k H1 H2. unfold RegionCore.Controls.P0 in *.
  destruct s1; [|discriminate]. destruct s2; [reflexivity|discriminate].
Qed.

Theorem loop_ok_footprint_bounded : forall host fuel n,
  match cycles RegionCore.Controls.P0 host RegionCore.Controls.loop_ok fuel n init,
        cycles baseline host RegionCore.Controls.loop_ok fuel n init with
  | Some sp, Some sb =>
      regs sp = regs sb /\ within RegionCore.Controls.P0 sp /\
      forall e,
        footprint_at RegionCore.Controls.P0 (regs sp) e <=
          1 + length (unplanned_at RegionCore.Controls.P0 (regs sp) e) /\
        footprint_at RegionCore.Controls.P0 (regs sp) e <= footprint_at baseline (regs sb) e /\
        footprint_at baseline (regs sb) e = length (allocs_at (regs sb) e)
  | None, None => True
  | _, _ => False
  end.
Proof.
  apply (reuse_footprint_bounded RegionCore.Controls.loop_ok [] RegionCore.Controls.loop_ok_accepted
           RegionCore.Controls.P0 1 P0_injective).
  intros s k H. unfold RegionCore.Controls.P0 in H.
  destruct s; [injection H as <-; lia|discriminate].
Qed.
