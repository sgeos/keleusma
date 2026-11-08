(** * Theorems.v -- the reuse theorems of Part I, end to end.

    Joins the comparison method of [Trace.v] to the clearance lemmas of
    [Clearance.v].  A flow model describes one baseline run when its
    allocations and dereferences sit at the run's positions.  Theorems B1,
    B1r, Corollary A3, Theorem B2, and Corollary B2b are then instances of one
    element theorem, composed by Lemma 4.

    The flow model is a certificate the caller supplies.  [describes] ties it
    to the run only through allocation and dereference positions, so its
    classification, scope ends and holdings are its own, and a certificate
    with no [Escapes] execution meets every clearance route.  The README
    records this limit and what increment B2 must supply. *)

From Stdlib Require Import List Arith Lia.
From RegionReuse Require Import Trace Clearance.
Import ListNotations.

Section Theorems.

Variables Site Bytes View Scalar : Type.
Variable sel : View -> Bytes -> Bytes.

Local Abbreviation event := (Trace.event Site Bytes View Scalar).
Local Abbreviation EAlloc := (Trace.EAlloc Site Bytes View Scalar).
Local Abbreviation plan := (Trace.plan Site).
Local Abbreviation baseline := (Trace.baseline Site).
Local Abbreviation nallocs := (Trace.nallocs Site Bytes View Scalar).
Local Abbreviation nadv := (Trace.nadv Site Bytes View Scalar).
Local Abbreviation is_reg_read := (Trace.is_reg_read Site Bytes View Scalar).
Local Abbreviation nowrite := (Trace.nowrite Site Bytes View Scalar).
Local Abbreviation slot_safe := (Trace.slot_safe Site Bytes View Scalar).
Local Abbreviation clear_after := (Trace.clear_after Site Bytes View Scalar).
Local Abbreviation once_per_epoch := (Trace.once_per_epoch Site Bytes View Scalar).

