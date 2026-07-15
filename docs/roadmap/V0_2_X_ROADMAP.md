# V0.2.X Roadmap: Self-Hosting the Toolchain

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Active plan. This document sequences the V0.2.x release line. It is a plan,
not a promise; the version-to-workstream mapping below is indicative and the operator
revises it as increments land. The architecture of the self-hosted compiler stays
authoritative in [`V0_3_0_SELF_HOSTING.md`](./V0_3_0_SELF_HOSTING.md); the release-by-release
compiler-stage plan stays in [`compiler/MILESTONES.md`](../../compiler/MILESTONES.md). This
document is broader than either: it covers the compiler, the validator, the runtime, the
cryptography, and a new trap analysis, and it states the version semantics that reconcile
them.

## Purpose and version semantics

The goal of the V0.2.x line is to move the entire Keleusma toolchain from Rust into
Keleusma, one reviewable increment per release, and to reach a self-hosting fixed point
over a deliberate language subset. The nominally complete compiler, meaning full-language
support, is the V0.3.0 release.

This re-scopes two earlier framings and both should be aligned to match:

- `compiler/MILESTONES.md` and `V0_3_0_SELF_HOSTING.md` currently state that completing the
  self-hosted compiler **is** V0.3.0. Under this roadmap, self-hosting over the subset is a
  V0.2.x milestone, and V0.3.0 is instead defined by **full-language** support. Self-hosting
  is a precondition for V0.3.0, not V0.3.0 itself.
- The runtime-in-Keleusma work described in
  [`V0_5_0_KELEUSMA_HOST.md`](./V0_5_0_KELEUSMA_HOST.md) is pulled forward into the V0.2.x
  line as Workstream D below. That document stays authoritative for the sub-coroutine
  runtime primitive it introduces; this roadmap schedules the meta-circular hosted runtime.

The distinction that organizes everything here is **first pass versus full language**. The
first pass of every workstream targets only the subset the toolchain's own source is written
in, which is enough to self-host. Full-language support is the last workstream and its
completion is V0.3.0.

## Current baseline (honest state)

What exists today, so the plan starts from fact rather than aspiration:

- **Compiler front and middle, self-compiling in tests.** Five Keleusma stages exist in
  `compiler/kel/`: `lexer.kel`, `parse.kel`, `reconstruct.kel`, `codegen.kel`, and
  `analyze.kel`. The first four self-compile byte-identically to the Rust compiler. This
  covers lexing, parsing, AST reconstruction, and bytecode code generation.
- **Resource analysis and validator, self-hosting in tests only.** `analyze.kel` computes
  per-iteration WCET and WCMU and emits an arena-capacity validation verdict that is a
  drop-in match for `verify_resource_bounds`, including transitive-call WCMU. This is proven
  in `tests/selfhost_codegen.rs`. It is **not** wired into the `keleusma-selfhost` binary,
  which still runs four stages and borrows the module scaffold and bounds from the reference.
- **Everything else is Rust.** Type checking, monomorphization, the structural verifier, the
  typed operand-stack verifier, wire-format serialization, the module scaffold assembly, the
  VM runtime, the marshalling boundary, the target descriptor, and all cryptography.

See the residual register in [`docs/process/REVERSE_PROMPT.md`](../process/REVERSE_PROMPT.md)
for the finer-grained open items behind this summary.

## Workstreams

