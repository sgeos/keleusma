# Kani Harnesses for the Verifier Core

> **Navigation**: [Tools](../README.md) | [Repository Root](../../README.md)

This package holds bounded model checking harnesses, written for the Kani Rust Verifier, over the core of the Keleusma bytecode verifier in `src/verify.rs` and over the load-time trust path of `keleusma-wire`. The operator ruled that Kani is to check the verifier core for the absence of panics and arithmetic overflow through its public application programming interface. Every harness reaches the code under check through the public interface of `keleusma` or `keleusma-wire` and through nothing else.

The package is detached. It has its own `[workspace]`, so the root workspace neither builds nor tests it, and `tools/` is excluded from the published crate. It changes no file outside `tools/`.

## Running

```sh
tools/proof-kani/check.sh
```

The script runs every harness, one at a time by default, and fails unless each gives its expected result. It fails when a harness expected to verify does not verify, when such a harness verifies with an unsatisfiable cover statement and is therefore vacuous, when a harness expected to fail does not fail with each of its listed failure descriptions, when the number of verified harnesses differs from the expected number, or when the number of `#[kani::proof]` attributes in `src/` differs from the number of harnesses the script lists.

`KANI_JOBS` sets how many harnesses run at once and defaults to one. **Serial is the safe default.** One harness can hold about eight gigabytes of memory while the model checker runs, and on 2026-11-08 eight harnesses run at once held about 63 gigabytes and nearly exhausted the swap of a shared 32 gigabyte machine. Raise it only on a machine with the memory to spare.

Each harness is gated before it starts. Where the `machine-status` machine monitor is installed, the script runs `machine-status --level` and reads its exit status, 0 for ok, 1 for elevated, 2 for critical and 3 for stale. A harness starts only at ok. At elevated the script waits and polls again every 60 seconds. At critical or stale it does not start the harness, records the harness as `BLOCKED`, and the run fails, so that the condition is reported rather than worsened. Where the monitor is absent, the script falls back to the swap figure from `sysctl vm.swapusage` and waits while more than `KANI_SWAP_LIMIT_MB`, default 10240, is in use. Where neither can be read, the harness starts ungated, so the script still works on other machines.

Where `/usr/bin/time -l` works, the script records each harness's peak memory in bytes, taken from the maximum resident set size because that figure covers the model checker while the peak memory footprint covers only `cargo`, appends it to the harness's verdict line, and writes one line per harness to `peaks.txt` in the log directory. Inside the development sandbox `/usr/bin/time -l` fails on a refused sysctl, and the peak is then recorded as `unknown` and never as zero. Ordering the largest harnesses first was considered and not adopted, because with one harness at a time the order does not change the peak load.

The toolchain is Kani 0.68.0 with the C Bounded Model Checker, CBMC, at 6.11.0 and the CaDiCaL 3.0.0 solver. `RUSTC_WRAPPER` is cleared by the script, because the global compiler cache wrapper is refused inside the development sandbox.

## Method

Kani translates the Rust program into a model and checks, for every input a harness admits, every explicit assertion, every panic, every arithmetic overflow, every division by zero and every out-of-bounds access. A harness admits inputs through `kani::any()` and narrows them with `kani::assume`. Unwinding assertions are left on, so a loop or recursion that would run past a harness's unwinding bound is reported as a failure and cannot pass silently.

**Why the chunk generators enumerate shapes.** A chunk whose instruction discriminants are symbolic defeats the constant propagation of symbolic execution. Every match on an instruction then explores every arm, and the recursive region walks compound this. A three-instruction chunk with one symbolic two-way choice of instruction did not finish within 300 seconds, while the same chunk with each choice concrete finished in 17 seconds. The generators in `src/generators.rs` therefore choose the instruction and every followed jump target by branching, and call the rest of the harness separately in each branch with a concrete instruction. Operands that the walks do not follow, such as slots, constant indices, callee indices, trap codes and the operands of `Break`, `BreakIf` and `EndLoop`, stay symbolic. Each concrete chunk shape costs about five seconds, which is what bounds the chunk sizes below.

