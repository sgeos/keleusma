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
From Accounting Require Accounting Footprint Controls ArmFootprint ArmControls.
From Accounting Require Claims.

Definition pin_baseline_footprint :
  forall (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.footprint_at RegionCore.Core.baseline rs e = Corelib.Init.Datatypes.length (Accounting.Accounting.allocs_at rs e)
  := Accounting.Claims.pin_baseline_footprint.

Definition pin_footprint_le_baseline :
  forall (P : RegionCore.Core.plan) (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.footprint_at P rs e <= Accounting.Accounting.footprint_at RegionCore.Core.baseline rs e
  := Accounting.Claims.pin_footprint_le_baseline.

Definition pin_footprint_slot_bound :
  forall (P : RegionCore.Core.plan) (K : Corelib.Init.Datatypes.nat), (forall s k : Corelib.Init.Datatypes.nat, P s = Corelib.Init.Datatypes.Some k -> k < K) -> forall (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.footprint_at P rs e <= K + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at P rs e)
  := Accounting.Claims.pin_footprint_slot_bound.

Definition pin_mem_within :
  forall (P : RegionCore.Core.plan) (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (m : RegionCore.Core.module) (fuel n : Corelib.Init.Datatypes.nat) (s : RegionCore.Core.state), RegionCore.Core.cycles P host m fuel n RegionCore.Core.init = Corelib.Init.Datatypes.Some s -> Accounting.Footprint.within P s
  := Accounting.Claims.pin_mem_within.

Definition pin_same_allocations :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall P : RegionCore.Core.plan, RegionCore.Shape.injective P -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_same_allocations.

Definition pin_reuse_footprint_bounded :
  forall (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), RegionCore.Shape.region_check m = Corelib.Init.Datatypes.Some ft -> forall (P : RegionCore.Core.plan) (K : Corelib.Init.Datatypes.nat), RegionCore.Shape.injective P -> (forall s k : Corelib.Init.Datatypes.nat, P s = Corelib.Init.Datatypes.Some k -> k < K) -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb /\ Accounting.Footprint.within P sp /\ (forall e : Corelib.Init.Datatypes.nat, Accounting.Accounting.footprint_at P (RegionCore.Core.regs sp) e <= K + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at P (RegionCore.Core.regs sp) e) /\ Accounting.Accounting.footprint_at P (RegionCore.Core.regs sp) e <= Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e /\ Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e = Corelib.Init.Datatypes.length (Accounting.Accounting.allocs_at (RegionCore.Core.regs sb) e)) | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_reuse_footprint_bounded.

Definition pin_loop_ok_footprints :
  (Accounting.Controls.fp RegionCore.Controls.P0 0, Accounting.Controls.fp RegionCore.Controls.P0 1, Accounting.Controls.fp RegionCore.Core.baseline 0, Accounting.Controls.fp RegionCore.Core.baseline 1) = (1, 1, 3, 3)
  := Accounting.Claims.pin_loop_ok_footprints.

Definition pin_slots_alone_do_not_bound :
  ~ (forall e : Corelib.Init.Datatypes.nat, Accounting.Controls.fp RegionCore.Core.baseline e <= 0)
  := Accounting.Claims.pin_slots_alone_do_not_bound.

Definition pin_same_allocations_arm :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), SharedSlots.ArmShape.region_checkA P m = Corelib.Init.Datatypes.Some ft -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_same_allocations_arm.

Definition pin_reuse_footprint_bounded_arm :
  forall (P : RegionCore.Core.plan) (m : RegionCore.Core.module) (ft : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.list RegionCore.Shape.tag)), SharedSlots.ArmShape.region_checkA P m = Corelib.Init.Datatypes.Some ft -> forall K : Corelib.Init.Datatypes.nat, (forall s k : Corelib.Init.Datatypes.nat, P s = Corelib.Init.Datatypes.Some k -> k < K) -> forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles P host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb /\ Accounting.Footprint.within P sp /\ (forall e : Corelib.Init.Datatypes.nat, Accounting.Accounting.footprint_at P (RegionCore.Core.regs sp) e <= K + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at P (RegionCore.Core.regs sp) e) /\ Accounting.Accounting.footprint_at P (RegionCore.Core.regs sp) e <= Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e /\ Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e = Corelib.Init.Datatypes.length (Accounting.Accounting.allocs_at (RegionCore.Core.regs sb) e)) | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host m fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_reuse_footprint_bounded_arm.

