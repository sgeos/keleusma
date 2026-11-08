(** * Check.v -- the bound checker for the core.

    The checker computes, for every block, an abstract bound of four parts.

    - [an] bounds the single segment of a run that never yields, and is
      [None] when every run of the block yields.
    - [ay] bounds a run that yields, by its first segment, the largest of its
      segments between two yields, and its last segment, and is [None] when no
      run of the block yields.
    - [am] bounds the arena the block allocates.
    - [ad] is the fuel the interpreter needs to run the block to completion.

    A module is accepted when every function's body has a bound computed only
    from the bounds of earlier functions, which rejects every recursive or
    forward call, when every [fn] function never yields, and when the main
    body yields on every path, which is the productivity rule.  The checker
    is a function, so it runs, and [Controls.v] runs it. *)

From Stdlib Require Import List Arith Lia.
From CoreBounds Require Import Core.
Import ListNotations.

(** ** Summaries of segment lists. *)
Inductive summ : Type := NoY (c : nat) | Y (f m l : nat).

Definition sseq (p q : summ) : summ :=
  match p, q with
  | NoY a, NoY b => NoY (a + b)
  | NoY a, Y f m l => Y (a + f) m l
  | Y f m l, NoY b => Y f m (l + b)
  | Y f1 m1 l1, Y f2 m2 l2 => Y f1 (Nat.max m1 (Nat.max (l1 + f2) m2)) l2
  end.

Fixpoint maxl (l : list nat) : nat :=
  match l with [] => 0 | x :: r => Nat.max x (maxl r) end.

(** A run with one segment never yields.  Otherwise the summary keeps the
    first segment, the largest middle segment, and the last segment. *)
Definition summary (a : segs) : summ :=
  match a with
  | [] => NoY 0
  | [c] => NoY c
  | f :: rest => Y f (maxl (removelast rest)) (last rest 0)
  end.

(** ** Abstract bounds. *)
Record abs : Type := mkabs {
  an : option nat;
  ay : option (nat * nat * nat);
  am : nat;
  ad : nat
}.

Definition omax (a b : option nat) : option nat :=
  match a, b with
  | Some x, Some y => Some (Nat.max x y)
  | Some x, None => Some x
  | None, y => y
  end.

Definition max3 (a b : nat * nat * nat) : nat * nat * nat :=
  let '(f1, m1, l1) := a in let '(f2, m2, l2) := b in (Nat.max f1 f2, Nat.max m1 m2, Nat.max l1 l2).

Definition omax3 (a b : option (nat * nat * nat)) : option (nat * nat * nat) :=
  match a, b with
  | Some x, Some y => Some (max3 x y)
  | Some x, None => Some x
  | None, y => y
  end.

Definition seq_n (a b : option nat) : option nat :=
  match a, b with Some x, Some y => Some (x + y) | _, _ => None end.

Definition seq_y (an1 : option nat) (ay1 : option (nat * nat * nat))
                 (an2 : option nat) (ay2 : option (nat * nat * nat)) : option (nat * nat * nat) :=
  omax3 (match an1, ay2 with Some a, Some (f, m, l) => Some (a + f, m, l) | _, _ => None end)
    (omax3 (match ay1, an2 with Some (f, m, l), Some b => Some (f, m, l + b) | _, _ => None end)
           (match ay1, ay2 with
            | Some (f1, m1, l1), Some (f2, m2, l2) => Some (f1, Nat.max m1 (Nat.max (l1 + f2) m2), l2)
            | _, _ => None
            end)).

(** Sequencing and joining, with the fuel of the result given separately. *)
Definition aseq (A B : abs) (d : nat) : abs :=
  mkabs (seq_n (an A) (an B)) (seq_y (an A) (ay A) (an B) (ay B)) (am A + am B) d.

Definition ajoin (A B : abs) (d : nat) : abs :=
  mkabs (omax (an A) (an B)) (omax3 (ay A) (ay B)) (Nat.max (am A) (am B)) d.

(** One instruction that allocates [k] words, and an empty run. *)
Definition one (k : nat) : abs := mkabs (Some 1) None k 1.
Definition zero : abs := mkabs (Some 0) None 0 1.

Fixpoint aloop (k : nat) (A : abs) : abs :=
  match k with
  | 0 => zero
  | S k' =>
      let R := aloop k' A in
      aseq A (aseq (one 0) R 0) (S (Nat.max (ad A) (ad R)))
  end.

Fixpoint abs_b (tab : list abs) (b : block) : option abs :=
  match b with
  | BNil => Some zero
  | BCons i r =>
      match abs_i tab i, abs_b tab r with
      | Some A, Some R => Some (aseq A R (S (Nat.max (ad A) (ad R))))
      | _, _ => None
      end
  end
with abs_i (tab : list abs) (i : instr) : option abs :=
  match i with
  | Const _ | Load _ | Store _ | Add | Sub | Lt => Some (one 0)
  | Alloc k => Some (one k)
  | Yield => Some (mkabs None (Some (1, 0, 0)) 0 1)
  | If t e =>
      match abs_b tab t, abs_b tab e with
      | Some T, Some E => Some (aseq (one 0) (ajoin T E 0) (S (Nat.max (ad T) (ad E))))
      | _, _ => None
      end
  | LoopN k body =>
      match abs_b tab body with
      | Some Bd => Some (aseq (one 0) (aloop k Bd) (S (ad (aloop k Bd))))
      | None => None
      end
  | Call f =>
      match nth_error tab f with
      | Some C => Some (aseq (one 0) C (S (ad C)))
      | None => None
      end
  end.

(** ** The checker. *)
Definition kind_ok (k : kind) (A : abs) : bool :=
  match k, ay A with KFn, Some _ => false | _, _ => true end.

Fixpoint check_funcs (tab : list abs) (gs : list func) : option (list abs) :=
  match gs with
  | [] => Some tab
  | g :: gs' =>
      match abs_b tab (fbody g) with
      | Some A => if kind_ok (fkind g) A then check_funcs (tab ++ [A]) gs' else None
      | None => None
      end
  end.

(** On acceptance, the function bounds and the main body's bound, whose
    every run yields. *)
Definition check (m : module) : option (list abs * abs) :=
  match check_funcs [] (mfuncs m) with
  | Some tab =>
      match abs_b tab (mmain m) with
      | Some A => match an A, ay A with None, Some _ => Some (tab, A) | _, _ => None end
      | None => None
      end
  | None => None
  end.

(** The published figures of an accepted module.  [wcet] bounds every segment
    of every stream, the junction through a reset included, which costs the
    last segment, the reset, and the next first segment.  [wcmu] bounds the
    arena of every cycle. *)
Definition wcet (A : abs) : nat :=
  match ay A with Some (f, m, l) => Nat.max f (Nat.max m (l + 1 + f)) | None => 0 end.

Definition wcmu (A : abs) : nat := am A.
