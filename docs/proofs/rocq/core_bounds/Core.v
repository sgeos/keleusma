(** * Core.v -- a small core of the structural instruction set and its semantics.

    The core of increment A of the proof line, from
    [docs/spec/STRUCTURAL_ISA.md] and the three function categories of
    [docs/architecture/LANGUAGE_DESIGN.md].

    - Instructions are block-structured.  [If] chooses one of two blocks,
      [LoopN] repeats a block a constant number of times, and there is no flat
      jump, which is the structural instruction set's control-flow rule.
    - Values are natural numbers.  Every operation is total, an empty operand
      stack reading as zero, so the semantics never faults, as the language's
      totality guarantee requires.  This replaces the typed stack discipline
      the real verifier checks, which the bound theorems do not need.
    - [Alloc k] takes [k] words of the arena, the quantity the worst-case
      memory bound counts.  [Yield] sends the top of the stack to the host and
      pushes the host's reply.
    - A module is a list of functions, each of the category [fn] or [yield],
      and one [loop main] body run once per stream cycle, the arena reset at
      each cycle's start.

    The semantics is an executable interpreter.  It takes fuel, which every
    recursive step consumes, and returns [None] only when that fuel runs out,
    which [Soundness.v] shows never happens for an accepted module given the
    fuel its checker computes.  Each
    run also returns its segments, the number of units executed between
    consecutive yields, so time is measured by the semantics itself.  Every
    core instruction costs one unit, each loop iteration costs one more for
    its back edge, and each reset costs one.  The block delimiters [EndIf],
    [Return] and the stream entry are not charged separately. *)

From Stdlib Require Import List Arith Lia.
Import ListNotations.

Inductive kind : Type := KFn | KYield.

Inductive instr : Type :=
| Const (n : nat)
| Load (i : nat)
| Store (i : nat)
| Add
| Sub
| Lt
| If (t e : block)
| LoopN (n : nat) (body : block)
| Call (f : nat)
| Alloc (k : nat)
| Yield
with block : Type :=
| BNil
| BCons (i : instr) (rest : block).

Scheme instr_mut := Induction for instr Sort Prop
with block_mut := Induction for block Sort Prop.

Record func : Type := mkfunc { fkind : kind; fbody : block }.
Record module : Type := mkmodule { mfuncs : list func; mmain : block }.

Record state : Type := mkstate {
  stk : list nat;
  locs : nat -> nat;
  arena : nat;
  outs : list nat
}.

(** ** Segments.

    A run's segments are the instruction counts between consecutive yields,
    in order.  The first counts up to and including the first yield, each
    yield closes one segment and opens the next, and the last is the count
    after the final yield.  A run that never yields has one segment.  Running
    two pieces in sequence joins their segment lists, the last segment of the
    first continuing into the first segment of the second. *)
Definition segs := list nat.

Fixpoint join (a b : segs) : segs :=
  match a with
  | [] => b
  | [x] => match b with [] => [x] | y :: b' => (x + y) :: b' end
  | x :: a' => x :: join a' b
  end.

Definition push (n : nat) (s : state) : state := mkstate (n :: stk s) (locs s) (arena s) (outs s).

Definition pop (s : state) : nat * state :=
  match stk s with
  | x :: r => (x, mkstate r (locs s) (arena s) (outs s))
  | [] => (0, s)
  end.

Definition store (i v : nat) (s : state) : state :=
  mkstate (stk s) (fun j => if Nat.eqb j i then v else locs s j) (arena s) (outs s).

Definition grow (k : nat) (s : state) : state :=
  mkstate (stk s) (locs s) (arena s + k) (outs s).

Section Semantics.

(** The host answers each yield, given everything the program has sent. *)
Variable host : list nat -> nat.

Definition yield_step (s : state) : state :=
  let (v, s1) := pop s in
  let o := outs s1 ++ [v] in
  mkstate (host o :: stk s1) (locs s1) (arena s1) o.

