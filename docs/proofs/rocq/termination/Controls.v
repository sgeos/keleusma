(** * Controls.v -- evidence that the termination result is not vacuous.

    - Increment B2's accepted loop [loop_ok] finishes at [bound loop_ok] but
      stops at fuel 1, so the bound does work and finishing is not automatic.
    - A function calling itself, and a function calling a later function, are
      both rejected by B2's checker, which is what the termination rests on.
    - An accepted module whose main calls a function that calls another
      twice finishes at its bound.
    - The corollary applies to [loop_ok] under an injective plan, so its
      premises are satisfiable. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Reuse.
From RegionCore Require Controls.
From SharedSlots Require Import ArmShape Arm.
From SharedSlots Require Controls.
From Termination Require Import Termination TerminationArm.
Import ListNotations.

Definition finishes (P : plan) (m : module) (fuel : nat) : bool :=
  match cycles P RegionCore.Controls.host_len m fuel 2 init with Some _ => true | None => false end.

Example loop_ok_bound_is_needed :
  (finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok (bound RegionCore.Controls.loop_ok),
   finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok 1) = (true, false).
Proof. vm_compute. reflexivity. Qed.

Example self_call_rejected :
  region_check (mkmodule [RegionCore.Controls.blk [Call 0]] BNil) = None.
Proof. vm_compute. reflexivity. Qed.

Example forward_call_rejected :
  region_check (mkmodule [RegionCore.Controls.blk [Call 1]; BNil] BNil) = None.
Proof. vm_compute. reflexivity. Qed.

Definition call_chain : module :=
  mkmodule [RegionCore.Controls.blk [Const 1]; RegionCore.Controls.blk [Call 0; Call 0]]
           (RegionCore.Controls.blk [Call 1; Add; Yield; Drop]).

Example call_chain_accepted_and_finishes :
  region_check call_chain = Some [[TS]; [TS; TS]] /\
  finishes baseline call_chain (bound call_chain) = true.
Proof. vm_compute. split; reflexivity. Qed.

Lemma P0_injective : injective RegionCore.Controls.P0.
Proof.
  intros s1 s2 k H1 H2. unfold RegionCore.Controls.P0 in *.
  destruct s1; [|discriminate]. destruct s2; [reflexivity|discriminate].
Qed.

Theorem loop_ok_runs_agree : forall host n,
  exists sp sb,
    cycles RegionCore.Controls.P0 host RegionCore.Controls.loop_ok (bound RegionCore.Controls.loop_ok) n init = Some sp /\
    cycles baseline host RegionCore.Controls.loop_ok (bound RegionCore.Controls.loop_ok) n init = Some sb /\
    log sp = log sb.
Proof.
  exact (accepted_runs_agree RegionCore.Controls.loop_ok [] RegionCore.Controls.loop_ok_accepted
           RegionCore.Controls.P0 P0_injective).
Qed.

(** The join checker's corollary applies to a plan that is not injective,
    the shared-slot module [sequential] under [Pshare], so its premise is
    satisfiable where B2's corollary cannot apply. *)
Example sequential_accepted_by_join :
  region_checkA SharedSlots.Controls.Pshare SharedSlots.Controls.sequential = Some [].
Proof. vm_compute. reflexivity. Qed.

Theorem sequential_runs_agree : forall host n,
  exists sp sb,
    cycles SharedSlots.Controls.Pshare host SharedSlots.Controls.sequential
      (bound SharedSlots.Controls.sequential) n init = Some sp /\
    cycles baseline host SharedSlots.Controls.sequential (bound SharedSlots.Controls.sequential) n init = Some sb /\
    log sp = log sb.
Proof.
  exact (accepted_runs_agree_arm SharedSlots.Controls.Pshare SharedSlots.Controls.sequential []
           sequential_accepted_by_join).
Qed.