Definition pin_sequential_shares_one_cell :
  match Accounting.ArmControls.seq_run SharedSlots.Controls.Pshare with | Corelib.Init.Datatypes.Some sp => match Accounting.ArmControls.seq_run RegionCore.Core.baseline with | Corelib.Init.Datatypes.Some sb => (Accounting.Accounting.footprint_at SharedSlots.Controls.Pshare (RegionCore.Core.regs sp) 0, Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) 0, Accounting.Accounting.footprint_at SharedSlots.Controls.Pshare (RegionCore.Core.regs sp) 1, Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) 1) | Corelib.Init.Datatypes.None => (0, 0, 0, 0) end | Corelib.Init.Datatypes.None => (0, 0, 0, 0) end = (1, 2, 1, 2)
  := Accounting.Claims.pin_sequential_shares_one_cell.

Definition pin_loop_ok_bound_is_tight :
  match Accounting.Controls.run_of RegionCore.Controls.P0 with | Corelib.Init.Datatypes.Some s => (Accounting.Accounting.footprint_at RegionCore.Controls.P0 (RegionCore.Core.regs s) 0, 1 + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at RegionCore.Controls.P0 (RegionCore.Core.regs s) 0)) | Corelib.Init.Datatypes.None => (0, 0) end = (1, 1)
  := Accounting.Claims.pin_loop_ok_bound_is_tight.

Definition pin_loop_ok_footprint_bounded :
  forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles RegionCore.Controls.P0 host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb /\ Accounting.Footprint.within RegionCore.Controls.P0 sp /\ (forall e : Corelib.Init.Datatypes.nat, Accounting.Accounting.footprint_at RegionCore.Controls.P0 (RegionCore.Core.regs sp) e <= 1 + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at RegionCore.Controls.P0 (RegionCore.Core.regs sp) e) /\ Accounting.Accounting.footprint_at RegionCore.Controls.P0 (RegionCore.Core.regs sp) e <= Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e /\ Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e = Corelib.Init.Datatypes.length (Accounting.Accounting.allocs_at (RegionCore.Core.regs sb) e)) | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host RegionCore.Controls.loop_ok fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_loop_ok_footprint_bounded.

Definition pin_sequential_footprint_bounded :
  forall (host : Corelib.Init.Datatypes.list RegionCore.Core.ev -> Corelib.Init.Datatypes.nat) (fuel n : Corelib.Init.Datatypes.nat), match RegionCore.Core.cycles SharedSlots.Controls.Pshare host SharedSlots.Controls.sequential fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sp => match RegionCore.Core.cycles RegionCore.Core.baseline host SharedSlots.Controls.sequential fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some sb => RegionCore.Core.regs sp = RegionCore.Core.regs sb /\ Accounting.Footprint.within SharedSlots.Controls.Pshare sp /\ (forall e : Corelib.Init.Datatypes.nat, Accounting.Accounting.footprint_at SharedSlots.Controls.Pshare (RegionCore.Core.regs sp) e <= 1 + Corelib.Init.Datatypes.length (Accounting.Accounting.unplanned_at SharedSlots.Controls.Pshare (RegionCore.Core.regs sp) e) /\ Accounting.Accounting.footprint_at SharedSlots.Controls.Pshare (RegionCore.Core.regs sp) e <= Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e /\ Accounting.Accounting.footprint_at RegionCore.Core.baseline (RegionCore.Core.regs sb) e = Corelib.Init.Datatypes.length (Accounting.Accounting.allocs_at (RegionCore.Core.regs sb) e)) | Corelib.Init.Datatypes.None => Corelib.Init.Logic.False end | Corelib.Init.Datatypes.None => match RegionCore.Core.cycles RegionCore.Core.baseline host SharedSlots.Controls.sequential fuel n RegionCore.Core.init with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Logic.False | Corelib.Init.Datatypes.None => Corelib.Init.Logic.True end end
  := Accounting.Claims.pin_sequential_footprint_bounded.

