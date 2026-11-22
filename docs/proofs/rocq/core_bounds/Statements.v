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

From CoreBounds Require Core Check Algebra Soundness Controls.
From CoreBounds Require Claims.

Definition pin_function_bounded :
  forall (host : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat -> Corelib.Init.Datatypes.nat) (m : CoreBounds.Core.module) (tab : Corelib.Init.Datatypes.list CoreBounds.Check.abs) (A : CoreBounds.Check.abs), CoreBounds.Check.check m = Corelib.Init.Datatypes.Some (tab, A) -> forall (f : Corelib.Init.Datatypes.nat) (g : CoreBounds.Core.func) (B : CoreBounds.Check.abs) (s : CoreBounds.Core.state), Stdlib.Lists.List.nth_error (CoreBounds.Core.mfuncs m) f = Corelib.Init.Datatypes.Some g -> Stdlib.Lists.List.nth_error tab f = Corelib.Init.Datatypes.Some B -> exists (s' : CoreBounds.Core.state) (p : CoreBounds.Core.segs), CoreBounds.Core.run_b host (CoreBounds.Core.mfuncs m) (CoreBounds.Check.ad B) (CoreBounds.Core.fbody g) s = Corelib.Init.Datatypes.Some (s', p) /\ CoreBounds.Soundness.good B s s' p
  := CoreBounds.Claims.pin_function_bounded.

Definition pin_fn_never_yields :
  forall (host : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat -> Corelib.Init.Datatypes.nat) (m : CoreBounds.Core.module) (tab : Corelib.Init.Datatypes.list CoreBounds.Check.abs) (A : CoreBounds.Check.abs), CoreBounds.Check.check m = Corelib.Init.Datatypes.Some (tab, A) -> forall (f : Corelib.Init.Datatypes.nat) (g : CoreBounds.Core.func) (B : CoreBounds.Check.abs) (s s' : CoreBounds.Core.state) (p : CoreBounds.Core.segs) (n : Corelib.Init.Datatypes.nat), Stdlib.Lists.List.nth_error (CoreBounds.Core.mfuncs m) f = Corelib.Init.Datatypes.Some g -> Stdlib.Lists.List.nth_error tab f = Corelib.Init.Datatypes.Some B -> CoreBounds.Core.fkind g = CoreBounds.Core.KFn -> CoreBounds.Core.run_b host (CoreBounds.Core.mfuncs m) n (CoreBounds.Core.fbody g) s = Corelib.Init.Datatypes.Some (s', p) -> exists c : Corelib.Init.Datatypes.nat, p = (c :: Corelib.Init.Datatypes.nil)%list
  := CoreBounds.Claims.pin_fn_never_yields.

Definition pin_cycle_bounded :
  forall (host : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat -> Corelib.Init.Datatypes.nat) (m : CoreBounds.Core.module) (tab : Corelib.Init.Datatypes.list CoreBounds.Check.abs) (A : CoreBounds.Check.abs), CoreBounds.Check.check m = Corelib.Init.Datatypes.Some (tab, A) -> forall s : CoreBounds.Core.state, exists (s' : CoreBounds.Core.state) (p : CoreBounds.Core.segs), CoreBounds.Core.cycle host m (CoreBounds.Check.ad A) s = Corelib.Init.Datatypes.Some (s', p) /\ (exists f mi l : Corelib.Init.Datatypes.nat, CoreBounds.Check.summary p = CoreBounds.Check.Y f mi l) /\ (forall x : Corelib.Init.Datatypes.nat, Stdlib.Lists.List.In x p -> x <= CoreBounds.Check.wcet A) /\ CoreBounds.Core.arena s' <= CoreBounds.Check.wcmu A
  := CoreBounds.Claims.pin_cycle_bounded.

Definition pin_stream_bounded :
  forall (host : Corelib.Init.Datatypes.list Corelib.Init.Datatypes.nat -> Corelib.Init.Datatypes.nat) (m : CoreBounds.Core.module) (tab : Corelib.Init.Datatypes.list CoreBounds.Check.abs) (A : CoreBounds.Check.abs), CoreBounds.Check.check m = Corelib.Init.Datatypes.Some (tab, A) -> forall (n : Corelib.Init.Datatypes.nat) (s : CoreBounds.Core.state), exists cs : Corelib.Init.Datatypes.list (CoreBounds.Core.segs * Corelib.Init.Datatypes.nat), CoreBounds.Core.cycles host m (CoreBounds.Check.ad A) n s = Corelib.Init.Datatypes.Some cs /\ Corelib.Init.Datatypes.length cs = n /\ (forall c : CoreBounds.Core.segs * Corelib.Init.Datatypes.nat, Stdlib.Lists.List.In c cs -> Corelib.Init.Datatypes.snd c <= CoreBounds.Check.wcmu A) /\ (forall x : Corelib.Init.Datatypes.nat, Stdlib.Lists.List.In x (CoreBounds.Core.stream_segs cs) -> x <= CoreBounds.Check.wcet A)
  := CoreBounds.Claims.pin_stream_bounded.

Definition pin_good_pinned :
  forall (A : CoreBounds.Check.abs) (s s' : CoreBounds.Core.state) (p : CoreBounds.Core.segs), CoreBounds.Soundness.good A s s' p = (p <> Corelib.Init.Datatypes.nil /\ CoreBounds.Algebra.le_summ (CoreBounds.Check.summary p) A /\ CoreBounds.Core.arena s <= CoreBounds.Core.arena s' <= CoreBounds.Core.arena s + CoreBounds.Check.am A)
  := CoreBounds.Claims.good_pinned.

Definition pin_wcet_pinned :
  forall A : CoreBounds.Check.abs, CoreBounds.Check.wcet A = match CoreBounds.Check.ay A with | Corelib.Init.Datatypes.Some (f, m, l) => Corelib.Init.Nat.max f (Corelib.Init.Nat.max m (l + 1 + f)) | Corelib.Init.Datatypes.None => 0 end
  := CoreBounds.Claims.wcet_pinned.

Definition pin_wcmu_pinned :
  forall A : CoreBounds.Check.abs, CoreBounds.Check.wcmu A = CoreBounds.Check.am A
  := CoreBounds.Claims.wcmu_pinned.
