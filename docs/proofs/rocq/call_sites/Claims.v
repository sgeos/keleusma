(** * Claims.v -- formerly Statements.v, the statements of the call-site results, pinned.

    Each pin is a definition whose stated type must match the result's type
    up to conversion.  Statements.v pins every item of this
    file with fully qualified names, and the script counts the pins there. *)

From Stdlib Require Import List.
From RegionCore Require Import Core Shape.
From CallSites Require Import ShapeC Frame CallSites Contains.
From CallSites Require Controls.
Import ListNotations.

Definition pin_reuse_sound_calls : forall m ft, region_checkC m = Some ft ->
  forall P, injective P -> forall host fuel n,
  match cycles P host m fuel n init, cycles baseline host m fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := reuse_sound_calls.

Definition pin_frame : forall Q host fs ft,
  (forall f g, nth_error fs f = Some g -> exists R A, nth_error ft f = Some (R, A) /\ incl (sites_b ft g) A) ->
  forall n,
  (forall b s s', run_b Q host fs n b s = Some s' -> framed Q (sites_b ft b) s s') /\
  (forall i s s', run_i Q host fs n i s = Some s' -> framed Q (sites_i ft i) s s') /\
  (forall k b s s', run_loop Q host fs n k b s = Some s' -> framed Q (sites_b ft b) s s')
  := frame.

Definition pin_twice_rejected_by_B2 : region_check Controls.twice = None
  := Controls.twice_rejected_by_B2.

Definition pin_twice_accepted : region_checkC Controls.twice = Some [([TS], [1])]
  := Controls.twice_accepted.

Definition pin_twice_agrees :
  Controls.log_of Controls.P1 Controls.twice = Controls.log_of baseline Controls.twice /\
  Controls.log_of Controls.P1 Controls.twice = [ReadE (Some 7); OutE 7; ReadE (Some 7); OutE 7;
                                                 ReadE (Some 7); OutE 7; ReadE (Some 7); OutE 7]
  := Controls.twice_agrees.

Definition pin_live_across_call_rejected : region_checkC Controls.live_across_call = None
  := Controls.live_across_call_rejected.

Definition pin_live_across_call_diverges :
  Controls.log_of Controls.P1 Controls.live_across_call <> Controls.log_of baseline Controls.live_across_call
  := Controls.live_across_call_diverges.

Definition pin_calls_contains_b2 : forall m ft, region_check m = Some ft -> region_checkC m = Some (lift ft)
  := calls_contains_b2.

Definition pin_twice_sound : forall host fuel n,
  match cycles Controls.P1 host Controls.twice fuel n init, cycles baseline host Controls.twice fuel n init with
  | Some sp, Some sb => log sp = log sb
  | None, None => True
  | _, _ => False
  end
  := Controls.twice_sound.

(** The control modules, pinned by proved equations, so that what the
    controls say is about these modules. *)
Example alloc_fn_pinned : Controls.alloc_fn = Controls.blk [Const 7; New 1; Get].
Proof. reflexivity. Qed.

Example twice_pinned :
  Controls.twice = mkmodule [Controls.alloc_fn] (Controls.blk [LoopN 2 (Controls.blk [Call 0; Yield; Store 1])]).
Proof. reflexivity. Qed.

Example live_across_call_pinned :
  Controls.live_across_call = mkmodule [Controls.alloc_fn] (Controls.blk [Const 5; New 1; Call 0; Drop; Get; Yield; Drop]).
Proof. reflexivity. Qed.

Example blk_pinned : forall l, Controls.blk l = RegionCore.Controls.blk l.
Proof. reflexivity. Qed.

(** The definitions the conclusions and controls rest on, pinned by proved
    equations. *)
Example live_in_pinned : forall A s, live_in A (TR s) = existsb (Nat.eqb s) A.
Proof. reflexivity. Qed.

Example call_rule_pinned : forall ft reg S f R A, nth_error ft f = Some (R, A) ->
  shapeC_i ft reg S (Call f) = if existsb (live_in A) S then None else Some (R ++ S).
Proof.
  intros ft reg S f R A H.
  change (shapeC_i ft reg S (Call f)) with
    (match nth_error ft f with
     | Some (R, A) => if existsb (live_in A) S then None else Some (R ++ S)
     | None => None
     end).
  rewrite H. reflexivity.
Qed.

Example framed_pinned : forall Q A s s', framed Q A s s' =
  ((exists u, regs s' = regs s ++ u) /\ epoch s' = epoch s /\
   forall c, (forall s0 r, In s0 A -> length (regs s) <= r -> c <> cell_of Q s0 r) -> mem s' c = mem s c).
Proof. reflexivity. Qed.

Example P1_pinned : forall s, Controls.P1 s = match s with 1 => Some 0 | _ => None end.
Proof. reflexivity. Qed.

Example log_of_pinned : forall P m, Controls.log_of P m =
  match cycles P RegionCore.Controls.host_len m 100 2 init with Some s => log s | None => [] end.
Proof. reflexivity. Qed.
