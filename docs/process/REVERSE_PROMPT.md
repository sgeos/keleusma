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

### Merged in pull request 628, 2026-10-30

The stage executor boundary for every self-hosted stage and the framing stage, V0.3.X
sequences 134 and 137. Items 11c, 11e, 11d's script part and 10. The stream resume after
`Reset`, which had put each later iteration one slot above the verified operand-stack bound.
The composite seed of a hot swap. Item 9, `keleusma native`, which delegates to `kel-native`.

### In the third batch

The branch `integrate/v02-batch-20261030c`.

- Item 12. The generic template header scan is `template_scan.kel`, the fifteenth stage
  source, on your ruling that it be a stand-alone stage. It compiles byte-identically through
  the self-hosted compiler. The probe rounds stay in the driver as orchestration, which is my
  reading and is recorded in the Order 6 plan.
- Item 6. The schema hash covers each data slot's declared type, with a struct expanded to its
  fields, so a swap to a reordered struct is refused. A `Fixed` field's token omits its
  fraction-bit count, which does not change the layout and which the self-hosted driver
  cannot see.
- Item 11d's host part. A host-built value of a generic struct instance is read by its
  instance layout.
- A hot swap refuses a module whose working set the arena cannot hold, as `Vm::new` does.
- `Vm::register_external_native_with_ctx_closure`.
- The book's chapter 18 says only scalar data fields start at zero, in English and Japanese.

### In progress

Item 5. The design is `docs/decisions/FLAT_TEXT_LAYOUT_PROOF.md`. The reference pass now
tracks a flat body's `Text` field offsets where the program built it and refuses a `Text` read
elsewhere, on branch `feat/v02-flat-text-proof`. The self-hosted twin in `verify_typed.kel` is
next.

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
4. Withdrawn. The kaizen line's pull request 624, merged, adds `--locked` to CI for the
   lockfile drift I reported.

5. Composite host inputs and the auto-sized arena, from pykeleusma. The VM copies a
   composite entry argument and each composite resume value into the arena's top region, and
   `auto_arena_capacity_for` counts neither. Its documentation now says so.
   (a) A second entry point that takes the entry argument's flat size and a per-iteration
   resume allowance and adds them, with the per-iteration count of composite resumes bounded
   by the verifier.
   (b) Leave it to the host, as the documentation now states.
6. Private slot values across a hot swap, from pykeleusma. A host cannot read a private
   slot, so a swap cannot carry private state forward. Private data is private to the script
   by design. Seeding a composite private value at a swap, the defect beside this request, is
   fixed in pull request 628.
   (a) Keep it private.
   (b) Add a host accessor for private slots, which weakens that privacy.

7. `Text<N>`, the bundled operations. V0.2.0 removed the bundled text library because its
   operations could not be bounded, and `Text<N>`'s can be. The design proposes giving them
   back with `Text<N>` signatures, a reversal the record leaves to you.
   (a) Restore them with `Text<N>` signatures.
   (b) Provide only language-level operations.
8. `Text<N>`, the surface form, which blocks emission. A literal is static text and the grammar
   forbids an implicit conversion, so a value needs an explicit way in.
   (a) An `as` cast for a literal, `"abc" as Text<8>`, refused at compile time when the literal
   is longer than `N`, and free functions for the operations.
   (b) Methods on `Text<N>` through a built-in impl.
   (c) A constructor function only.
9. Item 5's boundary. The proof covers bodies the program builds. A body from an entry
   argument, a resume value, a native result, a shared slot or a swap seed carries no layout on
   the wire, so its `Text` reads cannot be proven.
   (a) A constant-cost range check at the VM-internal `Text` read, the decode check without
   its UTF-8 pass, for those five sources.
   (b) Refuse such reads, which refuses every read of a struct parameter's `Text` field.
   (c) Record a trust assumption.
10. A halting error for hosts, from pykeleusma. A host exception the native-error construct
   must not catch has no variant of its own, because every soft host error is caught.
   (a) Add `VmError::HostHalt`, which breaks a host's exhaustive match on `VmError`.
   (b) Make `VmError` non-exhaustive first, then add it.
   (c) Leave it, and document `InvalidBytecode` as the way to halt.

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

Merge the third batch, then finish item 5 with its self-hosted twin. Item 8 waits on
questions 7 and 8, item 7 on question 1, and the host struct contract on question 2.
