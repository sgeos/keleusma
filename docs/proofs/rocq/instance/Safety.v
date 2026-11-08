(** * Safety.v -- the checker discharges increment B1's per-slot obligation.

    For every module [region_check] accepts and every plan giving distinct
    sites distinct slots, every event sequence the oracle interpreter
    produces, under any oracle, meets [Trace.slot_safe] for every slot.  So
    at every read of a region allocated into a slot in the same epoch, no
    allocation since has targeted the slot.

    The argument is the checker's own.  [Live] says every handle on the stack
    was allocated at its tagged site with no write to its cell since.  At a
    [New s], the checker guarantees no handle of [s] is on the stack, and a
    different site's allocation writes a different cell, so [Live] survives.
    At a [Get], the handle read is on the stack, so [Live] gives the
    obligation.  No handle is stored, yielded or passed into a call, and a
    called function allocates nothing, so nothing outside the stack needs
    tracking.  The obligation does not depend on dereference outcomes, which
    is why it holds under every oracle. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim Reuse.
From RegionReuse Require Trace.
From Instance Require Import Encode.
Import ListNotations.

Abbreviation tslot_safe := (Trace.slot_safe nat nat unit nat).
Abbreviation tnowrite := (Trace.nowrite nat nat unit nat).
Abbreviation tis_read := (Trace.is_reg_read nat nat unit nat).

Definition Good (P : plan) (tr : list tevent) : Prop := forall k, tslot_safe P k tr.

(** [Dec P tr r s] says region [r] was allocated at site [s] in [tr], with
    no allocation since into its cell. *)
Definition Dec (P : plan) (tr : list tevent) (r s : nat) : Prop :=
  exists p1 b m, tr = p1 ++ TAlloc s b :: m /\ tnallocs p1 = r /\
                 tnowrite P (Trace.cell_of nat P s r) (S r) m.

Definition Live (P : plan) (st : state) (tr : list tevent) : Prop :=
  forall r e s, In (VH r e) (stk st) -> nth_error (regs st) r = Some (s, e) -> Dec P tr r s.

Definition Cnt (st : state) (tr : list tevent) : Prop := length (regs st) = tnallocs tr.

(** ** Facts about event lists. *)
Lemma nallocs_cons : forall e l, tnallocs (e :: l) = tnallocs [e] + tnallocs l.
Proof. intros e l. change (e :: l) with ([e] ++ l). apply Trace.nallocs_app. Qed.

Lemma alloc_unique : forall p1 p1' s s' b b' m m',
  p1 ++ TAlloc s b :: m = p1' ++ TAlloc s' b' :: m' -> tnallocs p1 = tnallocs p1' ->
  p1 = p1' /\ s = s' /\ m = m'.
Proof.
  induction p1 as [|e p1 IH]; intros [|e' p1'] s s' b b' m m' H Hn; simpl in H.
  - injection H as -> _ ->. auto.
  - injection H as <- H. simpl in Hn. lia.
  - injection H as -> H. simpl in Hn. lia.
  - injection H as <- H. rewrite (nallocs_cons e p1), (nallocs_cons e p1') in Hn.
    destruct (IH p1' s s' b b' m m' H ltac:(lia)) as [-> [-> ->]]. auto.
Qed.

Lemma nowrite_app : forall P c a b n,
  tnowrite P c n a -> tnowrite P c (n + tnallocs a) b -> tnowrite P c n (a ++ b).
Proof.
  intros P c a. induction a as [|e a IH]; intros b n Ha Hb; simpl in *; [rewrite Nat.add_0_r in Hb; exact Hb|].
  destruct e; simpl in *; try (apply IH; assumption).
  destruct Ha as [H1 H2]. split; [exact H1|]. apply IH; [exact H2|].
  replace (S n + tnallocs a) with (n + S (tnallocs a)) by lia. exact Hb.
Qed.

Lemma nowrite_noalloc : forall P c d n, tnallocs d = 0 -> tnowrite P c n d.
Proof.
  intros P c d. induction d as [|e d IH]; intros n H; simpl; [exact I|].
  destruct e; simpl in H; try (apply IH; exact H). discriminate.
Qed.

Lemma dec_quiet : forall P tr d r s, Dec P tr r s -> tnallocs d = 0 -> Dec P (tr ++ d) r s.
Proof.
  intros P tr d r s [p1 [b [m [-> [Hn Hw]]]]] Hd. exists p1, b, (m ++ d).
  split; [rewrite <- app_assoc; reflexivity|]. split; [exact Hn|].
  apply nowrite_app; [exact Hw|apply nowrite_noalloc; exact Hd].
Qed.

Lemma dec_alloc : forall P tr r s s' b',
  Dec P tr r s -> Trace.cell_of nat P s' (tnallocs tr) <> Trace.cell_of nat P s r ->
  Dec P (tr ++ [TAlloc s' b']) r s.
Proof.
  intros P tr r s s' b' [p1 [b [m [-> [Hn Hw]]]]] Hc. exists p1, b, (m ++ [TAlloc s' b']).
  split; [rewrite <- app_assoc; reflexivity|]. split; [exact Hn|].
  apply nowrite_app; [exact Hw|]. simpl. split; [|exact I].
  rewrite Trace.nallocs_app in Hc. simpl in Hc. rewrite Hn in Hc.
  replace (r + S (tnallocs m)) with (S r + tnallocs m) in Hc by lia. exact Hc.
Qed.

Lemma dec_new : forall P tr s b, Dec P (tr ++ [TAlloc s b]) (tnallocs tr) s.
Proof. intros. exists tr, b, []. repeat split. Qed.

Lemma cells_differ_t : forall P, injective P -> forall s0 r0 s rn, s0 <> s -> r0 < rn ->
  Trace.cell_of nat P s0 r0 <> Trace.cell_of nat P s rn.
Proof.
  intros P Pinj s0 r0 s rn Hs Hr. unfold Trace.cell_of.
  destruct (P s0) as [k0|] eqn:E0; destruct (P s) as [k|] eqn:E; try discriminate.
  - intros H. injection H as ->. exact (Hs (Pinj _ _ _ E0 E)).
  - intros H. injection H. lia.
Qed.

(** ** The obligation grows one event at a time. *)
Lemma good_snoc : forall P tr e, Good P tr ->
  (forall p1 s b m k, tr = p1 ++ TAlloc s b :: m -> P s = Some k -> tis_read e (tnallocs p1) ->
     tnadv m = 0 -> tnowrite P (Trace.Slot k) (S (tnallocs p1)) m) ->
  Good P (tr ++ [e]).
Proof.
  intros P tr e Hg Hn k p1 s b m e' q Heq Hk Hr Hm.
  destruct q as [|x q] using rev_ind.
  - assert (H : (p1 ++ TAlloc s b :: m) ++ [e'] = tr ++ [e])
      by (rewrite Heq, <- app_assoc; reflexivity).
    apply app_inj_tail in H as [H1 ->]. exact (Hn p1 s b m k (eq_sym H1) Hk Hr Hm).
  - assert (H : (p1 ++ TAlloc s b :: m ++ e' :: q) ++ [x] = tr ++ [e])
      by (rewrite Heq; repeat (rewrite <- app_assoc; simpl); reflexivity).
    apply app_inj_tail in H as [H1 _]. exact (Hg k p1 s b m e' q (eq_sym H1) Hk Hr Hm).
Qed.

Lemma good_quiet : forall P tr e, Good P tr -> (forall r, ~ tis_read e r) -> Good P (tr ++ [e]).
Proof. intros P tr e Hg Hn. apply good_snoc; [exact Hg|]. intros p1 s b m k _ _ Hr. exfalso. exact (Hn _ Hr). Qed.

Lemma good_nil : forall P, Good P [].
Proof. intros P k p1 s b m e q H. destruct p1; discriminate. Qed.

Lemma slot_safe_firstn : forall P k l N, tslot_safe P k l -> tslot_safe P k (firstn N l).
Proof.
  intros P k l N H p1 s b m e q Heq. apply (H p1 s b m e (q ++ skipn N l)).
  rewrite <- (firstn_skipn N l) at 1. rewrite Heq. repeat (rewrite <- app_assoc; simpl). reflexivity.
Qed.

(** ** Stack facts. *)
Lemma pop_m : forall st S t r v st1, matches st S -> tpop S = (t, r) -> pop st = (v, st1) ->
  tagm st t v /\ matches st1 r /\ regs st1 = regs st /\ epoch st1 = epoch st /\ locs st1 = locs st /\
  (forall x, In x (stk st1) -> In x (stk st)) /\ (forall r e, v = VH r e -> In v (stk st)).
Proof.
  intros st S t r v st1 Hm Ht Hp.
  destruct (pop_spec st S t r v st1 Hm Ht Hp) as [Htag [Hf [L1 [_ [R1 [_ [E1 Hin]]]]]]].
  split; [exact Htag|]. split.
  { unfold matches. eapply forall2_ext; [exact E1| |exact Hf]. intros r0 x H. rewrite R1. exact H. }
  split; [exact R1|]. split; [exact E1|]. split; [exact L1|]. split; [exact Hin|].
  intros r0 e0 ->. unfold pop in Hp. destruct (stk st) as [|y l]; [discriminate|].
  injection Hp as -> _. left. reflexivity.
Qed.

Lemma tagm_ext' : forall st st' t v, epoch st' = epoch st -> (exists d, regs st' = regs st ++ d) ->
  tagm st t v -> tagm st' t v.
Proof.
  intros st st' t v He [d Hd] H. eapply tagm_ext; [exact He| |exact H].
  intros r x Hx. rewrite Hd, nth_error_app1; [exact Hx|apply nth_error_Some; rewrite Hx; discriminate].
Qed.

Lemma matches_ext : forall st st' S l, epoch st' = epoch st -> (exists d, regs st' = regs st ++ d) ->
  Forall2 (tagm st) S l -> Forall2 (tagm st') S l.
Proof. intros st st' S l He Hd H. induction H; constructor; [eapply tagm_ext'; eassumption|assumption]. Qed.

(** A handle on a matched stack has its tag's site, which no extension of
    the region table changes. *)
Lemma site_of : forall st S l r e s rg, Forall2 (tagm st) S l -> In (VH r e) l ->
  nth_error (regs st ++ rg) r = Some (s, e) -> In (TR s) S /\ nth_error (regs st) r = Some (s, e).
Proof.
  intros st S l r e s rg Hm Hin Hn. destruct (in_handle_site st S l r e Hm Hin) as [s' [Hs Hn']].
  rewrite nth_error_app1 in Hn by (apply nth_error_Some; rewrite Hn'; discriminate).
  rewrite Hn' in Hn. injection Hn as <-. auto.
Qed.

Section Safety.

Variable A : oracle.
Variable host : list ev -> nat.
Variable fs : list block.
Variable ft : list (list tag).
Variable P : plan.
Hypothesis Pinj : injective P.
Hypothesis WF : forall f R g, nth_error ft f = Some R -> nth_error fs f = Some g ->
  shape_b ft false [] g = Some R /\ forallb is_ts R = true.

Definition SInv (S : list tag) (o : ostate) : Prop :=
  matches (ost o) S /\ locs_ok (ost o) /\ Live P (ost o) (otr o) /\ Cnt (ost o) (otr o).

Definition sres (reg : bool) (S' : list tag) (o o' : ostate) : Prop :=
  Good P (otr o') /\
  (ohalt o' = false ->
     SInv S' o' /\ epoch (ost o') = epoch (ost o) /\ (exists d, regs (ost o') = regs (ost o) ++ d) /\
     exists d, otr o' = otr o ++ d /\ (reg = false -> tnallocs d = 0)).

Lemma sres_compose : forall reg S1 S2 o o1 o2,
  sres reg S1 o o1 -> ohalt o1 = false -> sres reg S2 o1 o2 -> sres reg S2 o o2.
Proof.
  intros reg S1 S2 o o1 o2 [_ H1] Hh [G2 H2]. split; [exact G2|]. intros Hh2.
  destruct (H1 Hh) as [_ [E1 [[r1 R1] [d1 [D1 N1]]]]].
  destruct (H2 Hh2) as [I2 [E2 [[r2 R2] [d2 [D2 N2]]]]].
  split; [exact I2|]. split; [congruence|]. split; [exists (r1 ++ r2); rewrite R2, R1, app_assoc; reflexivity|].
  exists (d1 ++ d2). split; [rewrite D2, D1, app_assoc; reflexivity|].
  intros Hr. rewrite Trace.nallocs_app, (N1 Hr), (N2 Hr). reflexivity.
Qed.

Lemma sres_halted : forall reg S' o o', ohalt o' = true -> Good P (otr o') -> sres reg S' o o'.
Proof. intros reg S' o o' Hh Hg. split; [exact Hg|]. intros H. congruence. Qed.

(** A step appending an event that neither allocates nor reads a region. *)
Lemma quiet_sres : forall reg S' o s' e,
  ohalt o = false -> Live P (ost o) (otr o) -> Cnt (ost o) (otr o) -> Good P (otr o) ->
  matches s' S' -> locs_ok s' -> regs s' = regs (ost o) -> epoch s' = epoch (ost o) ->
  (forall v, In v (stk s') -> In v (stk (ost o)) \/ exists n, v = VN n) ->
  tnallocs [e] = 0 -> (forall r, ~ tis_read e r) ->
  sres reg S' o (ostep o s' e).
Proof.
  intros reg S' o s' e Hh Hl Hc Hg Hm Hlo Hr He Hs Hna Hnr. split; [apply good_quiet; assumption|].
  intros _. unfold ostep. simpl. split; [|split; [exact He|split]].
  - split; [exact Hm|]. split; [exact Hlo|]. split.
    + intros r e0 s Hin Hn. simpl in Hin, Hn |- *. destruct (Hs _ Hin) as [Hin'|[n Hv]]; [|discriminate].
      rewrite Hr in Hn. apply dec_quiet; [exact (Hl r e0 s Hin' Hn)|exact Hna].
    + unfold Cnt. simpl. rewrite Hr, Trace.nallocs_app, Hna, Nat.add_0_r. exact Hc.
  - exists []. simpl. rewrite app_nil_r. exact Hr.
  - exists [e]. split; [reflexivity|intros _; exact Hna].
Qed.

Ltac quiet := apply quiet_sres; try assumption; try reflexivity; try (simpl; congruence);
  try (intros ?r ?H; simpl in H; contradiction).

Theorem safe_sim : forall n,
  (forall b reg S S' o, shape_b ft reg S b = Some S' -> ohalt o = false -> SInv S o -> Good P (otr o) ->
     sres reg S' o (orun_b A host fs n b o)) /\
  (forall i reg S S' o, shape_i ft reg S i = Some S' -> ohalt o = false -> SInv S o -> Good P (otr o) ->
     sres reg S' o (orun_i A host fs n i o)) /\
  (forall k body reg S o, shape_b ft reg S body = Some S -> ohalt o = false -> SInv S o -> Good P (otr o) ->
     sres reg S o (orun_loop A host fs n k body o)).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; apply sres_halted; first [reflexivity|assumption]. }
  split; [|split].
  - intros [|i r] reg S S' o Hs Hh Hi Hg; simpl in Hs |- *.
    + injection Hs as <-. split; [exact Hg|]. intros _. split; [exact Hi|]. split; [reflexivity|].
      split; [exists []; rewrite app_nil_r; reflexivity|]. exists []. split; [rewrite app_nil_r; reflexivity|reflexivity].
    + destruct (shape_i ft reg S i) as [S1|] eqn:Ei; [|discriminate].
      pose proof (IHi i reg S S1 o Ei Hh Hi Hg) as H1.
      destruct (ohalt (orun_i A host fs n i o)) eqn:Eh; [apply sres_halted; [exact Eh|exact (proj1 H1)]|].
      destruct H1 as [G1 H1']. destruct (H1' Eh) as [I1 _].
      exact (sres_compose _ _ _ _ _ _ (conj G1 H1') Eh (IHb r reg S1 S' _ Hs Eh I1 G1)).
  - intros i reg S S' o Hs Hh Hi Hg. destruct Hi as [Hm [Hlo [Hl Hc]]].
    destruct i as [kc|j|j| | | |t e|kc body|f| |site| | |]; simpl in Hs |- *.
    + (* Const *)
      injection Hs as <-. quiet; [constructor; [exact I|exact Hm]|].
      intros v [<-|Hv]; [right; eexists; reflexivity|left; exact Hv].
    + (* Load *)
      injection Hs as <-. destruct (Hlo j) as [x Hx]. quiet.
      * unfold matches. simpl. rewrite Hx. constructor; [exact I|exact Hm].
      * intros v [<-|Hv]; [right; exists x; exact Hx|left; exact Hv].
    + (* Store *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [Htag [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      destruct v as [x|]; [|contradiction]. quiet.
      * intros j'. unfold store. simpl. destruct (Nat.eqb j' j); [eexists; reflexivity|rewrite L1; apply Hlo].
      * intros v Hv. left. exact (Hin1 v Hv).
    + (* Add *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (pop (ost o)) as [b s1] eqn:Ep. destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      destruct (pop s1) as [a s2] eqn:Ep2. destruct (pop_m _ _ _ _ _ _ Hm1 Et2 Ep2) as [_ [Hm2 [R2 [E2 [L2 [Hin2 _]]]]]].
      quiet.
      * constructor; [exact I|exact Hm2].
      * intros j. simpl. rewrite L2, L1. apply Hlo.
      * intros v [<-|Hv]; [right; eexists; reflexivity|left; apply Hin1, Hin2; exact Hv].
    + (* Sub *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (pop (ost o)) as [b s1] eqn:Ep. destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      destruct (pop s1) as [a s2] eqn:Ep2. destruct (pop_m _ _ _ _ _ _ Hm1 Et2 Ep2) as [_ [Hm2 [R2 [E2 [L2 [Hin2 _]]]]]].
      quiet.
      * constructor; [exact I|exact Hm2].
      * intros j. simpl. rewrite L2, L1. apply Hlo.
      * intros v [<-|Hv]; [right; eexists; reflexivity|left; apply Hin1, Hin2; exact Hv].
    + (* Lt *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (pop (ost o)) as [b s1] eqn:Ep. destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      destruct (pop s1) as [a s2] eqn:Ep2. destruct (pop_m _ _ _ _ _ _ Hm1 Et2 Ep2) as [_ [Hm2 [R2 [E2 [L2 [Hin2 _]]]]]].
      quiet.
      * constructor; [exact I|exact Hm2].
      * intros j. simpl. rewrite L2, L1. apply Hlo.
      * intros v [<-|Hv]; [right; eexists; reflexivity|left; apply Hin1, Hin2; exact Hv].
    + (* If *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (shape_b ft reg r t) as [T|] eqn:ET; [|discriminate].
      destruct (shape_b ft reg r e) as [E|] eqn:EE; [|discriminate].
      destruct (list_eq_dec tag_eq_dec T E) as [<-|]; [|discriminate]. injection Hs as <-.
      destruct (pop (ost o)) as [c s1] eqn:Ep. destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      assert (H1 : sres reg r o (ostep o s1 TStep)).
      { quiet. intros j; rewrite L1; apply Hlo. intros v Hv; left; exact (Hin1 v Hv). }
      destruct H1 as [G1 H1']. destruct (H1' Hh) as [I1 _].
      apply (sres_compose _ _ _ _ _ _ (conj G1 H1') Hh).
      destruct (Nat.eqb (num c) 0); [apply (IHb e reg r)|apply (IHb t reg r)]; assumption.
    + (* LoopN *)
      destruct (shape_b ft reg S body) as [S2|] eqn:Eb; [|discriminate].
      destruct (list_eq_dec tag_eq_dec S2 S) as [->|]; [|discriminate]. injection Hs as <-.
      assert (H1 : sres reg S o (ostep o (ost o) TStep)) by (quiet; intros v Hv; left; exact Hv).
      destruct H1 as [G1 H1']. destruct (H1' Hh) as [I1 _].
      apply (sres_compose _ _ _ _ _ _ (conj G1 H1') Hh). apply IHl; assumption.
    + (* Call *)
      destruct (nth_error ft f) as [R|] eqn:Ef; [|discriminate]. injection Hs as <-.
      destruct (nth_error fs f) as [g|] eqn:Eg; [|apply sres_halted; [reflexivity|exact Hg]].
      destruct (WF _ _ _ Ef Eg) as [Hsg Hts].
      set (o1 := ostep o (with_stk (ost o) []) TStep).
      assert (H1 : sres false [] o o1).
      { quiet. constructor. }
      destruct H1 as [G1 H1']. destruct (H1' Hh) as [I1 _].
      pose proof (IHb g false [] R o1 Hsg Hh I1 G1) as H2.
      destruct (ohalt (orun_b A host fs n g o1)) eqn:Eh2; [apply sres_halted; [exact Eh2|exact (proj1 H2)]|].
      destruct H2 as [G2 H2']. destruct (H2' Eh2) as [[Hm2 [Hlo2 [Hl2 Hc2]]] [E2 [[rg R2] [d [D2 N2]]]]].
      simpl in E2, R2, D2. split; [exact G2|]. intros _. simpl.
      split; [|split; [exact E2|split; [exists rg; exact R2|]]].
      * split; [|split; [exact Hlo2|split]].
        -- unfold matches. simpl. apply Forall2_app; [exact Hm2|].
           apply (matches_ext (ost o)); [exact E2|exists rg; exact R2|exact Hm].
        -- intros r e0 s Hin Hn. simpl in Hin, Hn. apply in_app_or in Hin as [Hin|Hin].
           ++ exfalso. exact (ts_no_handle _ R _ Hts Hm2 r e0 Hin).
           ++ rewrite R2 in Hn. destruct (site_of _ _ _ _ _ _ _ Hm Hin Hn) as [_ Hn'].
              rewrite D2, <- app_assoc. apply dec_quiet; [exact (Hl r e0 s Hin Hn')|].
              simpl. exact (N2 eq_refl).
        -- exact Hc2.
      * exists (TStep :: d). split; [rewrite D2, <- app_assoc; reflexivity|].
        intros _. rewrite nallocs_cons, (N2 eq_refl). reflexivity.
    + (* Yield *)
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      unfold yield_step. destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      quiet.
      * constructor; [exact I|exact Hm1].
      * intros j. simpl. rewrite L1. apply Hlo.
      * intros x [<-|Hx]; [right; eexists; reflexivity|left; exact (Hin1 x Hx)].
    + (* New *)
      destruct reg; [|discriminate].
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (in_dec tag_eq_dec (TR site) r) as [_|Hnot]; [discriminate|]. injection Hs as <-.
      unfold onew. destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      split; [apply good_quiet; [exact Hg|intros r0 []]|]. intros _. unfold ostep. simpl.
      assert (Hcnt : length (regs s1) = tnallocs (otr o)) by (rewrite R1; exact Hc).
      split; [|split; [exact E1|split; [exists [(site, epoch s1)]; rewrite R1; reflexivity|]]].
      * split; [|split; [intros j; simpl; rewrite L1; apply Hlo|split]].
        -- unfold matches. simpl. constructor.
           ++ simpl. rewrite nth_error_app2, Nat.sub_diag by lia. split; reflexivity.
           ++ apply (matches_ext s1); [reflexivity|eexists; reflexivity|exact Hm1].
        -- intros r0 e0 s Hin Hn. simpl in Hin, Hn. destruct Hin as [Heq|Hin].
           ++ injection Heq as <- <-. rewrite nth_error_app2, Nat.sub_diag in Hn by lia.
              simpl in Hn. injection Hn as <-. rewrite Hcnt. apply dec_new.
           ++ destruct (site_of _ _ _ _ _ _ _ Hm1 Hin Hn) as [Hts Hn'].
              apply dec_alloc.
              ** apply (Hl r0 e0 s (Hin1 _ Hin)). rewrite <- R1. exact Hn'.
              ** apply not_eq_sym. apply (cells_differ_t P Pinj).
                 --- intros ->. exact (Hnot Hts).
                 --- rewrite <- Hcnt. apply nth_error_Some. rewrite Hn'. discriminate.
        -- unfold Cnt. simpl. rewrite length_app, Trace.nallocs_app, Hcnt. simpl. lia.
      * exists [TAlloc site (num v)]. split; [reflexivity|discriminate].
    + (* Get *)
      destruct reg; [|discriminate].
      destruct (tpop S) as [[|s0] r] eqn:Et; [discriminate|]. injection Hs as <-.
      unfold oget. destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [Htag [Hm1 [R1 [E1 [L1 [Hin1 Hv]]]]]].
      destruct v as [x|rr e0]; simpl in Htag; [contradiction|]. destruct Htag as [Hn0 He0].
      split.
      * apply good_snoc; [exact Hg|]. intros p1 s b m k Htr Hk Hrd Hm0. simpl in Hrd. subst rr.
        destruct (Hl _ _ _ (Hv _ _ eq_refl) Hn0) as [p1' [b' [m' [Htr' [Hn' Hw]]]]].
        rewrite Htr in Htr'. destruct (alloc_unique _ _ _ _ _ _ _ _ Htr' (eq_sym Hn')) as [<- [-> ->]].
        unfold Trace.cell_of in Hw. rewrite Hk in Hw. exact Hw.
      * intros _. unfold ostep. simpl.
        split; [|split; [exact E1|split; [exists []; rewrite app_nil_r; exact R1|]]].
        -- split; [|split; [intros j; simpl; rewrite L1; apply Hlo|split]].
           ++ unfold matches. simpl. constructor; [exact I|exact Hm1].
           ++ intros r0 e1 s Hin Hn. simpl in Hin. destruct Hin as [Heq|Hin]; [discriminate|].
              apply dec_quiet; [|reflexivity]. apply (Hl r0 e1 s (Hin1 _ Hin)). rewrite <- R1. exact Hn.
           ++ unfold Cnt. simpl. rewrite Trace.nallocs_app, R1, Hc. simpl. lia.
        -- exists [TDeref (Trace.SReg rr) tt]. split; [reflexivity|reflexivity].
    + (* Dup *)
      destruct (tpop S) as [t r] eqn:Et. injection Hs as <-.
      destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [Htag [Hm1 [R1 [E1 [L1 [Hin1 Hv]]]]]].
      quiet.
      * assert (Ht1 : tagm s1 t v) by (apply (tagm_ext' (ost o)); [exact E1|exists []; rewrite app_nil_r; exact R1|exact Htag]).
        constructor; [exact Ht1|constructor; [exact Ht1|exact Hm1]].
      * intros j. simpl. rewrite L1. apply Hlo.
      * intros x Hx. simpl in Hx. destruct v as [nv|rv ev].
        -- destruct Hx as [<-|[<-|Hx]]; [right; eexists; reflexivity|right; eexists; reflexivity|left; exact (Hin1 x Hx)].
        -- destruct Hx as [<-|[<-|Hx]]; [left; exact (Hv _ _ eq_refl)|left; exact (Hv _ _ eq_refl)|left; exact (Hin1 x Hx)].
    + (* Drop *)
      injection Hs as <-. destruct (tpop S) as [t r] eqn:Et. simpl.
      destruct (pop (ost o)) as [v s1] eqn:Ep.
      destruct (pop_m _ _ _ _ _ _ Hm Et Ep) as [_ [Hm1 [R1 [E1 [L1 [Hin1 _]]]]]].
      quiet.
      * intros j. rewrite L1. apply Hlo.
      * intros x Hx. left. exact (Hin1 x Hx).
  - intros [|k] body reg S o Hs Hh Hi Hg; simpl.
    + split; [exact Hg|]. intros _. split; [exact Hi|]. split; [reflexivity|].
      split; [exists []; rewrite app_nil_r; reflexivity|]. exists []. split; [rewrite app_nil_r; reflexivity|reflexivity].
    + pose proof (IHb body reg S S o Hs Hh Hi Hg) as H1.
      destruct (ohalt (orun_b A host fs n body o)) eqn:Eh; [apply sres_halted; [exact Eh|exact (proj1 H1)]|].
      destruct H1 as [G1 H1']. destruct (H1' Eh) as [I1 _].
      exact (sres_compose _ _ _ _ _ _ (conj G1 H1') Eh (IHl k body reg S _ Hs Eh I1 G1)).
Qed.

End Safety.

(** ** Every run of a checked module meets the obligation. *)
Theorem checked_slot_safe : forall m ft, region_check m = Some ft ->
  forall P, injective P -> forall A host fuel n k,
  tslot_safe P k (otr (orun_all A host m fuel n)).
Proof.
  intros m ft Hc P Pinj A host fuel n k.
  destruct (region_check_wf m ft Hc) as [WF [S' Hmain]].
  assert (Hgen : forall n o, ohalt o = false -> stk (ost o) = [] -> locs_ok (ost o) ->
            Cnt (ost o) (otr o) -> Good P (otr o) -> Good P (otr (ocycles A host m fuel n o))).
  { induction n0 as [|n0 IH]; intros o Hh Hs Hl Hcn Hg; simpl; [exact Hg|].
    assert (Hi : SInv P [] o).
    { split; [unfold matches; rewrite Hs; constructor|]. split; [exact Hl|]. split; [|exact Hcn].
      intros r e s Hin. rewrite Hs in Hin. destruct Hin. }
    pose proof (proj1 (safe_sim A host (mfuncs m) ft P Pinj WF fuel) (mmain m) true [] S' o Hmain Hh Hi Hg)
      as [G1 H1].
    destruct (ohalt (orun_b A host (mfuncs m) fuel (mmain m) o)) eqn:Eh; [exact G1|].
    destruct (H1 eq_refl) as [[_ [Hl1 [_ Hc1]]] _].
    apply IH.
    - exact Eh.
    - reflexivity.
    - intros j. apply Hl1.
    - unfold Cnt, oreset, ostep. simpl. rewrite Trace.nallocs_app, <- Hc1. simpl. lia.
    - apply good_quiet; [exact G1|intros r []]. }
  apply (Hgen n o0 eq_refl eq_refl (fun j => ex_intro _ 0 eq_refl) eq_refl (good_nil P)).
Qed.
