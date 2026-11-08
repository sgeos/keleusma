(* model_main.ml -- the driver around the extracted Rocq model.

   It reads core modules in the text format written by the Rust harness, runs the
   extracted checker on each, and for an accepted module runs the extracted
   interpreter for a fixed number of stream cycles under each host the case names.
   Every verdict, figure, segment and output it prints is computed by extracted
   code. The driver only parses, converts integers to the extracted unary natural
   numbers and back, and prints.

   The host of the model is a function from the outputs so far to the next reply.
   Here it answers the yield whose index is the number of earlier outputs with the
   corresponding character of the case's reply string, so it reproduces one fixed
   reply sequence, which the Rust harness feeds to the virtual machine in the same
   order.

   Negative control. With the argument --break accept-recursion the driver
   replaces, before checking and running, every call in function i to a function
   whose index is i or greater by the constant zero. That is a checker that
   ignores the calls it cannot resolve, so it accepts recursive modules. The
   harness must then report disagreements. *)

open Corebounds

exception Bad_input of string

let rec nat_of_int n = if n <= 0 then O else S (nat_of_int (n - 1))

let int_of_nat n =
  let rec go acc = function O -> acc | S m -> go (acc + 1) m in
  go 0 n

let rec list_of = function Nil -> [] | Cons (x, r) -> x :: list_of r
let rec of_list = function [] -> Nil | x :: r -> Cons (x, of_list r)

(* ---- Parsing of blocks. The nesting depth is bounded by the generator. ---- *)

let toks = ref []

let next () =
  match !toks with
  | t :: r -> toks := r; t
  | [] -> raise (Bad_input "unexpected end of block")

let int_tok () =
  let t = next () in
  match int_of_string_opt t with
  | Some n when n >= 0 -> n
  | _ -> raise (Bad_input ("expected a natural number, found " ^ t))

let rec block () =
  if next () <> "[" then raise (Bad_input "expected [");
  let rec items acc =
    match !toks with
    | "]" :: r -> toks := r; List.rev acc
    | _ -> items (instr () :: acc)
  in
  List.fold_right (fun i b -> BCons (i, b)) (items []) BNil

and instr () =
  match next () with
  | "C" -> Const (nat_of_int (int_tok ()))
  | "LD" -> Load (nat_of_int (int_tok ()))
  | "ST" -> Store (nat_of_int (int_tok ()))
  | "ADD" -> Add
  | "SUB" -> Sub
  | "LT" -> Lt
  | "Y" -> Yield
  | "CALL" -> Call (nat_of_int (int_tok ()))
  | "ALLOC" -> Alloc (nat_of_int (int_tok ()))
  | "IF" -> let t = block () in let e = block () in If (t, e)
  | "LOOP" -> let n = int_tok () in let b = block () in LoopN (nat_of_int n, b)
  | t -> raise (Bad_input ("unknown instruction " ^ t))

let parse_block words =
  toks := words;
  let b = block () in
  if !toks <> [] then raise (Bad_input "trailing tokens after block");
  b

(* ---- The negative control. ---- *)

let rec strip_b limit = function
  | BNil -> BNil
  | BCons (i, r) -> BCons (strip_i limit i, strip_b limit r)

and strip_i limit = function
  | Call f when int_of_nat f >= limit -> Const O
  | If (t, e) -> If (strip_b limit t, strip_b limit e)
  | LoopN (n, b) -> LoopN (n, strip_b limit b)
  | i -> i

let break_mode = ref ""

let prepare funcs =
  if !break_mode = "accept-recursion" then
    List.mapi (fun i f -> { f with fbody = strip_b i f.fbody }) funcs
  else funcs

(* ---- Running one case. ---- *)

let words s = List.filter (fun w -> w <> "") (String.split_on_char ' ' s)

let print_nats prefix l =
  print_string prefix;
  List.iter (fun n -> print_char ' '; print_int (int_of_nat n)) l;
  print_newline ()

