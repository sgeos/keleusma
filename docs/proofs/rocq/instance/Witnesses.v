(** * Witnesses.v -- evidence that the instantiation is not vacuous.

    - The machine really generates the core's events.  For the accepted loop
      of increment B2's controls, one cycle generates three allocations,
      three dereferences of real regions, and one epoch advance.
    - Increment B1's soundness is refuted for the instance machine of a
      module the checker rejects, under a plan giving distinct sites
      distinct slots.  So [instance_sound] does not hold of every module,
      and its checker premise does work.
    - It is also refuted for the accepted two-site module under the plan
      sharing one slot between both sites, so the injectivity premise does
      work.  Through [Trace.lemma4_composition], the per-slot premise of
      increment B1 then fails on that module's baseline run, so the premise
      [checked_slot_safe] discharges is not trivially true. *)

From Stdlib Require Import List Arith Lia.
From RegionCore Require Import Core Shape Controls.
From RegionReuse Require Trace.
From Instance Require Import Encode Memory Machine Safety Instance.
Import ListNotations.

Definition count_sreg (l : list tevent) : nat :=
  length (filter (fun e => match e with
                           | Trace.EDeref _ _ _ _ (Trace.SReg _) _ => true
                           | _ => false
                           end) l).

Example loop_ok_events :
  let E := run_events baseline host_len loop_ok 100 1 in
  (tnallocs E, count_sreg E, nder E, tnadv E) = (3, 3, 3, 1).
Proof. vm_compute. reflexivity. Qed.

Example loop_bad_unsound_in_B1 : ~ tsound (next loop_bad host_len 100 2) P0.
Proof.
  intros H. specialize (H 80). vm_compute in H. discriminate H.
Qed.

Example loop_bad_plan_injective : injective P0.
Proof.
  intros [|[|s1]] [|[|s2]] k H1 H2; simpl in *; try discriminate; reflexivity.
Qed.

Example shared_slot_unsound_in_B1 : ~ tsound (next two_sites host_len 100 2) P01.
Proof.
  intros H. specialize (H 60). vm_compute in H. discriminate H.
Qed.

Theorem shared_slot_premise_fails :
  ~ (forall N k, tslot_safe P01 k (tgen (next two_sites host_len 100 2) (Trace.baseline nat) N)).
Proof.
  intros H. apply shared_slot_unsound_in_B1. exact (Trace.lemma4_composition _ _ _ _ _ _ P01 H).
Qed.
