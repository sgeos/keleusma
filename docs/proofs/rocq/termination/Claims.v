(** * Claims.v -- formerly Statements.v, the statements of the termination results, pinned.

    Each pin is a definition whose stated type must match the result's type
    up to conversion, so a statement weakened by an added premise or a
    changed conclusion fails to compile.  The definition of [bound] is fixed
    below by a proved equation.  The script requires the number of pins
    below. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape.
From RegionCore Require Controls.
From Termination Require Import Termination.
From Termination Require Controls.
Import ListNotations.

Definition pin_shape_calls : forall ft b reg S S',
  shape_b ft reg S b = Some S' -> calls_b (length ft) b
  := shape_calls.

Definition pin_accepted_terminates : forall m ft, region_check m = Some ft ->
  forall P host n s, exists s', cycles P host m (bound m) n s = Some s'
  := accepted_terminates.

Definition pin_accepted_runs_agree : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host n,
  exists sp sb, cycles P host m (bound m) n init = Some sp /\
                cycles baseline host m (bound m) n init = Some sb /\ log sp = log sb
  := accepted_runs_agree.

Definition pin_loop_ok_bound_is_needed :
  (Termination.Controls.finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok
     (bound RegionCore.Controls.loop_ok),
   Termination.Controls.finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok 1) = (true, false)
  := Termination.Controls.loop_ok_bound_is_needed.

Definition pin_self_call_rejected :
  region_check (mkmodule [RegionCore.Controls.blk [Call 0]] BNil) = None
  := Termination.Controls.self_call_rejected.

(** The definition the headline's fuel rests on, pinned by a proved
    equation, and the calls predicate by its one nontrivial case. *)
Example bound_pinned : forall m, bound m = fuel_b (ftab [] (mfuncs m)) (mmain m).
Proof. reflexivity. Qed.

Example calls_call_pinned : forall K f, calls_i K (Call f) = (f < K).
Proof. reflexivity. Qed.
