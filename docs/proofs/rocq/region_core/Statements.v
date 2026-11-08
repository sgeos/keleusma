(** * Statements.v -- the headline statement and the controls, pinned.

    The pin is a definition whose stated type must match [reuse_sound]'s type
    up to conversion, so a statement weakened by an added premise or a changed
    conclusion fails to compile.  The definition of a well-formed plan is
    pinned by a proved equation.  The definitions the statement mentions,
    among them [cycles], [run_b], [log], [init] and [region_check], are not
    pinned and rest on the trusted base, as the README records. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape Sim Reuse Controls.
Import ListNotations.

Definition pin_reuse_sound : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := reuse_sound.

(** The divergence controls, pinned by type, so a control cannot be weakened
    to a tautology without failing to compile. *)
Definition pin_loop_ok_sound : forall P, injective P -> forall host fuel n,
  match cycles P host loop_ok fuel n init, cycles baseline host loop_ok fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := loop_ok_sound.

Lemma pin_loop_bad_diverges :
  region_check loop_bad = None /\ log_of P0 loop_bad <> log_of baseline loop_bad.
Proof.
  split; [exact loop_bad_rejected|]. destruct loop_bad_diverges as [E1 E2].
  rewrite E1, E2. discriminate.
Qed.

Lemma pin_sep_only_diverges :
  shape_b [] true [] (blk [Const 1; New 0; Dup; Get; Yield; Store 1; Const 2]) = Some [TS; TR 0] /\
  region_check sep_only = None /\ log_of P0 sep_only <> log_of baseline sep_only.
Proof.
  destruct sep_only_rejected_at_allocation as [A1 A2]. split; [exact A1|]. split; [exact A2|].
  destruct sep_only_diverges as [E1 E2]. rewrite E1, E2. discriminate.
Qed.

Definition pin_shared_slot_diverges :
  region_check two_sites = Some [] /\ log_of P01 two_sites <> log_of baseline two_sites /\ ~ injective P01
  := conj two_sites_accepted (conj shared_slot_diverges shared_slot_not_injective).

Example injective_pinned : forall P, injective P = (forall s1 s2 k, P s1 = Some k -> P s2 = Some k -> s1 = s2).
Proof. reflexivity. Qed.

Example baseline_pinned : forall s, baseline s = None.
Proof. reflexivity. Qed.
