(** * Soundness.v -- an accepted module terminates within its bounds.

    The headline results, for every host behaviour.

    - [function_bounded]: every function of an accepted module runs to
      completion on the fuel the checker computes, within its bound.
    - [fn_never_yields]: every [fn] function of an accepted module runs as
      one segment, so it never yields.
    - [cycle_bounded]: every cycle of an accepted module completes, yields at
      least once, keeps every segment within [wcet], and allocates at most
      [wcmu] words of arena.
    - [stream_bounded]: every stream of any number of cycles completes, keeps
      every segment within [wcet], the junction across each reset included,
      and keeps every cycle's arena within [wcmu]. *)

From Stdlib Require Import List Arith Lia.
From CoreBounds Require Import Core Check Algebra.
Import ListNotations.

Section Soundness.

Variable host : list nat -> nat.
Variable fs : list func.
Variable tab : list abs.

(** Each function's bound is the bound of its body, computed from [tab]. *)
Hypothesis WF : forall f A g, nth_error tab f = Some A -> nth_error fs f = Some g ->
  abs_b tab (fbody g) = Some A.
Hypothesis WF_len : length tab = length fs.

(** [good A s s' p] says a run from [s] to [s'] with segments [p] is within
    the bound [A]. *)
Definition good (A : abs) (s s' : state) (p : segs) : Prop :=
  p <> [] /\ le_summ (summary p) A /\ arena s <= arena s' /\ arena s' <= arena s + am A.

Lemma pop_arena : forall s v s1, pop s = (v, s1) -> arena s1 = arena s.
Proof. intros s v s1 H. unfold pop in H. destruct (stk s); injection H as <- <-; reflexivity. Qed.

Lemma good_seq : forall A B d s s1 s2 p1 p2,
  good A s s1 p1 -> good B s1 s2 p2 -> good (aseq A B d) s s2 (join p1 p2).
Proof.
  intros A B d s s1 s2 p1 p2 [N1 [L1 [M1 X1]]] [N2 [L2 [M2 X2]]].
  split; [apply join_nonempty; assumption|].
  split; [rewrite summary_join by assumption; apply le_seq; assumption|].
  simpl. lia.
Qed.

Lemma good_same : forall s s', arena s' = arena s -> good (one 0) s s' [1].
Proof. intros s s' H. repeat split; [discriminate| |lia|simpl; lia]. exists 1. split; [reflexivity|lia]. Qed.

Lemma good_one : forall s, good (one 0) s s [1].
Proof. intros s. apply good_same. reflexivity. Qed.

Lemma good_prefix : forall A d s s' p, good A s s' p -> good (aseq (one 0) A d) s s' (join [1] p).
Proof.
  intros A d s s' p H. replace (aseq (one 0) A d) with (aseq (one 0) A d) by reflexivity.
  exact (good_seq (one 0) A d s s s' [1] p (good_one s) H).
Qed.

Lemma good_join_l : forall T E d s s' p, good T s s' p -> good (ajoin T E d) s s' p.
Proof.
  intros T E d s s' p [N [L [M X]]]. split; [exact N|]. split; [apply le_join_l; exact L|].
  simpl. lia.
Qed.

Lemma good_join_r : forall T E d s s' p, good E s s' p -> good (ajoin T E d) s s' p.
Proof.
  intros T E d s s' p [N [L [M X]]]. split; [exact N|]. split; [apply le_join_r; exact L|].
  simpl. lia.
Qed.

(** ** Every completed run is within its bound. *)
Theorem runs_within_bounds : forall n,
  (forall b A s s' p, abs_b tab b = Some A -> run_b host fs n b s = Some (s', p) -> good A s s' p) /\
  (forall i A s s' p, abs_i tab i = Some A -> run_i host fs n i s = Some (s', p) -> good A s s' p) /\
  (forall k body Bd s s' p, abs_b tab body = Some Bd ->
     run_loop host fs n k body s = Some (s', p) -> good (aloop k Bd) s s' p).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros; simpl in *; discriminate. }
  split; [|split].
  - intros b A s s' p Ha Hr. destruct b as [|i r]; simpl in Ha, Hr.
    + injection Ha as <-. injection Hr as <- <-. repeat split; [discriminate| |lia|simpl; lia].
      exists 0. split; [reflexivity|lia].
    + destruct (abs_i tab i) as [Ai|] eqn:Ei; [|discriminate].
      destruct (abs_b tab r) as [Ar|] eqn:Er; [|discriminate]. injection Ha as <-.
      destruct (run_i host fs n i s) as [[s1 p1]|] eqn:R1; [|discriminate]. simpl in Hr.
      destruct (run_b host fs n r s1) as [[s2 p2]|] eqn:R2; [|discriminate]. simpl in Hr.
      injection Hr as <- <-. eapply good_seq; [exact (IHi _ _ _ _ _ Ei R1)|exact (IHb _ _ _ _ _ Er R2)].
  - intros i A s s' p Ha Hr. destruct i as [k|j|j| | | |t e|k body|f|k|]; simpl in Ha, Hr.
    + injection Ha as <-. injection Hr as <- <-. apply good_same. reflexivity.
    + injection Ha as <-. injection Hr as <- <-. apply good_same. reflexivity.
    + injection Ha as <-. destruct (pop s) as [v s1] eqn:E. injection Hr as <- <-.
      pose proof (pop_arena _ _ _ E). repeat split; [discriminate| |simpl; lia|simpl; lia].
      exists 1. split; [reflexivity|lia].
    + injection Ha as <-. destruct (pop s) as [b s1] eqn:E1. destruct (pop s1) as [a s2] eqn:E2.
      injection Hr as <- <-. pose proof (pop_arena _ _ _ E1). pose proof (pop_arena _ _ _ E2).
      repeat split; [discriminate| |simpl; lia|simpl; lia]. exists 1. split; [reflexivity|lia].
    + injection Ha as <-. destruct (pop s) as [b s1] eqn:E1. destruct (pop s1) as [a s2] eqn:E2.
      injection Hr as <- <-. pose proof (pop_arena _ _ _ E1). pose proof (pop_arena _ _ _ E2).
      repeat split; [discriminate| |simpl; lia|simpl; lia]. exists 1. split; [reflexivity|lia].
    + injection Ha as <-. destruct (pop s) as [b s1] eqn:E1. destruct (pop s1) as [a s2] eqn:E2.
      injection Hr as <- <-. pose proof (pop_arena _ _ _ E1). pose proof (pop_arena _ _ _ E2).
      repeat split; [discriminate| |simpl; lia|simpl; lia]. exists 1. split; [reflexivity|lia].
    + destruct (abs_b tab t) as [T|] eqn:Et; [|discriminate].
      destruct (abs_b tab e) as [E|] eqn:Ee; [|discriminate]. injection Ha as <-.
      destruct (pop s) as [c s1] eqn:Ep. pose proof (pop_arena _ _ _ Ep) as Ha1.
      destruct (Nat.eqb c 0).
      * destruct (run_b host fs n e s1) as [[s2 p2]|] eqn:R; [|discriminate]. simpl in Hr.
        injection Hr as <- <-. apply good_prefix. apply good_join_r.
        destruct (IHb _ _ _ _ _ Ee R) as [N [L [M X]]]. repeat split; [exact N|exact L|lia|lia].
      * destruct (run_b host fs n t s1) as [[s2 p2]|] eqn:R; [|discriminate]. simpl in Hr.
        injection Hr as <- <-. apply good_prefix. apply good_join_l.
        destruct (IHb _ _ _ _ _ Et R) as [N [L [M X]]]. repeat split; [exact N|exact L|lia|lia].
    + destruct (abs_b tab body) as [Bd|] eqn:Eb; [|discriminate]. injection Ha as <-.
      destruct (run_loop host fs n k body s) as [[s2 p2]|] eqn:R; [|discriminate]. simpl in Hr.
      injection Hr as <- <-. apply good_prefix. exact (IHl _ _ _ _ _ _ Eb R).
    + destruct (nth_error tab f) as [C|] eqn:Et; [|discriminate]. injection Ha as <-.
      destruct (nth_error fs f) as [g|] eqn:Eg; [|discriminate].
      destruct (run_b host fs n (fbody g) s) as [[s2 p2]|] eqn:R; [|discriminate]. simpl in Hr.
      injection Hr as <- <-. apply good_prefix. exact (IHb _ _ _ _ _ (WF _ _ _ Et Eg) R).
    + injection Ha as <-. injection Hr as <- <-. repeat split; [discriminate| |simpl; lia|simpl; lia].
      exists 1. split; [reflexivity|lia].
    + injection Ha as <-. injection Hr as <- <-. unfold yield_step.
      destruct (pop s) as [v s1] eqn:E. pose proof (pop_arena _ _ _ E).
      repeat split; [discriminate| |simpl; lia|simpl; lia].
      exists (1, 0, 0). split; [reflexivity|simpl; lia].
  - intros k body Bd s s' p Hb Hr. destruct k as [|k]; simpl in Hr.
    + injection Hr as <- <-. repeat split; [discriminate| |lia|simpl; lia].
      exists 0. split; [reflexivity|lia].
    + destruct (run_b host fs n body s) as [[s1 p1]|] eqn:R1; [|discriminate]. simpl in Hr.
      destruct (run_loop host fs n k body s1) as [[s2 p2]|] eqn:R2; [|discriminate]. simpl in Hr.
      injection Hr as <- <-.
      change (aloop (S k) Bd) with (aseq Bd (aseq (one 0) (aloop k Bd) 0) (S (Nat.max (ad Bd) (ad (aloop k Bd))))).
      eapply good_seq; [exact (IHb _ _ _ _ _ Hb R1)|apply good_prefix; exact (IHl _ _ _ _ _ _ Hb R2)].
Qed.

(** ** The checker's fuel always suffices. *)
Lemma ad_pos_b : forall b A, abs_b tab b = Some A -> 1 <= ad A.
Proof.
  intros [|i r] A H; simpl in H; [injection H as <-; simpl; lia|].
  destruct (abs_i tab i); [|discriminate]. destruct (abs_b tab r); [|discriminate].
  injection H as <-. simpl. lia.
Qed.

Lemma ad_pos_i : forall i A, abs_i tab i = Some A -> 1 <= ad A.
Proof.
  intros i A H. destruct i; simpl in H;
    repeat match type of H with
    | context [match ?x with _ => _ end] => destruct x; try discriminate
    end; injection H as <-; simpl; lia.
Qed.

Lemma ad_pos_loop : forall k A, 1 <= ad (aloop k A).
Proof. intros [|k] A; simpl; lia. Qed.

Theorem fuel_suffices : forall n,
  (forall b A s, abs_b tab b = Some A -> ad A <= n -> exists r, run_b host fs n b s = Some r) /\
  (forall i A s, abs_i tab i = Some A -> ad A <= n -> exists r, run_i host fs n i s = Some r) /\
  (forall k body Bd s, abs_b tab body = Some Bd -> ad (aloop k Bd) <= n ->
     exists r, run_loop host fs n k body s = Some r).
Proof.
  induction n as [|n [IHb [IHi IHl]]].
  { repeat split; intros.
    - pose proof (ad_pos_b _ _ H). lia.
    - pose proof (ad_pos_i _ _ H). lia.
    - pose proof (ad_pos_loop k Bd). lia. }
  split; [|split].
  - intros b A s Ha Hn. destruct b as [|i r]; simpl in Ha |- *; [eexists; reflexivity|].
    destruct (abs_i tab i) as [Ai|] eqn:Ei; [|discriminate].
    destruct (abs_b tab r) as [Ar|] eqn:Er; [|discriminate]. injection Ha as <-. simpl in Hn.
    destruct (IHi i Ai s Ei ltac:(lia)) as [[s1 p1] R1]. rewrite R1. simpl.
    destruct (IHb r Ar s1 Er ltac:(lia)) as [[s2 p2] R2]. rewrite R2. simpl. eexists; reflexivity.
  - intros i A s Ha Hn. destruct i as [k|j|j| | | |t e|k body|f|k|]; simpl in Ha |- *;
      try (destruct (pop s); try destruct (pop _); eexists; reflexivity);
      try (eexists; reflexivity).
    + destruct (abs_b tab t) as [T|] eqn:Et; [|discriminate].
      destruct (abs_b tab e) as [E|] eqn:Ee; [|discriminate]. injection Ha as <-. simpl in Hn.
      destruct (pop s) as [c s1]. destruct (Nat.eqb c 0).
      * destruct (IHb e E s1 Ee ltac:(lia)) as [[s2 p2] R]. rewrite R. simpl. eexists; reflexivity.
      * destruct (IHb t T s1 Et ltac:(lia)) as [[s2 p2] R]. rewrite R. simpl. eexists; reflexivity.
    + destruct (abs_b tab body) as [Bd|] eqn:Eb; [|discriminate]. injection Ha as <-. simpl in Hn.
      destruct (IHl k body Bd s Eb ltac:(lia)) as [[s2 p2] R]. rewrite R. simpl. eexists; reflexivity.
    + destruct (nth_error tab f) as [C|] eqn:Et; [|discriminate]. injection Ha as <-. simpl in Hn.
      assert (Hf : f < length fs) by (rewrite <- WF_len; apply nth_error_Some; rewrite Et; discriminate).
      destruct (nth_error fs f) as [g|] eqn:Eg; [|apply nth_error_Some in Hf; contradiction].
      destruct (IHb (fbody g) C s (WF _ _ _ Et Eg) ltac:(lia)) as [[s2 p2] R]. rewrite R. simpl.
      eexists; reflexivity.
  - intros k body Bd s Hb Hn. destruct k as [|k]; simpl; [eexists; reflexivity|].
    simpl in Hn.
    destruct (IHb body Bd s Hb ltac:(lia)) as [[s1 p1] R1]. rewrite R1. simpl.
    destruct (IHl k body Bd s1 Hb ltac:(lia)) as [[s2 p2] R2]. rewrite R2. simpl. eexists; reflexivity.
Qed.

End Soundness.

(** ** What the checker establishes about its table. *)
Lemma abs_extend : forall t u,
  (forall b A, abs_b t b = Some A -> abs_b (t ++ u) b = Some A).
Proof.
  intros t u.
  apply (block_mut (fun i => forall A, abs_i t i = Some A -> abs_i (t ++ u) i = Some A)
                   (fun b => forall A, abs_b t b = Some A -> abs_b (t ++ u) b = Some A)).
  1-6: intros; simpl in *; assumption.
  - intros te Ht e He A H. simpl in *.
    destruct (abs_b t te) eqn:E1; [|discriminate]. destruct (abs_b t e) eqn:E2; [|discriminate].
    rewrite (Ht _ eq_refl), (He _ eq_refl). exact H.
  - intros k body Hb A H. simpl in *.
    destruct (abs_b t body) eqn:E1; [|discriminate]. rewrite (Hb _ eq_refl). exact H.
  - intros f A H. simpl in *. destruct (nth_error t f) eqn:E1; [|discriminate].
    rewrite nth_error_app1 by (apply nth_error_Some; rewrite E1; discriminate). rewrite E1. exact H.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros; simpl in *; assumption.
  - intros i Hi r Hr A H. simpl in *.
    destruct (abs_i t i) eqn:E1; [|discriminate]. destruct (abs_b t r) eqn:E2; [|discriminate].
    rewrite (Hi _ eq_refl), (Hr _ eq_refl). exact H.
Qed.

Lemma check_funcs_inv : forall gs tab0 tab, check_funcs tab0 gs = Some tab ->
  length tab = length tab0 + length gs /\
  (forall i, i < length tab0 -> nth_error tab i = nth_error tab0 i) /\
  (forall i g A, nth_error gs i = Some g -> nth_error tab (length tab0 + i) = Some A ->
     abs_b tab (fbody g) = Some A /\ kind_ok (fkind g) A = true).
Proof.
  induction gs as [|g gs IH]; intros tab0 tab H; simpl in H.
  - injection H as <-. split; [simpl; lia|]. split; [intros; reflexivity|]. intros [|i] g A Hg; discriminate.
  - destruct (abs_b tab0 (fbody g)) as [A0|] eqn:E; [|discriminate].
    destruct (kind_ok (fkind g) A0) eqn:K; [|discriminate].
    destruct (IH _ _ H) as [Hlen [Hpre Hall]]. rewrite length_app in Hlen. simpl in Hlen.
    split; [simpl; lia|]. split.
    + intros i Hi. rewrite Hpre by (rewrite length_app; simpl; lia). apply nth_error_app1. exact Hi.
    + intros [|i] g' A Hg HA; simpl in Hg.
      * injection Hg as <-. rewrite Nat.add_0_r in HA.
        rewrite Hpre in HA by (rewrite length_app; simpl; lia).
        rewrite nth_error_app2, Nat.sub_diag in HA by lia. simpl in HA. injection HA as <-.
        split; [|exact K].
        destruct (IH _ _ H) as [_ [Hpre' _]].
        assert (Htab : exists u, tab = tab0 ++ u).
        { exists (skipn (length tab0) tab).
          apply (nth_error_ext). intros j.
          destruct (Nat.lt_ge_cases j (length tab0)) as [Hj|Hj].
          - rewrite nth_error_app1 by exact Hj.
            rewrite <- (nth_error_app1 tab0 [A0] Hj). apply Hpre'. rewrite length_app. simpl. lia.
          - rewrite nth_error_app2 by exact Hj. rewrite nth_error_skipn.
            f_equal. lia. }
        destruct Htab as [u ->]. apply abs_extend. exact E.
      * replace (length tab0 + S i) with (length (tab0 ++ [A0]) + i) in HA
          by (rewrite length_app; simpl; lia).
        exact (Hall i g' A Hg HA).
Qed.

(** ** The headline results. *)
Section Headline.

Variable host : list nat -> nat.
Variable m : module.
Variables (tab : list abs) (A : abs).
Hypothesis Hcheck : check m = Some (tab, A).

Lemma check_parts :
  check_funcs [] (mfuncs m) = Some tab /\ abs_b tab (mmain m) = Some A /\
  an A = None /\ exists t, ay A = Some t.
Proof.
  unfold check in Hcheck. destruct (check_funcs [] (mfuncs m)) as [t|] eqn:E; [|discriminate].
  destruct (abs_b t (mmain m)) as [B|] eqn:Eb; [|discriminate].
  destruct (an B) eqn:En; [discriminate|]. destruct (ay B) as [y|] eqn:Ey; [|discriminate].
  injection Hcheck as <- <-. repeat split; try assumption. exists y. exact Ey.
Qed.

Lemma table_wf : forall f B g, nth_error tab f = Some B -> nth_error (mfuncs m) f = Some g ->
  abs_b tab (fbody g) = Some B /\ kind_ok (fkind g) B = true.
Proof.
  intros f B g Ht Hg. destruct check_parts as [Hc _].
  destruct (check_funcs_inv _ _ _ Hc) as [_ [_ Hall]]. exact (Hall f g B Hg Ht).
Qed.

Lemma table_len : length tab = length (mfuncs m).
Proof. destruct check_parts as [Hc _]. destruct (check_funcs_inv _ _ _ Hc) as [H _]. exact H. Qed.

Let WF := fun f B g Ht Hg => proj1 (table_wf f B g Ht Hg).

Theorem function_bounded : forall f g B s,
  nth_error (mfuncs m) f = Some g -> nth_error tab f = Some B ->
  exists s' p, run_b host (mfuncs m) (ad B) (fbody g) s = Some (s', p) /\ good B s s' p.
Proof.
  intros f g B s Hg Ht. pose proof (WF f B g Ht Hg) as Hb.
  destruct (proj1 (fuel_suffices host (mfuncs m) tab WF table_len (ad B)) _ _ s Hb (le_n _))
    as [[s' p] R].
  exists s', p. split; [exact R|].
  exact (proj1 (runs_within_bounds host (mfuncs m) tab WF table_len (ad B)) _ _ _ _ _ Hb R).
Qed.

Theorem fn_never_yields : forall f g B s s' p n,
  nth_error (mfuncs m) f = Some g -> nth_error tab f = Some B -> fkind g = KFn ->
  run_b host (mfuncs m) n (fbody g) s = Some (s', p) -> exists c, p = [c].
Proof.
  intros f g B s s' p n Hg Ht Hk R. destruct (table_wf f B g Ht Hg) as [Hb Hko].
  destruct (proj1 (runs_within_bounds host (mfuncs m) tab WF table_len n) _ _ _ _ _ Hb R) as [N [L _]].
  rewrite Hk in Hko. unfold kind_ok in Hko. destruct (ay B) eqn:Ey; [discriminate|].
  destruct p as [|c [|d p']]; [contradiction|exists c; reflexivity|].
  simpl in L. destruct L as [t [Ht' _]]. congruence.
Qed.

Lemma segments_within : forall B p f mi l, le_summ (summary p) B -> ay B = Some (f, mi, l) -> p <> [] ->
  an B = None -> forall x, In x p -> x <= wcet B.
Proof.
  intros B p f mi l L Hy N Hn x Hx. pose proof (summary_elements p N) as E.
  unfold wcet. rewrite Hy.
  destruct (summary p) as [c|f' m' l'] eqn:Es; simpl in L.
  - destruct L as [c' [Hc _]]. congruence.
  - destruct L as [[[f2 m2] l2] [Ht Hle]]. rewrite Hy in Ht. injection Ht as <- <- <-.
    simpl in Hle. specialize (E x Hx). lia.
Qed.

Theorem cycle_bounded : forall s, exists s' p,
  cycle host m (ad A) s = Some (s', p) /\
  (exists f mi l, summary p = Y f mi l) /\
  (forall x, In x p -> x <= wcet A) /\ arena s' <= wcmu A.
Proof.
  intros s. destruct check_parts as [_ [Hm [Hn [[[f mi] l] Hy]]]].
  destruct (proj1 (fuel_suffices host (mfuncs m) tab WF table_len (ad A)) _ _ (reset s) Hm (le_n _))
    as [[s' p] R].
  destruct (proj1 (runs_within_bounds host (mfuncs m) tab WF table_len (ad A)) _ _ _ _ _ Hm R) as [N [L [M X]]].
  exists s', p. split; [exact R|]. split.
  - destruct (summary p) as [c|f' m' l']; simpl in L.
    + destruct L as [c' [Hc _]]. congruence.
    + eauto.
  - split; [exact (segments_within A p f mi l L Hy N Hn)|]. unfold wcmu. simpl in X. lia.
Qed.

(** Every cycle's summary is within the main bound. *)
Lemma cycle_summary : forall s s' p, cycle host m (ad A) s = Some (s', p) ->
  p <> [] /\ le_summ (summary p) A /\ arena s' <= wcmu A.
Proof.
  intros s s' p R. destruct check_parts as [_ [Hm _]].
  destruct (proj1 (runs_within_bounds host (mfuncs m) tab WF table_len (ad A)) _ _ _ _ _ Hm R) as [N [L [_ X]]].
  split; [exact N|]. split; [exact L|]. unfold wcmu. simpl in X. lia.
Qed.

Lemma stream_nonempty : forall cs, Forall (fun c => fst c <> []) cs -> stream_segs cs <> [].
Proof.
  induction cs as [|c cs IH]; intros H; [discriminate|].
  change (stream_segs (c :: cs)) with (join (fst c) (join [1] (stream_segs cs))).
  inversion H; subst. apply join_nonempty; [assumption|].
  apply join_nonempty; [discriminate|apply IH; assumption].
Qed.

Theorem stream_bounded : forall n s, exists cs,
  cycles host m (ad A) n s = Some cs /\ length cs = n /\
  (forall c, In c cs -> snd c <= wcmu A) /\
  (forall x, In x (stream_segs cs) -> x <= wcet A).
Proof.
  destruct check_parts as [_ [_ [Hn [[[F M] L] Hy]]]].
  (** The stream's summary is a yielding one within the main bound, its
      middle widened by the reset junction, or the empty stream's. *)
  assert (Hsum : forall n s cs, cycles host m (ad A) n s = Some cs ->
            Forall (fun c => fst c <> [] /\ le_summ (summary (fst c)) A /\ snd c <= wcmu A) cs /\
            (cs = [] \/ exists f mi l, summary (stream_segs cs) = Y f mi l /\
                         f <= F /\ mi <= Nat.max M (L + 1 + F) /\ l <= L + 1)).
  { induction n as [|n IH]; intros s cs Hc; simpl in Hc.
    - injection Hc as <-. split; [constructor|left; reflexivity].
    - destruct (cycle host m (ad A) s) as [[s1 p]|] eqn:R; [|discriminate].
      destruct (cycles host m (ad A) n s1) as [cs'|] eqn:Rs; [|discriminate]. injection Hc as <-.
      destruct (IH s1 cs' Rs) as [Hall Hst].
      destruct (cycle_summary s s1 p R) as [N [Lp X]].
      split; [constructor; [split; [exact N|split; [exact Lp|exact X]]|exact Hall]|right].
      assert (Hne : stream_segs cs' <> []) by (apply stream_nonempty; eapply Forall_impl; [|exact Hall]; simpl; tauto).
      assert (Hne1 : join [1] (stream_segs cs') <> []) by (apply join_nonempty; [discriminate|exact Hne]).
      change (stream_segs ((p, arena s1) :: cs')) with (join p (join [1] (stream_segs cs'))).
      rewrite summary_join by assumption. rewrite summary_join by (discriminate || exact Hne).
      destruct (summary p) as [c|f1 m1 l1] eqn:Es; simpl in Lp.
      + destruct Lp as [c' [Hc _]]. congruence.
      + destruct Lp as [[[f2 m2] l2] [Ht Hle]]. rewrite Hy in Ht. injection Ht as <- <- <-. simpl in Hle.
        destruct Hst as [->|[f3 [m3 [l3 [Es3 [B1 [B2 B3]]]]]]].
        * simpl. exists f1, m1, (l1 + (1 + 0)). repeat split; lia.
        * rewrite Es3. simpl. exists f1, (Nat.max m1 (Nat.max (l1 + (1 + f3)) m3)), l3. repeat split; lia. }
  intros n s.
  assert (Hex : forall n s, exists cs, cycles host m (ad A) n s = Some cs /\ length cs = n).
  { induction n0 as [|n0 IH]; intros s0; [exists []; split; reflexivity|].
    destruct (cycle_bounded s0) as [s1 [p [R _]]]. destruct (IH s1) as [cs [Rc Hl]].
    exists ((p, arena s1) :: cs). simpl. rewrite R, Rc. split; [reflexivity|simpl; lia]. }
  destruct (Hex n s) as [cs [Rc Hl]]. exists cs. split; [exact Rc|]. split; [exact Hl|].
  destruct (Hsum n s cs Rc) as [Hall Hst]. split.
  - intros c Hc. rewrite Forall_forall in Hall. exact (proj2 (proj2 (Hall c Hc))).
  - intros x Hx. unfold wcet. rewrite Hy.
    destruct Hst as [->|[f [mi [l [Es [B1 [B2 B3]]]]]]].
    + simpl in Hx. destruct Hx as [<-|[]]. lia.
    + assert (Hne : stream_segs cs <> []) by (apply stream_nonempty; eapply Forall_impl; [|exact Hall]; simpl; tauto).
      pose proof (summary_elements _ Hne) as E. rewrite Es in E. specialize (E x Hx). lia.
Qed.

End Headline.