**The instruction sets.** `with_walk_op(n)` chooses among `Stream`, `Reset`, `Yield`, `EndIf`, `Return`, `Trap`, `Break`, `BreakIf`, `EndLoop`, `Add`, `Const`, `GetLocal`, `SetLocal`, `PopN`, `Call`, `CallVerifiedNative`, `CallExternalNative`, `NewComposite`, and `If`, `Else` and `Loop` with every target from zero to `n`. Every count and argument count is at most three, because the analyses loop once per counted operand. `with_close_op(lo, n)` chooses among `Yield`, `EndIf`, `Return`, `Trap`, `EndLoop`, `Break`, `BreakIf`, and `Else` with every target from `lo` to `n`. `with_plain_op` chooses among `PushImmediate`, `Dup`, `Yield`, `Mul`, `Sub`, `Not`, `Neg`, `PopN` and `BitXor`. `any_op` chooses among all 66 constructors of `Op` with every field symbolic. A match without a wildcard arm makes this package fail to build when a constructor is added to `Op`.

## Harnesses

Times are wall-clock seconds measured on 2026-11-08 at base commit `502d2385`, on a shared machine whose load varied, so they are indicative only. Memory is the maximum resident set size where it was recorded. The wire harnesses, the finding harnesses and the negative control were run in the forms committed here. Where a row says otherwise, it says what was run.

## Status

**`check.sh` has not produced a complete green run, and it cannot pass in the committed state.** Of the 23 harnesses expected to verify, five verified in their committed form and one verified in an earlier, larger form. The first chunk-walk harness, `wcet_stream_single_op`, ran for more than 30 minutes, reached about 30 gigabytes, and was stopped. The remaining walk and module harnesses were not run, because the machine reported elevated memory pressure and starting them would have added load. The three expected failures all failed as required.

The cause of the blow-up in `wcet_stream_single_op` is not established. A hypothesis is that 33 concrete shapes in one harness, together with a symbolic cost model and symbolic text-byte cost, grow the formula faster than the five seconds per shape measured on small harnesses would suggest. The 81-shape `wcet_straight_line_is_exact`, which has a concrete cost model, completed in 521 seconds. The next step is to split each enumeration into one harness per instruction selector, measure one such harness alone, and only then restore the full list.

### Expected to verify

| Harness | Property | Bounds | Result | Time |
|---------|----------|--------|--------|------|
| `op_tables_are_total_and_agree` | For every `Op` constructor with every field symbolic, `op_depth_effect`, `stack_growth`, `stack_shrink` and the nominal cost table do not panic or overflow, the required operand count is not negative, the net effect never removes more than is required, the net effect equals `stack_growth - stack_shrink`, and the nominal cost is at least one | All 66 constructors, no loops | Verified | 7 s, 336 MB |
| `wcet_stream_single_op` | `wcet_stream_iteration_with_cost_model` does not panic or overflow on `Stream, a, Yield, Reset` for every `a` of `with_walk_op(4)`, under any cost model with every cost at most half of `u32::MAX` | 33 shapes, unwind 6 | **Not completed.** Stopped after 30 minutes at about 30 GB | over 1800 s |
| `wcet_stream_if_region` | As above on `Stream, If(t), b, Reset`, `t` from 2 to 4, `b` of `with_close_op(3, 4)` | 27 shapes, unwind 6 | Not run | |
| `wcet_stream_loop_region` | As above with `Loop(t)` in place of `If(t)` | 27 shapes, unwind 6 | Not run | |
| `wcmu_stream_single_op` | `wcmu_stream_iteration` does not panic or overflow on the single-instruction template with any `local_count`, and on success the stack bound is a multiple of the slot width and at least `local_count` slots | 33 shapes, unwind 6 | Not run | |
| `wcmu_stream_if_region` | As above on the `If` nested template | 27 shapes, unwind 6 | Not run | |
| `wcmu_stream_loop_region` | As above on the `Loop` nested template | 27 shapes, unwind 6 | Not run | |
| `wcet_whole_chunk_walk_does_not_panic` | `wcet_whole_chunk_with_cost_model` does not panic or overflow on the `Reentrant` chunk `a, Yield, Return` for every `a` of `with_walk_op(3)` | 30 shapes, unwind 6 | Not run | |
| `wcet_straight_line_is_exact` | Under the unit cost model `Stream, b1, b2, Reset` is accepted with bound exactly four, `b1` of `with_plain_op`, `b2` either `Dup` or `Yield` | 18 shapes, unwind 6 | **Verified in an earlier, larger form**, with both `b1` and `b2` drawn from `with_plain_op`, 81 shapes. The current form is a subset and was not rerun | 521 s for the earlier form |
| `wcet_is_monotone_in_the_cost_model` | For two class-based cost models with low at most high and high at most `2^20`, acceptance agrees and the low bound is at most the high bound, on `Stream, a, Yield, Reset` for every `a` of `with_walk_op(4)` | 33 shapes, two calls each, unwind 6 | Not run | |
| `module_call_depth_is_bounded` | `module_call_depth` rejects exactly the cyclic call graphs and otherwise returns a depth from one to three, and one when there is no `Call` | Three chunks, two enumerated over `Return` and `Call` to callee 0 to 3, 25 shapes, unwind 5 | Not run in its current form. An earlier form with 125 shapes did not finish | |
| `module_wcmu_does_not_panic` | `module_wcmu` does not panic or overflow on two `Func` chunks, `a, Return` and `Return`, for every `a` of `with_walk_op(2)`, and reports one entry per chunk | 27 shapes, unwind 6 | Not run | |
| `verify_does_not_panic` | `verify` does not panic or overflow on the `Stream` chunk `Stream, a, Yield, Reset` for every `a` of `with_walk_op(4)` | 33 shapes, unwind 6 | Not run | |
| `maj3_is_bitwise_majority` | `maj3` is the bitwise majority, symmetric, and two agreeing copies outvote any third | All `u8` triples | Verified | 7 s |
| `crc32_detects_bit_errors_in_four_bytes` | `crc32` changes under every one-bit and every two-bit flip of a four-byte message | All four-byte messages, unwind 9 | Verified | 14 s |
| `secded_corrects_single_bit_errors` | `decode_word` reports a clean word clean, and corrects every one of the 72 single-bit flips of the codeword back to the original word | All `u64` words, unwind 73 | Verified | 24 s |
| `secded_detects_double_bit_errors` | `decode_word` reports every flip of two distinct codeword bits as uncorrectable, so it never miscorrects | All `u64` words and bit pairs, unwind 73 | Verified | 171 s |