Definition pin_run_of_pinned :
  forall P : RegionCore.Core.plan, Accounting.Controls.run_of P = RegionCore.Core.cycles P RegionCore.Controls.host_len RegionCore.Controls.loop_ok 100 2 RegionCore.Core.init
  := Accounting.Claims.run_of_pinned.

Definition pin_fp_pinned :
  forall (P : RegionCore.Core.plan) (e : Corelib.Init.Datatypes.nat), Accounting.Controls.fp P e = match Accounting.Controls.run_of P with | Corelib.Init.Datatypes.Some s => Accounting.Accounting.footprint_at P (RegionCore.Core.regs s) e | Corelib.Init.Datatypes.None => 0 end
  := Accounting.Claims.fp_pinned.

Definition pin_seq_run_pinned :
  forall P : RegionCore.Core.plan, Accounting.ArmControls.seq_run P = RegionCore.Core.cycles P RegionCore.Controls.host_len SharedSlots.Controls.sequential 100 2 RegionCore.Core.init
  := Accounting.Claims.seq_run_pinned.

Definition pin_footprint_at_pinned :
  forall (P : RegionCore.Core.plan) (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.footprint_at P rs e = Corelib.Init.Datatypes.length (Stdlib.Lists.List.nodup RegionCore.Core.cell_eq_dec (Corelib.Lists.ListDef.map (Accounting.Accounting.cell_at P) (Accounting.Accounting.allocs_at rs e)))
  := Accounting.Claims.footprint_at_pinned.

Definition pin_cell_at_pinned :
  forall (P : RegionCore.Core.plan) (a : Accounting.Accounting.alloc), Accounting.Accounting.cell_at P a = RegionCore.Core.cell_of P (Corelib.Init.Datatypes.fst (Corelib.Init.Datatypes.snd a)) (Corelib.Init.Datatypes.fst a)
  := Accounting.Claims.cell_at_pinned.

Definition pin_allocs_at_pinned :
  forall (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.allocs_at rs e = Stdlib.Lists.List.filter (fun a : Corelib.Init.Datatypes.nat * (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat) => Corelib.Init.Nat.eqb (Corelib.Init.Datatypes.snd (Corelib.Init.Datatypes.snd a)) e) (Accounting.Accounting.indexed rs)
  := Accounting.Claims.allocs_at_pinned.

Definition pin_indexed_pinned :
  forall rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat), Corelib.Lists.ListDef.map Corelib.Init.Datatypes.fst (Accounting.Accounting.indexed rs) = Corelib.Lists.ListDef.seq 0 (Corelib.Init.Datatypes.length rs) /\ Corelib.Lists.ListDef.map Corelib.Init.Datatypes.snd (Accounting.Accounting.indexed rs) = rs
  := Accounting.Claims.indexed_pinned.

Definition pin_unplanned_at_pinned :
  forall (P : RegionCore.Core.plan) (rs : Corelib.Init.Datatypes.list (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat)) (e : Corelib.Init.Datatypes.nat), Accounting.Accounting.unplanned_at P rs e = Stdlib.Lists.List.filter (fun a : Corelib.Init.Datatypes.nat * (Corelib.Init.Datatypes.nat * Corelib.Init.Datatypes.nat) => match P (Corelib.Init.Datatypes.fst (Corelib.Init.Datatypes.snd a)) with | Corelib.Init.Datatypes.Some _ => Corelib.Init.Datatypes.false | Corelib.Init.Datatypes.None => Corelib.Init.Datatypes.true end) (Accounting.Accounting.allocs_at rs e)
  := Accounting.Claims.unplanned_at_pinned.

Definition pin_within_pinned :
  forall (P : RegionCore.Core.plan) (s : RegionCore.Core.state), Accounting.Footprint.within P s = (forall (c : RegionCore.Core.cell) (x : Corelib.Init.Datatypes.nat), RegionCore.Core.mem s c = Corelib.Init.Datatypes.Some x -> Stdlib.Lists.List.In c (Corelib.Lists.ListDef.map (Accounting.Accounting.cell_at P) (Accounting.Accounting.indexed (RegionCore.Core.regs s))))
  := Accounting.Claims.within_pinned.
