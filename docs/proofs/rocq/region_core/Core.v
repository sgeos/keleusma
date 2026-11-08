(** * Core.v -- the region core, increment A's core extended with composites.

    Increment B2 of the proof line.  The core of [../core_bounds/Core.v]
    gains the machinery of the region-reuse proof's Part I.

    - [New s] pops a scalar, allocates a fresh region at the static site [s]
      holding it, and pushes a handle to the region.  Under a plan, a site
      the plan reuses writes its slot instead of a fresh bump cell, which is
      M1's slot reallocation.
    - [Dup] duplicates the top entry and [Drop] discards it, so a handle may
      be copied, read many times and discarded.
    - [Get] pops a handle and dereferences it.  A handle carries its region's
      creation epoch, and a dereference succeeds exactly when that epoch is
      current, which is Definition 2 with M2.
    - The reset at the end of each cycle advances the epoch and clears the
      operand stack, which is M4 with M5.
    - The observations are the log of every value yielded and every
      dereference outcome, Definition 5's observations.  The host replies to
      each yield as a function of the log so far, which is Definition 5's
      symbolic environment behaviour.

    Time and the arena bound are increment A's subject and are dropped here,
    so the interpreter returns only the final state.  A call runs its callee
    on a fresh operand stack and pushes the callee's final stack onto the
    caller's, so a callee never touches its caller's entries. *)

From Stdlib Require Import List Arith Lia.
Import ListNotations.

Inductive val : Type := VN (n : nat) | VH (r e : nat).

Definition num (v : val) : nat := match v with VN n => n | VH _ _ => 0 end.

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
| Yield
| New (s : nat)
| Get
| Dup
| Drop
with block : Type :=
| BNil
| BCons (i : instr) (rest : block).

Scheme instr_mut := Induction for instr Sort Prop
with block_mut := Induction for block Sort Prop.

Record module : Type := mkmodule { mfuncs : list block; mmain : block }.

(** An observation is a yielded value or a dereference outcome, [None] being
    a stale or failed dereference. *)
Inductive ev : Type := OutE (n : nat) | ReadE (o : option nat).

Inductive cell : Type := Bump (r : nat) | Slot (k : nat).

Definition cell_eq_dec : forall a b : cell, {a = b} + {a <> b}.
Proof. decide equality; apply Nat.eq_dec. Defined.

(** A plan gives a reused site its slot.  The baseline reuses nothing. *)
Definition plan := nat -> option nat.
Definition baseline : plan := fun _ => None.

Definition cell_of (P : plan) (s r : nat) : cell :=
  match P s with Some k => Slot k | None => Bump r end.

Record state : Type := mkstate {
  stk : list val;
  locs : nat -> val;
  log : list ev;
  regs : list (nat * nat);
  mem : cell -> option nat;
  epoch : nat
}.

Definition with_stk (s : state) (l : list val) : state :=
  mkstate l (locs s) (log s) (regs s) (mem s) (epoch s).

Definition push (v : val) (s : state) : state := with_stk s (v :: stk s).

Definition pop (s : state) : val * state :=
  match stk s with
  | x :: r => (x, with_stk s r)
  | [] => (VN 0, s)
  end.

Definition store (i : nat) (v : val) (s : state) : state :=
  mkstate (stk s) (fun j => if Nat.eqb j i then v else locs s j) (log s) (regs s) (mem s) (epoch s).

Definition note (e : ev) (s : state) : state :=
  mkstate (stk s) (locs s) (log s ++ [e]) (regs s) (mem s) (epoch s).

Section Semantics.

Variable P : plan.
Variable host : list ev -> nat.

Definition yield_step (s : state) : state :=
  let (v, s1) := pop s in
  let s2 := note (OutE (num v)) s1 in
  push (VN (host (log s2))) s2.

Definition new_step (site : nat) (s : state) : state :=
  let (v, s1) := pop s in
  let r := length (regs s1) in
  let c := cell_of P site r in
  mkstate (VH r (epoch s1) :: stk s1) (locs s1) (log s1) (regs s1 ++ [(site, epoch s1)])
          (fun c' => if cell_eq_dec c' c then Some (num v) else mem s1 c') (epoch s1).

Definition deref (s : state) (v : val) : option nat :=
  match v with
  | VH r e =>
      if Nat.eqb e (epoch s) then
        match nth_error (regs s) r with
        | Some (site, _) => mem s (cell_of P site r)
        | None => None
        end
      else None
  | VN _ => None
  end.

Definition get_step (s : state) : state :=
  let (v, s1) := pop s in
  let o := deref s1 v in
  push (VN (match o with Some x => x | None => 0 end)) (note (ReadE o) s1).

Definition bind (r : option state) (k : state -> option state) : option state :=
  match r with Some s => k s | None => None end.

Fixpoint run_b (fs : list block) (n : nat) (b : block) (s : state) {struct n} : option state :=
  match n with
  | 0 => None
  | S n' =>
      match b with
      | BNil => Some s
      | BCons i r => bind (run_i fs n' i s) (run_b fs n' r)
      end
  end
with run_i (fs : list block) (n : nat) (i : instr) (s : state) {struct n} : option state :=
  match n with
  | 0 => None
  | S n' =>
      match i with
      | Const k => Some (push (VN k) s)
      | Load j => Some (push (locs s j) s)
      | Store j => let (v, s1) := pop s in Some (store j v s1)
      | Add => let (b, s1) := pop s in let (a, s2) := pop s1 in Some (push (VN (num a + num b)) s2)
      | Sub => let (b, s1) := pop s in let (a, s2) := pop s1 in Some (push (VN (num a - num b)) s2)
      | Lt => let (b, s1) := pop s in let (a, s2) := pop s1 in
              Some (push (VN (if Nat.ltb (num a) (num b) then 1 else 0)) s2)
      | If t e => let (c, s1) := pop s in run_b fs n' (if Nat.eqb (num c) 0 then e else t) s1
      | LoopN k body => run_loop fs n' k body s
      | Call f =>
          match nth_error fs f with
          | Some g => bind (run_b fs n' g (with_stk s [])) (fun s2 => Some (with_stk s2 (stk s2 ++ stk s)))
          | None => None
          end
      | Yield => Some (yield_step s)
      | New site => Some (new_step site s)
      | Get => Some (get_step s)
      | Dup => let (v, s1) := pop s in Some (push v (push v s1))
      | Drop => let (_, s1) := pop s in Some s1
      end
  end
with run_loop (fs : list block) (n k : nat) (body : block) (s : state) {struct n} : option state :=
  match n with
  | 0 => None
  | S n' =>
      match k with
      | 0 => Some s
      | S k' => bind (run_b fs n' body s) (run_loop fs n' k' body)
      end
  end.

(** The reset ends a cycle, advancing the epoch and clearing the stack. *)
Definition reset (s : state) : state :=
  mkstate [] (locs s) (log s) (regs s) (mem s) (S (epoch s)).

Fixpoint cycles (m : module) (fuel n : nat) (s : state) : option state :=
  match n with
  | 0 => Some s
  | S n' => bind (run_b (mfuncs m) fuel (mmain m) s) (fun s1 => cycles m fuel n' (reset s1))
  end.

End Semantics.

Definition init : state := mkstate [] (fun _ => VN 0) [] [] (fun _ => None) 0.