### Expected to fail

| Harness | Purpose | Required failure | Result | Time |
|---------|---------|------------------|--------|------|
| `finding_f1_wcet_overhead_overflows` | Reproduces finding F1 | `attempt to add with overflow` in `wcet_stream_iteration_with_cost_model` | Failed as required | 13 s |
| `finding_f2_wcmu_slot_width_unvalidated` | Reproduces finding F2 | `attempt to divide by zero` in `wcmu_region_at` and `attempt to multiply with overflow` in `wcmu_stream_iteration_with_value_slot_bytes` | Failed as required | 11 s |
| `negative_control_false_wcet_bound` | Negative control, a false bound | The harness's own assertion | Failed as required | 31 s |

## Negative control

`negative_control_false_wcet_bound` asserts that a straight-line chunk `Stream, b, Reset` costs at most two cycles under the unit cost model, with `b` any instruction of `with_plain_op`. The true cost is three, as `wcet_straight_line_is_exact` establishes. The negative control runs through the same continuation-based enumeration and the same public entry as the passing harnesses. Kani reports the assertion as failed, and `check.sh` requires that failure. A green run of the passing harnesses is evidence only together with this failure, because it shows that the toolchain, the enumeration and the entry point really check the property they are given.

The finding harnesses act as further controls. Each fails on an overflow or a division by zero inside `keleusma::verify`, which shows that Kani's arithmetic checks reach into the verifier code and are not confined to the harness.

## Findings for the V0.2.X line

These are real defects in `src/verify.rs`. They are recorded here and are not fixed, because that code belongs to the V0.2.X line. Each is reproduced by a harness that is expected to fail and is excluded from the pass count.

### F1. Unchecked sum of the stream envelope costs

`wcet_stream_iteration_with_cost_model` computes `cost_model.cycles(&ops[stream_pos]) + cost_model.cycles(&ops[reset_pos])` with a plain addition. Every other cost sum in the walk, as read on 2026-11-08, saturates. A host cost model that charges more than half of `u32::MAX` for `Stream` and `Reset` together therefore panics in a build with overflow checks and wraps in a build without them. The wrapped value is a worst-case execution time bound, a WCET bound, that is far too small, which is the unsound direction.

Minimal reproduction, reproduced by `finding_f1_wcet_overhead_overflows`.

```rust
use keleusma::bytecode::{BlockType, Chunk, CostModel, Op, VALUE_SLOT_SIZE_BYTES};

fn huge(_: &Op) -> u32 {
    u32::MAX
}

let chunk = Chunk {
    name: "k".into(),
    ops: vec![Op::Stream, Op::Yield, Op::Reset],
    constants: vec![],
    struct_templates: vec![],
    local_count: 1,
    param_count: 0,
    block_type: BlockType::Stream,
    param_types: vec![],
    debug_pool: None,
};
let cm = CostModel { value_slot_bytes: VALUE_SLOT_SIZE_BYTES, op_cycles: huge, text_byte_cycles: 1 };
// Panics with "attempt to add with overflow" when overflow checks are on.
let _ = keleusma::verify::wcet_stream_iteration_with_cost_model(&chunk, &cm, &[]);
```

