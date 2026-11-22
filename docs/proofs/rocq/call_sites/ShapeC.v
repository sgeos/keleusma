(** * ShapeC.v -- a confinement checker that lets functions allocate.

    Increment B2's checker forbids allocation inside functions, which
    excludes the case of a site executed twice in one scope through two
    calls.  This checker lets a function allocate, on two conditions.  Its
    result is all scalars, as before, so no handle escapes it.  And a call
    is refused while the caller holds a live handle of any site the callee
    may allocate, directly or through its own calls, because the callee's
    allocation would then write the slot that handle names.  Each entry of
    the function table records a function's result shape and its allocation
    sites, computed in function order.  Every other rule is B2's. *)

From Stdlib Require Import List Arith Lia Bool.
From RegionCore Require Import Core Shape.
Import ListNotations.

(** A function's result shape and the sites it may allocate. *)
Definition fentry : Type := (list tag * list nat)%type.

(** Whether an abstract entry is a live handle of a site in [A]. *)
Definition live_in (A : list nat) (t : tag) : bool :=
  match t with TR s => existsb (Nat.eqb s) A | TS => false end.

Fixpoint shapeC_b (ft : list fentry) (regions : bool) (S : list tag) (b : block)
  : option (list tag) :=
  match b with
  | BNil => Some S
  | BCons i r =>
      match shapeC_i ft regions S i with
      | Some S1 => shapeC_b ft regions S1 r
      | None => None
      end
  end
with shapeC_i (ft : list fentry) (regions : bool) (S : list tag) (i : instr)
  : option (list tag) :=
  match i with
  | Const _ | Load _ => Some (TS :: S)
  | Store _ => match tpop S with (TS, r) => Some r | _ => None end
  | Add | Sub | Lt =>
      match tpop S with
      | (TS, r) => match tpop r with (TS, r2) => Some (TS :: r2) | _ => None end
      | _ => None
      end
  | If t e =>
      match tpop S with
      | (TS, r) =>
          match shapeC_b ft regions r t, shapeC_b ft regions r e with
          | Some T, Some E => if list_eq_dec tag_eq_dec T E then Some T else None
          | _, _ => None
          end
      | _ => None
      end
  | LoopN _ body =>
      match shapeC_b ft regions S body with
      | Some S' => if list_eq_dec tag_eq_dec S' S then Some S else None
      | None => None
      end
  | Call f =>
      match nth_error ft f with
      | Some (R, A) => if existsb (live_in A) S then None else Some (R ++ S)
      | None => None
      end
  | Yield => match tpop S with (TS, r) => Some (TS :: r) | _ => None end
  | New s =>
      if regions then
        match tpop S with
        | (TS, r) => if in_dec tag_eq_dec (TR s) r then None else Some (TR s :: r)
        | _ => None
        end
      else None
  | Get =>
      if regions then
        match tpop S with (TR _, r) => Some (TS :: r) | _ => None end
      else None
  | Dup => let (t, r) := tpop S in Some (t :: t :: r)
  | Drop => Some (snd (tpop S))
  end.

(** The sites a block may allocate, directly or through calls. *)
Fixpoint sites_b (ft : list fentry) (b : block) : list nat :=
  match b with BNil => [] | BCons i r => sites_i ft i ++ sites_b ft r end
with sites_i (ft : list fentry) (i : instr) : list nat :=
  match i with
  | If t e => sites_b ft t ++ sites_b ft e
  | LoopN _ body => sites_b ft body
  | Call f => match nth_error ft f with Some (_, A) => A | None => [] end
  | New s => [s]
  | _ => []
  end.

(** Functions are checked in order, each against the earlier ones, with
    allocation allowed and an all-scalar result required. *)
Fixpoint check_funcsC (ft : list fentry) (gs : list block) : option (list fentry) :=
  match gs with
  | [] => Some ft
  | g :: gs' =>
      match shapeC_b ft true [] g with
      | Some R => if forallb is_ts R then check_funcsC (ft ++ [(R, sites_b ft g)]) gs' else None
      | None => None
      end
  end.

Definition region_checkC (m : module) : option (list fentry) :=
  match check_funcsC [] (mfuncs m) with
  | Some ft => match shapeC_b ft true [] (mmain m) with Some _ => Some ft | None => None end
  | None => None
  end.

(** ** The table only grows, and earlier entries keep their meaning. *)
Lemma shapeC_extend : forall ft u,
  (forall b reg S S', shapeC_b ft reg S b = Some S' ->
     shapeC_b (ft ++ u) reg S b = Some S' /\ sites_b (ft ++ u) b = sites_b ft b).
Proof.
  intros ft u.
  apply (block_mut (fun i => forall reg S S', shapeC_i ft reg S i = Some S' ->
                               shapeC_i (ft ++ u) reg S i = Some S' /\ sites_i (ft ++ u) i = sites_i ft i)
                   (fun b => forall reg S S', shapeC_b ft reg S b = Some S' ->
                               shapeC_b (ft ++ u) reg S b = Some S' /\ sites_b (ft ++ u) b = sites_b ft b)).
  1-6: intros; simpl in *; split; [assumption|reflexivity].
  - intros te Ht e He reg S S' H. simpl in *.
    destruct (tpop S) as [[|q] r]; [|discriminate].
    destruct (shapeC_b ft reg r te) as [T|] eqn:E1; [|discriminate].
    destruct (shapeC_b ft reg r e) as [E|] eqn:E2; [|discriminate].
    destruct (Ht _ _ _ E1) as [A1 B1]. destruct (He _ _ _ E2) as [A2 B2].
    rewrite A1, A2, B1, B2. split; [exact H|reflexivity].
  - intros k body Hb reg S S' H. simpl in *.
    destruct (shapeC_b ft reg S body) as [S2|] eqn:E1; [|discriminate].
    destruct (Hb _ _ _ E1) as [A1 B1]. rewrite A1, B1. split; [exact H|reflexivity].
  - intros f reg S S' H. simpl in *. destruct (nth_error ft f) as [[R A]|] eqn:E1; [|discriminate].
    rewrite nth_error_app1 by (apply nth_error_Some; rewrite E1; discriminate). rewrite E1.
    split; [exact H|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros; simpl in *; split; [assumption|reflexivity].
  - intros i Hi r Hr reg S S' H. simpl in *.
    destruct (shapeC_i ft reg S i) as [S1|] eqn:E1; [|discriminate].
    destruct (Hi _ _ _ E1) as [A1 B1]. destruct (Hr _ _ _ H) as [A2 B2].
    rewrite A1, B1, B2. split; [exact A2|reflexivity].
Qed.

Lemma check_funcsC_inv : forall gs ft0 ft, check_funcsC ft0 gs = Some ft ->
  (exists u, ft = ft0 ++ u) /\ length ft = length ft0 + length gs /\
  (forall i g R A, nth_error gs i = Some g -> nth_error ft (length ft0 + i) = Some (R, A) ->
     shapeC_b ft true [] g = Some R /\ forallb is_ts R = true /\ A = sites_b ft g).
Proof.
  induction gs as [|g gs IH]; intros ft0 ft H; simpl in H.
  - injection H as <-. split; [exists []; rewrite app_nil_r; reflexivity|].
    split; [simpl; lia|]. intros [|i] g R A Hg; discriminate.
  - destruct (shapeC_b ft0 true [] g) as [R0|] eqn:E; [|discriminate].
    destruct (forallb is_ts R0) eqn:K; [|discriminate].
    destruct (IH _ _ H) as [[u Hu] [Hl Hall]].
    split; [exists ([(R0, sites_b ft0 g)] ++ u); rewrite Hu, app_assoc; reflexivity|].
    split; [rewrite Hl, length_app; simpl; lia|].
    intros [|i] g' R A Hg HR; simpl in Hg.
    + injection Hg as <-. rewrite Nat.add_0_r, Hu, <- app_assoc in HR.
      rewrite nth_error_app2, Nat.sub_diag in HR by lia. simpl in HR. injection HR as <- <-.
      rewrite Hu, <- app_assoc. destruct (shapeC_extend ft0 ([(R0, sites_b ft0 g)] ++ u) g true [] R0 E) as [A1 B1].
      split; [exact A1|]. split; [exact K|]. symmetry. exact B1.
    + replace (length ft0 + S i) with (length (ft0 ++ [(R0, sites_b ft0 g)]) + i) in HR
        by (rewrite length_app; simpl; lia).
      exact (Hall i g' R A Hg HR).
Qed.
