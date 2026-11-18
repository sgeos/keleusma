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

From RegionReuse Require Trace.
From RegionCore Require Core Shape Sim Reuse Controls.
From Instance Require Encode Causal Memory Machine Safety Instance Witnesses.
From Instance Require Claims.

Definition pin_instance_sound :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), RegionReuse.Trace.sound Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 (Instance.Machine.next m host fuel n) P
  := Instance.Claims.pin_instance_sound.

Definition pin_checked_slot_safe :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (A : Instance.Encode.oracle) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n k : Corelib.Init.Datatypes.nat), RegionReuse.Trace.slot_safe Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat P k (Instance.Encode.otr (Instance.Encode.orun_all A host m fuel n))
  := Instance.Claims.pin_checked_slot_safe.

Definition pin_gen_is_run :
  forall (Q : RegionReuse.Trace.plan Corelib.Init.Datatypes.nat) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (m : RegionCore.Core.module) (fuel n N : Corelib.Init.Datatypes.nat), RegionReuse.Trace.gen Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 (Instance.Machine.next m host fuel n) Q N = Corelib.Lists.ListDef.firstn N (Instance.Machine.run_events Q host m fuel n)
  := Instance.Claims.pin_gen_is_run.

Definition pin_core_is_oracle :
  forall (Q : RegionCore.Core.plan) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (m : RegionCore.Core.module) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles Q host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some s => Instance.Encode.ohalt (Instance.Encode.orun_all (Instance.Memory.ans_plan Q) host m fuel n) = Corelib.Init.Datatypes.false /\ Instance.Encode.ost (Instance.Encode.orun_all (Instance.Memory.ans_plan Q) host m fuel n) = Instance.Memory.strip s /\ RegionCore.Core.log s = Instance.Memory.to_log (RegionReuse.Trace.obs_of Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 Q (Instance.Encode.otr (Instance.Encode.orun_all (Instance.Memory.ans_plan Q) host m fuel n))) | Corelib.Init.Datatypes.None => Instance.Encode.ohalt (Instance.Encode.orun_all (Instance.Memory.ans_plan Q) host m fuel n) = Corelib.Init.Datatypes.true end
  := Instance.Claims.pin_core_is_oracle.

Definition pin_core_log_is_B1_obs :
  forall (Q : RegionCore.Core.plan) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (m : RegionCore.Core.module) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles Q host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some s => exists N0 : Corelib.Init.Datatypes.nat, forall N : Corelib.Init.Datatypes.nat, N0 <= N -> RegionCore.Core.log s = Instance.Memory.to_log (RegionReuse.Trace.obs_of Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 Q (RegionReuse.Trace.gen Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 (Instance.Machine.next m host fuel n) Q N)) | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end
  := Instance.Claims.pin_core_log_is_B1_obs.

Definition pin_replay :
  forall (Q : RegionReuse.Trace.plan Corelib.Init.Datatypes.nat) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (m : RegionCore.Core.module) (fuel n : Corelib.Init.Datatypes.nat), Instance.Encode.orun_all (Instance.Machine.ans_of (RegionReuse.Trace.obs_of Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 Q (Instance.Machine.run_events Q host m fuel n))) host m fuel n = Instance.Encode.orun_all (Instance.Memory.ans_plan Q) host m fuel n
  := Instance.Claims.pin_replay.

Definition pin_reuse_sound_via_B1 :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Instance.Claims.pin_reuse_sound_via_B1.

Definition pin_same_as_B2 :
  RegionCore.Reuse.reuse_sound = Instance.Instance.reuse_sound_via_B1 -> Corelib.Init.Logic.True
  := Instance.Claims.pin_same_as_B2.

Definition pin_loop_bad_unsound_in_B1 :
  RegionCore.Shape.region_check RegionCore.Controls.loop_bad = Corelib.Init.Datatypes.None /\ RegionCore.Shape.injective RegionCore.Controls.P0 /\ ~ RegionReuse.Trace.sound Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 (Instance.Machine.next RegionCore.Controls.loop_bad RegionCore.Controls.host_len 100 2) RegionCore.Controls.P0
  := Instance.Claims.pin_loop_bad_unsound_in_B1.

Definition pin_shared_slot_unsound_in_B1 :
  RegionCore.Shape.region_check RegionCore.Controls.two_sites = Corelib.Init.Datatypes.Some Corelib.Init.Datatypes.nil /\ ~ RegionReuse.Trace.sound Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.nat Corelib.Init.Datatypes.unit Corelib.Init.Datatypes.nat Instance.Encode.sel0 (Instance.Machine.next RegionCore.Controls.two_sites RegionCore.Controls.host_len 100 2) RegionCore.Controls.P01
  := Instance.Claims.pin_shared_slot_unsound_in_B1.

Definition pin_sel0_pinned :
  forall (v : Corelib.Init.Datatypes.unit) (b : Corelib.Init.Datatypes.nat), Instance.Encode.sel0 v b = b
  := Instance.Claims.sel0_pinned.

Definition pin_baseline_pinned :
  forall s : Corelib.Init.Datatypes.nat, RegionReuse.Trace.baseline Corelib.Init.Datatypes.nat s = Corelib.Init.Datatypes.None /\ RegionCore.Core.baseline s = Corelib.Init.Datatypes.None
  := Instance.Claims.baseline_pinned.