(** ** Positions of a run, read as a flow model's allocations and reads. *)
Definition alloc_at (evs : list event) (t : nat) : option nat :=
  match nth_error evs t with
  | Some (Trace.EAlloc _ _ _ _ _ _) => Some (nallocs (firstn t evs))
  | _ => None
  end.

Definition read_at (evs : list event) (t r : nat) : Prop :=
  exists e, nth_error evs t = Some e /\ is_reg_read e r.

(** [f] describes the run [evs]. *)
Definition describes (f : flow) (evs : list event) : Prop :=
  (forall t, alloc_of f t = alloc_at evs t) /\
  (forall t r, read_at evs t r -> reads f t r).

Lemma firstn_prefix : forall (p x : list event), firstn (length p) (p ++ x) = p.
Proof.
  intros p x. rewrite firstn_app, Nat.sub_diag, firstn_O, app_nil_r. apply firstn_all.
Qed.

Lemma pos_alloc : forall evs p1 s b rest,
  evs = p1 ++ EAlloc s b :: rest -> alloc_at evs (length p1) = Some (nallocs p1).
Proof.
  intros evs p1 s b rest ->. unfold alloc_at.
  rewrite nth_error_app2, Nat.sub_diag by lia. simpl. rewrite firstn_prefix. reflexivity.
Qed.

Lemma pos_read : forall evs p e q r,
  evs = p ++ e :: q -> is_reg_read e r -> read_at evs (length p) r.
Proof.
  intros evs p e q r -> Hr. exists e. split; [|exact Hr].
  rewrite nth_error_app2, Nat.sub_diag by lia. reflexivity.
Qed.

(** The injectivity field of [flow_ok] holds of every run, so an
    instantiation discharges it with this lemma. *)
Lemma alloc_at_inj : forall evs t t' r,
  alloc_at evs t = Some r -> alloc_at evs t' = Some r -> t = t'.
Proof.
  assert (Hlt : forall evs t t' r r', t < t' -> alloc_at evs t = Some r -> alloc_at evs t' = Some r' ->
                r < r').
  { intros evs t t' r r' Htt Ha Ha'. unfold alloc_at in Ha, Ha'.
    destruct (nth_error evs t) as [e|] eqn:Ht; [|discriminate].
    destruct e; try discriminate. injection Ha as <-.
    destruct (nth_error evs t') as [e'|] eqn:Ht'; [|discriminate].
    destruct e'; try discriminate. injection Ha' as <-.
    destruct (nth_error_split evs t Ht) as [l1 [l2 [-> Hl1]]].
    subst t. rewrite firstn_prefix.
    rewrite firstn_app, firstn_all2 by lia.
    rewrite Trace.nallocs_app.
    destruct (t' - length l1) as [|d] eqn:Hd; [lia|]. simpl. lia. }
  intros evs t t' r Ha Ha'. destruct (Nat.lt_trichotomy t t') as [H|[H|H]]; [|exact H|].
  - pose proof (Hlt evs t t' r r H Ha Ha'). lia.
  - pose proof (Hlt evs t' t r r H Ha' Ha). lia.
Qed.

(** A clearance lemma's conclusion is the read condition of the slot theorem. *)
Lemma clear_from_flow : forall f evs j r,
  describes f evs -> (forall t, send f j <= t -> ~ reads f t r) ->
  clear_after evs (send f) j r.
Proof.
  intros f evs j r [_ Hrd] Hno p e q Heq He Hle.
  exact (Hno (length p) Hle (Hrd _ _ (pos_read evs p e q r Heq He))).
Qed.

(** ** The element theorem.

    One reused slot is safe when consecutive allocations into it are separated
    by an epoch boundary or by the end of the earlier allocation's scope, and
    every region allocated into it is cleared by one of the three routes. *)
Definition separated_scopes (P : plan) (k : nat) (send : nat -> nat) (evs : list event) : Prop :=
  forall p1 s1 b1 m s2 b2 q,
    evs = p1 ++ EAlloc s1 b1 :: m ++ EAlloc s2 b2 :: q ->
    P s1 = Some k -> P s2 = Some k -> nowrite P (Trace.Slot k) (S (nallocs p1)) m ->
    0 < nadv m \/ send (length p1) <= S (length p1 + length m).

Inductive clearance (f : flow) (j r : nat) : Prop :=
| ByConfinement : confined f j r -> clearance f j r
| ByRefinement : forall D, refined_confined f j r D -> dead_at f j D -> clearance f j r
| ByDiscipline : discipline f r -> clearance f j r.

Definition cleared (f : flow) (P : plan) (k : nat) (evs : list event) : Prop :=
  forall p1 s b rest, evs = p1 ++ EAlloc s b :: rest -> P s = Some k ->
    clearance f (length p1) (nallocs p1).

Theorem element_safe : forall P k evs f,
  flow_ok f -> describes f evs ->
  separated_scopes P k (send f) evs -> cleared f P k evs ->
  slot_safe P k evs.
Proof.
  intros P k evs f Hf Hd Hsep Hcl. apply (Trace.slot_theorem _ _ _ _ P k (send f)).
  intros p1 s1 b1 m s2 b2 q Heq H1 H2 Hnw.
  destruct (Hsep p1 s1 b1 m s2 b2 q Heq H1 H2 Hnw) as [Hadv|Hle]; [left; exact Hadv|right].
  split; [exact Hle|].
  assert (Ha : alloc_of f (length p1) = Some (nallocs p1)).
  { destruct Hd as [Hal _]. rewrite Hal. apply (pos_alloc evs p1 s1 b1 (m ++ EAlloc s2 b2 :: q)).
    exact Heq. }
  apply (clear_from_flow f evs); [exact Hd|].
  destruct (Hcl p1 s1 b1 (m ++ EAlloc s2 b2 :: q) Heq H1) as [Hc|D Hrc Hdead|Hdisc].
  - exact (proj2 (lemma2_confinement_clearance f Hf _ _ Ha Hc)).
  - exact (lemma3_refined_clearance f Hf _ _ D Ha Hrc Hdead).
  - exact (proj2 (lemma5_unconditional_clearance f Hf _ _ Ha Hdisc)).
Qed.

(** ** The reuse theorems.

    [next] is the machine with its environment behavior, and every theorem
    holds for every [next], which is Definition 5's every path under every
    environment behavior.  Each premise is required of every baseline run. *)
Variable next : list (Trace.obs Bytes Scalar) -> option event.

Local Abbreviation gen := (Trace.gen Site Bytes View Scalar sel next).
Local Abbreviation sound := (Trace.sound Site Bytes View Scalar sel next).

(** The premise every element meets by one route or another. *)
Definition element_ok (P : plan) (k : nat) (evs : list event) : Prop :=
  once_per_epoch P k evs \/
  exists f, flow_ok f /\ describes f evs /\ separated_scopes P k (send f) evs /\ cleared f P k evs.

Lemma element_ok_safe : forall P k evs, element_ok P k evs -> slot_safe P k evs.
Proof.
  intros P k evs [Ho|[f [Hf [Hd [Hs Hc]]]]].
  - exact (Trace.A2_arm_overlap _ _ _ _ P k evs Ho).
  - exact (element_safe P k evs f Hf Hd Hs Hc).
Qed.

(** A slot no site uses meets the premise vacuously, so a plan's unused slots
    cost nothing. *)
Lemma unused_element_ok : forall (P : plan) k evs, (forall s, P s <> Some k) -> element_ok P k evs.
Proof. intros P k evs Hu. left. intros p1 s1 b1 m s2 b2 q _ H1. exfalso. exact (Hu s1 H1). Qed.

(** Lemma 4 for every element kind at once.  With designated elements cleared
    by the discipline and the others by confinement or refined confinement,
    this is Corollary B2b, the hybrid. *)
Theorem lemma4_plan_sound : forall P : plan,
  (forall n k, element_ok P k (gen baseline n)) -> sound P.
Proof.
  intros P H. apply Trace.lemma4_composition. intros n k. apply element_ok_safe. apply H.
Qed.

Theorem corollary_B2b_hybrid : forall P : plan,
  (forall n k, element_ok P k (gen baseline n)) -> sound P.
Proof. exact lemma4_plan_sound. Qed.

(** The plan's run executes the baseline's events, used by Theorem C. *)
Theorem plan_runs_baseline_events : forall P : plan,
  (forall n k, element_ok P k (gen baseline n)) -> forall n, gen P n = gen baseline n.
Proof.
  intros P H. apply Trace.plan_follows_baseline. intros n k. apply element_ok_safe. apply H.
Qed.

(** [sole P k] says the plan's sole reuse is slot [k], one site or one arm
    group. *)
Definition sole (P : plan) (k : nat) : Prop := forall s k', P s = Some k' -> k' = k.

Lemma sole_other : forall P k k' evs, sole P k -> k' <> k -> element_ok P k' evs.
Proof.
  intros P k k' evs Hs Hne. apply unused_element_ok. intros s Hk. exact (Hne (Hs s k' Hk)).
Qed.

Lemma sole_sound : forall P k,
  sole P k -> (forall n, element_ok P k (gen baseline n)) -> sound P.
Proof.
  intros P k Hs H. apply lemma4_plan_sound. intros n k'.
  destruct (Nat.eq_dec k' k) as [->|Hne]; [apply H|exact (sole_other P k k' _ Hs Hne)].
Qed.

(** ** Theorem A2, arm overlap.  The trace-level premise is that at most one
    allocation into the shared slot occurs per epoch. *)
Theorem theorem_A2 : forall P k,
  sole P k -> (forall n, once_per_epoch P k (gen baseline n)) -> sound P.
Proof. intros P k Hs H. apply (sole_sound P k Hs). intros n. left. apply H. Qed.

(** The confinement premise of Theorem B1, for every region of the slot. *)
Definition all_confined (f : flow) (P : plan) (k : nat) (evs : list event) : Prop :=
  forall p1 s b rest, evs = p1 ++ EAlloc s b :: rest -> P s = Some k ->
    confined f (length p1) (nallocs p1).

(** ** Theorem B1, confined-site reuse. *)
Theorem theorem_B1 : forall P k,
  sole P k ->
  (forall n, exists f, flow_ok f /\ describes f (gen baseline n) /\
     separated_scopes P k (send f) (gen baseline n) /\ all_confined f P k (gen baseline n)) ->
  sound P.
Proof.
  intros P k Hs H. apply (sole_sound P k Hs). intros n.
  destruct (H n) as [f [Hf [Hd [Hsep Hc]]]]. right. exists f. split; [exact Hf|]. split; [exact Hd|]. split; [try exact Hsep|].
  intros p1 s b rest Heq Hk. apply ByConfinement. exact (Hc p1 s b rest Heq Hk).
Qed.

(** The refined premise of Theorem B1r, each region with its own dead set. *)
Definition all_refined (f : flow) (P : plan) (k : nat) (evs : list event) : Prop :=
  forall p1 s b rest, evs = p1 ++ EAlloc s b :: rest -> P s = Some k ->
    exists D, refined_confined f (length p1) (nallocs p1) D /\ dead_at f (length p1) D.

(** ** Theorem B1r, the refined form. *)
Theorem theorem_B1r : forall P k,
  sole P k ->
  (forall n, exists f, flow_ok f /\ describes f (gen baseline n) /\
     separated_scopes P k (send f) (gen baseline n) /\ all_refined f P k (gen baseline n)) ->
  sound P.
Proof.
  intros P k Hs H. apply (sole_sound P k Hs). intros n.
  destruct (H n) as [f [Hf [Hd [Hsep Hc]]]]. right. exists f. split; [exact Hf|]. split; [exact Hd|]. split; [try exact Hsep|].
  intros p1 s b rest Heq Hk. destruct (Hc p1 s b rest Heq Hk) as [D [H1 H2]].
  exact (ByRefinement f _ _ D H1 H2).
Qed.

(** ** Corollary A3, arm overlap inside loops.  The slot is shared by an arm
    group, each region cleared by confinement or refined confinement.  The
    premise [separated_scopes] is stated for the slot, which is the first
    step of the prose proof, derived there from the instance hypothesis and
    the inner loops' exits. *)
Theorem corollary_A3 : forall P k,
  sole P k ->
  (forall n, exists f, flow_ok f /\ describes f (gen baseline n) /\
     separated_scopes P k (send f) (gen baseline n) /\
     (forall p1 s b rest, gen baseline n = p1 ++ EAlloc s b :: rest -> P s = Some k ->
        confined f (length p1) (nallocs p1) \/
        exists D, refined_confined f (length p1) (nallocs p1) D /\ dead_at f (length p1) D)) ->
  sound P.
Proof.
  intros P k Hs H. apply (sole_sound P k Hs). intros n.
  destruct (H n) as [f [Hf [Hd [Hsep Hc]]]]. right. exists f. split; [exact Hf|]. split; [exact Hd|]. split; [try exact Hsep|].
  intros p1 s b rest Heq Hk. destruct (Hc p1 s b rest Heq Hk) as [H1|[D [H1 H2]]].
  - exact (ByConfinement f _ _ H1).
  - exact (ByRefinement f _ _ D H1 H2).
Qed.

(** ** Theorem B2, universal reuse under the escape-copy discipline.  Any set
    of reused sites, each separated, in a machine whose [Escapes] executions
    copy every region reference, which [discipline] states for each region.
    The baseline is the same discipline machine under the empty plan. *)
Theorem theorem_B2 : forall P : plan,
  (forall n, exists f, flow_ok f /\ describes f (gen baseline n) /\
     (forall k, separated_scopes P k (send f) (gen baseline n)) /\
     (forall r, discipline f r)) ->
  sound P.
Proof.
  intros P H. apply lemma4_plan_sound. intros n k.
  destruct (H n) as [f [Hf [Hd [Hsep Hdisc]]]]. right. exists f. split; [exact Hf|]. split; [exact Hd|]. split; [try exact Hsep|].
  { apply Hsep. }
  intros p1 s b rest _ _. apply ByDiscipline. apply Hdisc.
Qed.

End Theorems.
