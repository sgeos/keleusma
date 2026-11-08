(** * Machine.v -- the region core as an increment B1 machine.

    [next m host fuel n] is a strategy in increment B1's sense, a function
    from the observation history to the next event, and it depends on no
    plan.  Given a history, it replays the core under the oracle that answers
    the dereferences in order from the history's dereference outcomes, and
    returns the event at the history's length.

    [gen_is_run] shows the strategy generates, under every plan [Q], exactly
    the events of the core's own run under [Q], which [Memory.v] relates to
    the core's interpreter.  [replay] shows the run is determined by the
    observations, so two plans with the same observations end in the same
    state. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core.
From RegionReuse Require Trace.
From Instance Require Import Encode Causal Memory.
Import ListNotations.

Abbreviation tgen := (Trace.gen nat nat unit nat sel0).

(** The oracle reading the dereference outcomes of a history in order. *)
Definition ans_of (os : list tobs) : oracle := fun tr _ => nth (nder tr) (answers os) None.

(** The machine. *)
Definition next (m : module) (host : list ev -> nat) (fuel n : nat) : list tobs -> option tevent :=
  fun os => nth_error (otr (orun_all (ans_of os) host m fuel n)) (length os).

(** The events of the core's run under plan [Q]. *)
Definition run_events (Q : plan) (host : list ev -> nat) (m : module) (fuel n : nat) : list tevent :=
  otr (orun_all (ans_plan Q) host m fuel n).

(** ** Observations of single events. *)
Lemma read_shape_any : forall Q B x (v : unit), tread Q B x v = Trace.OStale _ _ \/
  exists b, tread Q B x v = Trace.OBytes _ _ b.
Proof. intros Q B x []. apply read_shape. Qed.

Lemma answers_one : forall Q B e, length (answers [fst (texec Q e B)]) = nder [e].
Proof.
  intros Q B e. destruct e as [|s b|x v|x v| |o]; try reflexivity.
  - simpl. destruct (read_shape_any Q B x v) as [E|[b E]]; rewrite E; reflexivity.
  - simpl. destruct (read_shape_any Q B x v) as [E|[b E]]; rewrite E; reflexivity.
Qed.

Lemma answers_len : forall Q tr, length (answers (tobs_of Q tr)) = nder tr.
Proof.
  intros Q tr. induction tr as [|e tr IH] using rev_ind; [reflexivity|].
  rewrite obs_snoc, answers_app, length_app, IH, answers_one, nder_app. reflexivity.
Qed.

Lemma obs_len : forall Q tr, length (tobs_of Q tr) = length tr.
Proof.
  intros Q tr. induction tr as [|e tr IH] using rev_ind; [reflexivity|].
  rewrite obs_snoc, !length_app, IH. reflexivity.
Qed.

Lemma obs_prefix : forall Q a b, exists z, tobs_of Q (a ++ b) = tobs_of Q a ++ z.
Proof. intros. unfold Trace.obs_of. rewrite Trace.run_app. eexists. reflexivity. Qed.

(** The outcome of the read after [tr] is at the read's index among the
    history's outcomes. *)
Lemma answer_at : forall Q tr x rest,
  nth (nder tr) (answers (tobs_of Q (tr ++ TDeref x tt :: rest))) None =
  to_ans (tread Q (tafter Q tr) x tt).
Proof.
  intros Q tr x rest.
  replace (tr ++ TDeref x tt :: rest) with ((tr ++ [TDeref x tt]) ++ rest) by (rewrite <- app_assoc; reflexivity).
  destruct (obs_prefix Q (tr ++ [TDeref x tt]) rest) as [z ->].
  rewrite obs_snoc, !answers_app, <- app_assoc, app_nth2; rewrite answers_len; [|lia].
  rewrite Nat.sub_diag. simpl. destruct (read_shape Q (tafter Q tr) x) as [E|[b E]]; rewrite E; reflexivity.
Qed.

Lemma firstn_split : forall (tr : list tevent) e rest i, length tr < i ->
  firstn i (tr ++ e :: rest) = tr ++ e :: firstn (i - length tr - 1) rest.
Proof.
  intros tr e rest i H. rewrite firstn_app, firstn_all2 by lia.
  destruct (i - length tr) as [|j] eqn:Ej; [lia|]. simpl. f_equal. f_equal. f_equal. lia.
Qed.

Section Run.

Variable Q : plan.
Variable host : list ev -> nat.
Variable m : module.
Variables fuel n : nat.

Abbreviation FIN := (run_events Q host m fuel n).

(** The oracle of a prefix agrees with the plan's oracle on the reads of
    the prefix. *)
Lemma agree_prefix : forall i tr x rest, FIN = tr ++ TDeref x tt :: rest ->
  nder tr < nder (firstn i FIN) ->
  ans_plan Q tr x = ans_of (tobs_of Q (firstn i FIN)) tr x.
Proof.
  intros i tr x rest Hf Hk. unfold ans_of, ans_plan.
  pose proof Hk as Hk'. rewrite Hf in Hk'. apply read_before in Hk'.
  rewrite Hf, (firstn_split _ _ _ _ Hk'). rewrite answer_at. reflexivity.
Qed.

Lemma rel_prefix : forall i,
  Rel (nder (firstn i FIN)) (orun_all (ans_plan Q) host m fuel n)
      (orun_all (ans_of (tobs_of Q (firstn i FIN))) host m fuel n).
Proof.
  intros i. unfold orun_all.
  apply (causal_cycles _ _ host (mfuncs m) FIN _ (agree_prefix i) m fuel n o0 eq_refl).
  exists []. rewrite app_nil_r. reflexivity.
Qed.

Theorem next_follows : forall i, i <= length FIN ->
  next m host fuel n (tobs_of Q (firstn i FIN)) = nth_error FIN i.
Proof.
  intros i Hi. unfold next. rewrite obs_len, firstn_length_le by exact Hi.
  destruct (rel_prefix i) as [E|[H1 [H2 H3]]]; [rewrite <- E; reflexivity|].
  destruct (Nat.eq_dec i (length FIN)) as [->|Hne].
  - rewrite firstn_all in H1. unfold run_events in H1. lia.
  - assert (Hlt : i < length FIN) by lia.
    assert (Hu : nth_error (upto (nder (firstn i FIN)) FIN) i = nth_error FIN i)
      by (apply upto_nth; [exact Hlt|lia]).
    destruct (nth_error FIN i) as [e|] eqn:Ee; [|apply nth_error_None in Ee; lia].
    change (otr (orun_all (ans_plan Q) host m fuel n)) with FIN in H3. rewrite H3 in Hu.
    set (T2 := otr (orun_all (ans_of (tobs_of Q (firstn i FIN))) host m fuel n)) in *.
    destruct (upto_prefix T2 (nder (firstn i FIN))) as [d Hd].
    rewrite Hd, nth_error_app1; [exact Hu|apply nth_error_Some; rewrite Hu; discriminate].
Qed.

Lemma firstn_snoc : forall (l : list tevent) i e, nth_error l i = Some e ->
  firstn (S i) l = firstn i l ++ [e].
Proof.
  intros l i e H. destruct (nth_error_split l i H) as [l1 [l2 [-> Hl]]].
  rewrite !firstn_app, Hl, Nat.sub_diag, Nat.sub_succ_l, Nat.sub_diag by lia.
  rewrite (firstn_all2 l1) by lia. rewrite (firstn_all2 l1) by lia. simpl. rewrite app_nil_r. reflexivity.
Qed.

(** The machine generates the core's run under [Q]. *)
Theorem gen_is_run : forall N, tgen (next m host fuel n) Q N = firstn N FIN.
Proof.
  induction N as [|N IH]; [reflexivity|]. simpl. rewrite IH.
  destruct (Nat.lt_ge_cases N (length FIN)) as [Hl|Hl].
  - rewrite (next_follows N) by lia.
    destruct (nth_error FIN N) as [e|] eqn:Ee; [|apply nth_error_None in Ee; lia].
    symmetry. apply firstn_snoc. exact Ee.
  - rewrite (firstn_all2 FIN Hl).
    pose proof (next_follows (length FIN) (le_n _)) as H. rewrite firstn_all in H. rewrite H.
    rewrite (proj2 (nth_error_None FIN (length FIN)) (le_n _)).
    change (FIN = firstn (S N) FIN). rewrite firstn_all2 by lia. reflexivity.
Qed.

(** The run is the replay of its own observations. *)
Theorem replay : orun_all (ans_of (tobs_of Q FIN)) host m fuel n = orun_all (ans_plan Q) host m fuel n.
Proof.
  pose proof (rel_prefix (length FIN)) as H. rewrite firstn_all in H.
  destruct H as [E|[H1 _]]; [symmetry; exact E|]. unfold run_events in H1. lia.
Qed.

End Run.
