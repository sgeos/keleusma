(** * Statements.v -- the headline statements of the instantiation, pinned.

    Each pin is a definition whose stated type must match its theorem's type
    up to conversion, so a statement weakened by an added premise or a
    changed conclusion fails to compile.  [pin_reuse_sound_via_B1] repeats
    the text of increment B2's own pin of [reuse_sound], and
    [pin_same_as_B2] checks that the two theorems have one type.  The
    definitions the statements mention, among them [next], [run_events],
    [orun_all], [ans_plan], [strip] and [to_log], are not pinned and rest on
    the trusted base, as the README records. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape Reuse Controls.
From RegionReuse Require Trace.
From Instance Require Import Encode Memory Machine Safety Instance Witnesses.
Import ListNotations.

Definition pin_instance_sound : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  Trace.sound nat nat unit nat sel0 (next m host fuel n) P
  := instance_sound.

Definition pin_checked_slot_safe : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall A host fuel n k,
  Trace.slot_safe nat nat unit nat P k (otr (orun_all A host m fuel n))
  := checked_slot_safe.

Definition pin_gen_is_run : forall Q host m fuel n N,
  Trace.gen nat nat unit nat sel0 (next m host fuel n) Q N = firstn N (run_events Q host m fuel n)
  := gen_is_run.

Definition pin_core_is_oracle : forall Q host m fuel n,
  match cycles Q host m fuel n init with
  | Some s => ohalt (orun_all (ans_plan Q) host m fuel n) = false /\
              ost (orun_all (ans_plan Q) host m fuel n) = strip s /\
              log s = to_log (Trace.obs_of nat nat unit nat sel0 Q (otr (orun_all (ans_plan Q) host m fuel n)))
  | None => ohalt (orun_all (ans_plan Q) host m fuel n) = true
  end
  := core_is_oracle.

Definition pin_core_log_is_B1_obs : forall Q host m fuel n,
  match cycles Q host m fuel n init with
  | Some s => exists N0, forall N, N0 <= N ->
      log s = to_log (Trace.obs_of nat nat unit nat sel0 Q (Trace.gen nat nat unit nat sel0 (next m host fuel n) Q N))
  | None => True
  end
  := core_log_is_B1_obs.

Definition pin_replay : forall Q host m fuel n,
  orun_all (ans_of (Trace.obs_of nat nat unit nat sel0 Q (run_events Q host m fuel n))) host m fuel n =
  orun_all (ans_plan Q) host m fuel n
  := replay.

Definition pin_reuse_sound_via_B1 : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := reuse_sound_via_B1.

(** The instantiation reproves increment B2's theorem with the same type. *)
Definition pin_same_as_B2 : reuse_sound = reuse_sound_via_B1 -> True := fun _ => I.

(** The witnesses, pinned so that none can be weakened to a tautology. *)
Definition pin_loop_bad_unsound_in_B1 :
  region_check loop_bad = None /\ injective P0 /\
  ~ Trace.sound nat nat unit nat sel0 (next loop_bad host_len 100 2) P0
  := conj loop_bad_rejected (conj loop_bad_plan_injective loop_bad_unsound_in_B1).

Definition pin_shared_slot_unsound_in_B1 :
  region_check two_sites = Some [] /\
  ~ Trace.sound nat nat unit nat sel0 (next two_sites host_len 100 2) P01
  := conj two_sites_accepted shared_slot_unsound_in_B1.

Example sel0_pinned : forall v b, sel0 v b = b.
Proof. reflexivity. Qed.

Example baseline_pinned : forall s, Trace.baseline nat s = None /\ baseline s = None.
Proof. split; reflexivity. Qed.
