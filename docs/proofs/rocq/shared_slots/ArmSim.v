(** * ArmSim.v -- the simulation for the join checker.

    [simulation_arm] is [SharedSim.simulation_shared] for [ArmShape], derived
    from it by substitution and then changed at four places.  An abstract
    handle records a set of possible sites, so the stack relation [tagmA]
    says a handle belongs to one of them.  At [If] the result is the join of
    the arms, which [ajoin_sound_l] and [ajoin_sound_r] show holds of either
    arm's stack.  At [New] a live entry conflicts if any of its possible sites
    shares the allocation's slot.  At [Call] the callee is simulated by
    [simulation_shared], whose scalar result shape embeds into the new tags.
    B2's lemmas about pops and memory correspondence are reused unchanged. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim.
From SharedSlots Require Import SharedShape SharedSim ArmShape.
Import ListNotations.

Lemma existsb_false_all_a : forall (f : atag -> bool) l, existsb f l = false -> forall x, In x l -> f x = false.
Proof.
  intros f l. induction l as [|y l IH]; intros H x Hx; [contradiction|].
  simpl in H. apply orb_false_iff in H as [H1 H2]. destruct Hx as [<-|Hx]; [exact H1|exact (IH H2 x Hx)].
Qed.

Lemma existsb_false_all_n : forall (f : nat -> bool) l, existsb f l = false -> forall x, In x l -> f x = false.
Proof.
  intros f l. induction l as [|y l IH]; intros H x Hx; [contradiction|].
  simpl in H. apply orb_false_iff in H as [H1 H2]. destruct Hx as [<-|Hx]; [exact H1|exact (IH H2 x Hx)].
Qed.

Section ArmSim.

Variable P : plan.
Variable host : list ev -> nat.
Variable fs : list block.
Variable ft : list (list tag).
Hypothesis WF : forall f R g, nth_error ft f = Some R -> nth_error fs f = Some g ->
  shape_b ft false [] g = Some R /\ forallb is_ts R = true.

Local Abbreviation rel := (Sim.rel P).
Local Abbreviation rel_push_vn := (Sim.rel_push_vn P).
Local Abbreviation rel_note := (Sim.rel_note P).

Definition tagmA (st : state) (t : atag) (v : val) : Prop :=
  match t, v with
  | AS, VN _ => True
  | AR l, VH r e => exists s, In s l /\ nth_error (regs st) r = Some (s, e) /\ e = epoch st
  | _, _ => False
  end.

Definition matchesA (st : state) (S : list atag) : Prop := Forall2 (tagmA st) S (stk st).

Definition sim_resA (S' : list atag) (op ob : option state) : Prop :=
  match op, ob with
  | Some sp', Some sb' => rel sp' sb' /\ matchesA sb' S' /\ locs_ok sb'
  | None, None => True
  | _, _ => False
  end.

Lemma tagmA_ext : forall st st' t v,
  epoch st' = epoch st -> (forall r x, nth_error (regs st) r = Some x -> nth_error (regs st') r = Some x) ->
  tagmA st t v -> tagmA st' t v.
Proof.
  intros st st' [|l] [n|r e] He Hr H; simpl in *; try exact I; try contradiction.
  destruct H as [s [H1 [H2 H3]]]. exists s. split; [exact H1|split; [apply Hr; exact H2|congruence]].
Qed.

