(** * Extract.v -- extraction of increment A's interpreter and checker to OCaml.

    This file adds no definition of its own.  It extracts, unchanged, the
    interpreter [run_b], the stream functions [cycle], [cycles] and
    [stream_segs], and the checker [check] with its figures [wcet] and [wcmu],
    from [docs/proofs/rocq/core_bounds/Core.v] and [Check.v].

    Natural numbers are extracted as the unary inductive type, with no
    realisation by machine integers, so the extracted code computes exactly
    what the Rocq definitions compute.  The trusted base of the differential
    test therefore adds the extraction mechanism and the OCaml compiler to
    increment A's trusted base, and nothing else.  The cost of the unary
    representation is speed, which the generator keeps small by bounding loop
    counts and nesting. *)

From CoreBounds Require Import Core Check.
From Stdlib Require Extraction.

Extraction Language OCaml.

Extraction "corebounds.ml"
  run_b cycle cycles stream_segs reset
  check check_funcs abs_b wcet wcmu
  an ay am ad.
