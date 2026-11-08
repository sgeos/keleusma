(** * Clearance.v -- Lemmas 1, 2, 3, and the loop clause of Lemma 5.

    An abstract model of where references reside, over the positions of one
    baseline run, from [docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md].

    - [holds t l r] says location [l] holds a reference to region [r] just
      before the execution at position [t].
    - [ops t l] says location [l] is an operand of that execution, slot and
      storage contents included, which is Definition 6.
    - [writes t l] says the execution overwrites [l].
    - [escapes t] says the execution is [Escapes]-classified.
    - [death l] is the end of [l]'s lifetime, [None] for persistent and
      environment storage, which is Definition 4.
    - [send t] is the end of the execution's scope, which is Definition 3.
    - [alloc_of t] is the region the execution freshly allocates, if any.
    - [reads t r] says the execution dereferences region [r], by machine or by
      environment.

    The flow hypotheses below are the obligations an instantiation discharges.
    Each names the axioms of the prose it carries. *)

From Stdlib Require Import Arith Lia.

(** The model of one run. *)
Record flow : Type := {
  Loc : Type;
  holds : nat -> Loc -> nat -> Prop;
  ops : nat -> Loc -> Prop;
  writes : nat -> Loc -> Prop;
  escapes : nat -> Prop;
  death : Loc -> option nat;
  send : nat -> nat;
  alloc_of : nat -> option nat;
  reads : nat -> nat -> Prop
}.

(** [l] outlives time [e] when its lifetime extends past [e]. *)
Definition outlives (f : flow) (l : Loc f) (e : nat) : Prop :=
  match death f l with None => True | Some d => e < d end.

