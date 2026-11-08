(** * SharedShape.v -- the confinement checker made aware of the plan.

    Increment B2's checker, [RegionCore.Shape], with one rule changed.  At
    [New s] the original rejects a live handle of [s].  This one rejects a live
    handle of [s] or of any other site the plan puts in the same slot, so a
    plan may let several sites share one slot whenever their handles are never
    live together.  Sites in different arms of one conditional are the case of
    Part I's Theorem A2 and Corollary A3, and a site whose handles die before
    another site reuses its slot is covered as well.  Every other rule is the
    original's, and [SharedShape] coincides with [Shape] for a plan that gives
    distinct sites distinct slots, as [Shared.v] proves. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
Import ListNotations.

(** A live entry conflicts with an allocation at [s] when it is a handle of
    [s], or of another site sharing [s]'s slot under [P]. *)
Definition conflict (P : plan) (s : nat) (t : tag) : bool :=
  match t with
  | TS => false
  | TR s' =>
      Nat.eqb s' s ||
      match P s, P s' with Some k, Some k' => Nat.eqb k k' | _, _ => false end
  end.

Fixpoint shapeP_b (P : plan) (ft : list (list tag)) (regions : bool) (S : list tag) (b : block)
  : option (list tag) :=
  match b with
  | BNil => Some S
  | BCons i r =>
      match shapeP_i P ft regions S i with
      | Some S1 => shapeP_b P ft regions S1 r
      | None => None
      end
  end
with shapeP_i (P : plan) (ft : list (list tag)) (regions : bool) (S : list tag) (i : instr)
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
          match shapeP_b P ft regions r t, shapeP_b P ft regions r e with
          | Some T, Some E => if list_eq_dec tag_eq_dec T E then Some T else None
          | _, _ => None
          end
      | _ => None
      end
  | LoopN _ body =>
      match shapeP_b P ft regions S body with
      | Some S' => if list_eq_dec tag_eq_dec S' S then Some S else None
      | None => None
      end
  | Call f => match nth_error ft f with Some R => Some (R ++ S) | None => None end
  | Yield => match tpop S with (TS, r) => Some (TS :: r) | _ => None end
  | New s =>
      if regions then
        match tpop S with
        | (TS, r) => if existsb (conflict P s) r then None else Some (TR s :: r)
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

(** Functions are checked as by [Shape], since they may not allocate. *)
Definition region_checkP (P : plan) (m : module) : option (list (list tag)) :=
  match check_funcs [] (mfuncs m) with
  | Some ft => match shapeP_b P ft true [] (mmain m) with Some _ => Some ft | None => None end
  | None => None
  end.