The likely remedy is `saturating_add` at that line, matching the rest of the walk. The passing WCET harnesses bound every per-instruction cost at half of `u32::MAX` so that they check the rest of the walk past this defect.

### F2. Unvalidated slot width in the memory bound

`wcmu_stream_iteration_with_value_slot_bytes` takes the host-supplied slot width `value_slot_bytes` and never validates it.

1. A slot width of zero makes the `Op::Call` arm of `wcmu_region_at` divide by zero in `callee_stack_bytes / value_slot_bytes`, which panics in every build.
2. A large slot width overflows the plain product `stack_slots * value_slot_bytes`. That panics with overflow checks on and wraps without them, giving a worst-case memory usage bound, a WCMU bound, that is too small.

Minimal reproduction, reproduced by `finding_f2_wcmu_slot_width_unvalidated`, with the chunk built as in F1 but with `ops` set to `Stream, Call(0, 0), Yield, Reset`.

```rust
// Panics with "attempt to divide by zero".
let _ = keleusma::verify::wcmu_stream_iteration_with_value_slot_bytes(&chunk, 0);
// With local_count = 65535, panics with "attempt to multiply with overflow".
let _ = keleusma::verify::wcmu_stream_iteration_with_value_slot_bytes(&chunk, 1 << 20);
```

**Inferred by reading and not checked.** The same unchecked product appears in `wcmu_whole_chunk_with_value_slot_bytes` and in `compute_chunk_wcmu`, which `module_wcmu_with_value_slot_bytes` reaches, so those entries are expected to share both failure modes. At the default slot width of 32 bytes the product cannot overflow within one chunk of the sizes checked here. Whether a long call chain in `module_wcmu` can accumulate enough stack slots to overflow it at the default width was not checked.

## Trusted base

A result here is only as good as the following, none of which this package checks.

- **Kani, CBMC and the solver.** The translation of Rust to the model, the symbolic execution and the satisfiability solver are trusted. The negative control guards against the grossest failure of this base, a checker that checks nothing, and does not guard against subtler ones.
- **The Rust standard library.** Kani models `alloc` and `core` with its own library, and their behaviour is trusted.
- **The generators.** A property holds only for the inputs a generator admits. Each harness states its inputs, and the cover statements show that accepted and rejected inputs are both reached.
- **The default feature set.** The harnesses check `keleusma` with its default features, `compile`, `verify` and `floats`, at the default 64-bit word, address and float widths. The narrow-word, narrow-address and float-free configurations are not checked.

## Limits

**A bounded model check is not a proof beyond its bounds.** Every verified property holds for the stated chunk shapes, lengths, counts and unwinding bounds and for no others. In particular the walk harnesses check chunks of three or four instructions, so they say nothing direct about deeper nesting, about loop iteration bounds extracted from the canonical counted-loop pattern, which needs a longer chunk, or about the region nesting limit of 256.

**The signed operand-depth arithmetic is exercised only at small magnitudes.** The memory walk keeps the running operand depth in an `i32` and adds to it without a check. With at most four instructions and counts of at most three, the depth stays small, so these harnesses cannot reach an overflow there even if one exists for long chunks.

**Functions and properties not covered, and why.**

- `module_wcet_with_bounds`, `module_wcmu_with_bounds`, `verify_resource_bounds` and its variants, `module_runtime_footprint`, `max_resumes_before_reset`, `module_chunk_text_analyses` and `chunk_verification_obligations` were not given harnesses. They compose the entries that were checked, and each additional entry costs several minutes of checking per shape set.
- Shared data layouts are not generated, so `data_layout` is always absent and the shared-composite copy-out path is not reached.
- Constants are never generated, so the text-length analysis sees no string literal and its length-dependent costs are reached only through the unbounded case.
- No property relates a bound to a concrete execution of the virtual machine. That a WCET bound is at least the cost of a real run would need the interpreter inside the model, which is far beyond what these bounds allow.
- The typed operand-stack pass and the definite-initialization pass are reached only through `verify_does_not_panic`, with one-instruction bodies.
- `keleusma-wire` is checked only at its primitives, `maj3`, `crc32`, `check_byte` and `decode_word`. The container reader `WireView`, the directory vote over three copies, `scrub` and `EccPlane` were not given harnesses.
- The cyclic redundancy check harness covers messages of exactly four bytes. It shows detection of every one-bit and two-bit error at that length and no more.
