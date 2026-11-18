# Termination, an Excluded Case of Increment B2

> **Navigation**: [Increment B2](../region_core/README.md) | [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md)

Increment B2's headline `reuse_sound` says the reuse run and the baseline run either both stop for
lack of fuel or both finish with the same log. It left termination among the cases it does not
cover, so for all it showed, both runs of an accepted module might always stop and the theorem
would hold by that alone. This directory closes that, in the Rocq Prover, version 9.3.0. **For
every module B2's checker accepts, a fuel `bound m` computed from the module's structure suffices
for every run to finish, for every plan, host, starting state and number of cycles. With
`reuse_sound`, the reuse run under any injective plan and the baseline run then both finish at
that fuel with the same log.** Every named result is checked with no axiom.

## Why it terminates

B2's checker checks each function against the result shapes of earlier functions only, and a call
must find its callee's shape. So a call inside function `i` names a function below `i`, and the
main block calls only functions of the module. No call chain recurses. Loops are counted, with a
constant count. `shape_calls` proves the first fact, and `check_funcs_calls` lifts it to the
whole function list.

The fuel is `fuel_b`, computed over the block structure. An instruction that neither branches,
loops nor calls needs one unit. A conditional needs one more than its larger arm. A loop of `k`
iterations needs `k` plus one more than its body. A call needs one more than its callee's fuel,
taken from a table `ftab` built in function order, so each entry depends only on earlier ones.
`runs` proves by induction on fuel that at or above this measure every block, instruction and
loop finishes, and `accepted_terminates` lifts it to any number of cycles.

## The results

| Result | Statement |
|---|---|
| `shape_calls` | A block the shape checker accepts against a table of `n` result shapes calls only functions below `n` |
| `check_funcs_calls` | Each accepted function calls only functions before it |
| `runs` | Within the fuel measure, every block, instruction and loop finishes, for any plan and host |
| `accepted_terminates` | Every accepted module finishes `n` cycles at fuel `bound m`, from any state, for any plan and host |
| `accepted_runs_agree` | For an accepted module and an injective plan, both runs finish at `bound m` with the same log |

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/termination/check.sh
```

The script follows the hardened scripts of the other directories. It builds and checks B2's
sources as its dependency, then this directory's three modules. It fails unless no source holds
an admitted or assumed proof, a warning silencer or a `Redirect`, the pin and control files
open no section and declare no hypothesis, variable or context, every module compiles with no
output, `rocqchk` reports no axiom and no unsafe construct, all 21 named results are closed, and
`Statements.v` holds its 5 pins. On 2026-11-18 a premise added to `accepted_terminates`, with
its uses patched to supply it, was refused at its pin in `Statements.v`, in a copy under `tmp/`.
No log of that run is kept in the tree. Continuous integration does not yet run the script,
under the operator's ruling that a new continuous-integration job waits for the release or for
agreement with the V0.2.X line.

## The controls

- B2's accepted loop `loop_ok` finishes at `bound loop_ok` but stops at fuel 1, so the bound does
  work and finishing is not automatic.
- A function calling itself and a function calling a later function are both rejected by B2's
  checker, which is what the termination rests on.
- `call_chain`, whose main calls a function that calls another twice, is accepted and finishes at
  its bound.
- `loop_ok_runs_agree` applies the corollary to `loop_ok` under the injective plan `P0`.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith` and `Lia`, B2's
definitions, and the statements as written. The pins fix the headline statements, two controls,
the definition of `bound`, and the one nontrivial case of `calls_i`. They do not fix `fuel_b` or
`ftab`, which only make the bound larger or smaller. The headline holds of whatever value
`bound m` takes, since it states that the run finishes at that fuel.

## What is not covered

- **The size of the bound.** It is a sufficient fuel, not a tight one, and it is not related to
  increment A's time bound, which counts cycles of a different core.
- **The join checker.** `accepted_runs_agree` uses B2's checker and an injective plan. The join
  checker of [`../shared_slots/`](../shared_slots/README.md) checks functions the same way but
  its main block with its own rules, and the same reading of its calls was not done here.
- **Keleusma itself.** Nothing relates the region core to the shipping virtual machine. The
  shipping compiler's refusal of recursion is the V0.2.X line's. This line's differential test
  reported it missing, and that line's fix, `f2346760` on pull request 778, was not merged when
  this was written.
