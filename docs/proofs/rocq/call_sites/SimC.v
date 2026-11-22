(** * SimC.v -- the comparison method when functions may allocate.

    Increment B2's simulation, [Sim.simulation], with the function table of
    [ShapeC.v].  Every case is B2's except the call.  There a function body
    is checked with allocation allowed, so its two runs differ in memory,
    and B2's lemma that a function touches no region no longer applies.
    Instead the simulation itself relates the callee's two runs, and the
    frame lemma of [Frame.v] shows that neither run writes a cell a caller's
    live handle names.  A caller's handle is of a site the callee does not
    allocate, which the call rule of [ShapeC.v] ensures, and of an index
    below every index the callee allocates. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
From CallSites Require Import ShapeC Frame.
Import ListNotations.

Section SimC.

Variable P : plan.
Hypothesis Pinj : injective P.
Variable host : list ev -> nat.
Variable fs : list block.
Variable ft : list fentry.
Hypothesis WF : forall f R A g, nth_error ft f = Some (R, A) -> nth_error fs f = Some g ->
  shapeC_b ft true [] g = Some R /\ forallb is_ts R = true /\ A = sites_b ft g.
Hypothesis Hfs : forall f g, nth_error fs f = Some g ->
  exists R A, nth_error ft f = Some (R, A) /\ incl (sites_b ft g) A.

(** A stack entry has its tag's shape, a handle naming a current region of
    the tagged site. *)
Definition tagm (st : state) (t : tag) (v : val) : Prop :=
  match t, v with
  | TS, VN _ => True
  | TR s, VH r e => nth_error (regs st) r = Some (s, e) /\ e = epoch st
  | _, _ => False
  end.

Definition matches (st : state) (S : list tag) : Prop := Forall2 (tagm st) S (stk st).
Definition locs_ok (st : state) : Prop := forall j, exists n, locs st j = VN n.

Definition memok (sp sb : state) : Prop :=
  forall r e s, In (VH r e) (stk sb) -> nth_error (regs sb) r = Some (s, e) ->
  exists x, mem sb (Bump r) = Some x /\ mem sp (cell_of P s r) = Some x.

Definition rel (sp sb : state) : Prop :=
  stk sp = stk sb /\ locs sp = locs sb /\ log sp = log sb /\ regs sp = regs sb /\
  epoch sp = epoch sb /\ memok sp sb.

Definition sim_res (S' : list tag) (op ob : option state) : Prop :=
  match op, ob with
  | Some sp', Some sb' => rel sp' sb' /\ matches sb' S' /\ locs_ok sb'
  | None, None => True
  | _, _ => False
  end.

(** ** Lemmas about the stack. *)
Lemma tagm_ext : forall st st' t v,
  epoch st' = epoch st -> (forall r x, nth_error (regs st) r = Some x -> nth_error (regs st') r = Some x) ->
  tagm st t v -> tagm st' t v.
Proof.
  intros st st' [|s] [n|r e] He Hr H; simpl in *; try exact I; try contradiction.
  destruct H as [H1 H2]. split; [apply Hr; exact H1|congruence].
Qed.

