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
From RegionCore Require Claims.

Definition pin_reuse_sound :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := RegionCore.Claims.pin_reuse_sound.

Definition pin_loop_ok_sound :
  forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := RegionCore.Claims.pin_loop_ok_sound.

Definition pin_loop_bad_diverges :
  RegionCore.Shape.region_check RegionCore.Controls.loop_bad = Corelib.Init.Datatypes.None /\ RegionCore.Controls.log_of RegionCore.Controls.P0 RegionCore.Controls.loop_bad <> RegionCore.Controls.log_of RegionCore.Core.baseline RegionCore.Controls.loop_bad
  := RegionCore.Claims.pin_loop_bad_diverges.

Definition pin_sep_only_diverges :
  RegionCore.Shape.shape_b Corelib.Init.Datatypes.nil Corelib.Init.Datatypes.true Corelib.Init.Datatypes.nil (RegionCore.Controls.blk (RegionCore.Core.Const 1 :: RegionCore.Core.New 0 :: RegionCore.Core.Dup :: RegionCore.Core.Get :: RegionCore.Core.Yield :: RegionCore.Core.Store 1 :: RegionCore.Core.Const 2 :: Corelib.Init.Datatypes.nil)) = Corelib.Init.Datatypes.Some (RegionCore.Shape.TS :: RegionCore.Shape.TR 0 :: Corelib.Init.Datatypes.nil)%list /\ RegionCore.Shape.region_check RegionCore.Controls.sep_only = Corelib.Init.Datatypes.None /\ RegionCore.Controls.log_of RegionCore.Controls.P0 RegionCore.Controls.sep_only <> RegionCore.Controls.log_of RegionCore.Core.baseline RegionCore.Controls.sep_only
  := RegionCore.Claims.pin_sep_only_diverges.

Definition pin_shared_slot_diverges :
  RegionCore.Shape.region_check RegionCore.Controls.two_sites = Corelib.Init.Datatypes.Some Corelib.Init.Datatypes.nil /\ RegionCore.Controls.log_of RegionCore.Controls.P01 RegionCore.Controls.two_sites <> RegionCore.Controls.log_of RegionCore.Core.baseline RegionCore.Controls.two_sites /\ ~ RegionCore.Shape.injective RegionCore.Controls.P01
  := RegionCore.Claims.pin_shared_slot_diverges.

Definition pin_injective_pinned :
  forall P : RegionCore.Core.plan, RegionCore.Shape.injective P = (forall s1 s2 k : Corelib.Init.Datatypes.nat, P s1 = Corelib.Init.Datatypes.Some k -> P s2 = Corelib.Init.Datatypes.Some k -> s1 = s2)
  := RegionCore.Claims.injective_pinned.

Definition pin_baseline_pinned :
  forall s : Corelib.Init.Datatypes.nat, RegionCore.Core.baseline s = Corelib.Init.Datatypes.None
  := RegionCore.Claims.baseline_pinned.
