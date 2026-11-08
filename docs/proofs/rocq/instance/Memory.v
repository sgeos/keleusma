(** * Memory.v -- the core's memory behaves as increment B1's owner model.

    For every plan [Q] and every module, checked or not, the core run under
    [Q] and the oracle interpreter run under [ans_plan Q] agree.  The oracle
    [ans_plan Q] answers each dereference by increment B1's own [read] under
    [Q], on the B1 state reached by the events so far.  The two runs stop
    together, end in the same state apart from the memory, and the core's
    observation log is the projection of B1's observations of the events.

    The relation [Inv] is the heart of the matter.  The core's region table is
    B1's region table without the bytes, the epochs agree, the copy store is
    empty, and the core's memory at a cell holds the bytes of the region B1
    names as the cell's owner.  So the core's slot reallocation and B1's
    owner update are one operation, and a dereference returns the same
    outcome in both. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Sim.
From RegionReuse Require Trace.
From Instance Require Import Encode.
Import ListNotations.

Abbreviation tregs := (Trace.st_regs nat nat).
Abbreviation tepoch := (Trace.st_epoch nat nat).
Abbreviation towner := (Trace.st_owner nat nat).
Abbreviation tcopies := (Trace.st_copies nat nat).
Abbreviation rsite := (Trace.r_site nat nat).
Abbreviation repoch := (Trace.r_epoch nat nat).
Abbreviation rbytes := (Trace.r_bytes nat nat).

(** The core's memory is never read by the oracle interpreter, which keeps
    the empty memory. *)
Definition strip (s : state) : state :=
  mkstate (stk s) (locs s) (log s) (regs s) (fun _ => None) (epoch s).

(** The oracle that answers each dereference by increment B1's [read]. *)
Definition ans_plan (Q : plan) : oracle := fun tr x => to_ans (tread Q (tafter Q tr) x tt).

(** The core's observation log read off increment B1's observations. *)
Definition to_log (os : list tobs) : list ev :=
  flat_map (fun o => match o with
                     | Trace.OBytes _ _ b => [ReadE (Some b)]
                     | Trace.OStale _ _ => [ReadE None]
                     | Trace.OScalar _ _ n => [OutE n]
                     | Trace.ONone _ _ => []
                     end) os.

Definition tc (c : cell) : Trace.cell :=
  match c with Bump r => Trace.Bump r | Slot k => Trace.Slot k end.

Lemma tc_cell_of : forall (Q : plan) s r, tc (cell_of Q s r) = Trace.cell_of nat Q s r.
Proof. intros. unfold cell_of, Trace.cell_of. destruct (Q s); reflexivity. Qed.

Lemma tc_inj : forall a b, tc a = tc b -> a = b.
Proof. intros [a|a] [b|b] H; simpl in H; congruence. Qed.

(** A handle names a region of the table with the epoch it was made in. *)
Definition wfv (rg : list (nat * nat)) (v : val) : Prop :=
  match v with VH r e => exists site, nth_error rg r = Some (site, e) | VN _ => True end.

Lemma wfv_app : forall rg d v, wfv rg v -> wfv (rg ++ d) v.
Proof.
  intros rg d [n|r e] H; simpl in *; [exact I|]. destruct H as [site H]. exists site.
  rewrite nth_error_app1; [exact H|apply nth_error_Some; rewrite H; discriminate].
Qed.

Definition Inv (Q : plan) (s : state) (tr : list tevent) : Prop :=
  let B := tafter Q tr in
  map (fun g => (rsite g, repoch g)) (tregs B) = regs s /\
  tepoch B = epoch s /\
  tcopies B = [] /\
  (forall c, mem s c = match towner B (tc c) with
                       | Some r' => option_map rbytes (nth_error (tregs B) r')
                       | None => None
                       end) /\
  (forall c r', towner B c = Some r' -> r' < length (tregs B)) /\
  (forall v, In v (stk s) -> wfv (regs s) v) /\
  (forall j, wfv (regs s) (locs s j)) /\
  log s = to_log (tobs_of Q tr).

(** ** One event at a time. *)
Lemma after_snoc : forall Q tr e, tafter Q (tr ++ [e]) = snd (texec Q e (tafter Q tr)).
Proof.
  intros. rewrite Trace.after_app. simpl. destruct (texec Q e (tafter Q tr)). reflexivity.
Qed.

Lemma obs_snoc : forall Q tr e, tobs_of Q (tr ++ [e]) = tobs_of Q tr ++ [fst (texec Q e (tafter Q tr))].
Proof.
  intros. unfold Trace.obs_of. rewrite Trace.run_app. simpl. fold (tafter Q tr).
  destruct (texec Q e (tafter Q tr)). reflexivity.
Qed.

Lemma to_log_app : forall a b, to_log (a ++ b) = to_log a ++ to_log b.
Proof. intros. unfold to_log. apply flat_map_app. Qed.

Lemma read_shape : forall Q B x, tread Q B x tt = Trace.OStale _ _ \/
  exists b, tread Q B x tt = Trace.OBytes _ _ b.
Proof.
  intros Q B x. unfold Trace.read.
  repeat match goal with |- context [match ?e with _ => _ end] => destruct e end; eauto.
Qed.

Lemma read_log : forall Q B x, to_log [tread Q B x tt] = [ReadE (to_ans (tread Q B x tt))].
Proof. intros. destruct (read_shape Q B x) as [E|[b E]]; rewrite E; reflexivity. Qed.

(** The core's dereference is B1's read. *)
Lemma deref_read : forall Q s tr v, Inv Q s tr -> wfv (regs s) v ->
  deref Q s v = to_ans (tread Q (tafter Q tr) (src_of v) tt).
Proof.
  intros Q s tr v [Hr [He [Hc [Hm _]]]] Hw. destruct v as [n|r e]; simpl.
  - unfold Trace.read. rewrite Hc. reflexivity.
  - destruct Hw as [site Hw]. unfold Trace.read.
    rewrite <- Hr, nth_error_map in Hw.
    destruct (nth_error (tregs (tafter Q tr)) r) as [g|] eqn:Eg; [|discriminate].
    simpl in Hw. injection Hw as Hs Hep. rewrite He, Hep.
    destruct (Nat.eqb e (epoch s)); [|reflexivity].
    rewrite <- Hr, nth_error_map, Eg. simpl. rewrite Hm, tc_cell_of, Hs.
    destruct (towner _ _) as [r'|]; [|reflexivity].
    destruct (nth_error (tregs (tafter Q tr)) r'); reflexivity.
Qed.

(** An event that leaves B1's state alone. *)
Lemma inv_quiet : forall Q s s' tr e,
  Inv Q s tr -> snd (texec Q e (tafter Q tr)) = tafter Q tr ->
  regs s' = regs s -> mem s' = mem s -> epoch s' = epoch s ->
  log s' = log s ++ to_log [fst (texec Q e (tafter Q tr))] ->
  (forall v, In v (stk s') -> wfv (regs s) v) -> (forall j, wfv (regs s) (locs s' j)) ->
  Inv Q s' (tr ++ [e]).
Proof.
  intros Q s s' tr e [H1 [H2 [H3 [H4 [H5 [_ [_ H8]]]]]]] Hx Hr Hm He Hl Hs Hj.
  unfold Inv. rewrite after_snoc, Hx, obs_snoc, to_log_app, Hr, Hm, He, Hl, <- H8.
  repeat split; assumption.
Qed.

Lemma inv_with_stk : forall Q s tr l, Inv Q s tr -> (forall v, In v l -> wfv (regs s) v) ->
  Inv Q (with_stk s l) tr.
Proof.
  intros Q s tr l [H1 [H2 [H3 [H4 [H5 [_ [H7 H8]]]]]]] Hl. unfold Inv, with_stk. simpl.
  repeat split; assumption.
Qed.

Lemma pop_strip : forall s v s1, pop s = (v, s1) -> pop (strip s) = (v, strip s1).
Proof.
  intros s v s1 H. unfold pop, strip in *. simpl. destruct (stk s) as [|x l] eqn:Es; injection H as <- <-;
    rewrite ?Es; reflexivity.
Qed.

Lemma pop_wf : forall rg s v s1, (forall x, In x (stk s) -> wfv rg x) -> pop s = (v, s1) ->
  wfv rg v /\ (forall x, In x (stk s1) -> wfv rg x).
Proof.
  intros rg s v s1 H E. unfold pop in E. destruct (stk s) as [|y l] eqn:Es.
  - injection E as <- <-. split; [exact I|]. rewrite Es. intros x [].
  - injection E as <- <-. split; [apply H; left; reflexivity|]. simpl. intros x Hx. apply H. right. exact Hx.
Qed.

(** A pop leaves every field but the stack alone. *)
Lemma pop_quiet : forall s v s1, pop s = (v, s1) ->
  regs s1 = regs s /\ mem s1 = mem s /\ epoch s1 = epoch s /\ locs s1 = locs s /\ log s1 = log s.
Proof. exact pop_keeps. Qed.

Section MemSim.

Variable Q : plan.
Variable host : list ev -> nat.
Variable fs : list block.

Abbreviation A := (ans_plan Q).

Definition res_ok (o : ostate) (r : option state) (s : state) : Prop :=
  match r with
  | Some s' => ohalt o = false /\ ost o = strip s' /\ Inv Q s' (otr o) /\ exists d, regs s' = regs s ++ d
  | None => ohalt o = true
  end.

#[local] Arguments res_ok : simpl never.

Lemma quiet_step : forall tr, snd (texec Q TStep (tafter Q tr)) = tafter Q tr.
Proof. reflexivity. Qed.

Lemma noregs : forall s, exists d, regs s = regs s ++ d.
Proof. intros s. exists []. rewrite app_nil_r. reflexivity. Qed.

(** A step that appends [EStep] and changes no region state. *)
Lemma pure_ok : forall o s s' X,
  ohalt o = false -> Inv Q s (otr o) -> X = strip s' ->
  regs s' = regs s -> mem s' = mem s -> epoch s' = epoch s -> log s' = log s ->
  (forall v, In v (stk s') -> wfv (regs s) v) -> (forall j, wfv (regs s) (locs s' j)) ->
  res_ok (ostep o X TStep) (Some s') s.
Proof.
  intros o s s' X Hh Hi -> Hr Hm He Hl Hs Hj. unfold res_ok, ostep. simpl.
  split; [exact Hh|]. split; [reflexivity|]. split.
  - apply (inv_quiet Q s); try assumption; [reflexivity|]. rewrite Hl. simpl. rewrite app_nil_r. reflexivity.
  - exists []. rewrite app_nil_r. exact Hr.
Qed.

Lemma res_ext : forall o r s s0 d, regs s = regs s0 ++ d -> res_ok o r s -> res_ok o r s0.
Proof.
  intros o [s'|] s s0 d Hd H; [|exact H]. destruct H as [H1 [H2 [H3 [d' H4]]]].
  split; [exact H1|]. split; [exact H2|]. split; [exact H3|]. exists (d ++ d'). rewrite H4, Hd, app_assoc. reflexivity.
Qed.

Theorem mem_sim : forall n,
  (forall b s o, ohalt o = false -> ost o = strip s -> Inv Q s (otr o) ->
     res_ok (orun_b A host fs n b o) (run_b Q host fs n b s) s) /\
  (forall i s o, ohalt o = false -> ost o = strip s -> Inv Q s (otr o) ->
     res_ok (orun_i A host fs n i o) (run_i Q host fs n i s) s) /\
  (forall k body s o, ohalt o = false -> ost o = strip s -> Inv Q s (otr o) ->
     res_ok (orun_loop A host fs n k body o) (run_loop Q host fs n k body s) s).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; reflexivity. }
  split; [|split].
  - intros [|i r] s o Hh Hs Hi; simpl.
    + split; [exact Hh|]. split; [exact Hs|]. split; [exact Hi|apply noregs].
    + pose proof (IHi i s o Hh Hs Hi) as H1.
      destruct (run_i Q host fs n i s) as [s1|]; simpl in H1 |- *.
      * destruct H1 as [H1h [H1s [H1i [d Hd]]]]. rewrite H1h.
        exact (res_ext _ _ _ _ _ Hd (IHb r s1 _ H1h H1s H1i)).
      * rewrite H1. exact H1.
  - intros i s o Hh Hs Hi. pose proof Hi as [_ [_ [_ [_ [_ [Hws [Hwl _]]]]]]].
    destruct i as [kc|j|j| | | |t e|kc body|f| |site| | |]; simpl; try rewrite Hs.
    + apply pure_ok; try assumption; try reflexivity.
      intros v [<-|Hv]; [exact I|apply Hws; exact Hv].
    + apply pure_ok; try assumption; try reflexivity.
      intros v [<-|Hv]; [apply Hwl|apply Hws; exact Hv].
    + destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [Wv W1].
      apply pure_ok; try assumption; try reflexivity.
      intros j'. unfold store. simpl. destruct (Nat.eqb j' j); [exact Wv|rewrite L1; apply Hwl].
    + destruct (pop s) as [b s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      destruct (pop s1) as [a s2] eqn:Ep2. rewrite (pop_strip _ _ _ Ep2).
      destruct (pop_quiet _ _ _ Ep2) as [R2 [M2 [E2 [L2 G2]]]].
      rewrite <- R1 in W1. destruct (pop_wf _ _ _ _ W1 Ep2) as [_ W2].
      apply pure_ok; try reflexivity; simpl; try congruence.
      * intros v [<-|Hv]; [exact I|rewrite <- R1; apply W2; exact Hv].
      * intros j. rewrite L2, L1. apply Hwl.
    + destruct (pop s) as [b s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      destruct (pop s1) as [a s2] eqn:Ep2. rewrite (pop_strip _ _ _ Ep2).
      destruct (pop_quiet _ _ _ Ep2) as [R2 [M2 [E2 [L2 G2]]]].
      rewrite <- R1 in W1. destruct (pop_wf _ _ _ _ W1 Ep2) as [_ W2].
      apply pure_ok; try reflexivity; simpl; try congruence.
      * intros v [<-|Hv]; [exact I|rewrite <- R1; apply W2; exact Hv].
      * intros j. rewrite L2, L1. apply Hwl.
    + destruct (pop s) as [b s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      destruct (pop s1) as [a s2] eqn:Ep2. rewrite (pop_strip _ _ _ Ep2).
      destruct (pop_quiet _ _ _ Ep2) as [R2 [M2 [E2 [L2 G2]]]].
      rewrite <- R1 in W1. destruct (pop_wf _ _ _ _ W1 Ep2) as [_ W2].
      apply pure_ok; try reflexivity; simpl; try congruence.
      * intros v [<-|Hv]; [exact I|rewrite <- R1; apply W2; exact Hv].
      * intros j. rewrite L2, L1. apply Hwl.
    + (* If *)
      destruct (pop s) as [c s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      assert (Hi1 : Inv Q s1 (otr o ++ [TStep])).
      { apply (inv_quiet Q s); try assumption; try reflexivity;
          [rewrite G1; simpl; rewrite app_nil_r; reflexivity|intros j; rewrite L1; apply Hwl]. }
      apply (res_ext _ _ s1 s [] (eq_sym (eq_trans (app_nil_r _) (eq_sym R1)))).
      apply IHb; [exact Hh|reflexivity|exact Hi1].
    + (* LoopN *)
      assert (Hi1 : Inv Q s (otr o ++ [TStep])).
      { apply (inv_quiet Q s); try assumption; try reflexivity. simpl. rewrite app_nil_r. reflexivity. }
      apply IHl; [exact Hh|reflexivity|exact Hi1].
    + (* Call *)
      destruct (nth_error fs f) as [g|]; [|reflexivity].
      assert (Hi1 : Inv Q (with_stk s []) (otr o ++ [TStep])).
      { apply (inv_quiet Q s); try assumption; try reflexivity.
        - simpl. rewrite app_nil_r. reflexivity.
        - intros v []. }
      pose proof (IHb g (with_stk s []) (ostep o (with_stk (strip s) []) TStep) Hh eq_refl Hi1) as H1.
      destruct (run_b Q host fs n g (with_stk s [])) as [s2|]; simpl in H1 |- *.
      * destruct H1 as [H1h [H1s [H1i [d Hd]]]]. rewrite H1h. unfold res_ok. simpl.
        split; [reflexivity|]. split; [rewrite H1s; reflexivity|]. split.
        -- apply inv_with_stk; [exact H1i|]. intros v Hv. apply in_app_or in Hv as [Hv|Hv].
           ++ exact (proj1 (proj2 (proj2 (proj2 (proj2 (proj2 H1i))))) v Hv).
           ++ simpl in Hd. rewrite Hd. apply wfv_app. apply Hws. exact Hv.
        -- exists d. exact Hd.
      * rewrite H1. exact H1.
    + (* Yield *)
      unfold yield_step. destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep). simpl.
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      unfold res_ok, ostep. simpl. split; [exact Hh|]. split; [reflexivity|]. split.
      * apply (inv_quiet Q s); simpl; try assumption; try reflexivity.
        -- rewrite G1. reflexivity.
        -- intros x [<-|Hx]; [exact I|apply W1; exact Hx].
        -- intros j; rewrite L1; apply Hwl.
      * exists []. rewrite app_nil_r. exact R1.
    + (* New *)
      unfold new_step, onew. rewrite Hs. destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep). simpl.
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      destruct Hi as [I1 [I2 [I3 [I4 [I5 [_ [_ I8]]]]]]].
      assert (Hlen : length (tregs (tafter Q (otr o))) = length (regs s1)).
      { rewrite R1, <- I1, length_map. reflexivity. }
      unfold res_ok, ostep. simpl. split; [exact Hh|]. split; [reflexivity|]. split.
      * unfold Inv. rewrite after_snoc, obs_snoc, to_log_app. simpl.
        rewrite Hlen. repeat split.
        -- rewrite map_app, I1, R1, I2, E1. reflexivity.
        -- rewrite I2, E1. reflexivity.
        -- exact I3.
        -- intros c. rewrite <- tc_cell_of.
           destruct (cell_eq_dec c (cell_of Q site (length (regs s1)))) as [->|Hne].
           ++ destruct (Trace.cell_eq_dec _ _) as [_|C]; [|contradiction].
              rewrite nth_error_app2, <- Hlen, Nat.sub_diag by lia. reflexivity.
           ++ destruct (Trace.cell_eq_dec _ _) as [C|_]; [exfalso; exact (Hne (tc_inj _ _ C))|].
              rewrite M1, I4. destruct (towner (tafter Q (otr o)) (tc c)) as [r'|] eqn:Eo; [|reflexivity].
              rewrite nth_error_app1; [reflexivity|exact (I5 _ _ Eo)].
        -- intros c r' Ho. rewrite length_app. simpl.
           destruct (Trace.cell_eq_dec _ _) as [_|_]; [injection Ho as <-; lia|].
           pose proof (I5 _ _ Ho). lia.
        -- intros x [<-|Hx].
           ++ exists site. rewrite nth_error_app2, Nat.sub_diag by lia. reflexivity.
           ++ apply wfv_app. rewrite R1. apply W1. exact Hx.
        -- intros j. apply wfv_app. rewrite R1, L1. apply Hwl.
        -- rewrite G1, I8. simpl. rewrite app_nil_r. reflexivity.
      * exists [(site, epoch s1)]. rewrite R1. reflexivity.
    + (* Get *)
      unfold get_step, oget. rewrite Hs. destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep). simpl.
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [Wv W1].
      assert (Hs1 : Inv Q s1 (otr o)).
      { destruct Hi as [I1 [I2 [I3 [I4 [I5 [_ [_ I8]]]]]]].
        unfold Inv. rewrite R1, M1, E1, G1, L1. repeat split; assumption. }
      assert (Wv1 : wfv (regs s1) v) by (rewrite R1; exact Wv).
      change (ans_plan Q (otr o) (src_of v)) with (to_ans (tread Q (tafter Q (otr o)) (src_of v) tt)).
      rewrite <- (deref_read Q s1 (otr o) v Hs1 Wv1).
      unfold res_ok, ostep. simpl. split; [exact Hh|]. split; [reflexivity|]. split.
      * apply (inv_quiet Q s); simpl; try assumption; try reflexivity.
        -- rewrite G1, (deref_read Q s1 (otr o) v Hs1 Wv1).
           destruct (read_shape Q (tafter Q (otr o)) (src_of v)) as [E|[b E]]; rewrite E; reflexivity.
        -- intros x [<-|Hx]; [exact I|apply W1; exact Hx].
        -- intros j; rewrite L1; apply Hwl.
      * exists []. rewrite app_nil_r. exact R1.
    + (* Dup *)
      destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [Wv W1].
      apply pure_ok; try reflexivity; simpl; try congruence.
      * intros x [<-|[<-|Hx]]; [exact Wv|exact Wv|apply W1; exact Hx].
      * intros j. rewrite L1. apply Hwl.
    + (* Drop *)
      destruct (pop s) as [v s1] eqn:Ep. rewrite (pop_strip _ _ _ Ep).
      destruct (pop_quiet _ _ _ Ep) as [R1 [M1 [E1 [L1 G1]]]].
      destruct (pop_wf _ _ _ _ Hws Ep) as [_ W1].
      apply pure_ok; try reflexivity; try congruence; [exact W1|intros j; rewrite L1; apply Hwl].
  - intros [|k] body s o Hh Hs Hi; simpl.
    + split; [exact Hh|]. split; [exact Hs|]. split; [exact Hi|apply noregs].
    + pose proof (IHb body s o Hh Hs Hi) as H1.
      destruct (run_b Q host fs n body s) as [s1|]; simpl in H1 |- *.
      * destruct H1 as [H1h [H1s [H1i [d Hd]]]]. rewrite H1h.
        exact (res_ext _ _ _ _ _ Hd (IHl k body s1 _ H1h H1s H1i)).
      * rewrite H1. exact H1.
Qed.

End MemSim.

Lemma inv_reset : forall Q s tr, Inv Q s tr -> Inv Q (reset s) (tr ++ [TAdv]).
Proof.
  intros Q s tr [I1 [I2 [I3 [I4 [I5 [_ [I7 I8]]]]]]]. unfold Inv.
  rewrite after_snoc, obs_snoc, to_log_app. simpl. rewrite I2.
  repeat split; try assumption.
  - intros v [].
  - rewrite I8. simpl. rewrite app_nil_r. reflexivity.
Qed.

Lemma cycles_sim : forall Q host m fuel n s o,
  ohalt o = false -> ost o = strip s -> Inv Q s (otr o) ->
  match cycles Q host m fuel n s with
  | Some s' => ohalt (ocycles (ans_plan Q) host m fuel n o) = false /\
               ost (ocycles (ans_plan Q) host m fuel n o) = strip s' /\
               Inv Q s' (otr (ocycles (ans_plan Q) host m fuel n o))
  | None => ohalt (ocycles (ans_plan Q) host m fuel n o) = true
  end.
Proof.
  intros Q host m fuel n. induction n as [|n IH]; intros s o Hh Hs Hi; simpl.
  - split; [exact Hh|split; assumption].
  - pose proof (proj1 (mem_sim Q host (mfuncs m) fuel) (mmain m) s o Hh Hs Hi) as H1.
    destruct (run_b Q host (mfuncs m) fuel (mmain m) s) as [s1|]; simpl in H1 |- *.
    + destruct H1 as [H1h [H1s [H1i _]]]. rewrite H1h. apply IH.
      * exact H1h.
      * unfold oreset, ostep. simpl. rewrite H1s. reflexivity.
      * exact (inv_reset Q s1 _ H1i).
    + rewrite H1. exact H1.
Qed.

Lemma inv_init : forall Q, Inv Q init [].
Proof.
  intros Q. unfold Inv. simpl. repeat split; try reflexivity.
  - intros c r' H. discriminate.
  - intros v [].
Qed.

(** ** The correspondence.

    For every plan, every module, every host, every fuel and every number of
    cycles, the core run stops exactly when the oracle run under [ans_plan Q]
    halts, and otherwise the two end in the same state apart from the memory,
    with the core's log the projection of B1's observations of the events. *)
Theorem core_is_oracle : forall Q host m fuel n,
  match cycles Q host m fuel n init with
  | Some s => ohalt (orun_all (ans_plan Q) host m fuel n) = false /\
              ost (orun_all (ans_plan Q) host m fuel n) = strip s /\
              log s = to_log (tobs_of Q (otr (orun_all (ans_plan Q) host m fuel n)))
  | None => ohalt (orun_all (ans_plan Q) host m fuel n) = true
  end.
Proof.
  intros Q host m fuel n. unfold orun_all.
  pose proof (cycles_sim Q host m fuel n init o0 eq_refl eq_refl (inv_init Q)) as H.
  destruct (cycles Q host m fuel n init) as [s|]; [|exact H].
  destruct H as [H1 [H2 H3]]. split; [exact H1|]. split; [exact H2|].
  exact (proj2 (proj2 (proj2 (proj2 (proj2 (proj2 (proj2 H3))))))).
Qed.
