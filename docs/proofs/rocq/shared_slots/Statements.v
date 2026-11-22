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
From SharedSlots Require SharedShape SharedSim Shared ArmShape ArmSim Arm Controls.
From SharedSlots Require Claims.

Definition pin_reuse_sound_shared :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), SharedSlots.SharedShape.region_checkP P m = Corelib.Init.Datatypes.Some ft -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := SharedSlots.Claims.pin_reuse_sound_shared.

Definition pin_shared_generalises :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module), RegionCore.Shape.injective P -> SharedSlots.SharedShape.region_checkP P m = RegionCore.Shape.region_check m
  := SharedSlots.Claims.pin_shared_generalises.

Definition pin_arm_group_covered :
  ~ RegionCore.Shape.injective SharedSlots.Controls.Pshare /\ SharedSlots.SharedShape.region_checkP SharedSlots.Controls.Pshare SharedSlots.Controls.arms = Corelib.Init.Datatypes.Some Corelib.Init.Datatypes.nil /\ RegionCore.Controls.log_of SharedSlots.Controls.Pshare SharedSlots.Controls.arms = RegionCore.Controls.log_of RegionCore.Core.baseline SharedSlots.Controls.arms
  := SharedSlots.Claims.pin_arm_group_covered.

Definition pin_co_live_rejected :
  SharedSlots.SharedShape.region_checkP SharedSlots.Controls.Pshare RegionCore.Controls.two_sites = Corelib.Init.Datatypes.None /\ SharedSlots.ArmShape.region_checkA SharedSlots.Controls.Pshare RegionCore.Controls.two_sites = Corelib.Init.Datatypes.None /\ RegionCore.Shape.region_check RegionCore.Controls.two_sites = Corelib.Init.Datatypes.Some Corelib.Init.Datatypes.nil /\ RegionCore.Controls.log_of SharedSlots.Controls.Pshare RegionCore.Controls.two_sites <> RegionCore.Controls.log_of RegionCore.Core.baseline RegionCore.Controls.two_sites
  := SharedSlots.Claims.pin_co_live_rejected.

Definition pin_joined_live :
  SharedSlots.ArmShape.region_checkA SharedSlots.Controls.P012 SharedSlots.Controls.joined_live = Corelib.Init.Datatypes.None /\ RegionCore.Controls.log_of SharedSlots.Controls.P012 SharedSlots.Controls.joined_live <> RegionCore.Controls.log_of RegionCore.Core.baseline SharedSlots.Controls.joined_live
  := SharedSlots.Claims.pin_joined_live.

Definition pin_reuse_sound_arm :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), SharedSlots.ArmShape.region_checkA P m = Corelib.Init.Datatypes.Some ft -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := SharedSlots.Claims.pin_reuse_sound_arm.

Definition pin_arm_contains_b2 :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.injective P -> RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> SharedSlots.ArmShape.region_checkA P m = Corelib.Init.Datatypes.Some ft
  := SharedSlots.Claims.pin_arm_contains_b2.

Definition pin_choose_covered :
  ~ RegionCore.Shape.injective SharedSlots.Controls.Pshare /\ SharedSlots.SharedShape.region_checkP SharedSlots.Controls.Pshare SharedSlots.Controls.choose = Corelib.Init.Datatypes.None /\ SharedSlots.ArmShape.region_checkA SharedSlots.Controls.Pshare SharedSlots.Controls.choose = Corelib.Init.Datatypes.Some Corelib.Init.Datatypes.nil /\ RegionCore.Controls.log_of SharedSlots.Controls.Pshare SharedSlots.Controls.choose = RegionCore.Controls.log_of RegionCore.Core.baseline SharedSlots.Controls.choose
  := SharedSlots.Claims.pin_choose_covered.
