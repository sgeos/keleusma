# Order 6 completion, full native code generation

> **Navigation** [Decisions](./README.md) | [Plan](./ORDER_6_PLAN.md) | [Workstream I](./WORKSTREAM_I_NATIVE_CONSTRAINTS.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

The roadmap gives Order 6 the gate "the full language lowers to native across
the target set; native is a first-class deployment shape", which defines
V0.4.0. The requirements are those of [`ORDER_6_PLAN.md`](./ORDER_6_PLAN.md).

## The gate

**Met in part on 2026-10-25.** Everything this line can establish on its own is
established. The rest waits on decisions and capacity outside it, each named
below. V0.4.0 is not released, since publication is held.

- **Met.** Every opcode a compiler can emit lowers. Every construct of the
  grammar either agrees with the virtual machine natively or is covered by a
  named file. Tier 1 targets build, and both macOS architectures execute.
  Native code has a command of its own.
- **Not met.** A `Text` from a call, a parameter, a native or a field cannot be
  compared, a `Text` cannot be stored in a composite, and opaque handles are
  refused. Both now have rulings and are being built. The Linux targets are built and not
  executed. The native self-hosted compiler of question 19 does not exist
  yet.
- **The operator's rulings, 2026-10-28.** Every open question was answered.
  The V0.2.X dependency is met, because native code consumes the reference
  compiler's bytecode, which is question 18. The `Text`, opaque and Linux
  items now have rulings and are being built. A new V0.4.0 requirement was
  added, a self-hosted compiler compiled to native code and used as the
  compiler, which is question 19. Each item and its status is in
  [`ORDER_6_RULINGS_PLAN.md`](./ORDER_6_RULINGS_PLAN.md).

## Requirements

| Id | Requirement | Executable evidence | Disposition |
|---|---|---|---|
| A1 | Every opcode a compiler can emit lowers | `isa_lowering_census.rs`. 61 of 66 lower with corpus evidence, and each of the other five carries a disposition. `Div` and `Mod` lower and are driven by witnesses, `Len` is refused because the VM faults on it, `Reset` is consumed by the stream shape, and `IsStruct` has no producer | Met |
| A2 | Every construct of the grammar lowers or is accounted for | [`language_construct_census.rs`](../../native_codegen/tests/language_construct_census.rs) walks `GRAMMAR.md`. 32 constructs agree with the VM on several inputs, optimised and not, and 6 are covered by named files. None is refused and none is rejected | Met |
| A3 | `Text` is carried as the VM carries it | [`text_comparison.rs`](../../native_codegen/tests/text_comparison.rs). String literals compare by content and agree with the VM, including a match on literals. A comparison of any other `Text` is refused by a provenance analysis in `text.rs` | Met for literals. Other `Text` waits on a host string ABI |
| A4 | Each refusal the census finds is closed or dispositioned | The census found one refusal, an enum whose variants carry payloads of different sizes, now lowered with its slack zeroed. It found one miscompile, `Text` comparison, now repaired | Met |
| W1 | Tier 1 targets build and those this machine runs are executed | [`tier_one_targets.rs`](../../native_codegen/tests/tier_one_targets.rs) builds the lexer library for all four Tier 1 triples, with exactly the exported symbols and no imports. Both macOS architectures run it against the VM, `x86_64` under Rosetta, and run a fallible native through the System V two-register return | Met for macOS. Linux built and not executed |
| W2 | Native is a deployment shape of its own | `kel-native` builds a library with its header, bindings, archive and shared object, and inspects a contract. [`kel_native_cli.rs`](../../native_codegen/tests/kel_native_cli.rs) links what it builds and agrees with the VM, and reaches every documented exit status | Met |
| I1 | Constraints native lowering surfaced are recorded and sent | [`WORKSTREAM_I_NATIVE_CONSTRAINTS.md`](./WORKSTREAM_I_NATIVE_CONSTRAINTS.md) lists nine, each with its evidence and owner, and they were sent to the V0.2.X line | Met |

## Two defects the construct census found

1. **`Text` comparison was miscompiled, silently.** The backend compared two
   `Text` operands as integers, and the words are addresses. `"ab" == "ab"` was
   false natively and true on the VM, and comparing a call's `Text` result
   with a literal took the wrong branch. Neither was refused, so no census of
   refusals could see it. Only a construct subject that ran both sides did.
2. **An enum with payloads of different sizes was refused.** Its body is sized
   by the largest variant, and the plain route required every construction to
   pack exactly that size. A smaller variant now packs its fields and zeroes
   the rest, as the reference and the coroutine route do. The zero fill is not
   observable in any current subject, because each construction site is
   written by one variant only and the harness zeroes the region. It is kept
   because the reference specifies it.

## Residual limits

- A `Text` result of a function, a `Text` parameter, a native's `Text` and a
  `Text` field cannot be compared natively, and a `Text` cannot be a composite
  field. Ordering comparisons of `Text` are refused.
- Opaque handles are refused on every route.
- The Linux objects are built and read, not linked or run.
- The Rust lexer host, which measures the stack, ran on `arm64` only until
  2026-10-28. With the `x86_64` Rust target installed under question 15, it
  runs under Rosetta too. Its first `x86_64` run measured 1,288 bytes against
  an exported bound of 1,256, because LLVM's reported frame sizes left out a
  leaf's System V red zone. Emitted functions are now marked `noredzone`, and
  the bound is 1,304 against the same measurement. The `arm64` figures, 1,344
  measured against 1,376, were unchanged by the attribute.
- The construct census drives scalar functions. Streams, reentrant entries,
  natives, shared data and `Text` are covered by the files its rows name.

## Verification

Run locally on 2026-10-25 with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`
on a tree that did not change during the run. Logs are under ignored
`tmp/order-6/`.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 841 passed, 0 failed, status zero |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 841 passed, 0 failed, status zero |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 16 passed, status zero |

The guards that read this record and the handoff were rerun after this table
was written. These checks do not run the root workspace suite, the self-host
tier or the backend gate's optimised phase. The hosted run on the pull request
is the version-branch integration check.