Each workstream lists its first-pass scope (subset, enough to self-host) and its
full-language scope (deferred toward V0.3.0). The trust story that ties them together is in
[The oracle and trust story](#the-oracle-and-trust-story) below: for every self-hosted
component, the Rust reference stays as a differential oracle until the component is
independently reviewed.

### A. Compiler stages in Keleusma

The front and middle of the compiler, ported until the whole compile path is Keleusma and
the host is glue.

**First pass (self-hosting subset).**

- Complete the type checker in Keleusma. The compiler's own source uses monomorphic
  `Word`/`Byte` code, so the first-pass checker need only cover that subset. This is the
  largest single stage in Rust (`src/typecheck.rs`) and the highest-risk port because
  Hindley-Milner inference is not a streaming shape; see the recursion and inference-scope
  resolutions in `V0_3_0_SELF_HOSTING.md`.
- Complete the monomorphizer in Keleusma. Over the subset the toolchain source uses no
  generics, so first-pass monomorphization is close to identity; the effort is real only
  when full-language generics arrive (Workstream F).
- Self-host wire-format serialization. Today `codegen.kel` emits opcode records into shared
  memory and a Rust driver frames them into a module and calls `to_bytes`. The framing
  header, operand-pool encoding, parity, and CRC trailer must move into Keleusma so the
  emitted artifact is produced end to end by the self-hosted path.
- Self-host the module scaffold assembly. The `DataLayout`, enum layouts, typed-verifier
  signatures, schema hash, and chunk-table metadata are assembled in a Rust test driver
  today; they must be emitted by the self-hosted codegen.
- Close the reconstruct gaps the subset needs: a conditional used as a call argument, and a
  user-written `break;` statement (a parse plus reconstruct plus codegen node).
- Integrate into the shipping tool. The self-hosted stages and scaffold assembly must be
  driven by the `keleusma-selfhost` binary, not only by `tests/selfhost_codegen.rs`. This is
  the highest-leverage residual: the artifact must match the claim.

**Full language (Workstream F).** Widen every stage to the full grammar.

### B. Validator in Keleusma

The full load-time verifier, not only the resource-bound admission that is already
self-hosted.

**First pass.**

- Structural verifier: block nesting and offset validation, block-type constraints, and
  productive-divergence and yield-coverage analysis (`verify.rs` structural passes).
- Typed operand-stack verifier: the A.2.1 abstract-interpretation pass (`verify_typed.rs`)
  that reconstructs each operand and slot shape and validates every baked offset.
- Fold in the one unmodelled WCMU term, the text-size string-allocation bound
  (`text_size.rs`). It is zero for every text-free program, so it does not block the
  text-free toolchain, but it is required for a universal validator.
- Self-host the transitive orchestration. `analyze.kel` computes each chunk in isolation;
  the topological call-graph ordering and per-chunk memoization live in the Rust driver
  today and should move into Keleusma so the whole validator, not just the per-chunk kernel,
  is self-hosted.
- Integrate into the shipping tool, as in Workstream A.

**Full language.** Extend the shapes the verifier reconstructs to cover every full-language
value form (Workstream F).

### C. Unhandled-trap analysis (new)

A static analysis that proves a program cannot reach a runtime trap, so that running only
provably non-trapping programs becomes a host-selectable mode. This is additive to the
validator and is itself a self-hosting candidate.

- **Scope of traps to close.** Checked-arithmetic overflow (add, sub, mul, neg, div, mod),
  division and modulo by zero, array and indexed-data bounds, `LoopLimitExceeded` for a
  bare `for .. limit`, cast range violations, newtype refinement failures, and unhandled
  native errors.
- **Method.** A partial operation is trap-free when its operands are provably in range or it
  carries an outcome handler (the checked-arithmetic and index-guard constructs, the
  `on { .. }` loop-outcome block). Prove the in-range case with interval and refinement
  reasoning; the interval substrate already exists in Rust (`src/interval.rs`) and the
  refinement and value-range machinery in `value_layout.rs` and the newtype system.
- **Output.** A `trap-free` verdict per chunk and per module. Two host uses follow: refuse
  to load a possibly-trapping program when the host requests trap-free-only, and permit a
  no-check execution mode that omits the runtime guards a proven-total program cannot need.
- **First pass versus full language.** The first pass covers the trap classes the toolchain
  source can raise (arithmetic and bounds over `Word`/`Byte`); the newtype-refinement and
  native-error classes widen with Workstream F.

### D. Runtime in Keleusma (hosted meta-circular VM)

A Keleusma program that is a bytecode interpreter for Keleusma, running on the Rust VM. It
reads a module's bytes and executes it. This is the runtime-in-Keleusma goal; it aligns with
and pulls forward [`V0_5_0_KELEUSMA_HOST.md`](./V0_5_0_KELEUSMA_HOST.md), which stays
authoritative for the sub-coroutine primitive the eventual native host needs.

- **First pass.** Interpret the bytecode the self-hosted compiler emits over the subset:
  the opcode dispatch loop, the operand stack, the call-frame discipline, the arena model,
  and the Stream, Yield, and Reset control-flow semantics.
- **The native seam stays host.** Calls to host-registered native functions cross a
  marshalling boundary that is inherently a host concern; the meta-circular runtime defines
  and calls that ABI but does not absorb it. See Workstream E and the marshalling boundary
  in `src/marshall.rs`.
- **Bound tension to resolve.** A meta-circular interpreter is a `loop` whose per-tick cost
  is the interpreter overhead times the interpreted program's structure, so its WCET and
  WCMU are parametric on the interpreted module rather than constant. The design must state
  how the bound composes: the host budget for the interpreter must dominate the declared
  budget of the interpreted program. This is an open design question, flagged below.

### E. Signing and encryption in Keleusma

Move the cryptographic framing and, optionally, the primitives into Keleusma. This
workstream carries a principle constraint that must be stated up front: Keleusma forbids
**inventing** cryptography, not implementing a **published standard**. SHA-256, Ed25519,
X25519, and AES-GCM are standards, and re-implementing them from their specifications is
permitted, but it is not free of risk and must be gated by test-vector validation and a
constant-time and side-channel review.

Two approaches, sequenced:

1. **Orchestrate host-native primitives (near-term, recommended first).** Keep the
   primitives (`ed25519-dalek`, `x25519-dalek`, `aes-gcm`, `sha2`, `hkdf`) as host natives.
   Move into Keleusma the framing logic that surrounds them: what bytes are signed, the
   signature and encryption metadata layout, the recipient-key fingerprint flow, and the
   verify-before-decrypt ordering. This gets the orchestration self-hosted with no new
   cryptographic risk.
2. **Implement the standard algorithms in Keleusma (later, review-gated).** Re-implement
   SHA-256, Ed25519, X25519, and AES-GCM in Keleusma from their standards. Keleusma's total,
   WCET-bounded, branch-disciplined nature is a good fit for constant-time implementation,
   and the absence of secret-dependent divergence is exactly what the verifier can help
   attest. Each primitive lands only behind published test vectors and an explicit security
   review, and the host-native path remains the trusted default until then.

Both approaches keep the wire format and the `signatures` and `encryption` cargo features as
they are; only the implementation locus moves.

### F. Full-language support (defines V0.3.0)

Widen the self-hosted compiler and validator from the self-hosting subset to the whole
language. This is the largest and last workstream, and its completion is the V0.3.0 release.

The gaps to close, from the current subset to the full grammar:

- **Types.** `Float` and float operations, `Fixed<N>` multi-word fixed-point, `Text` and
  strings, `bool` as a first-class value, structs, tuples, array literals, payload-bearing
  enum variants, newtypes with refinement, generics, and traits.
- **Expressions.** Float, fixed, string, bool, array, and tuple literals; struct
  construction; field and tuple access; method calls; the pipe operator; and the
  checked-outcome constructs (checked arithmetic, index guards, newtype construction,
  discriminant-to-enum, native-error handling), plus classify and declassify for
  information-flow labels.
- **Statements and declarations.** `assert`; `struct`, payload `enum`, `newtype`, `trait`,
  and `impl` declarations; and the function modifiers `signed`, `ephemeral`, `pure`, and
  `external`.
- **Operators and lexing.** Eager `and`, `or`, `xor`; the shift mnemonics `lsl`, `asl`,
  `lsr`, `asr`; `bnot`; hexadecimal and binary integer notation; numeric type suffixes; and
  information-flow label syntax.
- **Generics and monomorphization.** The full monomorphizer (Workstream A's deferred half):
  generic functions, structs, and enums, const generics, trait bounds, and specialization.

## Dependency ordering and indicative release mapping

The workstreams are not independent. The ordering below is by dependency; the version
mapping is indicative and revised as increments land (per the MILESTONES convention that
version numbers past the current release are a plan, not a promise).

| Order | Milestone | Workstreams | Gate |
|-------|-----------|-------------|------|
| 1 | Compiler self-hosting subset, wired into the binary | A (first pass) | The `keleusma-selfhost` tool compiles all five `.kel` stages end to end with no Rust scaffold borrow, and the result is byte-identical to the reference. |
| 2 | Full validator in Keleusma | B (first pass) | The self-hosted validator reproduces the whole `verify()` verdict (structural, typed, resource) for the stage corpus, diff-tested against the reference. |
| 3 | Trap analysis in the validator | C (first pass) | A `trap-free` verdict that the reference agrees with on a trapping and non-trapping corpus; a host no-check mode gated on it. |
| 4 | Hosted runtime in Keleusma | D (first pass) | The meta-circular interpreter runs the self-hosted compiler's own output for the subset, with the interpreter-versus-interpreted bound composition resolved. |
| 5 | Cryptography orchestration | E (approach 1); E (approach 2) opt-in | Signing and encryption framing self-hosted with host-native primitives; primitive re-implementation is a separate, review-gated opt-in. |
| 6 | **Full-language support → V0.3.0** | F, plus A/B/C widening | The self-hosted compiler and validator accept the full grammar and the toolchain compiles arbitrary conforming programs, not only its own subset. |

Steps 2 through 5 can overlap once step 1 lands, because the validator, the runtime, and the
cryptography depend on a self-hosting compiler but not tightly on each other. Step 6 depends
on all of 1 through 3 (the compiler, the validator, and the trap analysis must widen
together) and benefits from 4 and 5 but does not strictly require them.

## The oracle and trust story

Self-hosting a verifier and cryptography raises a trust question a self-hosting compiler does
not: a wrong compiler produces wrong programs that fail visibly, but a wrong **validator**
admits unsafe programs silently, and wrong **cryptography** fails closed at best and leaks at
worst. The discipline for the whole line:

- **The Rust reference stays as a differential oracle** for every self-hosted component until
  that component is independently reviewed. Each self-hosted stage, validator pass, and trap
  analysis is diff-tested for byte-identical or verdict-identical agreement with the Rust
  reference over a growing corpus, exactly as `analyze.kel` is today.
- **Safety-critical analyses get independent review before the reference is retired.** The
  resource, structural, typed, and trap analyses reimplement audited logic in a second
  language; agreement with the reference is necessary but not sufficient, because the corpus
  is not exhaustive. An independent review of each against its Rust source is a gate, not an
  afterthought.
- **Cryptography lands against published test vectors and a side-channel review**, and the
  host-native primitives remain the trusted default until the Keleusma re-implementations
  clear that gate. See Workstream E.

## Cross-cutting concerns (the "anything else" list)

Items that are not a single workstream but must not be forgotten:

- **Native ABI definition.** The boundary between the Keleusma runtime and host-registered
  natives (`marshall.rs`) is a permanent host seam; its ABI must be specified, not just
  implemented, because both the meta-circular runtime and the eventual native host call it.
- **Target descriptor and cross-architecture.** The self-hosted compiler must bake target
  widths and capability flags (`target.rs`) so narrow and no-float targets stay supported.
- **Debug metadata.** The strippable debug section (`debug_meta.rs`) must be emitted by the
  self-hosted codegen for stack traces and fault highlighting.
- **Standard library.** A nominally complete compiler and runtime imply a growing standard
  library of host-registered natives and Keleusma-side helpers; its scope grows across the
  line.
- **Diagnostics.** The self-hosted stages carry minimal error reporting today; usable
  diagnostics (source spans, messages) are required before the reference compiler is retired
  as the user-facing tool.
- **Determinism and reproducibility.** The self-hosted toolchain must be byte-reproducible so
  the fixed-point and differential-oracle checks are meaningful.
- **Test-infrastructure migration.** The self-hosting proofs live in the root crate's
  `tests/selfhost_codegen.rs` today; they should move into the `compiler/` subproject and
  drive the shipping binary, closing the "proven in tests, not shipped" gap.

## Open decisions

Genuine forks the operator resolves; the plan above records a default for each but flags it:

1. **Cryptography implementation locus.** Orchestrate host-native primitives only (safe,
   recommended first), or also re-implement the standards in Keleusma (opt-in, review-gated).
   Default: approach 1 first, approach 2 as a separate later milestone. See Workstream E.
2. **Meta-circular runtime bound composition.** How the interpreter's WCET and WCMU compose
   with the interpreted program's declared budget, and whether the hosted runtime targets
   bounded execution of bounded programs only or admits the productive-divergent `loop` case
   directly. See Workstream D and `V0_5_0_KELEUSMA_HOST.md`.
3. **Version granularity.** Whether self-hosting-over-subset earns its own tagged V0.2.x
   release before V0.3.0, or is a continuous internal milestone. Default: continuous, per the
   MILESTONES "plan, not promise" convention.
4. **Reference retirement.** When, if ever, the Rust reference stops being the user-facing
   tool and the differential oracle. Default: not before independent review of the
   self-hosted validator and cryptography.

## Relationship to later roadmaps

- [`V0_3_0_SELF_HOSTING.md`](./V0_3_0_SELF_HOSTING.md): authoritative for the compiler's
  stream-processor architecture. Its framing that self-hosting equals V0.3.0 is superseded
  here; self-hosting is a V0.2.x milestone and V0.3.0 is full-language support.
- [`compiler/MILESTONES.md`](../../compiler/MILESTONES.md): the compiler-stage
  release-by-release plan; it should adopt this document's broader scope (validator, runtime,
  cryptography, trap analysis) and version semantics.
- [`V0_4_0_NATIVE_CODEGEN.md`](./V0_4_0_NATIVE_CODEGEN.md): native code generation via LLVM,
  which consumes the V0.3.0 self-hosted compiler as its input.
- [`V0_5_0_KELEUSMA_HOST.md`](./V0_5_0_KELEUSMA_HOST.md): the Keleusma-hosted runtime and its
  sub-coroutine primitive; Workstream D pulls the meta-circular hosted runtime forward while
  that document stays authoritative for the runtime primitive.

## Success criteria

The V0.2.x line is complete, and V0.3.0 is ready, when:

1. The self-hosted compiler compiles the full language, not only its own subset, and its
   output is byte-identical to the reference over a full-language corpus.
2. The self-hosted validator reproduces the whole `verify()` verdict, including the
   trap-free analysis, and has passed independent review against the Rust reference.
3. The hosted runtime executes the self-hosted compiler's output for the full language.
4. Signing and encryption are self-hosted at least at the orchestration layer, with the
   primitive-implementation decision resolved and any Keleusma primitives review-gated.
5. The self-hosted toolchain is driven by the shipping tool, not only by tests, and the Rust
   reference's role is reduced to the differential oracle pending its retirement decision.