Lemma forall2_ext : forall st st' S l,
  epoch st' = epoch st -> (forall r x, nth_error (regs st) r = Some x -> nth_error (regs st') r = Some x) ->
  Forall2 (tagm st) S l -> Forall2 (tagm st') S l.
Proof. intros st st' S l He Hr H. induction H; constructor; [eapply tagm_ext; eassumption|assumption]. Qed.

Lemma pop_spec : forall st S t r v st1,
  matches st S -> tpop S = (t, r) -> pop st = (v, st1) ->
  tagm st t v /\ Forall2 (tagm st) r (stk st1) /\
  locs st1 = locs st /\ log st1 = log st /\ regs st1 = regs st /\ mem st1 = mem st /\
  epoch st1 = epoch st /\ (forall x, In x (stk st1) -> In x (stk st)).
Proof.
  intros st S t r v st1 Hm Ht Hp. unfold matches, pop in *.
  destruct (stk st) as [|x l] eqn:Es.
  - injection Hp as <- <-. inversion Hm; subst. simpl in Ht. injection Ht as <- <-.
    rewrite Es. repeat split; try reflexivity; try constructor. intros x [].
  - injection Hp as <- <-. inversion Hm as [|t0 v0 S0 l0 H1 H2]; subst.
    simpl in Ht. injection Ht as <- <-. simpl. repeat split; try assumption; try reflexivity.
    intros y Hy. right. exact Hy.
Qed.

Lemma pop_rel : forall sp sb, rel sp sb ->
  fst (pop sp) = fst (pop sb) /\ rel (snd (pop sp)) (snd (pop sb)).
Proof.
  intros sp sb [Hs [Hl [Hg [Hr [He Hm]]]]]. unfold pop. rewrite Hs.
  destruct (stk sb) as [|x l] eqn:Es; simpl.
  - split; [reflexivity|]. repeat split; try assumption; congruence.
  - split; [reflexivity|]. unfold with_stk; repeat split; simpl; try assumption.
    intros r e s Hin Hn. simpl in Hin, Hn |- *. apply (Hm r e s); [rewrite Es; right; exact Hin|exact Hn].
Qed.

Lemma in_handle_site : forall st S l r e, Forall2 (tagm st) S l -> In (VH r e) l ->
  exists s, In (TR s) S /\ nth_error (regs st) r = Some (s, e).
Proof.
  intros st S l r e H. induction H as [|t v S l Ht H IH]; intros Hin; [contradiction|].
  destruct Hin as [Heq|Hin].
  - subst v. destruct t as [|s]; simpl in Ht; [contradiction|]. destruct Ht as [Hn _].
    exists s. split; [left; reflexivity|exact Hn].
  - destruct (IH Hin) as [s [H1 H2]]. exists s. split; [right; exact H1|exact H2].
Qed.

Lemma ts_no_handle : forall st R l, forallb is_ts R = true -> Forall2 (tagm st) R l ->
  forall r e, ~ In (VH r e) l.
Proof.
  intros st R l Hts H. induction H as [|t v R l Ht H IH]; intros r e Hin; [contradiction|].
  simpl in Hts. apply andb_prop in Hts as [Ht1 Ht2].
  destruct Hin as [Heq|Hin]; [subst v; destruct t; simpl in Ht1, Ht; [contradiction|discriminate]|].
  exact (IH Ht2 r e Hin).
Qed.

Lemma cells_differ : forall s0 r0 s rn, s0 <> s -> r0 < rn -> cell_of P s0 r0 <> cell_of P s rn.
Proof.
  intros s0 r0 s rn Hs Hr. unfold cell_of.
  destruct (P s0) as [k0|] eqn:E0; destruct (P s) as [k|] eqn:E; try discriminate.
  - intros H. injection H as ->. exact (Hs (Pinj _ _ _ E0 E)).
  - intros H. injection H. lia.
Qed.

Lemma pop_keeps : forall s v s1, pop s = (v, s1) ->
  regs s1 = regs s /\ mem s1 = mem s /\ epoch s1 = epoch s /\ locs s1 = locs s /\ log s1 = log s.
Proof. intros s v s1 H. unfold pop in H. destruct (stk s); injection H as <- <-; repeat split. Qed.

(** ** One step of the simulation. *)
Lemma step_pop : forall sp sb S t r, rel sp sb -> matches sb S -> locs_ok sb -> tpop S = (t, r) ->
  exists v sp1 sb1, pop sp = (v, sp1) /\ pop sb = (v, sb1) /\ rel sp1 sb1 /\ tagm sb t v /\
    matches sb1 r /\ locs_ok sb1 /\ regs sb1 = regs sb /\ epoch sb1 = epoch sb /\
    mem sb1 = mem sb /\ mem sp1 = mem sp /\ (forall x, In x (stk sb1) -> In x (stk sb)).
Proof.
  intros sp sb S t r Hrel Hm Hl Ht. destruct (pop_rel sp sb Hrel) as [Hv Hrel1].
  destruct (pop sp) as [vp sp1] eqn:Ep. destruct (pop sb) as [vb sb1] eqn:Eb. simpl in Hv, Hrel1. subst vp.
  destruct (pop_spec sb S t r vb sb1 Hm Ht Eb) as [Htag [Hf [L1 [G1 [R1 [M1 [E1 Hin]]]]]]].
  destruct (pop_keeps _ _ _ Ep) as [_ [Mp _]].
  exists vb, sp1, sb1. split; [reflexivity|]. split; [reflexivity|]. split; [exact Hrel1|].
  split; [exact Htag|]. split.
  { unfold matches. eapply forall2_ext; [exact E1| |exact Hf]. intros r0 x H. rewrite R1. exact H. }
  split; [intros j; rewrite L1; apply Hl|].
  split; [exact R1|]. split; [exact E1|]. split; [exact M1|]. split; [exact Mp|exact Hin].
Qed.

Lemma rel_push_vn : forall sp sb n, rel sp sb -> rel (push (VN n) sp) (push (VN n) sb).
Proof.
  intros sp sb n [Hs [Hl [Hg [Hr [He Hm]]]]]. unfold push, with_stk. repeat split; simpl; try assumption.
  - rewrite Hs. reflexivity.
  - intros r e s0 [H|H] Hn; [discriminate|]. exact (Hm r e s0 H Hn).
Qed.

Lemma matches_push_vn : forall st S n, matches st S -> matches (push (VN n) st) (TS :: S).
Proof. intros st S n H. unfold matches, push, with_stk. simpl. constructor; [exact I|exact H]. Qed.

Lemma rel_note : forall sp sb e, rel sp sb -> rel (note e sp) (note e sb).
Proof.
  intros sp sb e [Hs [Hl [Hg [Hr [He Hm]]]]]. unfold note. repeat split; simpl; try assumption.
  rewrite Hg. reflexivity.
Qed.

Theorem simulationC : forall n,
  (forall b reg S S' sp sb, shapeC_b ft reg S b = Some S' -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S' (run_b P host fs n b sp) (run_b baseline host fs n b sb)) /\
  (forall i reg S S' sp sb, shapeC_i ft reg S i = Some S' -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S' (run_i P host fs n i sp) (run_i baseline host fs n i sb)) /\
  (forall k body reg S sp sb, shapeC_b ft reg S body = Some S -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S (run_loop P host fs n k body sp) (run_loop baseline host fs n k body sb)).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; exact I. }
  split; [|split].
  - intros b reg S S' sp sb Hs Hrel Hm Hl. destruct b as [|i r]; simpl in Hs |- *.
    + injection Hs as <-. split; [exact Hrel|split; assumption].
    + destruct (shapeC_i ft reg S i) as [S1|] eqn:Ei; [|discriminate].
      pose proof (IHi i reg S S1 sp sb Ei Hrel Hm Hl) as H1.
      destruct (run_i P host fs n i sp) as [sp1|]; destruct (run_i baseline host fs n i sb) as [sb1|];
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [Hrel1 [Hm1 Hl1]]. exact (IHb r reg S1 S' sp1 sb1 Hs Hrel1 Hm1 Hl1).
  - intros i reg S S' sp sb Hs Hrel Hm Hl.
    destruct i as [k|j|j| | | |t e|k body|f| |site| | |]; simpl in Hs |- *.
    + injection Hs as <-. split; [apply rel_push_vn; exact Hrel|].
      split; [apply matches_push_vn; exact Hm|intros j; apply Hl].
    + injection Hs as <-. destruct Hrel as [Hs [Hlc Hrest]].
      destruct (Hl j) as [x Hx]. rewrite Hlc, Hx.
      split; [apply rel_push_vn; split; [exact Hs|split; [exact Hlc|exact Hrest]]|].
      split; [apply matches_push_vn; exact Hm|intros j'; apply Hl].
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [Htag [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb. destruct v as [x|r0 e0]; simpl in Htag; [|contradiction].
      destruct Hrel1 as [Hs1 [Hlc1 [Hg1 [Hr1 [He1 Hmo1]]]]].
      split; [unfold store; repeat split; simpl; try assumption; rewrite Hlc1; reflexivity|].
      split; [exact Hm1|]. intros j'. unfold store. simpl. destruct (Nat.eqb j' j); [eexists; reflexivity|apply Hl1].
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_pop sp1 sb1 r TS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matches_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_pop sp1 sb1 r TS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matches_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (tpop r) as [[|q] r2] eqn:Et2; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [b [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      destruct (step_pop sp1 sb1 r TS r2 Hrel1 Hm1 Hl1 Et2)
        as [a [sp2 [sb2 [Ep2 [Eb2 [Hrel2 [_ [Hm2 [Hl2 _]]]]]]]]].
      rewrite Ep, Eb. simpl. rewrite Ep2, Eb2.
      split; [apply rel_push_vn; exact Hrel2|]. split; [apply matches_push_vn; exact Hm2|intros j; apply Hl2].
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (shapeC_b ft reg r t) as [T|] eqn:ET; [|discriminate].
      destruct (shapeC_b ft reg r e) as [E|] eqn:EE; [|discriminate].
      destruct (list_eq_dec tag_eq_dec T E) as [<-|]; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [c [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb. destruct (Nat.eqb (num c) 0).
      * exact (IHb e reg r T sp1 sb1 EE Hrel1 Hm1 Hl1).
      * exact (IHb t reg r T sp1 sb1 ET Hrel1 Hm1 Hl1).
    + destruct (shapeC_b ft reg S body) as [S2|] eqn:Eb; [|discriminate].
      destruct (list_eq_dec tag_eq_dec S2 S) as [->|]; [|discriminate]. injection Hs as <-.
      exact (IHl k body reg S sp sb Eb Hrel Hm Hl).
    + destruct (nth_error ft f) as [[R A]|] eqn:Ef; [|discriminate].
      destruct (existsb (live_in A) S) eqn:Elive; [discriminate|]. injection Hs as <-.
      destruct (nth_error fs f) as [g|] eqn:Eg; [|exact I].
      destruct (WF _ _ _ _ Ef Eg) as [Hg [Hts HA]].
      pose proof Hrel as [Hs [Hlc [Hgl [Hr [He Hmo]]]]].
      assert (Hrel0 : rel (with_stk sp []) (with_stk sb [])).
      { unfold with_stk. repeat split; simpl; try assumption. intros r0 e0 s0 []. }
      assert (Hm0 : matches (with_stk sb []) []) by constructor.
      assert (Hl0 : locs_ok (with_stk sb [])) by (intros j; apply Hl).
      pose proof (IHb g true [] R _ _ Hg Hrel0 Hm0 Hl0) as H1.
      destruct (run_b P host fs n g (with_stk sp [])) as [sp2|] eqn:Rp;
        destruct (run_b baseline host fs n g (with_stk sb [])) as [sb2|] eqn:Rb;
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [[Hs2 [Hlc2 [Hgl2 [Hr2 [He2 Hmo2]]]]] [Hm2 Hl2]].
      destruct (proj1 (frame P host fs ft Hfs n) _ _ _ Rp) as [[up Rp1] [Ep1 Mp1]].
      destruct (proj1 (frame baseline host fs ft Hfs n) _ _ _ Rb) as [[ub Rb1] [Eb1 Mb1]].
      simpl in Rp1, Ep1, Mp1, Rb1, Eb1, Mb1.
      split; [|split].
      * unfold with_stk. repeat split; simpl; try assumption.
        -- rewrite Hs2, Hs. reflexivity.
        -- intros r0 e0 s0 Hin Hn. apply in_app_or in Hin as [Hin|Hin].
           ++ exfalso. exact (ts_no_handle _ R _ Hts Hm2 r0 e0 Hin).
           ++ destruct (in_handle_site sb S (stk sb) r0 e0 Hm Hin) as [s1 [Hs1in Hn1]].
              assert (Hlt : r0 < length (regs sb)) by (apply nth_error_Some; rewrite Hn1; discriminate).
              simpl in Hn. rewrite Rb1, nth_error_app1 in Hn by exact Hlt. rewrite Hn in Hn1. injection Hn1 as <-.
              assert (Hnot : ~ In s0 (sites_b ft g)).
              { rewrite <- HA. intros HinA.
                assert (Hl1 : existsb (live_in A) S = true).
                { apply existsb_exists. exists (TR s0). split; [exact Hs1in|].
                  simpl. apply existsb_exists. exists s0. split; [exact HinA|apply Nat.eqb_refl]. }
                congruence. }
              destruct (Hmo r0 e0 s0 Hin Hn) as [x [X1 X2]]. exists x. split.
              ** simpl. rewrite Mb1; [exact X1|]. intros s' r' _ Hr' C. unfold cell_of, baseline in C.
                 injection C. lia.
              ** simpl. rewrite Mp1; [exact X2|]. intros s' r' Hs' Hr' C.
                 apply (cells_differ s0 r0 s' r'); [intros ->; exact (Hnot Hs')|rewrite Hr in Hr'; lia|exact C].
      * unfold matches, with_stk. simpl. apply Forall2_app; [exact Hm2|].
        unfold matches in Hm. eapply forall2_ext; [|intros r0 x H; simpl; rewrite Rb1, nth_error_app1;
          [exact H|apply nth_error_Some; rewrite H; discriminate]|exact Hm]. simpl. congruence.
      * intros j. apply Hl2.
    + destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate]. injection Hs as <-.
      unfold yield_step.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb.
      assert (Hrel2 : rel (note (OutE (num v)) sp1) (note (OutE (num v)) sb1)) by (apply rel_note; exact Hrel1).
      assert (Hg2 : log (note (OutE (num v)) sp1) = log (note (OutE (num v)) sb1)) by (destruct Hrel2 as [_ [_ [G _]]]; exact G).
      rewrite Hg2. split; [apply rel_push_vn; exact Hrel2|].
      split; [apply matches_push_vn; exact Hm1|intros j; apply Hl1].
    + destruct reg; [|discriminate].
      destruct (tpop S) as [[|q] r] eqn:Et; [|discriminate].
      destruct (in_dec tag_eq_dec (TR site) r) as [_|Hnot]; [discriminate|]. injection Hs as <-.
      unfold new_step.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
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
           { destruct (in_handle_site sb1 r (stk sb1) r0 e0 Hm1 Hin) as [s1 [_ Hn1]].
             apply nth_error_Some. rewrite Hn1. discriminate. }
           rewrite nth_error_app1 in Hn by exact Hlt.
           destruct (in_handle_site sb1 r (stk sb1) r0 e0 Hm1 Hin) as [s1 [Hs1in Hn1]].
           rewrite Hn in Hn1. injection Hn1 as <-.
           destruct (Hmo1 r0 e0 s0 Hin Hn) as [x [X1 X2]]. exists x.
           destruct (cell_eq_dec (Bump r0) (cell_of baseline site (length (regs sb1)))) as [C|_].
           { unfold cell_of, baseline in C. injection C. lia. }
           destruct (cell_eq_dec (cell_of P s0 r0) (cell_of P site (length (regs sb1)))) as [C|_].
           { exfalso. apply (cells_differ s0 r0 site (length (regs sb1))); [|exact Hlt|exact C].
             intros ->. exact (Hnot Hs1in). }
           split; assumption.
      * unfold matches. simpl. constructor.
        -- unfold tagm. cbn [regs epoch]. rewrite nth_error_app2, Nat.sub_diag by lia. split; reflexivity.
        -- unfold matches in Hm1. eapply forall2_ext; [| |exact Hm1]; [reflexivity|].
           intros r0 x H. cbn [regs]. rewrite nth_error_app1; [exact H|apply nth_error_Some; rewrite H; discriminate].
      * intros j. apply Hl1.
    + destruct reg; [|discriminate].
      destruct (tpop S) as [[|s0] r] eqn:Et; [discriminate|]. injection Hs as <-.
      unfold get_step.
      destruct (step_pop sp sb S (TR s0) r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [Htag [Hm1 [Hl1 [R1 [E1 [M1 [Mp1 Hin1]]]]]]]]]]]]].
      rewrite Ep, Eb. destruct v as [x|r0 e0]; simpl in Htag; [contradiction|].
      destruct Htag as [Hn He0].
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
      split; [apply matches_push_vn; exact Hm1|intros j; apply Hl1].
    + destruct (tpop S) as [t r] eqn:Et. injection Hs as <-.
      destruct (step_pop sp sb S t r Hrel Hm Hl Et)
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
      * unfold matches in Hm1 |- *. unfold push, with_stk. simpl.
        assert (Htag1 : tagm sb1 t v) by (eapply tagm_ext; [exact E1|intros r0 x H; rewrite R1; exact H|exact Htag]).
        constructor; [exact Htag1|constructor; [exact Htag1|exact Hm1]].
      * intros j. apply Hl1.
    + destruct (tpop S) as [t r] eqn:Et. injection Hs as <-. simpl.
      destruct (step_pop sp sb S t r Hrel Hm Hl Et)
        as [v [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb. split; [exact Hrel1|split; assumption].
  - intros k body reg S sp sb Hb Hrel Hm Hl. destruct k as [|k]; simpl.
    + split; [exact Hrel|split; assumption].
    + pose proof (IHb body reg S S sp sb Hb Hrel Hm Hl) as H1.
      destruct (run_b P host fs n body sp) as [sp1|]; destruct (run_b baseline host fs n body sb) as [sb1|];
        simpl in H1 |- *; try contradiction; [|exact I].
      destruct H1 as [Hrel1 [Hm1 Hl1]]. exact (IHl k body reg S sp1 sb1 Hb Hrel1 Hm1 Hl1).
Qed.

End SimC.
