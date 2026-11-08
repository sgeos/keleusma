(** * Statements.v -- the headline statements and the controls, pinned.

    Each pin is a definition or lemma whose stated type must match the
    result's type up to conversion, so a statement weakened by an added
    premise or a changed conclusion fails to compile.  The definitions the
    statements mention, among them [region_checkP], [conflict], [cycles],
    [log] and the control modules, are not pinned and rest on the trusted
    base, as the README records. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape Controls.
From SharedSlots Require Import SharedShape Shared ArmShape Arm Controls.
Import ListNotations.

Definition pin_reuse_sound_shared : forall P m ft, region_checkP P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := reuse_sound_shared.

Definition pin_shared_generalises : forall P m, injective P -> region_checkP P m = region_check m
  := shared_generalises.

Lemma pin_arm_group_covered :
  ~ injective Pshare /\ region_checkP Pshare arms = Some [] /\
  log_of Pshare arms = log_of baseline arms.
Proof.
  split; [exact Pshare_not_injective|]. split; [exact arms_accepted|exact (proj1 arms_agree)].
Qed.

Lemma pin_co_live_rejected :
  region_checkP Pshare two_sites = None /\ region_checkA Pshare two_sites = None /\
  region_check two_sites = Some [] /\ log_of Pshare two_sites <> log_of baseline two_sites.
Proof.
  split; [exact (proj1 co_live_rejected)|]. split; [exact co_live_rejected_by_join|].
  split; [exact (proj2 co_live_rejected)|exact co_live_diverges].
Qed.

Definition pin_reuse_sound_arm : forall P m ft, region_checkA P m = Some ft ->
  forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := reuse_sound_arm.

Definition pin_arm_contains_b2 : forall P m ft, injective P -> region_check m = Some ft ->
  region_checkA P m = Some ft
  := arm_contains_b2.

Lemma pin_choose_covered :
  ~ injective Pshare /\ region_checkP Pshare choose = None /\ region_checkA Pshare choose = Some [] /\
  log_of Pshare choose = log_of baseline choose.
Proof.
  split; [exact Pshare_not_injective|]. split; [exact (proj1 choose_needs_join)|].
  split; [exact (proj2 choose_needs_join)|exact (proj1 choose_agrees)].
Qed.
