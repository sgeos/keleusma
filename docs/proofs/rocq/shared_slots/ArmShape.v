(** * ArmShape.v -- the plan-aware checker with arm joins.

    [SharedShape] requires both arms of a conditional to leave the same
    abstract stack, so a handle produced in one arm and a handle of another
    site produced in the other can never both survive the conditional.  That
    excludes the commonest arm group, choosing between two composites and
    then using the one chosen.  This checker records, for a handle, the set of
    sites it may belong to, and joins the arms of a conditional position by
    position, so after [If] an entry may be a handle of either arm's site.  At
    [New s] an entry conflicts when any site it may belong to is [s] or shares
    [s]'s slot under the plan.  Every other rule is [SharedShape]'s.

    Functions are still checked by increment B2's [Shape.check_funcs], since
    they may not allocate, and their scalar results are embedded. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
From SharedSlots Require Import SharedShape.
Import ListNotations.

(** An abstract entry is a scalar, or a handle of one of the listed sites. *)
Inductive atag : Type := AS | AR (l : list nat).

Definition atag_eq_dec : forall a b : atag, {a = b} + {a <> b}.
Proof. decide equality. apply (list_eq_dec Nat.eq_dec). Defined.

Definition apop (S : list atag) : atag * list atag :=
  match S with t :: r => (t, r) | [] => (AS, []) end.

Definition embed (t : tag) : atag := match t with TS => AS | TR s => AR [s] end.

(** The join of two arms' stacks, position by position.  Scalars join with
    scalars, and handles with handles, by uniting their site lists.  A scalar
    meeting a handle, or stacks of different heights, cannot be joined. *)
Fixpoint ajoin (A B : list atag) : option (list atag) :=
  match A, B with
  | [], [] => Some []
  | AS :: a, AS :: b => option_map (cons AS) (ajoin a b)
  | AR l1 :: a, AR l2 :: b =>
      option_map (cons (AR (if list_eq_dec Nat.eq_dec l1 l2 then l1 else l1 ++ l2))) (ajoin a b)
  | _, _ => None
  end.

Definition aconflict (P : plan) (s : nat) (t : atag) : bool :=
  match t with AS => false | AR l => existsb (fun s' => conflict P s (TR s')) l end.

Fixpoint shapeA_b (P : plan) (ft : list (list tag)) (regions : bool) (S : list atag) (b : block)
  : option (list atag) :=
  match b with
  | BNil => Some S
  | BCons i r =>
      match shapeA_i P ft regions S i with
      | Some S1 => shapeA_b P ft regions S1 r
      | None => None
      end
  end
with shapeA_i (P : plan) (ft : list (list tag)) (regions : bool) (S : list atag) (i : instr)
  : option (list atag) :=
  match i with
  | Const _ | Load _ => Some (AS :: S)
  | Store _ => match apop S with (AS, r) => Some r | _ => None end
  | Add | Sub | Lt =>
      match apop S with
      | (AS, r) => match apop r with (AS, r2) => Some (AS :: r2) | _ => None end
      | _ => None
      end
  | If t e =>
      match apop S with
      | (AS, r) =>
          match shapeA_b P ft regions r t, shapeA_b P ft regions r e with
          | Some T, Some E => ajoin T E
          | _, _ => None
          end
      | _ => None
      end
  | LoopN _ body =>
      match shapeA_b P ft regions S body with
      | Some S' => if list_eq_dec atag_eq_dec S' S then Some S else None
      | None => None
      end
  | Call f => match nth_error ft f with Some R => Some (map embed R ++ S) | None => None end
  | Yield => match apop S with (AS, r) => Some (AS :: r) | _ => None end
  | New s =>
      if regions then
        match apop S with
        | (AS, r) => if existsb (aconflict P s) r then None else Some (AR [s] :: r)
        | _ => None
        end
      else None
  | Get =>
      if regions then
        match apop S with (AR _, r) => Some (AS :: r) | _ => None end
      else None
  | Dup => let (t, r) := apop S in Some (t :: t :: r)
  | Drop => Some (snd (apop S))
  end.

Definition region_checkA (P : plan) (m : module) : option (list (list tag)) :=
  match check_funcs [] (mfuncs m) with
  | Some ft => match shapeA_b P ft true [] (mmain m) with Some _ => Some ft | None => None end
  | None => None
  end.