Lemma forall2A_ext : forall st st' S l,
  epoch st' = epoch st -> (forall r x, nth_error (regs st) r = Some x -> nth_error (regs st') r = Some x) ->
  Forall2 (tagmA st) S l -> Forall2 (tagmA st') S l.
Proof. intros st st' S l He Hr H. induction H; constructor; [eapply tagmA_ext; eassumption|assumption]. Qed.

Lemma popA_spec : forall st S t r v st1,
  matchesA st S -> apop S = (t, r) -> pop st = (v, st1) ->
  tagmA st t v /\ Forall2 (tagmA st) r (stk st1) /\
  locs st1 = locs st /\ log st1 = log st /\ regs st1 = regs st /\ mem st1 = mem st /\
  epoch st1 = epoch st /\ (forall x, In x (stk st1) -> In x (stk st)).
Proof.
  intros st S t r v st1 Hm Ht Hp. unfold matchesA, pop in *.
  destruct (stk st) as [|x l] eqn:Es.
  - injection Hp as <- <-. inversion Hm; subst. simpl in Ht. injection Ht as <- <-.
    rewrite Es. repeat split; try reflexivity; try constructor. intros x [].
  - injection Hp as <- <-. inversion Hm as [|t0 v0 S0 l0 H1 H2]; subst.
    simpl in Ht. injection Ht as <- <-. simpl. repeat split; try assumption; try reflexivity.
    intros y Hy. right. exact Hy.
Qed.

Lemma step_popA : forall sp sb S t r, rel sp sb -> matchesA sb S -> locs_ok sb -> apop S = (t, r) ->
  exists v sp1 sb1, pop sp = (v, sp1) /\ pop sb = (v, sb1) /\ rel sp1 sb1 /\ tagmA sb t v /\
    matchesA sb1 r /\ locs_ok sb1 /\ regs sb1 = regs sb /\ epoch sb1 = epoch sb /\
    mem sb1 = mem sb /\ mem sp1 = mem sp /\ (forall x, In x (stk sb1) -> In x (stk sb)).
Proof.
  intros sp sb S t r Hrel Hm Hl Ht. destruct (Sim.pop_rel P sp sb Hrel) as [Hv Hrel1].
  destruct (pop sp) as [vp sp1] eqn:Ep. destruct (pop sb) as [vb sb1] eqn:Eb. simpl in Hv, Hrel1. subst vp.
  destruct (popA_spec sb S t r vb sb1 Hm Ht Eb) as [Htag [Hf [L1 [G1 [R1 [M1 [E1 Hin]]]]]]].
  destruct (pop_keeps _ _ _ Ep) as [_ [Mp _]].
  exists vb, sp1, sb1. split; [reflexivity|]. split; [reflexivity|]. split; [exact Hrel1|].
  split; [exact Htag|]. split.
  { unfold matchesA. eapply forall2A_ext; [exact E1| |exact Hf]. intros r0 x H. rewrite R1. exact H. }
  split; [intros j; rewrite L1; apply Hl|].
  split; [exact R1|]. split; [exact E1|]. split; [exact M1|]. split; [exact Mp|exact Hin].
Qed.

Lemma matchesA_push_vn : forall st S n, matchesA st S -> matchesA (push (VN n) st) (AS :: S).
Proof. intros st S n H. unfold matchesA, push, with_stk. simpl. constructor; [exact I|exact H]. Qed.

Lemma in_handle_siteA : forall st S l r e, Forall2 (tagmA st) S l -> In (VH r e) l ->
  exists L s, In (AR L) S /\ In s L /\ nth_error (regs st) r = Some (s, e).
Proof.
  intros st S l r e H. induction H as [|t v S l Ht H IH]; intros Hin; [contradiction|].
  destruct Hin as [Heq|Hin].
  - subst v. destruct t as [|L]; simpl in Ht; [contradiction|]. destruct Ht as [s [H1 [H2 _]]].
    exists L, s. split; [left; reflexivity|split; assumption].
  - destruct (IH Hin) as [L [s [H1 H2]]]. exists L, s. split; [right; exact H1|exact H2].
Qed.

Lemma ajoin_sound_l : forall st T E J l, ajoin T E = Some J -> Forall2 (tagmA st) T l -> Forall2 (tagmA st) J l.
Proof.
  intros st T. induction T as [|a T IH]; intros E J l Hj Hf.
  - destruct E; simpl in Hj; [injection Hj as <-; exact Hf|discriminate].
  - destruct E as [|b E]; [destruct a; discriminate|]. simpl in Hj.
    inversion Hf as [|t v T0 l0 Ht Hr]; subst.
    destruct a as [|l1]; destruct b as [|l2]; try discriminate.
    + destruct (ajoin T E) as [J1|] eqn:EJ; simpl in Hj; [|discriminate]. injection Hj as <-.
      constructor; [exact Ht|exact (IH E J1 l0 EJ Hr)].
    + destruct (ajoin T E) as [J1|] eqn:EJ; simpl in Hj; [|discriminate]. injection Hj as <-.
      constructor; [|exact (IH E J1 l0 EJ Hr)].
      destruct v as [n|r e]; simpl in Ht |- *; [contradiction|]. destruct Ht as [s [H1 H2]].
      exists s. split; [|exact H2]. destruct (list_eq_dec Nat.eq_dec l1 l2); [exact H1|apply in_or_app; left; exact H1].
Qed.

Lemma ajoin_sound_r : forall st T E J l, ajoin T E = Some J -> Forall2 (tagmA st) E l -> Forall2 (tagmA st) J l.
Proof.
  intros st T. induction T as [|a T IH]; intros E J l Hj Hf.
  - destruct E; simpl in Hj; [injection Hj as <-; exact Hf|discriminate].
  - destruct E as [|b E]; [destruct a; discriminate|]. simpl in Hj.
    inversion Hf as [|t v E0 l0 Ht Hr]; subst.
    destruct a as [|l1]; destruct b as [|l2]; try discriminate.
    + destruct (ajoin T E) as [J1|] eqn:EJ; simpl in Hj; [|discriminate]. injection Hj as <-.
      constructor; [exact Ht|exact (IH E J1 l0 EJ Hr)].
    + destruct (ajoin T E) as [J1|] eqn:EJ; simpl in Hj; [|discriminate]. injection Hj as <-.
      constructor; [|exact (IH E J1 l0 EJ Hr)].
      destruct v as [n|r e]; simpl in Ht |- *; [contradiction|]. destruct Ht as [s [H1 H2]].
      exists s. split; [|exact H2].
      destruct (list_eq_dec Nat.eq_dec l1 l2) as [->|]; [exact H1|apply in_or_app; right; exact H1].
Qed.

Lemma embed_ts : forall st R l, forallb is_ts R = true -> Forall2 (Sim.tagm st) R l ->
  Forall2 (tagmA st) (map embed R) l.
Proof.
  intros st R l Hts H. induction H as [|t v R l Ht H IH]; simpl; constructor.
  - simpl in Hts. apply andb_prop in Hts as [H1 _]. destruct t; [|discriminate]. destruct v; simpl in *; tauto.
  - simpl in Hts. apply andb_prop in Hts as [_ H2]. exact (IH H2).
Qed.

Lemma aconflict_member : forall s L s0, aconflict P s (AR L) = false -> In s0 L -> conflict P s (TR s0) = false.
Proof. intros s L s0 H Hin. simpl in H. exact (existsb_false_all_n _ _ H s0 Hin). Qed.

Theorem simulation_arm : forall n,
  (forall b reg S S' sp sb, shapeA_b P ft reg S b = Some S' -> rel sp sb -> matchesA sb S -> locs_ok sb ->
     sim_resA S' (run_b P host fs n b sp) (run_b baseline host fs n b sb)) /\
  (forall i reg S S' sp sb, shapeA_i P ft reg S i = Some S' -> rel sp sb -> matchesA sb S -> locs_ok sb ->
     sim_resA S' (run_i P host fs n i sp) (run_i baseline host fs n i sb)) /\
  (forall k body reg S sp sb, shapeA_b P ft reg S body = Some S -> rel sp sb -> matchesA sb S -> locs_ok sb ->
     sim_resA S (run_loop P host fs n k body sp) (run_loop baseline host fs n k body sb)).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; exact I. }
  split; [|split].
  - intros b reg S S' sp sb Hs Hrel Hm Hl. destruct b as [|i r]; simpl in Hs |- *.
    + injection Hs as <-. split; [exact Hrel|split; assumption].
    + destruct (shapeA_i P ft reg S i) as [S1|] eqn:Ei; [|discriminate].
      pose proof (IHi i reg S S1 sp sb Ei Hrel Hm Hl) as H1.
      destruct (run_i P host fs n i sp) as [sp1|]; destruct (run_i baseline host fs n i sb) as [sb1|];
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [Hrel1 [Hm1 Hl1]]. exact (IHb r reg S1 S' sp1 sb1 Hs Hrel1 Hm1 Hl1).
  - intros i reg S S' sp sb Hs Hrel Hm Hl.
    destruct i as [k|j|j| | | |t e|k body|f| |site| | |]; simpl in Hs |- *.
    + injection Hs as <-. split; [apply rel_push_vn; exact Hrel|].
      split; [apply matchesA_push_vn; exact Hm|intros j; apply Hl].
    + injection Hs as <-. destruct Hrel as [Hs [Hlc Hrest]].
      destruct (Hl j) as [x Hx]. rewrite Hlc, Hx.
      split; [apply rel_push_vn; split; [exact Hs|split; [exact Hlc|exact Hrest]]|].
      split; [apply matchesA_push_vn; exact Hm|intros j'; apply Hl].
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [Htag [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb. destruct v as [x|r0 e0]; simpl in Htag; [|contradiction].
      destruct Hrel1 as [Hs1 [Hlc1 [Hg1 [Hr1 [He1 Hmo1]]]]].
      split; [unfold store; repeat split; simpl; try assumption; rewrite Hlc1; reflexivity|].
      split; [exact Hm1|]. intros j'. unfold store. simpl. destruct (Nat.eqb j' j); [eexists; reflexivity|apply Hl1].
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (apop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_popA sp1 sb1 r AS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matchesA_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (apop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_popA sp1 sb1 r AS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matchesA_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (apop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_popA sp1 sb1 r AS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matchesA_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (shapeA_b P ft reg r t) as [T|] eqn:ET; [|discriminate].
      destruct (shapeA_b P ft reg r e) as [E|] eqn:EE; [|discriminate].
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [c [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb. destruct (Nat.eqb (num c) 0).
      * pose proof (IHb e reg r E sp1 sb1 EE Hrel1 Hm1 Hl1) as H1.
        destruct (run_b P host fs n e sp1); destruct (run_b baseline host fs n e sb1);
          simpl in H1 |- *; try contradiction; [|exact I].
        destruct H1 as [A1 [B1 C1]]. split; [exact A1|split; [exact (ajoin_sound_r _ _ _ _ _ Hs B1)|exact C1]].
      * pose proof (IHb t reg r T sp1 sb1 ET Hrel1 Hm1 Hl1) as H1.
        destruct (run_b P host fs n t sp1); destruct (run_b baseline host fs n t sb1);
          simpl in H1 |- *; try contradiction; [|exact I].
        destruct H1 as [A1 [B1 C1]]. split; [exact A1|split; [exact (ajoin_sound_l _ _ _ _ _ Hs B1)|exact C1]].
    + destruct (shapeA_b P ft reg S body) as [S2|] eqn:Eb; [|discriminate].
      destruct (list_eq_dec atag_eq_dec S2 S) as [->|]; [|discriminate]. injection Hs as <-.
      exact (IHl k body reg S sp sb Eb Hrel Hm Hl).
    + destruct (nth_error ft f) as [R|] eqn:Ef; [|discriminate]. injection Hs as <-.
      destruct (nth_error fs f) as [g|] eqn:Eg; [|exact I].
      destruct (WF _ _ _ Ef Eg) as [Hg Hts].
      pose proof Hrel as [Hs [Hlc [Hgl [Hr [He Hmo]]]]].
      assert (Hrel0 : rel (with_stk sp []) (with_stk sb [])).
      { unfold with_stk. repeat split; simpl; try assumption. intros r0 e0 s0 []. }
      assert (Hm0 : matches (with_stk sb []) []) by constructor.
      assert (Hl0 : locs_ok (with_stk sb [])) by (intros j; apply Hl).
      pose proof (proj1 (simulation_shared P host fs ft WF n) g false [] R _ _
                    (eq_trans (shape_false_eq P ft [] g) Hg) Hrel0 Hm0 Hl0) as H1.
      destruct (run_b P host fs n g (with_stk sp [])) as [sp2|] eqn:Rp;
        destruct (run_b baseline host fs n g (with_stk sb [])) as [sb2|] eqn:Rb;
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [[Hs2 [Hlc2 [Hgl2 [Hr2 [He2 Hmo2]]]]] [Hm2 Hl2]].
      destruct (proj1 (no_regions host fs ft WF P n) _ _ _ _ _ Hg Rp) as [Rp1 [Mp1 Ep1]].
      destruct (proj1 (no_regions host fs ft WF baseline n) _ _ _ _ _ Hg Rb) as [Rb1 [Mb1 Eb1]].
      simpl in Rp1, Mp1, Ep1, Rb1, Mb1, Eb1.
      split; [|split].
      * unfold with_stk. repeat split; simpl; try assumption.
        -- rewrite Hs2, Hs. reflexivity.
        -- intros r0 e0 s0 Hin Hn. apply in_app_or in Hin as [Hin|Hin].
           ++ exfalso. exact (Sim.ts_no_handle _ R _ Hts Hm2 r0 e0 Hin).
           ++ rewrite Rb1 in Hn. destruct (Hmo r0 e0 s0 Hin Hn) as [x [X1 X2]].
              exists x. rewrite Mb1, Mp1. split; assumption.
      * unfold matchesA, with_stk. simpl. apply Forall2_app; [exact (embed_ts _ _ _ Hts Hm2)|].
        unfold matchesA in Hm. eapply forall2A_ext; [|intros r0 x H; simpl; rewrite Rb1; exact H|exact Hm]. simpl. congruence.
      * intros j. apply Hl2.
    + destruct (apop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      unfold yield_step.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb.
      assert (Hrel2 : rel (note (OutE (num v)) sp1) (note (OutE (num v)) sb1)) by (apply rel_note; exact Hrel1).
      assert (Hg2 : log (note (OutE (num v)) sp1) = log (note (OutE (num v)) sb1)) by (destruct Hrel2 as [_ [_ [G _]]]; exact G).
      rewrite Hg2. split; [apply rel_push_vn; exact Hrel2|].
      split; [apply matchesA_push_vn; exact Hm1|intros j; apply Hl1].
    + destruct reg; [|discriminate].
      destruct (apop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (existsb (aconflict P site) r) eqn:Hnot; [discriminate|]. injection Hs as <-.
      pose proof (existsb_false_all_a _ _ Hnot) as Hnot'. clear Hnot. rename Hnot' into Hnot.
      unfold new_step.
      destruct (step_popA sp sb S AS r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb.
      destruct Hrel1 as [Hs1 [Hlc1 [Hg1 [Hr1 [He1 Hmo1]]]]].
      assert (Hlen : forall r0 e0 s0, In (VH r0 e0) (stk sb1) -> nth_error (regs sb1) r0 = Some (s0, e0) ->
                     r0 < length (regs sb1)).
      { intros r0 e0 s0 _ Hn. apply nth_error_Some. rewrite Hn. discriminate. }
      split; [|split].
      * repeat split; simpl; try (rewrite ?Hs1, ?Hlc1, ?Hg1, ?Hr1, ?He1; reflexivity).
        intros r0 e0 s0 Hin Hn. cbn [stk regs mem epoch] in Hin, Hn |- *. rewrite ?Hr1, ?He1.
        destruct Hin as [Heq|Hin].
        -- injection Heq as <- <-. rewrite nth_error_app2, Nat.sub_diag in Hn by lia. simpl in Hn.
           injection Hn as <-. exists (num v).
           destruct (cell_eq_dec (Bump (length (regs sb1))) (cell_of baseline site (length (regs sb1))))
             as [_|C]; [|exfalso; apply C; reflexivity].
           destruct (cell_eq_dec (cell_of P site (length (regs sb1))) (cell_of P site (length (regs sb1))))
             as [_|C]; [|contradiction].
           split; reflexivity.
        -- assert (Hlt : r0 < length (regs sb1)).
           { destruct (in_handle_siteA sb1 r (stk sb1) r0 e0 Hm1 Hin) as [L [s1 [_ [_ Hn1]]]].
             apply nth_error_Some. rewrite Hn1. discriminate. }
           rewrite nth_error_app1 in Hn by exact Hlt.
           destruct (in_handle_siteA sb1 r (stk sb1) r0 e0 Hm1 Hin) as [L [s1 [HLin [Hs1in Hn1]]]].
           rewrite Hn in Hn1. injection Hn1 as <-.
           destruct (Hmo1 r0 e0 s0 Hin Hn) as [x [X1 X2]]. exists x.
           destruct (cell_eq_dec (Bump r0) (cell_of baseline site (length (regs sb1)))) as [C|_].
           { unfold cell_of, baseline in C. injection C. lia. }
           destruct (cell_eq_dec (cell_of P s0 r0) (cell_of P site (length (regs sb1)))) as [C|_].
           { exfalso. exact (cells_differ_shared P s0 r0 site (length (regs sb1))
                               (aconflict_member _ _ _ (Hnot _ HLin) Hs1in) Hlt C). }
           split; assumption.
      * unfold matchesA. simpl. constructor.
        -- unfold tagmA. cbn [regs epoch]. rewrite nth_error_app2, Nat.sub_diag by lia.
           exists site. split; [left; reflexivity|split; reflexivity].
        -- unfold matchesA in Hm1. eapply forall2A_ext; [| |exact Hm1]; [reflexivity|].
           intros r0 x H. cbn [regs]. rewrite nth_error_app1; [exact H|apply nth_error_Some; rewrite H; discriminate].
      * intros j. apply Hl1.
    + destruct reg; [|discriminate].
      destruct (apop S) as [[|L] r] eqn:Et; [discriminate|]. injection Hs as <-.
      unfold get_step.
      destruct (step_popA sp sb S (AR L) r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [Htag [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb. destruct v as [x|r0 e0]; simpl in Htag; [contradiction|].
      destruct Htag as [s0 [_ [Hn He0]]].
      pose proof Hrel as [Hs [_ [_ [Hr [He Hmo]]]]].
      assert (Hin0 : In (VH r0 e0) (stk sb)).
      { unfold pop in Eb. destruct (stk sb) as [|y l] eqn:Es; [discriminate|].
        injection Eb as -> _. left. reflexivity. }
      destruct (Hmo r0 e0 s0 Hin0 Hn) as [x [X1 X2]].
      destruct Hrel1 as [Hs1 [Hlc1 [Hg1 [Hr1 [He1 Hmo1]]]]].
      assert (Hd : deref P sp1 (VH r0 e0) = Some x /\ deref baseline sb1 (VH r0 e0) = Some x).
      { split.
        - unfold deref. rewrite He1, E1, <- He0, Nat.eqb_refl, Hr1, R1, Hn, Mp1. exact X2.
        - unfold deref. rewrite E1, <- He0, Nat.eqb_refl, R1, Hn, M1. exact X1. }
      destruct Hd as [Dp Db]. rewrite Dp, Db.
      split; [apply rel_push_vn; apply rel_note; repeat split; assumption|].
      split; [apply matchesA_push_vn; exact Hm1|intros j; apply Hl1].
    + destruct (apop S) as [t r] eqn:Et. injection Hs as <-.
      destruct (step_popA sp sb S t r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [Htag [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb.
      assert (Hv : forall r0 e0 s0, v = VH r0 e0 -> nth_error (regs sb1) r0 = Some (s0, e0) ->
                   exists x, mem sb1 (Bump r0) = Some x /\ mem sp1 (cell_of P s0 r0) = Some x).
      { intros r0 e0 s0 -> Hn. pose proof Hrel as [_ [_ [_ [_ [_ Hmo]]]]].
        assert (Hin0 : In (VH r0 e0) (stk sb)).
        { unfold pop in Eb. destruct (stk sb) as [|y l]; [discriminate|]. injection Eb as -> _. left. reflexivity. }
        rewrite R1 in Hn. destruct (Hmo r0 e0 s0 Hin0 Hn) as [x [X1 X2]]. exists x. rewrite M1, Mp1. split; assumption. }
      destruct Hrel1 as [Hs1 [Hlc1 [Hg1 [Hr1 [He1 Hmo1]]]]].
      split; [|split].
      * unfold push, with_stk. repeat split; simpl; try (rewrite ?Hs1; reflexivity); try assumption.
        intros r0 e0 s0 Hin Hn. simpl in Hin. destruct Hin as [Heq|[Heq|Hin]];
          [subst v; exact (Hv r0 e0 s0 eq_refl Hn)|subst v; exact (Hv r0 e0 s0 eq_refl Hn)|exact (Hmo1 r0 e0 s0 Hin Hn)].
      * unfold matchesA in Hm1 |- *. unfold push, with_stk. simpl.
        assert (Htag1 : tagmA sb1 t v) by (eapply tagmA_ext; [exact E1|intros r0 x H; rewrite R1; exact H|exact Htag]).
        constructor; [exact Htag1|constructor; [exact Htag1|exact Hm1]].
      * intros j. apply Hl1.
    + destruct (apop S) as [t r] eqn:Et. injection Hs as <-. simpl.
      destruct (step_popA sp sb S t r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb. split; [exact Hrel1|split; assumption].
  - intros k body reg S sp sb Hb Hrel Hm Hl. destruct k as [|k]; simpl.
    + split; [exact Hrel|split; assumption].
    + pose proof (IHb body reg S S sp sb Hb Hrel Hm Hl) as H1.
      destruct (run_b P host fs n body sp) as [sp1|]; destruct (run_b baseline host fs n body sb) as [sb1|];
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [Hrel1 [Hm1 Hl1]]. exact (IHl k body reg S sp1 sb1 Hb Hrel1 Hm1 Hl1).
Qed.

End ArmSim.
