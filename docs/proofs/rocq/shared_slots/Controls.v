(** * Controls.v -- evidence that shared slots are not vacuous.

    - A loop whose two conditional arms allocate different sites into one
      shared slot is accepted under the sharing plan, which increment B2's
      theorem cannot cover because the plan is not injective, and its reuse
      run agrees with the baseline.  This is the arm group of Part I's
      Theorem A2 and Corollary A3.
    - Two sites sharing a slot one after the other, each handle dying before
      the next allocation, are accepted and agree.
    - Two sites sharing a slot while both handles are live are rejected, and
      their reuse run really diverges under the sharing plan.
    - A conditional whose arms allocate different sites and whose chosen
      region is used after it needs the join checker, and agrees. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Controls.
From SharedSlots Require Import SharedShape Shared ArmShape Arm.
Import ListNotations.

(** Sites 0 and 1 share slot 0. *)
Definition Pshare : plan := fun s => match s with 0 => Some 0 | 1 => Some 0 | _ => None end.

Example Pshare_not_injective : ~ injective Pshare.
Proof. intros H. discriminate (H 0 1 0 eq_refl eq_refl). Qed.

(** Each iteration counts in local 0 and allocates site 0 in its first arm
    while the count is below 3 and site 1 otherwise, then reads and yields.
    The count carries across cycles, so the second cycle takes only the
    second arm. *)
Definition arms : module :=
  mkmodule [] (blk [LoopN 4 (blk [Load 0; Const 1; Add; Store 0;
                                  Load 0; Const 3; Lt;
                                  If (blk [Load 0; New 0; Get; Yield; Store 1])
                                     (blk [Load 0; Const 100; Add; New 1; Get; Yield; Store 1])])]).

Example arms_accepted : region_checkP Pshare arms = Some [].
Proof. vm_compute. reflexivity. Qed.

Example arms_agree :
  log_of Pshare arms = log_of baseline arms /\
  log_of Pshare arms = [ReadE (Some 1); OutE 1; ReadE (Some 2); OutE 2;
                        ReadE (Some 103); OutE 103; ReadE (Some 104); OutE 104;
                        ReadE (Some 105); OutE 105; ReadE (Some 106); OutE 106;
                        ReadE (Some 107); OutE 107; ReadE (Some 108); OutE 108].
Proof. vm_compute. split; reflexivity. Qed.

(** Under the sharing plan both sites allocate, their regions occupy the one
    slot, and no bump cell is written. *)
Example arms_one_slot :
  match cycles Pshare host_len arms 100 2 init with
  | Some s => (map fst (regs s), map (fun r => mem s (Bump r)) [0; 1; 2; 3; 4; 5; 6; 7], mem s (Slot 0))
  | None => ([], [], None)
  end = ([0; 0; 1; 1; 1; 1; 1; 1], [None; None; None; None; None; None; None; None], Some 108).
Proof. vm_compute. reflexivity. Qed.

Theorem arms_sound : forall host fuel n,
  match cycles Pshare host arms fuel n init, cycles baseline host arms fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof. exact (reuse_sound_shared Pshare arms [] arms_accepted). Qed.

Definition sequential : module :=
  mkmodule [] (blk [Const 1; New 0; Get; Yield; Store 1; Const 2; New 1; Get; Yield; Store 1]).

Example sequential_accepted_and_agrees :
  region_checkP Pshare sequential = Some [] /\ log_of Pshare sequential = log_of baseline sequential.
Proof. vm_compute. split; reflexivity. Qed.

Example co_live_rejected : region_checkP Pshare two_sites = None /\ region_check two_sites = Some [].
Proof. vm_compute. split; reflexivity. Qed.

(** The co-live module really diverges under the sharing plan itself. *)
Example co_live_diverges : log_of Pshare two_sites <> log_of baseline two_sites.
Proof. vm_compute. discriminate. Qed.

(** ** Arm groups whose regions outlive the conditional.

    Each iteration chooses a site by a conditional, allocates it in the arm,
    and dereferences the chosen region after the conditional.  The arms leave
    different handles, so [SharedShape] rejects it, while [ArmShape] joins
    them and accepts it under the sharing plan, and its reuse run agrees. *)
Definition choose : module :=
  mkmodule [] (blk [LoopN 3 (blk [Load 0; Const 1; Add; Store 0; Load 0; Const 2; Lt;
     If (blk [Load 0; New 0]) (blk [Load 0; Const 100; Add; New 1]); Get; Yield; Store 1])]).

Example choose_needs_join :
  region_checkP Pshare choose = None /\ region_checkA Pshare choose = Some [].
Proof. vm_compute. split; reflexivity. Qed.

Example choose_agrees :
  log_of Pshare choose = log_of baseline choose /\
  match cycles Pshare host_len choose 100 2 init with
  | Some s => (map fst (regs s), mem s (Slot 0))
  | None => ([], None)
  end = ([0; 1; 1; 1; 1; 1], Some 106).
Proof. vm_compute. split; reflexivity. Qed.

Theorem choose_sound : forall host fuel n,
  match cycles Pshare host choose fuel n init, cycles baseline host choose fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof. exact (reuse_sound_arm Pshare choose [] (proj2 choose_needs_join)). Qed.

(** The join checker still rejects the co-live module. *)
Example co_live_rejected_by_join : region_checkA Pshare two_sites = None.
Proof. vm_compute. reflexivity. Qed.
