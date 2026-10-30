# Reverse prompt

## V0.2.X state, 2026-10-30

Every order of the V0.2.X roadmap is accepted for its first pass, per the rulings of
2026-10-28 recorded below. The work since then addresses the rulings' follow-ups, the
pykeleusma findings and the V0.3.X requests. No kaizen work has arrived for absorption since
pull request 595. The kaizen line is taking the changelog conflicts described under Concerns.

### Merged since the rulings

Pull requests 587, 588, 596, 598, 601, 603 and 615. In order, those are the rulings record,
decryption to the declared total, rejection of a native index past the native table at
admission, the Trap-only fault specification with the callee-aware time bound, the identity
cast, the shell-script SIGPIPE race, and the termination of the module and whole-chunk
verifier entries on an unverified module.

### Merged in pull request 623, 2026-10-30

Pull request 623 merged five branches at once, in place of five merges that each made the
others conflict in `CHANGELOG.md`, and GitHub marked all five merged.

- 618. The wrapping default of checked arithmetic is typed. A construct whose default can
  run must produce the operand's type, which closes a measured case of a `bool` function
  returning a `Word`. A fully covered construct drops its last class test and its default,
  so it adds no trap. An `underflow` arm is admissible on `Fixed` division. V0.3.X sequence
  136.
- 605. An in-memory hot swap is refused while a verifying key is registered.
- 621. `hosted::seeded_shared` and `hosted::protocol`, for the V0.3.X seeded interpreter
  differential.
- 592. GitHub action updates, with the 1.85, 1.88 and nightly toolchain pins restored.
  Dependabot had collapsed all three to the stable branch, which removed Miri.
- 593. ed25519-dalek 3, aes-gcm 0.11, toml 1 and other cargo updates, with the code
  migrated and `ctutils` taken to 0.4.7 for RUSTSEC-2026-0385. It also refreshed the root
  `Cargo.lock`, whose `syn` entries did not match the manifests.

### In the second batch

The branch `integrate/v02-batch-20261030b` merges seven branches, and its changelog entries
are fragments under `changelog.d/`, as the kaizen line's pull request 625 proposes on your
ruling of 2026-10-29.

- The stage executor boundary for every self-hosted stage and the framing stage, V0.3.X
  sequences 134 and 137.
- Item 11c. A whole-array store into a data field is refused, as a whole-array read is.
- Item 11e. `Vm::ref_context_for` and `Vm::bytecode_image`, which also answer pykeleusma's
  request for the opaque registry.
- Item 11d, its script part. A `let` annotated with a generic instance type compiles.
- Item 10. A native's declarations must agree, with each other and with a bundled library.
- A stream resume after `Reset` no longer pushes its input, which had put every later
  iteration one slot above the verified operand-stack bound.
- A hot swap seeds a composite private value.

## Accumulated questions for the operator

Each has lettered options with my recommendation first. None blocks the next item.

1. Definite initialization, held since 2026-10-28. The rule as implemented on
   `feat/v02-definite-init` refuses 62 light-tier tests, from the private-data persistence
   idiom, the handled indexed-write lowering, and two verifier unit tests.
   (a) and (b) together. Change the compiler so every path writes, and give composite
   private fields initializers so no private slot starts empty.
   (c) Narrow the rule to what the current compiler already guarantees.
2. The host struct layout defect from `volaw-daw-b2`, reproduced on `v0.2.3`. A struct the
   host passes to the entry function is checked only by kind and size, so a field reorder
   or a wrong type name is accepted and reinterprets the bytes.
   (b) now, a documented host contract with a size check, recommending
   `#[derive(KeleusmaType)]`, and (c) later.
   (a) The contract and the size check alone.
   (c) Field layouts on the wire, which needs your authorization.
3. The five Dependabot vulnerabilities GitHub reports on `main`, four high and one
   moderate, which I have not examined. Pull request 623 updated the cryptographic crates on
   `v0.2.3` only.
   (a) I examine them and report which reach a shipped crate.
   (b) They wait for the next release from `v0.2.3`.
4. Withdrawn. The kaizen line's pull request 624 adds `--locked` to CI for the lockfile
   drift I reported, and needs only your merge.

5. Composite host inputs and the auto-sized arena, from pykeleusma. The VM copies a
   composite entry argument and each composite resume value into the arena's top region, and
   `auto_arena_capacity_for` counts neither. Its documentation now says so.
   (a) A second entry point that takes the entry argument's flat size and a per-iteration
   resume allowance and adds them, with the per-iteration count of composite resumes bounded
   by the verifier.
   (b) Leave it to the host, as the documentation now states.
6. Private slot values across a hot swap, from pykeleusma. A host cannot read a private
   slot, so a swap cannot carry private state forward, and `replace_module`'s `initial_data`
   cannot seed a composite private value because the composite pool is cleared after the
   slots are written. Private data is private to the script by design.
   (a) Keep it private, and fix only the seeding defect for composite private values.
   (b) Add a host accessor for private slots, which weakens that privacy.

You ruled on 2026-10-29 for changelog fragments and for keeping every version-branch push
run, as the kaizen line records in pull request 625.

## Concerns

- I cancelled 16 superseded CI runs on 2026-10-30, under your discretion to resolve the pull
  request backlog. Eight were intermediate `v0.2.3` push runs. Those merge commits were not
  verified one by one, and the tip run covers their combined tree.
- An unexplained over-rejection. The test-side resolving driver in
  `tests/selfhost_typecheck.rs` refused some combination of Bool `let` bindings in a draft of
  `codegen.kel`, although each form type-checks alone. I rewrote the draft and did not
  isolate the combination.
- The definite-initialization branch and its self-hosted twin draft wait on question 1.

## Next

Merge the second batch. Then the host-built layout part of item 11d, item 12, the generic
header scan as its own stage, 9, delegation of native output to `kel-native`, 8, `Text<N>`,
5, the flat Text proof, and 6, the schema hash over slot types. Items 5, 6 and the host
struct question share one limit, that a flat shape on the wire records only kind and size.
