(** * Instance.v -- increment B1's composition theorem, applied to the region core.

    - [instance_sound] is increment B1's Definition 5 soundness, [Trace.sound],
      for the machine [next] derived from a module the checker accepts, under
      every plan giving distinct sites distinct slots.  It is proved by
      [Trace.lemma4_composition], whose per-slot premise [checked_slot_safe]
      discharges from [region_check].
    - [core_log_is_B1_obs] says the core's own observation log under a plan
      is the projection of B1's observations of the events the machine
      generates under the same plan, so B1's [sound] speaks of the core.
    - [reuse_sound_via_B1] concludes increment B2's headline statement, word
      for word, from [instance_sound] alone, without the lockstep simulation
      of [RegionCore.Sim]. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
From RegionReuse Require Trace.
From Instance Require Import Encode Causal Memory Machine Safety.
Import ListNotations.

Abbreviation tsound := (Trace.sound nat nat unit nat sel0).

Lemma baseline_same : Trace.baseline nat = baseline.
Proof. reflexivity. Qed.

(** ** Increment B1's theorem holds of the core. *)
Theorem instance_sound : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n, tsound (next m host fuel n) P.
Proof.
  intros m ft Hc P Pinj host fuel n. apply Trace.lemma4_composition. intros N k.
  rewrite (gen_is_run (Trace.baseline nat) host m fuel n N). apply slot_safe_firstn.
  apply (checked_slot_safe m ft Hc P Pinj).
Qed.

(** ** The core's log is B1's observations, for every module and plan.

    Once the machine has generated the whole run, which it does after
    finitely many steps, the core's log is the projection [to_log] of B1's
    observations, under the same plan, of the generated events. *)
Theorem core_log_is_B1_obs : forall Q host m fuel n,
  match cycles Q host m fuel n init with
  | Some s => exists N0, forall N, N0 <= N ->
      log s = to_log (tobs_of Q (tgen (next m host fuel n) Q N))
  | None => True
  end.
Proof.
  intros Q host m fuel n. pose proof (core_is_oracle Q host m fuel n) as H.
  destruct (cycles Q host m fuel n init) as [s|]; [|exact I].
  destruct H as [_ [_ Hl]]. exists (length (run_events Q host m fuel n)). intros N HN.
  rewrite (gen_is_run Q host m fuel n N), firstn_all2 by exact HN. exact Hl.
Qed.

(** ** Increment B2's headline statement, concluded from increment B1's. *)
Theorem reuse_sound_via_B1 : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end.
Proof.
  intros m ft Hc P Pinj host fuel n.
  pose proof (instance_sound m ft Hc P Pinj host fuel n
                (length (run_events P host m fuel n) + length (run_events baseline host m fuel n))) as Hs.
  rewrite (gen_is_run P host m fuel n), (gen_is_run (Trace.baseline nat) host m fuel n) in Hs.
  rewrite baseline_same in Hs. rewrite !firstn_all2 in Hs by lia.
  assert (Ho : orun_all (ans_plan P) host m fuel n = orun_all (ans_plan baseline) host m fuel n).
  { rewrite <- (replay P host m fuel n), <- (replay baseline host m fuel n), Hs. reflexivity. }
  pose proof (core_is_oracle P host m fuel n) as HP.
  pose proof (core_is_oracle baseline host m fuel n) as HB.
  rewrite Ho in HP.
  destruct (cycles P host m fuel n init) as [sp|]; destruct (cycles baseline host m fuel n init) as [sb|].
  - destruct HP as [_ [HP _]]. destruct HB as [_ [HB _]]. rewrite HP in HB.
    exact (f_equal log HB).
  - destruct HP as [HP _]. congruence.
  - destruct HB as [HB _]. congruence.
  - exact I.
Qed.
