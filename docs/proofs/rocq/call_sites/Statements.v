(** * Statements.v -- the pinned statements, fully qualified.

    Each pin is a definition whose stated type must match the type of one
    item of [Claims.v] up to conversion, so a statement weakened by an added
    premise or a changed conclusion fails to compile.  Each stated type is the
    type Rocq printed for the item with fully qualified names on the date of
    regeneration, by tools/rocq-pins/genpins.py.  This file imports no module
    of the project, so no definition elsewhere can shadow a name it uses.  An
    audit of 2026-11-18 showed by execution that pins written with bare names
    under [Import] could be bypassed by redefining such a name in an imported
    module.  [Claims.v] holds the statements as first written, with their
    proofs, including the proved equations that fix definitions. *)

From RegionCore Require Core Shape Sim Reuse Controls.
From CallSites Require ShapeC Frame SimC CallSites Controls.
From CallSites Require Claims.

Definition pin_reuse_sound_calls :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list CallSites.ShapeC.fentry), CallSites.ShapeC.region_checkC m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.log sp = RegionCore.Core.log sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := CallSites.Claims.pin_reuse_sound_calls.

Definition pin_frame :
  forall (Q : RegionCore.Core.plan) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fs : Corelib.Init.Datatypes.list RegionCore.Core.block) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag * Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat)), (forall (f : Corelib.Init.Datatypes.nat) (g : RegionCore.Core.block), Stdlib.Lists.List.nth_error fs f = Corelib.Init.Datatypes.Some g -> exists (R : Corelib.Init.Datatypes.list RegionCore.Shape.tag) (A : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat), Stdlib.Lists.List.nth_error ft f = Corelib.Init.Datatypes.Some (R, A) /\ Stdlib.Lists.List.incl (CallSites.ShapeC.sites_b ft g) A) -> forall n : Corelib.Init.Datatypes.nat, (forall (b : RegionCore.Core.block) (s s' : RegionCore.Core.state), RegionCore.Core.run_b Q host fs n b s = Corelib.Init.Datatypes.Some s' -> CallSites.Frame.framed Q (CallSites.ShapeC.sites_b ft b) s s') /\ (forall (i : RegionCore.Core.instr) (s s' : RegionCore.Core.state), RegionCore.Core.run_i Q host fs n i s = Corelib.Init.Datatypes.Some s' -> CallSites.Frame.framed Q (CallSites.ShapeC.sites_i ft i) s s') /\ (forall (k : Corelib.Init.Datatypes.nat) (b : RegionCore.Core.block) (s s' : RegionCore.Core.state), RegionCore.Core.run_loop Q host fs n k b s = Corelib.Init.Datatypes.Some s' -> CallSites.Frame.framed Q (CallSites.ShapeC.sites_b ft b) s s')
  := CallSites.Claims.pin_frame.

Definition pin_twice_rejected_by_B2 :
  RegionCore.Shape.region_check CallSites.Controls.twice = Corelib.Init.Datatypes.None
  := CallSites.Claims.pin_twice_rejected_by_B2.

Definition pin_twice_accepted :
  CallSites.ShapeC.region_checkC CallSites.Controls.twice = Corelib.Init.Datatypes.Some ((RegionCore.Shape.TS :: Corelib.Init.Datatypes.nil, 1 :: Corelib.Init.Datatypes.nil) :: Corelib.Init.Datatypes.nil)%list
  := CallSites.Claims.pin_twice_accepted.

Definition pin_twice_agrees :
  CallSites.Controls.log_of CallSites.Controls.P1 CallSites.Controls.twice = CallSites.Controls.log_of RegionCore.Core.baseline CallSites.Controls.twice /\ CallSites.Controls.log_of CallSites.Controls.P1 CallSites.Controls.twice = (RegionCore.Core.ReadE (Corelib.Init.Datatypes.Some 7) :: RegionCore.Core.OutE 7 :: RegionCore.Core.ReadE (Corelib.Init.Datatypes.Some 7) :: RegionCore.Core.OutE 7 :: RegionCore.Core.ReadE (Corelib.Init.Datatypes.Some 7) :: RegionCore.Core.OutE 7 :: RegionCore.Core.ReadE (Corelib.Init.Datatypes.Some 7) :: RegionCore.Core.OutE 7 :: Corelib.Init.Datatypes.nil)%list
  := CallSites.Claims.pin_twice_agrees.

Definition pin_live_across_call_rejected :
  CallSites.ShapeC.region_checkC CallSites.Controls.live_across_call = Corelib.Init.Datatypes.None
  := CallSites.Claims.pin_live_across_call_rejected.

Definition pin_live_across_call_diverges :
  CallSites.Controls.log_of CallSites.Controls.P1 CallSites.Controls.live_across_call <> CallSites.Controls.log_of RegionCore.Core.baseline CallSites.Controls.live_across_call
  := CallSites.Claims.pin_live_across_call_diverges.

Definition pin_live_in_pinned :
  forall (A : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat) (s : Corelib.Init.Datatypes.nat), CallSites.ShapeC.live_in A (RegionCore.Shape.TR s) = Stdlib.Lists.List.existsb (Corelib.Init.Nat.eqb s) A
  := CallSites.Claims.live_in_pinned.

Definition pin_call_rule_pinned :
  forall (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag * Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat)) (reg : Corelib.Init.Datatypes.bool) (S : Corelib.Init.Datatypes.list RegionCore.Shape.tag) (f : Corelib.Init.Datatypes.nat) (R : Corelib.Init.Datatypes.list RegionCore.Shape.tag) (A : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat), Stdlib.Lists.List.nth_error ft f = Corelib.Init.Datatypes.Some (R, A) -> CallSites.ShapeC.shapeC_i ft reg S (RegionCore.Core.Call f) = (if Stdlib.Lists.List.existsb (CallSites.ShapeC.live_in A) S then Corelib.Init.Datatypes.None else Corelib.Init.Datatypes.Some (R ++ S)%list)
  := CallSites.Claims.call_rule_pinned.

Definition pin_framed_pinned :
  forall (Q : RegionCore.Core.plan) (A : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat) (s s' : RegionCore.Core.state), CallSites.Frame.framed Q A s s' = ((exists u : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat), RegionCore.Core.regs s' = (RegionCore.Core.regs s ++ u)%list) /\ RegionCore.Core.epoch s' = RegionCore.Core.epoch s /\ (forall c : RegionCore.Core.cell, (forall s0 r : Corelib.Init.Datatypes.nat, Stdlib.Lists.List.In s0 A -> Corelib.Init.Datatypes.length (RegionCore.Core.regs s) <= r -> c <> RegionCore.Core.cell_of Q s0 r) -> RegionCore.Core.mem s' c = RegionCore.Core.mem s c))
  := CallSites.Claims.framed_pinned.

Definition pin_P1_pinned :
  forall s : Corelib.Init.Datatypes.nat, CallSites.Controls.P1 s = match s with | 1 => Corelib.Init.Datatypes.Some 0 | _ => Corelib.Init.Datatypes.None end
  := CallSites.Claims.P1_pinned.

Definition pin_log_of_pinned :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module), CallSites.Controls.log_of P m = match RegionCore.Core.cycles P RegionCore.Controls.host_len m 100 2 RegionCore.Core.init with | Corelib.Init.Datatypes.Some s => RegionCore.Core.log s | Corelib.Init.Datatypes.None => Corelib.Init.Datatypes.nil end
  := CallSites.Claims.log_of_pinned.
