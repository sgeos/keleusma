(** * Statements.v -- the headline statements, pinned.

    Each pin is a definition whose stated type must match the headline
    result's type up to conversion, so a statement weakened by an added
    premise or a changed conclusion fails to compile.  The adversarial audit of 2026-11-08 showed the other checks
    could not detect such a weakening.  The script requires the number of
    pins below. *)

From Stdlib Require Import List.
From CoreBounds Require Import Core Check Algebra Soundness.
Import ListNotations.

Definition pin_function_bounded : forall host m tab A, check m = Some (tab, A) ->
  forall f g B s, nth_error (mfuncs m) f = Some g -> nth_error tab f = Some B ->
  exists s' p, run_b host (mfuncs m) (ad B) (fbody g) s = Some (s', p) /\ good B s s' p
  := function_bounded.

Definition pin_fn_never_yields : forall host m tab A, check m = Some (tab, A) ->
  forall f g B s s' p n, nth_error (mfuncs m) f = Some g -> nth_error tab f = Some B ->
  fkind g = KFn -> run_b host (mfuncs m) n (fbody g) s = Some (s', p) -> exists c, p = [c]
  := fn_never_yields.

Definition pin_cycle_bounded : forall host m tab A, check m = Some (tab, A) ->
  forall s, exists s' p, cycle host m (ad A) s = Some (s', p) /\
  (exists f mi l, summary p = Y f mi l) /\
  (forall x, In x p -> x <= wcet A) /\ arena s' <= wcmu A
  := cycle_bounded.

Definition pin_stream_bounded : forall host m tab A, check m = Some (tab, A) ->
  forall n s, exists cs, cycles host m (ad A) n s = Some cs /\ length cs = n /\
  (forall c, In c cs -> snd c <= wcmu A) /\
  (forall x, In x (stream_segs cs) -> x <= wcet A)
  := stream_bounded.

(** The definitions the conclusions rest on, pinned by proved equations. *)
Example good_pinned : forall A s s' p, good A s s' p =
  (p <> [] /\ le_summ (summary p) A /\ arena s <= arena s' /\ arena s' <= arena s + am A).
Proof. reflexivity. Qed.

Example wcet_pinned : forall A, wcet A =
  match ay A with Some (f, m, l) => Nat.max f (Nat.max m (l + 1 + f)) | None => 0 end.
Proof. reflexivity. Qed.

Example wcmu_pinned : forall A, wcmu A = am A.
Proof. reflexivity. Qed.
