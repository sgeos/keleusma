(** * Controls.v -- evidence that the checker and the theorems are not vacuous.

    - An example module with one function of each category is accepted, its
      figures are computed, concrete runs stay within them, and the headline
      theorems apply to it.
    - Both figures are attained by a concrete run, the time figure by the
      junction through a reset, and a second module attains its time figure
      only at that junction, so the junction term of [wcet] is exercised.
    - An empty loop is charged for its iterations.
    - The checker's three rules reject seven modules, two for the call rule,
      two for the [fn] rule, and three for the productivity rule. *)

From Stdlib Require Import List Arith Lia.
From CoreBounds Require Import Core Check Algebra Soundness.
Import ListNotations.

Fixpoint blk (l : list instr) : block :=
  match l with [] => BNil | i :: r => BCons i (blk r) end.

(** [f0], an atomic total [fn], adds two and allocates one word three times. *)
Definition f0 : func := mkfunc KFn (blk [Const 2; Add; LoopN 3 (blk [Alloc 1])]).
(** [f1], a non-atomic total [yield] function, calls [f0], yields, and then
    allocates four words only when the host's reply is nonzero. *)
Definition f1 : func := mkfunc KYield (blk [Call 0; Yield; If (blk [Alloc 4]) (blk [Const 0])]).
(** The productive divergent [loop main] calls [f1] and yields once more. *)
Definition ex_main : block := blk [Load 0; Call 1; Store 0; Const 7; Yield].

Definition ex : module := mkmodule [f0; f1] ex_main.

Definition ex_tab : list abs := match check ex with Some (t, _) => t | None => [] end.
Definition ex_A : abs := match check ex with Some (_, a) => a | None => zero end.

Lemma ex_accepted : check ex = Some (ex_tab, ex_A).
Proof. vm_compute. reflexivity. Qed.

(** The published figures and the fuel of the example. *)
Example ex_figures : wcet ex_A = 14 /\ wcmu ex_A = 7 /\ ad ex_A = 14.
Proof. vm_compute. repeat split. Qed.

(** A host that replies with one, so [f1] takes its allocating arm. *)
Definition host1 (o : list nat) : nat := 1.

Example ex_two_cycles :
  match cycles host1 ex (ad ex_A) 2 (mkstate [] (fun _ => 0) 0 []) with
  | Some cs => (map fst cs, map snd cs, stream_segs cs)
  | None => ([], [], [])
  end = ([[13; 5; 0]; [13; 5; 0]], [7; 7], [13; 5; 14; 5; 1]).
Proof. vm_compute. reflexivity. Qed.

(** A host that replies with zero, so [f1] skips its allocation. *)
Definition host0 (o : list nat) : nat := 0.

Example ex_two_cycles_host0 :
  match cycles host0 ex (ad ex_A) 2 (mkstate [] (fun _ => 0) 0 []) with
  | Some cs => (map snd cs, stream_segs cs)
  | None => ([], [])
  end = ([3; 3], [13; 5; 14; 5; 1]).
Proof. vm_compute. reflexivity. Qed.

Definition s0 : state := mkstate [] (fun _ => 0) 0 [].

Definition ex_cs : list (segs * nat) :=
  match cycles host1 ex (ad ex_A) 2 s0 with Some cs => cs | None => [] end.

(** Both figures are attained by the run itself.  Under [host1] the stream
    reaches the time figure at the junction through the first reset, last
    segment 0, reset 1, next first segment 13, and every cycle reaches the
    memory figure. *)
Theorem ex_bounds_attained :
  cycles host1 ex (ad ex_A) 2 s0 = Some ex_cs /\
  In (wcet ex_A) (stream_segs ex_cs) /\ Forall (fun c => snd c = wcmu ex_A) ex_cs.
Proof.
  split; [vm_compute; reflexivity|]. split.
  - vm_compute. right. right. left. reflexivity.
  - vm_compute. repeat constructor.
Qed.

(** A module whose within-cycle segments are at most 2, and whose stream
    reaches its time figure 4 only at the junction, 2 plus the reset plus 1. *)
Definition jm : module := mkmodule [] (blk [Yield; Const 0; Const 0]).

Example junction_alone_attains :
  match check jm with
  | Some (_, A) =>
      ay A = Some (1, 0, 2) /\ wcet A = 4 /\
      match cycles host1 jm (ad A) 2 s0 with Some cs => stream_segs cs | None => [] end = [1; 4; 3]
  | None => False
  end.
Proof. vm_compute. repeat split. Qed.

(** An empty loop of twenty iterations is charged one unit per iteration.  A
    cycle's run is the loop instruction, twenty back edges and the yield, 22
    units, and the time figure 23 adds the reset at the junction. *)
Example loop_iterations_counted :
  match check (mkmodule [] (blk [LoopN 20 BNil; Yield])) with
  | Some (_, A) =>
      wcet A = 23 /\
      match cycle host1 (mkmodule [] (blk [LoopN 20 BNil; Yield])) (ad A) s0 with
      | Some (_, p) => p
      | None => []
      end = [22; 0]
  | None => False
  end.
Proof. vm_compute. split; reflexivity. Qed.

(** The headline theorem applied to the example, for every host and every
    number of cycles. *)
Theorem ex_stream_bounded : forall host n s, exists cs,
  cycles host ex (ad ex_A) n s = Some cs /\ length cs = n /\
  (forall c, In c cs -> snd c <= 7) /\
  (forall x, In x (stream_segs cs) -> x <= 14).
Proof.
  intros host n s.
  destruct (stream_bounded host ex ex_tab ex_A ex_accepted n s) as [cs [R [L [M T]]]].
  exists cs. split; [exact R|]. split; [exact L|].
  destruct ex_figures as [Ht [Hm _]]. rewrite <- Ht, <- Hm. split; assumption.
Qed.

(** ** Each rule rejects a module that breaks it. *)
Example reject_recursion :
  check (mkmodule [mkfunc KYield (blk [Call 0])] (blk [Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_forward_call :
  check (mkmodule [mkfunc KYield (blk [Call 1]); mkfunc KYield (blk [Const 0])] (blk [Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_yielding_fn :
  check (mkmodule [mkfunc KFn (blk [Yield])] (blk [Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_fn_calling_yield :
  check (mkmodule [mkfunc KYield (blk [Yield]); mkfunc KFn (blk [Call 0])] (blk [Yield])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_unproductive_path :
  check (mkmodule [] (blk [Const 0; If (blk [Yield]) (blk [Const 1])])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_unproductive_main :
  check (mkmodule [] (blk [Const 0])) = None.
Proof. vm_compute. reflexivity. Qed.

Example reject_zero_iteration_yield :
  check (mkmodule [] (blk [LoopN 0 (blk [Yield])])) = None.
Proof. vm_compute. reflexivity. Qed.

(** The productive main body with the same shape as the last rejection, but
    an iteration count of one, is accepted. *)
Example accept_one_iteration_yield :
  check (mkmodule [] (blk [LoopN 1 (blk [Yield])])) <> None.
Proof. vm_compute. discriminate. Qed.
