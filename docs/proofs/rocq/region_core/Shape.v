(** * Shape.v -- the confinement checker.

    The checker interprets each block over an abstract operand stack whose
    entries are tags, [TS] for a scalar and [TR s] for a handle to a region
    of site [s].  Its rules are the region-reuse proof's hypotheses, decided
    from the program text rather than supplied as a certificate.

    - A handle may be dereferenced by [Get], duplicated by [Dup] and
      discarded by [Drop], and nothing else.  It may not be stored to a local,
      yielded to the host, branched on, or computed with.
      These are the executions Part I classifies [Escapes], so every value is
      confined in Definition 6's sense.
    - A loop body must leave the abstract stack exactly as it found it, and
      both arms of a conditional must leave the same stack, which is M6's
      restoration of the entry stack.
    - [New s] is rejected whenever a handle of site [s] may still be on the
      stack.  This is the separation hypothesis of Theorem B1 together with
      the confinement that makes it suffice, checked at the allocation
      itself.
    - A function may not allocate or dereference a region, and must return
      only scalars, so no handle crosses a call.

    The abstract stack is exact.  Popping an empty abstract stack yields a
    scalar, as the semantics does, and conditionals and loops have one shape,
    so [Sim.v] shows the concrete stack always has the abstract stack's
    shape. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core.
Import ListNotations.

Inductive tag : Type := TS | TR (s : nat).

Definition tag_eq_dec : forall a b : tag, {a = b} + {a <> b}.
Proof. decide equality; apply Nat.eq_dec. Defined.

Definition tpop (S : list tag) : tag * list tag :=
  match S with t :: r => (t, r) | [] => (TS, []) end.

Definition is_ts (t : tag) : bool := match t with TS => true | TR _ => false end.

Fixpoint shape_b (ft : list (list tag)) (regions : bool) (S : list tag) (b : block)
  : option (list tag) :=
  match b with
  | BNil => Some S
  | BCons i r =>
      match shape_i ft regions S i with
      | Some S1 => shape_b ft regions S1 r
      | None => None
      end
  end
with shape_i (ft : list (list tag)) (regions : bool) (S : list tag) (i : instr)
  : option (list tag) :=
  match i with
  | Const _ | Load _ => Some (TS :: S)
  | Store _ => match tpop S with (TS, r) => Some r | _ => None end
  | Add | Sub | Lt =>
      match tpop S with
      | (TS, r) => match tpop r with (TS, r2) => Some (TS :: r2) | _ => None end
      | _ => None
      end
  | If t e =>
      match tpop S with
      | (TS, r) =>
          match shape_b ft regions r t, shape_b ft regions r e with
          | Some T, Some E => if list_eq_dec tag_eq_dec T E then Some T else None
          | _, _ => None
          end
      | _ => None
      end
  | LoopN _ body =>
      match shape_b ft regions S body with
      | Some S' => if list_eq_dec tag_eq_dec S' S then Some S else None
      | None => None
      end
  | Call f => match nth_error ft f with Some R => Some (R ++ S) | None => None end
  | Yield => match tpop S with (TS, r) => Some (TS :: r) | _ => None end
  | New s =>
      if regions then
        match tpop S with
        | (TS, r) => if in_dec tag_eq_dec (TR s) r then None else Some (TR s :: r)
        | _ => None
        end
      else None
  | Get =>
      if regions then
        match tpop S with (TR _, r) => Some (TS :: r) | _ => None end
      else None
  | Dup => let (t, r) := tpop S in Some (t :: t :: r)
  | Drop => Some (snd (tpop S))
  end.

(** Each function is checked from an empty stack, with regions forbidden,
    against the result shapes of earlier functions only, so a recursive or
    forward call is rejected, and its result shape must be all scalars. *)
Fixpoint check_funcs (ft : list (list tag)) (gs : list block) : option (list (list tag)) :=
  match gs with
  | [] => Some ft
  | g :: gs' =>
      match shape_b ft false [] g with
      | Some R => if forallb is_ts R then check_funcs (ft ++ [R]) gs' else None
      | None => None
      end
  end.

(** The main block is checked from an empty stack, the stack being empty at
    the start of every cycle. *)
Definition region_check (m : module) : option (list (list tag)) :=
  match check_funcs [] (mfuncs m) with
  | Some ft => match shape_b ft true [] (mmain m) with Some _ => Some ft | None => None end
  | None => None
  end.

(** A plan is well formed when distinct sites never share a slot. *)
Definition injective (P : plan) : Prop :=
  forall s1 s2 k, P s1 = Some k -> P s2 = Some k -> s1 = s2.