(** The flow hypotheses. *)
Record flow_ok (f : flow) : Prop := {
  (** The machine starts holding no reference. *)
  F_init : forall l r, ~ holds f 0 l r;
  (** A location holds a reference only while it is alive, Definition 4. *)
  F_alive : forall t l r, holds f t l r -> outlives f l t;
  (** Provenance, M3(i) with M9 and M7.  A holding persists unless its location
      is overwritten, and an overwrite places a reference only by transporting
      or deriving it from an operand, or by allocating it fresh. *)
  F_prov : forall t l r, holds f (S t) l r ->
    (~ writes f t l /\ holds f t l r) \/
    (writes f t l /\ ((exists l', ops f t l' /\ holds f t l' r) \/ alloc_of f t = Some r));
  (** Exhaustiveness, M3(v).  Placing a reference into a location that
      outlives the execution's scope is an [Escapes] execution. *)
  F_exh : forall t l r,
    writes f t l -> holds f (S t) l r -> outlives f l (send f t) -> escapes f t;
  (** M3(vi) with M6.  An allocation's fresh reference lands on its own
      frame's operand stack, an entry that dies within the allocation's scope. *)
  F_alloc : forall t l r,
    alloc_of f t = Some r -> writes f t l -> holds f (S t) l r -> ~ outlives f l (send f t);
  (** Regions are fresh, M3(vi) with M1. *)
  F_fresh : forall t r, alloc_of f t = Some r -> forall t' l, t' <= t -> ~ holds f t' l r;
  F_inj : forall t t' r, alloc_of f t = Some r -> alloc_of f t' = Some r -> t = t';
  (** Dynamic enclosure, Definition 3 with M6(c).  An execution during the
      scope of an allocation has a scope ending no later, and a scope ends
      after the execution it belongs to. *)
  F_nest : forall j t, alloc_of f j <> None -> j <= t -> t < send f j -> send f t <= send f j;
  F_after : forall t, t < send f t;
  (** Every dereference reads its handle from an operand, Definition 2 with M7. *)
  F_read : forall t r, reads f t r -> exists l, ops f t l /\ holds f t l r
}.

Section Clearance.

Variable f : flow.
Hypothesis Hf : flow_ok f.

Local Abbreviation Loc := (Loc f).
Local Abbreviation holds := (holds f).
Local Abbreviation ops := (ops f).
Local Abbreviation writes := (writes f).
Local Abbreviation escapes := (escapes f).
Local Abbreviation send := (send f).
Local Abbreviation alloc_of := (alloc_of f).
Local Abbreviation reads := (reads f).
Local Abbreviation outlives := (outlives f).
Local Abbreviation F_init := (F_init f Hf).
Local Abbreviation F_alive := (F_alive f Hf).
Local Abbreviation F_prov := (F_prov f Hf).
Local Abbreviation F_exh := (F_exh f Hf).
Local Abbreviation F_alloc := (F_alloc f Hf).
Local Abbreviation F_fresh := (F_fresh f Hf).
Local Abbreviation F_inj := (F_inj f Hf).
Local Abbreviation F_nest := (F_nest f Hf).
Local Abbreviation F_after := (F_after f Hf).
Local Abbreviation F_read := (F_read f Hf).

Lemma not_outlives_mono : forall l e e', ~ outlives l e -> e <= e' -> ~ outlives l e'.
Proof.
  unfold Clearance.outlives. intros l e e' H Hle. destruct (death f l) as [d|]; [|contradiction].
  intros Hd. apply H. lia.
Qed.

(** Lifetimes are given as numbers, so outliving is decidable without any
    classical axiom. *)
Lemma outlives_dec : forall l e, {outlives l e} + {~ outlives l e}.
Proof.
  intros l e. unfold outlives, Clearance.outlives. destruct (death f l) as [d|]; [|left; exact I].
  destruct (lt_dec e d); [left|right]; assumption.
Qed.

(** ** Lemma 1, provenance closure.

    Every holding of a reference to [r] arises after [r]'s allocation. *)
Lemma lemma1_provenance : forall t l r, holds t l r -> exists j, j < t /\ alloc_of j = Some r.
Proof.
  induction t as [|t IH]; intros l r H; [exfalso; exact (F_init l r H)|].
  destruct (F_prov t l r H) as [[_ H1]|[_ [[l' [_ H1]]|H1]]].
  - destruct (IH l r H1) as [j [Hj Ha]]. exists j. split; [lia|assumption].
  - destruct (IH l' r H1) as [j [Hj Ha]]. exists j. split; [lia|assumption].
  - exists t. split; [lia|assumption].
Qed.

(** ** The clearance core.

    Fix the allocation of [r] at position [j] and a set [D] of excused
    locations.  [excused] is the one hypothesis on [Escapes] executions.  An
    [Escapes] execution that has a reference to [r] among its operands places
    it into an outliving location only inside [D].  [dead] is Definition 7.
    Every read of a [D] location at or after the scope end is preceded by an
    overwrite of it at or after that end. *)
Section Core.

Variables (j r : nat) (D : Loc -> Prop).
Hypothesis Halloc : alloc_of j = Some r.
Hypothesis excused : forall t, j <= t -> escapes t ->
  (exists l', ops t l' /\ holds t l' r) ->
  forall l, writes t l -> holds (S t) l r -> outlives l (send t) -> D l.
Hypothesis dead : forall l, D l -> forall t, send j <= t -> ops t l ->
  exists w, send j <= w /\ w < t /\ writes w l.

(** Within the scope, every holder is excused or dies by the scope's end. *)
Lemma within_scope : forall t, j <= t -> t <= send j ->
  forall l, holds t l r -> D l \/ ~ outlives l (send j).
Proof.
  intros t Hjt. induction Hjt as [|t Hjt IH]; intros Hts l Hh.
  - exfalso. exact (F_fresh j r Halloc j l (le_n j) Hh).
  - assert (Hnest : send t <= send j)
      by (apply (F_nest j t); [rewrite Halloc; discriminate|assumption|lia]).
    destruct (F_prov t l r Hh) as [[_ H1]|[Hw [Hop|Hal]]].
    + apply IH; [lia|assumption].
    + destruct (outlives_dec l (send t)) as [Ho|Hno].
      * left. exact (excused t Hjt (F_exh t l r Hw Hh Ho) Hop l Hw Hh Ho).
      * right. eapply not_outlives_mono; eassumption.
    + right. eapply not_outlives_mono; [|exact Hnest]. eapply F_alloc; eassumption.
Qed.

(** At the scope's end, only excused locations hold a reference. *)
Lemma at_scope_end : forall l, holds (send j) l r -> D l.
Proof.
  intros l Hh. pose proof (F_after j) as Ha.
  destruct (within_scope (send j) ltac:(lia) (le_n _) l Hh) as [Hd|Hno]; [exact Hd|].
  exfalso. exact (Hno (F_alive _ _ _ Hh)).
Qed.

(** After the scope's end, a holder is an excused location not yet
    overwritten since that end. *)
Lemma after_scope : forall t, send j <= t -> forall l, holds t l r ->
  D l /\ forall w, send j <= w -> w < t -> ~ writes w l.
Proof.
  intros t Ht. induction Ht as [|t Ht IH]; intros l Hh.
  - split; [exact (at_scope_end l Hh)|]. intros w H1 H2. lia.
  - destruct (F_prov t l r Hh) as [[Hnw H1]|[Hw [[l' [Hop H1]]|Hal]]].
    + destruct (IH l H1) as [Hd Hnw']. split; [exact Hd|].
      intros w H2 H3. destruct (Nat.eq_dec w t) as [->|Hne]; [exact Hnw|].
      apply Hnw'; lia.
    + exfalso. destruct (IH l' H1) as [Hd Hnw'].
      destruct (dead l' Hd t Ht Hop) as [w [H2 [H3 H4]]].
      exact (Hnw' w H2 H3 H4).
    + exfalso. pose proof (F_inj t j r Hal Halloc) as ->. pose proof (F_after j). lia.
Qed.

(** No dereference of [r] occurs at or after the scope's end. *)
Theorem no_read_after : forall t, send j <= t -> ~ reads t r.
Proof.
  intros t Ht Hrd. destruct (F_read t r Hrd) as [l [Hop Hh]].
  destruct (after_scope t Ht l Hh) as [Hd Hnw].
  destruct (dead l Hd t Ht Hop) as [w [H1 [H2 H3]]].
  exact (Hnw w H1 H2 H3).
Qed.

End Core.

(** ** Lemma 2, confinement clearance.

    Definition 6's confinement, no [Escapes] execution ever has a reference to
    [r] among its operands.  Then no location holds a reference to [r] at the
    scope's end, and no dereference of [r] occurs at or after it. *)
Definition confined (j r : nat) : Prop :=
  forall t, j <= t -> escapes t -> forall l, ops t l -> ~ holds t l r.

Theorem lemma2_confinement_clearance : forall j r,
  alloc_of j = Some r -> confined j r ->
  (forall l, ~ holds (send j) l r) /\ (forall t, send j <= t -> ~ reads t r).
Proof.
  intros j r Ha Hc.
  assert (Hex : forall t, j <= t -> escapes t -> (exists l', ops t l' /\ holds t l' r) ->
                forall l, writes t l -> holds (S t) l r -> outlives l (send t) -> False).
  { intros t Hjt He [l' [Hop Hh]]. exfalso. exact (Hc t Hjt He l' Hop Hh). }
  split.
  - intros l Hh. exact (at_scope_end j r (fun _ => False) Ha
                          (fun t a b c l d e f => Hex t a b c l d e f) l Hh).
  - intros t Ht. apply (no_read_after j r (fun _ => False) Ha
                          (fun t a b c l d e f => Hex t a b c l d e f)); [|assumption].
    intros l [].
Qed.

(** ** Lemma 3, refined clearance.

    Definition 8, generalised to any set of excused locations.  An [Escapes]
    execution with a reference to [r] among its operands places it only into
    a location of [D], and Definition 7, each location of [D] is overwritten
    before it is read at or after the scope's end.  Then no dereference of [r]
    occurs at or after the scope's end. *)
Definition refined_confined (j r : nat) (D : Loc -> Prop) : Prop :=
  forall t, j <= t -> escapes t -> (exists l', ops t l' /\ holds t l' r) ->
  forall l, writes t l -> holds (S t) l r -> D l.

Definition dead_at (j : nat) (D : Loc -> Prop) : Prop :=
  forall l, D l -> forall t, send j <= t -> ops t l -> exists w, send j <= w /\ w < t /\ writes w l.

Theorem lemma3_refined_clearance : forall j r D,
  alloc_of j = Some r -> refined_confined j r D -> dead_at j D ->
  forall t, send j <= t -> ~ reads t r.
Proof.
  intros j r D Ha Hrc Hd. apply (no_read_after j r D Ha); [|exact Hd].
  intros t Hjt He Hop l Hw Hh _. exact (Hrc t Hjt He Hop l Hw Hh).
Qed.

(** ** Lemma 5, the loop clause, unconditional clearance.

    Definition 9's discipline, for region references.  An [Escapes] execution
    transports a fresh copy in place of a region reference, so it never places
    a region reference into an outliving location.  No confinement is needed.
    The cycle clause, that no dereference across the boundary succeeds, is the
    epoch check of [Trace.v]. *)
Definition discipline (r : nat) : Prop :=
  forall t, escapes t -> forall l, writes t l -> holds (S t) l r -> ~ outlives l (send t).

Theorem lemma5_unconditional_clearance : forall j r,
  alloc_of j = Some r -> discipline r ->
  (forall l, ~ holds (send j) l r) /\ (forall t, send j <= t -> ~ reads t r).
Proof.
  intros j r Ha Hdisc.
  assert (Hex : forall t, j <= t -> escapes t -> (exists l', ops t l' /\ holds t l' r) ->
                forall l, writes t l -> holds (S t) l r -> outlives l (send t) -> False).
  { intros t _ He _ l Hw Hh Ho. exact (Hdisc t He l Hw Hh Ho). }
  split.
  - intros l Hh. exact (at_scope_end j r (fun _ => False) Ha
                          (fun t a b c l d e f => Hex t a b c l d e f) l Hh).
  - intros t Ht. apply (no_read_after j r (fun _ => False) Ha
                          (fun t a b c l d e f => Hex t a b c l d e f)); [|assumption].
    intros l [].
Qed.

End Clearance.