let run_host m fuel ncycles replies =
  let exhausted = ref false in
  let host outs =
    let k = List.length (list_of outs) - 1 in
    if k >= 0 && k < String.length replies then
      (if replies.[k] = '1' then S O else O)
    else (exhausted := true; O)
  in
  let s0 = { stk = Nil; locs = (fun _ -> O); arena = O; outs = Nil } in
  (* The cycles one by one through the extracted [cycle], which also gives the
     final state and so the outputs. *)
  let rec go s k acc =
    if k = 0 then Some (List.rev acc, s)
    else
      match cycle host m fuel s with
      | Some (Pair (s1, p)) -> go s1 (k - 1) ((p, s1.arena) :: acc)
      | None -> None
  in
  match go s0 ncycles [] with
  | None -> print_endline "run fuel-exhausted"
  | Some (cs, final) ->
      print_endline (if !exhausted then "run replies-exhausted" else "run ok");
      List.iter (fun (p, a) -> print_nats ("cyc " ^ string_of_int (int_of_nat a)) (list_of p)) cs;
      print_nats "outs" (list_of final.outs);
      let pairs = of_list (List.map (fun (p, a) -> Pair (p, a)) cs) in
      print_nats "stream" (list_of (stream_segs pairs));
      (* The extracted [cycles] must give the same segments and arenas. *)
      let same =
        match cycles host m fuel (nat_of_int ncycles) s0 with
        | Some l ->
            List.map (fun (Pair (p, a)) -> (List.map int_of_nat (list_of p), int_of_nat a)) (list_of l)
            = List.map (fun (p, a) -> (List.map int_of_nat (list_of p), int_of_nat a)) cs
        | None -> false
      in
      print_endline (if same then "selfcheck ok" else "selfcheck mismatch")

let run_case id ncycles hosts funcs main =
  let m = { mfuncs = of_list (prepare (List.rev funcs)); mmain = main } in
  print_endline ("case " ^ id);
  match check m with
  | None -> print_endline "verdict reject"; print_endline "end"
  | Some (Pair (_, a)) ->
      print_endline "verdict accept";
      let f, mid, l =
        match a.ay with
        | Some (Pair (Pair (f, mid), l)) -> (int_of_nat f, int_of_nat mid, int_of_nat l)
        | None -> (-1, -1, -1)
      in
      Printf.printf "fig %d %d %d %d %d %d\n" (int_of_nat (wcet a)) (int_of_nat (wcmu a)) f mid l
        (int_of_nat a.ad);
      List.iteri
        (fun i r -> Printf.printf "host %d\n" i; run_host m a.ad ncycles r)
        (List.rev hosts);
      print_endline "end"

let () =
  let args = Array.to_list Sys.argv in
  (match args with
   | [ _; "--break"; mode ] -> break_mode := mode
   | [ _ ] -> ()
   | _ -> prerr_endline "usage: model [--break accept-recursion] < cases"; exit 2);
  if !break_mode <> "" && !break_mode <> "accept-recursion" then
    (prerr_endline ("model: unknown break mode " ^ !break_mode); exit 2);
  let id = ref "" and ncycles = ref 0 and hosts = ref [] and funcs = ref [] and main = ref None in
  let lineno = ref 0 in
  try
    while true do
      let line = input_line stdin in
      incr lineno;
      match words line with
      | [] -> ()
      | [ "case"; i ] -> id := i; ncycles := 0; hosts := []; funcs := []; main := None
      | [ "cycles"; n ] -> ncycles := int_of_string n
      | [ "host"; r ] -> hosts := r :: !hosts
      | "func" :: k :: rest ->
          let kind = match k with "F" -> KFn | "Y" -> KYield | _ -> raise (Bad_input "kind") in
          funcs := { fkind = kind; fbody = parse_block rest } :: !funcs
      | "main" :: rest -> main := Some (parse_block rest)
      | [ "end" ] ->
          (match !main with
           | Some b -> run_case !id !ncycles !hosts !funcs b
           | None -> raise (Bad_input "case without main"));
          flush stdout
      | _ -> raise (Bad_input "unrecognised line")
    done
  with
  | End_of_file -> ()
  | Bad_input msg ->
      Printf.eprintf "model: line %d: %s\n" !lineno msg;
      exit 1