Definition bind (r : option (state * segs)) (k : state -> segs -> option (state * segs)) :=
  match r with Some (s, p) => k s p | None => None end.

(** The interpreter.  [n] is fuel, which every recursive step consumes, so
    the functions are structurally recursive.  [Soundness.v] shows that the
    fuel the checker computes always suffices for an accepted module. *)
Fixpoint run_b (fs : list func) (n : nat) (b : block) (s : state) {struct n}
  : option (state * segs) :=
  match n with
  | 0 => None
  | S n' =>
      match b with
      | BNil => Some (s, [0])
      | BCons i r =>
          bind (run_i fs n' i s) (fun s1 p1 =>
            bind (run_b fs n' r s1) (fun s2 p2 => Some (s2, join p1 p2)))
      end
  end
with run_i (fs : list func) (n : nat) (i : instr) (s : state) {struct n}
  : option (state * segs) :=
  match n with
  | 0 => None
  | S n' =>
      match i with
      | Const k => Some (push k s, [1])
      | Load j => Some (push (locs s j) s, [1])
      | Store j => let (v, s1) := pop s in Some (store j v s1, [1])
      | Add => let (b, s1) := pop s in let (a, s2) := pop s1 in Some (push (a + b) s2, [1])
      | Sub => let (b, s1) := pop s in let (a, s2) := pop s1 in Some (push (a - b) s2, [1])
      | Lt => let (b, s1) := pop s in let (a, s2) := pop s1 in
              Some (push (if Nat.ltb a b then 1 else 0) s2, [1])
      | If t e =>
          let (c, s1) := pop s in
          bind (run_b fs n' (if Nat.eqb c 0 then e else t) s1) (fun s2 p => Some (s2, join [1] p))
      | LoopN k body => bind (run_loop fs n' k body s) (fun s2 p => Some (s2, join [1] p))
      | Call f =>
          match nth_error fs f with
          | Some g => bind (run_b fs n' (fbody g) s) (fun s2 p => Some (s2, join [1] p))
          | None => None
          end
      | Alloc k => Some (grow k s, [1])
      | Yield => Some (yield_step s, [1; 0])
      end
  end
with run_loop (fs : list func) (n : nat) (k : nat) (body : block) (s : state) {struct n}
  : option (state * segs) :=
  match n with
  | 0 => None
  | S n' =>
      match k with
      | 0 => Some (s, [0])
      | S k' =>
          bind (run_b fs n' body s) (fun s1 p1 =>
            bind (run_loop fs n' k' body s1) (fun s2 p2 => Some (s2, join p1 (join [1] p2))))
      end
  end.

(** One stream cycle runs the main body from a state whose arena and operand
    stack are reset.  Clearing the stack stands for the stack balance the
    shipping verifier enforces at the reset, so nothing accumulates across
    cycles. *)
Definition reset (s : state) : state := mkstate [] (locs s) 0 (outs s).

Definition cycle (m : module) (fuel : nat) (s : state) : option (state * segs) :=
  run_b (mfuncs m) fuel (mmain m) (reset s).

(** [cycles m fuel n s] runs [n] cycles, returning each cycle's segments and the
    arena it ended with. *)
Fixpoint cycles (m : module) (fuel n : nat) (s : state) : option (list (segs * nat)) :=
  match n with
  | 0 => Some []
  | S n' =>
      match cycle m fuel s with
      | Some (s1, p) =>
          match cycles m fuel n' s1 with
          | Some ps => Some ((p, arena s1) :: ps)
          | None => None
          end
      | None => None
      end
  end.

(** The segments of a stream of cycles.  Each cycle ends with the reset
    instruction, which costs one unit and is not a yield, so the last segment
    of one cycle continues through the reset into the first segment of the
    next. *)
Definition stream_segs (cs : list (segs * nat)) : segs :=
  fold_right (fun c acc => join (fst c) (join [1] acc)) [0] cs.

End Semantics.
