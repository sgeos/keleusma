(** * SharedSim.v -- the simulation for plans that share slots.

    [simulation_shared] is increment B2's [Sim.simulation] with the plan
    checked by [SharedShape] instead of required injective.  The proof text is
    B2's, derived by substitution, except at [New], where the plan-aware rule
    supplies the fact that no live handle names a cell the allocation writes,
    and at [Call], where a function's body is checked by [Shape], which agrees
    with [SharedShape] when regions are forbidden.  B2's lemmas about pops,
    stack shapes and memory correspondence are reused unchanged. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape Sim.
From SharedSlots Require Import SharedShape.
Import ListNotations.

(** With regions forbidden the two checkers coincide, whatever the plan. *)
Lemma shape_false_eq : forall P ft S b, shapeP_b P ft false S b = shape_b ft false S b.
Proof.
  intros P ft S b. revert S.
  apply (block_mut (fun i => forall S, shapeP_i P ft false S i = shape_i ft false S i)
                   (fun b => forall S, shapeP_b P ft false S b = shape_b ft false S b));
    intros; simpl; try reflexivity.
  - destruct (tpop S) as [[|q] r]; [|reflexivity]. rewrite H, H0. reflexivity.
  - rewrite H. reflexivity.
  - rewrite H. destruct (shape_i ft false S i); [apply H0|reflexivity].
Qed.

Lemma existsb_false_all : forall (f : tag -> bool) l, existsb f l = false -> forall x, In x l -> f x = false.
Proof.
  intros f l. induction l as [|y l IH]; intros H x Hx; [contradiction|].
  simpl in H. apply orb_false_iff in H as [H1 H2]. destruct Hx as [<-|Hx]; [exact H1|exact (IH H2 x Hx)].
Qed.

Section SharedSim.

Variable P : plan.
Variable host : list ev -> nat.
Variable fs : list block.
Variable ft : list (list tag).
Hypothesis WF : forall f R g, nth_error ft f = Some R -> nth_error fs f = Some g ->
  shape_b ft false [] g = Some R /\ forallb is_ts R = true.

Local Abbreviation rel := (Sim.rel P).
Local Abbreviation memok := (Sim.memok P).
Local Abbreviation sim_res := (Sim.sim_res P).
Local Abbreviation step_pop := (Sim.step_pop P).
Local Abbreviation rel_push_vn := (Sim.rel_push_vn P).
Local Abbreviation rel_note := (Sim.rel_note P).

(** An allocation's cell differs from every cell a non-conflicting live handle
    names. *)
Lemma cells_differ_shared : forall s0 r0 s rn,
  conflict P s (TR s0) = false -> r0 < rn -> cell_of P s0 r0 <> cell_of P s rn.
Proof.
  intros s0 r0 s rn Hc Hr. unfold conflict in Hc. apply orb_false_iff in Hc as [H1 H2].
  apply Nat.eqb_neq in H1. unfold cell_of.
  destruct (P s) as [k|] eqn:E; destruct (P s0) as [k0|] eqn:E0; try discriminate.
  - intros H. injection H as ->. rewrite Nat.eqb_refl in H2. discriminate.
  - intros H. injection H. lia.
Qed.

Theorem simulation_shared : forall n,
  (forall b reg S S' sp sb, shapeP_b P ft reg S b = Some S' -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S' (run_b P host fs n b sp) (run_b baseline host fs n b sb)) /\
  (forall i reg S S' sp sb, shapeP_i P ft reg S i = Some S' -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S' (run_i P host fs n i sp) (run_i baseline host fs n i sb)) /\
  (forall k body reg S sp sb, shapeP_b P ft reg S body = Some S -> rel sp sb -> matches sb S -> locs_ok sb ->
     sim_res S (run_loop P host fs n k body sp) (run_loop baseline host fs n k body sb)).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; exact I. }
  split; [|split].
  - intros b reg S S' sp sb Hs Hrel Hm Hl. destruct b as [|i r]; simpl in Hs |- *.
    + injection Hs as <-. split; [exact Hrel|split; assumption].
    + destruct (shapeP_i P ft reg S i) as [S1|] eqn:Ei; [|discriminate].
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
      destruct (shapeP_b P ft reg r t) as [T|] eqn:ET; [|discriminate].
      destruct (shapeP_b P ft reg r e) as [E|] eqn:EE; [|discriminate].
      destruct (list_eq_dec tag_eq_dec T E) as [<-|]; [|discriminate]. injection Hs as <-.
      destruct (step_pop sp sb S TS r Hrel Hm Hl Et)
        as [c [sp1 [sb1 [Ep [Eb [Hrel1 [_ [Hm1 [Hl1 _]]]]]]]]].
      rewrite Ep, Eb. destruct (Nat.eqb (num c) 0).
      * exact (IHb e reg r T sp1 sb1 EE Hrel1 Hm1 Hl1).
      * exact (IHb t reg r T sp1 sb1 ET Hrel1 Hm1 Hl1).
    + destruct (shapeP_b P ft reg S body) as [S2|] eqn:Eb; [|discriminate].
      destruct (list_eq_dec tag_eq_dec S2 S) as [->|]; [|discriminate]. injection Hs as <-.
      exact (IHl k body reg S sp sb Eb Hrel Hm Hl).
    + destruct (nth_error ft f) as [R|] eqn:Ef; [|discriminate]. injection Hs as <-.
      destruct (nth_error fs f) as [g|] eqn:Eg; [|exact I].
      destruct (WF _ _ _ Ef Eg) as [Hg Hts].
      pose proof Hrel as [Hs [Hlc [Hgl [Hr [He Hmo]]]]].
      assert (Hrel0 : rel (with_stk sp []) (with_stk sb [])).
      { unfold with_stk. repeat split; simpl; try assumption. intros r0 e0 s0 []. }
      assert (Hm0 : matches (with_stk sb []) []) by constructor.
      assert (Hl0 : locs_ok (with_stk sb [])) by (intros j; apply Hl).
      pose proof (IHb g false [] R _ _ (eq_trans (shape_false_eq P ft [] g) Hg) Hrel0 Hm0 Hl0) as H1.
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
           ++ exfalso. exact (ts_no_handle _ R _ Hts Hm2 r0 e0 Hin).
           ++ rewrite Rb1 in Hn. destruct (Hmo r0 e0 s0 Hin Hn) as [x [X1 X2]].
              exists x. rewrite Mb1, Mp1. split; assumption.
      * unfold matches, with_stk. simpl. apply Forall2_app; [exact Hm2|].
        unfold matches in Hm. eapply forall2_ext; [|intros r0 x H; simpl; rewrite Rb1; exact H|exact Hm]. simpl. congruence.
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
      destruct (existsb (conflict P site) r) eqn:Hnot; [discriminate|]. injection Hs as <-.
      pose proof (existsb_false_all _ _ Hnot) as Hnot'. clear Hnot. rename Hnot' into Hnot.
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
           { exfalso. exact (cells_differ_shared s0 r0 site (length (regs sb1)) (Hnot _ Hs1in) Hlt C). }
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

End SharedSim.
