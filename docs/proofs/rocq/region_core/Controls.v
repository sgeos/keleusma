(** * Controls.v -- evidence that the checker and the theorem are not vacuous.

    - An accepted loop reuses one slot for every iteration's region, its
      reuse run and baseline run agree, and the headline theorem applies.
    - A rejected loop keeps a handle across an iteration, and its reuse run
      really does diverge, so the loop rule rejects a real hazard.
    - A duplicated handle read twice is accepted and agrees, and a module
      that the separation rule alone rejects diverges, so that rule guards a
      real hazard.
    - Two sites sharing one slot also diverge, so the premise that a plan
      gives distinct sites distinct slots does work.
    - Each escape rule rejects a module that breaks it. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Sim Reuse.
Import ListNotations.

Fixpoint blk (l : list instr) : block :=
  match l with [] => BNil | i :: r => BCons i (blk r) end.

Definition host_len (l : list ev) : nat := length l.

(** The plan reusing site 0 in slot 0, and the plan sharing slot 0 between
    sites 0 and 1. *)
Definition P0 : plan := fun s => match s with 0 => Some 0 | _ => None end.
Definition P01 : plan := fun s => match s with 0 => Some 0 | 1 => Some 0 | _ => None end.

Definition log_of (P : plan) (m : module) : list ev :=
  match cycles P host_len m 100 2 init with Some s => log s | None => [] end.

(** Each iteration counts in local 0, allocates a region holding the count,
    dereferences it, and yields the bytes read. *)
Definition loop_ok : module :=
  mkmodule [] (blk [LoopN 3 (blk [Load 0; Const 1; Add; Store 0; Load 0; New 0; Get; Yield; Store 1])]).

Example loop_ok_accepted : region_check loop_ok = Some [].
Proof. vm_compute. reflexivity. Qed.

Example loop_ok_agrees :
  log_of P0 loop_ok = log_of baseline loop_ok /\
  log_of P0 loop_ok = [ReadE (Some 1); OutE 1; ReadE (Some 2); OutE 2; ReadE (Some 3); OutE 3;
                       ReadE (Some 4); OutE 4; ReadE (Some 5); OutE 5; ReadE (Some 6); OutE 6].
Proof. vm_compute. split; reflexivity. Qed.

(** Under the plan, six regions occupy one slot and no bump cell. *)
Example loop_ok_one_slot :
  match cycles P0 host_len loop_ok 100 2 init with
  | Some s => (length (regs s), map (fun r => mem s (Bump r)) [0; 1; 2; 3; 4; 5], mem s (Slot 0))
  | None => (0, [], None)
  end = (6, [None; None; None; None; None; None], Some 6).
Proof. vm_compute. reflexivity. Qed.

Theorem loop_ok_sound : forall P, injective P -> forall host fuel n,
  match cycles P host loop_ok fuel n init, cycles baseline host loop_ok fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof. intros P HP. exact (reuse_sound loop_ok [] loop_ok_accepted P HP). Qed.

(** The loop leaves each iteration's handle on the stack and reads both after
    it, so the second allocation overwrites the slot the first handle names.
    The checker rejects it by the loop rule, the body leaving a handle on the
    stack, before the separation rule is reached. *)
Definition loop_bad : module :=
  mkmodule [] (blk [LoopN 2 (blk [Load 0; Const 1; Add; Store 0; Load 0; New 0]);
                    Get; Yield; Store 1; Get; Yield; Store 1]).

Example loop_bad_rejected : region_check loop_bad = None.
Proof. vm_compute. reflexivity. Qed.

Example loop_bad_diverges :
  log_of P0 loop_bad = [ReadE (Some 2); OutE 2; ReadE (Some 2); OutE 2;
                        ReadE (Some 4); OutE 4; ReadE (Some 4); OutE 4] /\
  log_of baseline loop_bad = [ReadE (Some 2); OutE 2; ReadE (Some 1); OutE 1;
                              ReadE (Some 4); OutE 4; ReadE (Some 3); OutE 3].
Proof. vm_compute. split; reflexivity. Qed.

(** Two sites, both handles live together.  The checker accepts it, because
    distinct sites never conflict under a well-formed plan, and the plan that
    shares their slot diverges, so the injectivity premise is necessary. *)
Definition two_sites : module :=
  mkmodule [] (blk [Const 1; New 0; Const 2; New 1; Get; Yield; Store 1; Get; Yield; Store 1]).

Example two_sites_accepted : region_check two_sites = Some [].
Proof. vm_compute. reflexivity. Qed.

Example shared_slot_diverges : log_of P01 two_sites <> log_of baseline two_sites.
Proof. vm_compute. discriminate. Qed.

Example shared_slot_not_injective : ~ injective P01.
Proof. intros H. discriminate (H 0 1 0 eq_refl eq_refl). Qed.

(** A handle duplicated and read twice in each iteration.  It is accepted and
    the two runs agree. *)
Definition loop_dup : module :=
  mkmodule [] (blk [LoopN 2 (blk [Load 0; Const 1; Add; Store 0; Load 0; New 0; Dup; Get; Yield; Store 1;
                                  Get; Yield; Store 1])]).

Example loop_dup_agrees :
  region_check loop_dup = Some [] /\ log_of P0 loop_dup = log_of baseline loop_dup.
Proof. vm_compute. split; reflexivity. Qed.

(** A straight-line module whose only fault is allocating site 0 while a copy
    of an earlier handle of site 0 is live.  The separation rule alone
    rejects it, and its reuse run diverges, reading 2 where the baseline reads
    1, so that rule guards a real hazard. *)
Definition sep_only : module :=
  mkmodule [] (blk [Const 1; New 0; Dup; Get; Yield; Store 1; Const 2; New 0; Drop; Get; Yield; Store 1]).

Example sep_only_rejected_at_allocation :
  shape_b [] true [] (blk [Const 1; New 0; Dup; Get; Yield; Store 1; Const 2]) = Some [TS; TR 0] /\
  region_check sep_only = None.
Proof. vm_compute. split; reflexivity. Qed.

Example sep_only_diverges :
  log_of P0 sep_only = [ReadE (Some 1); OutE 1; ReadE (Some 2); OutE 2;
                        ReadE (Some 1); OutE 1; ReadE (Some 2); OutE 2] /\
  log_of baseline sep_only = [ReadE (Some 1); OutE 1; ReadE (Some 1); OutE 1;
                              ReadE (Some 1); OutE 1; ReadE (Some 1); OutE 1].
Proof. vm_compute. split; reflexivity. Qed.

(** Consuming both copies before the site runs again is accepted. *)
Example accept_copies_consumed :
  region_check (mkmodule [] (blk [Const 1; New 0; Dup; Get; Yield; Store 1; Get; Yield; Store 1;
                                  Const 2; New 0; Get; Yield; Store 1])) = Some [].
Proof. vm_compute. reflexivity. Qed.

(** ** Each escape rule rejects a module that breaks it. *)
Example reject_yield_handle : region_check (mkmodule [] (blk [Const 1; New 0; Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_store_handle : region_check (mkmodule [] (blk [Const 1; New 0; Store 0])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_branch_on_handle :
  region_check (mkmodule [] (blk [Const 1; New 0; If BNil BNil])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_arith_on_handle :
  region_check (mkmodule [] (blk [Const 1; New 0; Const 1; Add])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_get_scalar : region_check (mkmodule [] (blk [Const 1; Get])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_new_in_function :
  region_check (mkmodule [blk [Const 1; New 0; Get]] (blk [Call 0; Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_arms_differ :
  region_check (mkmodule [] (blk [Const 0; If (blk [Const 1; New 0]) (blk [Const 2])])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_second_live_allocation :
  region_check (mkmodule [] (blk [Const 1; New 0; Const 2; New 0])) = None.
Proof. vm_compute. reflexivity. Qed.

(** A handle consumed before its site runs again is accepted, even outside a
    loop, which shows the last rejection depends on the handle being live. *)
Example accept_consumed_then_reallocated :
  region_check (mkmodule [] (blk [Const 1; New 0; Get; Yield; Const 2; New 0; Get; Yield])) = Some [].
Proof. vm_compute. reflexivity. Qed.
