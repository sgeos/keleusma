(** * Encode.v -- the region core run as a sequence of increment B1 events.

    The region core of increment B2 is a big-step interpreter.  This module
    gives it a second, traced form, the oracle interpreter.  It executes the
    instructions of the core exactly as [Core.run_b] does, with two changes.

    - Every executed instruction appends one event of the abstract machine of
      [RegionReuse.Trace].  [New s] appends [EAlloc s b], where [b] is the
      scalar it stores.  [Get] appends [EDeref (SReg r) tt] for a handle to
      region [r], and [EDeref (SCopy 0) tt] for a scalar, which names the
      copy store.  No encoded run writes the copy store, so that dereference
      is stale under every plan, as the core's dereference of a scalar is.
      [Yield] appends [EOut n] for the yielded scalar [n], the reset appends
      [EAdvance], and every other instruction appends [EStep].
    - A dereference does not consult the memory.  Its outcome is supplied by
      an oracle, a function of the events so far and the source dereferenced.
      The oracle interpreter therefore depends on no plan, which is what an
      increment B1 machine must be.

    Running out of fuel sets a halt flag instead of discarding the run, so
    the events of a run that stops are kept.  The interpreter keeps a core
    state whose memory field it never writes. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core.
From RegionReuse Require Trace.
Import ListNotations.

(** ** The instantiation of increment B1's parameters.

    A site is a number, the bytes of a region are the scalar it stores, a
    view selects the whole of it, and a yielded value is a number. *)
Definition sel0 (_ : unit) (b : nat) : nat := b.

Abbreviation tevent := (Trace.event nat nat unit nat).
Abbreviation tobs := (Trace.obs nat nat).
Abbreviation tstate := (Trace.state nat nat).
Abbreviation TStep := (Trace.EStep nat nat unit nat).
Abbreviation TAlloc := (Trace.EAlloc nat nat unit nat).
Abbreviation TDeref := (Trace.EDeref nat nat unit nat).
Abbreviation TAdv := (Trace.EAdvance nat nat unit nat).
Abbreviation TOut := (Trace.EOut nat nat unit nat).
Abbreviation tobs_of := (Trace.obs_of nat nat unit nat sel0).
Abbreviation tafter := (Trace.after nat nat unit nat sel0).
Abbreviation tread := (Trace.read nat nat unit nat sel0).
Abbreviation texec := (Trace.exec nat nat unit nat sel0).
Abbreviation tnallocs := (Trace.nallocs nat nat unit nat).
Abbreviation tnadv := (Trace.nadv nat nat unit nat).

(** ** Bookkeeping over event lists. *)

(** [nder] counts the reads, the events whose observation is a dereference
    outcome. *)
Fixpoint nder (l : list tevent) : nat :=
  match l with
  | [] => 0
  | Trace.EDeref _ _ _ _ _ _ :: t => S (nder t)
  | Trace.ECopy _ _ _ _ _ _ :: t => S (nder t)
  | _ :: t => nder t
  end.

Lemma nder_app : forall a b, nder (a ++ b) = nder a + nder b.
Proof. induction a as [|e a IH]; intros b; [reflexivity|]. destruct e; simpl; rewrite ?IH; reflexivity. Qed.

(** [upto k l] is the longest prefix of [l] holding at most [k + 1] reads and
    ending at the read numbered [k + 1] if there is one. *)
Fixpoint upto (k : nat) (l : list tevent) : list tevent :=
  match l with
  | [] => []
  | e :: t =>
      match e with
      | Trace.EDeref _ _ _ _ _ _ | Trace.ECopy _ _ _ _ _ _ =>
          e :: match k with 0 => [] | S k' => upto k' t end
      | _ => e :: upto k t
      end
  end.

Lemma upto_app : forall a k b, k < nder a -> upto k (a ++ b) = upto k a.
Proof.
  induction a as [|e a IH]; intros k b H; simpl in H; [lia|].
  destruct e; simpl; try (rewrite IH by lia; reflexivity);
    (destruct k as [|k]; [reflexivity|rewrite IH by lia; reflexivity]).
Qed.

Lemma upto_prefix : forall l k, exists d, l = upto k l ++ d.
Proof.
  induction l as [|e l IH]; intros k; [exists []; reflexivity|].
  destruct e; simpl;
    try (destruct (IH k) as [d Hd]; exists d; rewrite <- Hd; reflexivity);
    (destruct k as [|k]; [exists l; reflexivity|destruct (IH k) as [d Hd]; exists d; rewrite <- Hd; reflexivity]).
Qed.

Lemma upto_nth : forall l i k, i < length l -> nder (firstn i l) <= k ->
  nth_error (upto k l) i = nth_error l i.
Proof.
  induction l as [|e l IH]; intros i k Hi Hk; simpl in Hi; [lia|].
  destruct i as [|i]; [destruct e; try reflexivity; destruct k; reflexivity|].
  simpl in Hk. destruct e; simpl;
    try (apply IH; [lia|exact Hk]);
    (destruct k as [|k]; [lia|apply IH; lia]).
Qed.

Lemma nder_firstn_le : forall l i, nder (firstn i l) <= nder l.
Proof.
  induction l as [|e l IH]; intros [|i]; simpl; try lia.
  destruct e; specialize (IH i); lia.
Qed.

(** A read whose prefix holds fewer reads than the first [i] events lies
    within them. *)
Lemma read_before : forall tr e rest i,
  nder tr < nder (firstn i (tr ++ e :: rest)) -> length tr < i.
Proof.
  intros tr e rest i H. destruct (Nat.lt_ge_cases (length tr) i) as [Hl|Hl]; [exact Hl|exfalso].
  rewrite firstn_app in H. replace (i - length tr) with 0 in H by lia.
  rewrite firstn_O, app_nil_r in H. pose proof (nder_firstn_le tr i). lia.
Qed.

(** ** Observations read back as dereference outcomes. *)
Definition to_ans (o : tobs) : option nat :=
  match o with Trace.OBytes _ _ b => Some b | _ => None end.

Definition answers (os : list tobs) : list (option nat) :=
  flat_map (fun o => match o with
                     | Trace.OBytes _ _ b => [Some b]
                     | Trace.OStale _ _ => [None]
                     | _ => []
                     end) os.

Lemma answers_app : forall a b, answers (a ++ b) = answers a ++ answers b.
Proof. intros. unfold answers. apply flat_map_app. Qed.

(** ** The oracle interpreter. *)
Definition oracle := list tevent -> Trace.src -> option nat.

Record ostate : Type := mko { ost : state; otr : list tevent; ohalt : bool }.

Definition halted (o : ostate) : ostate := mko (ost o) (otr o) true.
Definition ostep (o : ostate) (s : state) (e : tevent) : ostate := mko s (otr o ++ [e]) (ohalt o).

Definition src_of (v : val) : Trace.src :=
  match v with VH r _ => Trace.SReg r | VN _ => Trace.SCopy 0 end.

Section Oracle.

Variable A : oracle.
Variable host : list ev -> nat.

(** [New] as in [Core.new_step], with the memory left untouched. *)
Definition onew (site : nat) (o : ostate) : ostate :=
  let (v, s1) := pop (ost o) in
  ostep o (mkstate (VH (length (regs s1)) (epoch s1) :: stk s1) (locs s1) (log s1)
                   (regs s1 ++ [(site, epoch s1)]) (mem s1) (epoch s1))
        (TAlloc site (num v)).

(** [Get] as in [Core.get_step], the outcome supplied by the oracle. *)
Definition oget (o : ostate) : ostate :=
  let (v, s1) := pop (ost o) in
  let a := A (otr o) (src_of v) in
  ostep o (push (VN (match a with Some x => x | None => 0 end)) (note (ReadE a) s1))
        (TDeref (src_of v) tt).

Fixpoint orun_b (fs : list block) (n : nat) (b : block) (o : ostate) {struct n} : ostate :=
  match n with
  | 0 => halted o
  | S n' =>
      match b with
      | BNil => o
      | BCons i r => let o1 := orun_i fs n' i o in if ohalt o1 then o1 else orun_b fs n' r o1
      end
  end
with orun_i (fs : list block) (n : nat) (i : instr) (o : ostate) {struct n} : ostate :=
  match n with
  | 0 => halted o
  | S n' =>
      let s := ost o in
      match i with
      | Const k => ostep o (push (VN k) s) TStep
      | Load j => ostep o (push (locs s j) s) TStep
      | Store j => let (v, s1) := pop s in ostep o (store j v s1) TStep
      | Add => let (b, s1) := pop s in let (a, s2) := pop s1 in
               ostep o (push (VN (num a + num b)) s2) TStep
      | Sub => let (b, s1) := pop s in let (a, s2) := pop s1 in
               ostep o (push (VN (num a - num b)) s2) TStep
      | Lt => let (b, s1) := pop s in let (a, s2) := pop s1 in
              ostep o (push (VN (if Nat.ltb (num a) (num b) then 1 else 0)) s2) TStep
      | If t e => let (c, s1) := pop s in
                  orun_b fs n' (if Nat.eqb (num c) 0 then e else t) (ostep o s1 TStep)
      | LoopN k body => orun_loop fs n' k body (ostep o s TStep)
      | Call f =>
          match nth_error fs f with
          | Some g =>
              let o2 := orun_b fs n' g (ostep o (with_stk s []) TStep) in
              if ohalt o2 then o2
              else mko (with_stk (ost o2) (stk (ost o2) ++ stk s)) (otr o2) (ohalt o2)
          | None => halted o
          end
      | Yield => ostep o (yield_step host s) (TOut (num (fst (pop s))))
      | New site => onew site o
      | Get => oget o
      | Dup => let (v, s1) := pop s in ostep o (push v (push v s1)) TStep
      | Drop => let (_, s1) := pop s in ostep o s1 TStep
      end
  end
with orun_loop (fs : list block) (n k : nat) (body : block) (o : ostate) {struct n} : ostate :=
  match n with
  | 0 => halted o
  | S n' =>
      match k with
      | 0 => o
      | S k' => let o1 := orun_b fs n' body o in if ohalt o1 then o1 else orun_loop fs n' k' body o1
      end
  end.

(** The reset, which appends [EAdvance]. *)
Definition oreset (o : ostate) : ostate := ostep o (reset (ost o)) TAdv.

Fixpoint ocycles (m : module) (fuel n : nat) (o : ostate) : ostate :=
  match n with
  | 0 => o
  | S n' =>
      let o1 := orun_b (mfuncs m) fuel (mmain m) o in
      if ohalt o1 then o1 else ocycles m fuel n' (oreset o1)
  end.

(** ** Runs only append events. *)
Definition extends (o o' : ostate) : Prop := exists d, otr o' = otr o ++ d.

Lemma extends_refl : forall o, extends o o.
Proof. intros o. exists []. rewrite app_nil_r. reflexivity. Qed.

Lemma extends_trans : forall a b c, extends a b -> extends b c -> extends a c.
Proof. intros a b c [d1 H1] [d2 H2]. exists (d1 ++ d2). rewrite H2, H1, app_assoc. reflexivity. Qed.

Lemma extends_step : forall o s e, extends o (ostep o s e).
Proof. intros. exists [e]. reflexivity. Qed.

Lemma extends_halted : forall o, extends o (halted o).
Proof. intros o. exists []. rewrite app_nil_r. reflexivity. Qed.

Lemma orun_extends : forall fs n,
  (forall b o, extends o (orun_b fs n b o)) /\
  (forall i o, extends o (orun_i fs n i o)) /\
  (forall k body o, extends o (orun_loop fs n k body o)).
Proof.
  intros fs. induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; apply extends_halted. }
  split; [|split].
  - intros [|i r] o; simpl; [apply extends_refl|].
    destruct (ohalt (orun_i fs n i o)); [apply IHi|eapply extends_trans; [apply IHi|apply IHb]].
  - intros i o. destruct i as [k|j|j| | | |t e|k body|f| |site| | |]; simpl;
      try apply extends_step.
    + destruct (pop (ost o)). apply extends_step.
    + destruct (pop (ost o)) as [b s1]. destruct (pop s1). apply extends_step.
    + destruct (pop (ost o)) as [b s1]. destruct (pop s1). apply extends_step.
    + destruct (pop (ost o)) as [b s1]. destruct (pop s1). apply extends_step.
    + destruct (pop (ost o)) as [c s1]. eapply extends_trans; [apply extends_step|apply IHb].
    + eapply extends_trans; [apply extends_step|apply IHl].
    + destruct (nth_error fs f) as [g|]; [|apply extends_halted].
      destruct (ohalt _) eqn:E.
      * eapply extends_trans; [apply extends_step|apply IHb].
      * eapply extends_trans; [apply extends_step|]. apply (IHb g).
    + unfold onew. destruct (pop (ost o)). apply extends_step.
    + unfold oget. destruct (pop (ost o)). apply extends_step.
    + destruct (pop (ost o)). apply extends_step.
    + destruct (pop (ost o)). apply extends_step.
  - intros [|k] body o; simpl; [apply extends_refl|].
    destruct (ohalt (orun_b fs n body o)); [apply IHb|eapply extends_trans; [apply IHb|apply IHl]].
Qed.

Lemma ocycles_extends : forall m fuel n o, extends o (ocycles m fuel n o).
Proof.
  intros m fuel n. induction n as [|n IH]; intros o; simpl; [apply extends_refl|].
  destruct (ohalt _); [apply (proj1 (orun_extends (mfuncs m) fuel))|].
  eapply extends_trans; [apply (proj1 (orun_extends (mfuncs m) fuel))|].
  eapply extends_trans; [apply extends_step|apply IH].
Qed.

End Oracle.

(** The initial state of every run. *)
Definition o0 : ostate := mko init [] false.

(** The events of a whole run under oracle [A]. *)
Definition orun_all (A : oracle) (host : list ev -> nat) (m : module) (fuel n : nat) : ostate :=
  ocycles A host m fuel n o0.
