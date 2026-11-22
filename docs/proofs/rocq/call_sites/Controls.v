(** * Controls.v -- evidence that the call-site checker and theorem are not vacuous.

    - [twice] calls a function that allocates at site 1, twice in each cycle.
      Increment B2's checker rejects it, since its functions may not
      allocate.  This checker accepts it, its reuse run under [P1], which
      puts site 1 in slot 0, agrees with the baseline, and the headline
      applies to it.
    - [live_across_call] allocates at site 1, then calls the same function
      while that handle is live, then reads the handle.  This checker rejects
      it by the call rule, and its reuse run really diverges, the read
      seeing the callee's value instead of its own, so the call rule guards
      a real hazard. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape.
From RegionCore Require Controls.
From CallSites Require Import ShapeC CallSites.
Import ListNotations.

Definition blk := RegionCore.Controls.blk.

Definition P1 : plan := fun s => match s with 1 => Some 0 | _ => None end.

Lemma P1_injective : injective P1.
Proof.
  intros s1 s2 k H1 H2. unfold P1 in *.
  destruct s1 as [|[|s1]]; try discriminate. destruct s2 as [|[|s2]]; try discriminate. reflexivity.
Qed.

Definition log_of (P : plan) (m : module) : list ev :=
  match cycles P RegionCore.Controls.host_len m 100 2 init with Some s => log s | None => [] end.

(** A function allocating at site 1, reading the region and returning the
    number read. *)
Definition alloc_fn : block := blk [Const 7; New 1; Get].

Definition twice : module :=
  mkmodule [alloc_fn] (blk [LoopN 2 (blk [Call 0; Yield; Store 1])]).

Example twice_rejected_by_B2 : region_check twice = None.
Proof. vm_compute. reflexivity. Qed.

Example twice_accepted : region_checkC twice = Some [([TS], [1])].
Proof. vm_compute. reflexivity. Qed.

Example twice_agrees :
  log_of P1 twice = log_of baseline twice /\
  log_of P1 twice = [ReadE (Some 7); OutE 7; ReadE (Some 7); OutE 7;
                     ReadE (Some 7); OutE 7; ReadE (Some 7); OutE 7].
Proof. vm_compute. split; reflexivity. Qed.

Theorem twice_sound : forall host fuel n,
  match cycles P1 host twice fuel n init, cycles baseline host twice fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof. exact (reuse_sound_calls twice _ twice_accepted P1 P1_injective). Qed.

Definition live_across_call : module :=
  mkmodule [alloc_fn] (blk [Const 5; New 1; Call 0; Drop; Get; Yield; Drop]).

Example live_across_call_rejected : region_checkC live_across_call = None.
Proof. vm_compute. reflexivity. Qed.

Example live_across_call_diverges : log_of P1 live_across_call <> log_of baseline live_across_call.
Proof. vm_compute. discriminate. Qed.
