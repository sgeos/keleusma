(** * Statements.v -- the pinned statements, fully qualified.

    Each pin is a definition whose stated type must match the type of one
    item of [Claims.v] up to conversion, so a statement weakened by an added
    premise or a changed conclusion fails to compile.  Each stated type is the
    type Rocq printed for the item with fully qualified names on the date of
    regeneration.  This file imports no module of the project, so no
    definition elsewhere can shadow a name it uses.  An audit of 2026-11-18
    showed by execution that pins written with bare names under [Import] could
    be bypassed by redefining such a name in an imported module.  [Claims.v]
    holds the statements as first written, with their proofs, including the
    proved equations that fix definitions.  The script requires the number of
    pins below. *)

From RegionCore Require Core Shape Sim Reuse Controls.
From Termination Require Termination Controls.
From Termination Require Claims.

Definition pin_shape_calls :
  forall (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)) (b : RegionCore.Core.block) (reg : Corelib.Init.Datatypes.bool) (S S' : Corelib.Init.Datatypes.list RegionCore.Shape.tag), RegionCore.Shape.shape_b ft reg S b = Corelib.Init.Datatypes.Some S' -> Termination.Termination.calls_b (Corelib.Init.Datatypes.length ft) b
  := Termination.Claims.pin_shape_calls.

Definition pin_accepted_terminates :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall (P : RegionCore.Core.plan) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (n : Corelib.Init.Datatypes.nat) (s : RegionCore.Core.state), exists s' : RegionCore.Core.state, RegionCore.Core.cycles P host m (Termination.Termination.bound m) n s = Corelib.Init.Datatypes.Some s'
  := Termination.Claims.pin_accepted_terminates.

Definition pin_accepted_runs_agree :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (n : Corelib.Init.Datatypes.nat), exists sp sb : RegionCore.Core.state, RegionCore.Core.cycles P host m (Termination.Termination.bound m) n RegionCore.Core.init = Corelib.Init.Datatypes.Some sp /\ RegionCore.Core.cycles RegionCore.Core.baseline host m (Termination.Termination.bound m) n RegionCore.Core.init = Corelib.Init.Datatypes.Some sb /\ RegionCore.Core.log sp = RegionCore.Core.log sb
  := Termination.Claims.pin_accepted_runs_agree.

Definition pin_loop_ok_bound_is_needed :
  (Termination.Controls.finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok (Termination.Termination.bound RegionCore.Controls.loop_ok), Termination.Controls.finishes RegionCore.Controls.P0 RegionCore.Controls.loop_ok 1) = (Corelib.Init.Datatypes.true, Corelib.Init.Datatypes.false)
  := Termination.Claims.pin_loop_ok_bound_is_needed.

Definition pin_self_call_rejected :
  RegionCore.Shape.region_check {| RegionCore.Core.mfuncs := RegionCore.Controls.blk (RegionCore.Core.Call 0 :: Corelib.Init.Datatypes.nil) :: Corelib.Init.Datatypes.nil; RegionCore.Core.mmain := RegionCore.Core.BNil |} = Corelib.Init.Datatypes.None
  := Termination.Claims.pin_self_call_rejected.

Definition pin_bound_pinned :
  forall m : RegionCore.Core.module, Termination.Termination.bound m = Termination.Termination.fuel_b (Termination.Termination.ftab Corelib.Init.Datatypes.nil (RegionCore.Core.mfuncs m)) (RegionCore.Core.mmain m)
  := Termination.Claims.bound_pinned.

Definition pin_calls_call_pinned :
  forall K f : Corelib.Init.Datatypes.nat, Termination.Termination.calls_i K (RegionCore.Core.Call f) = (f < K)
  := Termination.Claims.calls_call_pinned.
