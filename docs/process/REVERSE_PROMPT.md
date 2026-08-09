# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-08 (session 39)

## THE NEXT INCREMENT — step 6, slice 1: CRC-32 in Keleusma

The six-step wire-format programme stands at **1, 2, 3, 4, 5 done; 6 not started.**

**`docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md` was stale and would have misdirected you.** It
concluded that self-hosting the wire format was blocked because the auxiliary body was rkyv, and
recommended doing the monomorphizer and type checker instead. That blocker was removed by changing
the wire format, which is what this whole programme did. The document is now re-scoped with the
work re-sliced against the v2 container; read the top of it, not the 2026-08-03 body.

**Slice 1 is CRC-32**, a pure bitwise table-free function over a byte range, oracle-checked against
`crate::bytecode::crc32`. It is small, and it exists to establish the byte-emission harness every
later slice needs.

**Probe this first**, because it is not established and every slice inherits it: how a `.kel` stage
addresses a byte buffer, for emission and for reading. The `secret/` prototype used a data segment.
That prototype also **predates format lock-in** — it encodes a 12-byte directory entry where the
shipped entry is 16, and the triplicated prologue postdates it entirely. Evidence of feasibility,
not a starting implementation.

**A constraint that will bite:** `ConstTable::value` uses `BTreeSet` and `BTreeMap`. Correct for the
Rust VM, unavailable in Keleusma. The Keleusma decoder needs a bounded array-based walk. The
forward-ordering invariant makes such a walk terminating, so the shape exists; it must be written
rather than transliterated.

## Completed this session — all merged, pushed, gate-green

**`v0.2.3` = `cd48c30`, pushed.** The wire-format v2 cutover is merged, on a green full gate (all
twelve steps) and a green pre-push hook. `BYTECODE_VERSION` is 2.

**In flight**: `feat/perf-guardrails`, batching the performance canary, the gate preflight, the
tiered-verification process, and one more `chunk_const` optimisation. Tier 1 is green; the full
gate for this batch has not yet run.

### The headline number

The v2 read path is **5.2× faster than the rkyv encoding it replaced**, measured on the same
200k-iteration constant-load loop: rkyv 6.42 s, v2 as first committed 67.29 s, v2 repaired 1.23 s.
End to end on a stage self-compile, 54.26 s → 30.29 s.

### The guardrail that did not exist

`tests/perf_canary.rs`. The cutover merged green on twelve gate steps and **would also have merged
green while forty times slower**, because nothing measures time. The canary was validated against
the real regression — reverting the repair takes it from 1.7 s to 67.3 s. Its ceiling is set from
the failure mode, not the observed runtime; do not tighten it. If it fails, profile before touching
it.

`scripts/release-gate.sh` now reaps orphaned test binaries as a preflight. An interrupted gate
leaves one reparented to PID 1 at full CPU, they accumulate, and they corrupt the canary's signal.

### Verification tiers (PROCESS_STRATEGY.md, operator-directed 2026-08-08)

Full gate before every **merge**, not after every **change**. Tier 0 `fast-check.sh` per edit,
Tier 1 (clippy `--all-targets`, `--no-default-features`, the `-D warnings` doc build) per increment,
Tier 2 full gate per merge, **batching three or four increments**. The feature matrix was
deliberately not narrowed; the reasoning is recorded there.

### Known follow-up, audited and consciously deferred

Fifteen `self.aux()` sites remain; none is hot. `shared_layout_entry`,
`private_composite_pool_offset`, `private_composite_slot_end`, `enum_variant_layout` and
`struct_template` are per-composite-access or per-construction rather than per-op. A legitimate
increment, not a blocker.

**The cutover as originally committed was correct and unshippable.** Every test passed; one of them
took over 37 minutes against 54 seconds on `v0.2.3`. Two hot-path reads were doing work proportional
to the whole module. Both are fixed:

- `AuxResolved` resolves the module scalars once per installed image, instead of `Vm::aux()`
  rebuilding fifteen sub-tables to read one scalar inside the interpreter loop.
- `ConstTable::value` materialises one constant's subtree, with a scalar fast path, instead of
  `decode_constant_pool` re-parsing the artifact and materialising every constant in the module per
  constant load.

Measured, same machine, uncontended, no memoization:

| | `lexer.kel` self-compile |
|---|---|
| `v0.2.3` (rkyv) | 54.26 s |
| cutover as committed | >2220 s (killed) |
| **cutover fixed** | **30.29 s** |

**Verified green**: 1231 lib tests, 94 schema tests, the ten-stage corpus, `fmt`,
`clippy --workspace --all-targets -D warnings`.

**NOT verified**: the full `scripts/release-gate.sh` — `signatures`, `signatures,shell`, `self-host`
in full, both `keleusma-wire` configurations, the markdown-link check, and the detached `compiler/`
subproject. **Do not merge until it is green.** Ask the operator to run it with
`! scripts/release-gate.sh`; agent-launched background runs do not survive this environment.

### Three things a resuming agent must not relearn the hard way

1. **A killed gate orphans its test binary to PID 1**, still at full CPU. One was found burning four
   cores for ten hours, halving the machine. Reap `target/debug/deps` strays before any gate.
2. **Never measure performance on a build you have not just re-verified.** A 3.7× "speedup" recorded
   during this session came from a build where constant loads were erroring out early and returning
   `Unit`. The real figure was an 11% regression until a further fix landed.
3. **A scalar constant record overlays its payload on the range bytes.** "Has children" is a question
   about the tag, never the range fields. `Int(i64::MIN)` reads as a child count of 0x8000_0000, and a
   differential test built from small integers passes right through it.

### Still open

- `CLAUDE.md` and the golden-bytes comment are corrected to wire format v2 on this branch.
- Publication remains **held**. MSRV 1.85 still unverified.
- A `v0.2.3` worktree for A/B measurement is parked outside the repo in the session scratchpad.

---

**Date**: 2026-08-04 (session 37)

## Current state — the wire-format programme, steps 1 and 2 done

The work has moved from "replace rkyv" to designing a wire format against a stated requirement set,
with its own reusable crate. **Steps 1 and 2 are complete**: the prototype closed both
layout-sensitive gaps, and the `keleusma-wire` / `keleusma-wire-derive` crates exist, are tested, and
are covered by the gate. **No `keleusma` runtime code consumes them yet** — that is step 4.

**Before writing anything tracked — documentation, commit message, or code comment — read
`secret/notes/APPENDIX_B.md`.** It defines what must not appear in this repository. Tracked material
was sanitized against it on 2026-08-04. This is a hard constraint.

## The programme (operator-stated, six steps, in order)

1. Prototype the wire format until it can be locked in.
2. Add a new wire-format crate, usable by other projects as an alternative to `rkyv`, as
   `keleusma-arena` is nominally useful outside this repository.
3. Document the WHAT of the format, without the internal reasoning about the WHY.
4. Implement the wire format in Rust.
5. Port Keleusma to it.
6. Self-host the wire format in Keleusma — which implies **the Rust must be Keleusma-like**.

Operator resolutions, 2026-08-04:
- **Crate is MECHANISM ONLY**, named `keleusma-wire`. It must not depend on the Keleusma runtime nor
  hardcode `WireChunk` / `ConstValue`; Keleusma's schema layers on top in the `keleusma` crate.
- **Step 6 covers BOTH encoder and decoder.**
- **Lock-in is a judgement call.** A proof of concept need only be good enough to decide and move on;
  do not gold-plate it.

## Design state

The current design is [`../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md`](../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md):
word-oriented with a 64-bit unit, word-indexed offsets, fixed-size records with variable data in
byte-addressed pools, a (72,64) SECDED plane held parallel to the data, per-region encryption, and a
triplicated header and region directory.

It **supersedes** [`../decisions/WIRE_FORMAT_V2_FLAT_AUX.md`](../decisions/WIRE_FORMAT_V2_FLAT_AUX.md)
on record structure. That document's **P10 analysis still governs and is not repeated**: string
constants materialise as `KStr` aliasing the bytecode image, so the accessor layer must be a
**borrowed view, never an owned decode**. Routing the runtime through an owned decode would allocate
per load and silently undo P10 with no test failing.

`src/wire_aux.rs` on `v0.2.3` implements the **superseded** variable-length design. Its primitive
layer, explicit tag discipline, and totality tests are reusable; **its record structure is not.**

## Prototype state (all in `secret/`, gitignored, reproducible)

- `kel-format-probe/wireimage.kel` — Keleusma **producer and consumer**, 408-byte artifact, 12/12.
- `kel-format-probe/image.py` — independent reference emitter. **Checksums agree at 5093.** It also
  generates both hardware image packages, so the clean and corrupt images cannot drift — they had,
  before this revision.
- `kel-format-probe/stream.kel` — Keleusma **streaming stage**, emitting across yields. 9/9.
- `silicon-prototype/wire_decode.vhd` + `tb_wire.vhd` — hardware **consumer** of those exact bytes.
  **PASS** on the header vote, block trailer, all three regions, absent-region not-found, every chunk
  descriptor, every constant record, a string constant resolved to real pool bytes, and the
  reverse-sweep aggregate.
- `silicon-prototype/tb_wire_corrupt.vhd` — one corrupted header copy outvoted **and** flagged, and it
  now asserts the damaged copy actually differs from the voted value so it cannot pass vacuously. PASS.
- `silicon-prototype/secded_*` — (72,64) SECDED validated in Python and simulated in VHDL:
  432/432 single-bit corrected, 15336/15336 double-bit detected.
- Toolchain: `nvc` 1.23-devel at `/usr/local/bin/nvc`. Build notes in
  `secret/silicon-prototype/README.md` — MacPorts needs `--enable-static-llvm`, and `make` will not
  relink an existing `bin/nvc` after reconfiguring.

## Both layout-sensitive gaps are now CLOSED (revision 2, 2026-08-04)

The fetch path runs past the chunk descriptor into the constant table and out into the string pool,
and emission is tested from a yielding stage. Results, each expected value taken from an independent
implementation rather than from the code under test:

| Implementation | Result |
|---|---|
| Keleusma producer + consumer (`wireimage.kel`) | 12/12 |
| Reference emitter (`image.py`) | byte-identical, checksum 5093 |
| Hardware decoder, simulated (`wire_decode.vhd`) | 24 checks |
| Keleusma streaming stage (`stream.kel`) | 9/9 across suspensions |

Both hardware testbenches were checked against a **negative control** (mutate an expected value,
confirm the failure fires), since a testbench that passes first try has not been shown able to fail.

**It found five things, which is why the design document required it before freezing.**

1. **The directory entry was 12 bytes** — one and a half words, contradicting the format's own rule
   that every record is an integral number of words. Now 16.
2. **The block check cannot be a header field.** Its input is the directory written after it, so a
   leading position requires back-patching. Moved to a trailer.
3. **The composite-range ordering invariant is load-bearing and must be CHECKED.** A composite's range
   must lie strictly after the composite; that is what makes a bottom-up walk a single reverse linear
   sweep with no stack. Violating it yields a **wrong answer rather than a fault**, so it replaces
   `MAX_CONST_DEPTH` as the hostile-input check rather than simply removing it.
4. **A leading directory and globally contiguous regions are both incompatible with streaming
   emission.** This forces an encoder choice — buffer per region (keeps the leading directory), or a
   trailing directory with per-unit segments (true single pass). Option (b) was implemented and works;
   **the recommendation is (a)**. Now an explicit open question in the design document.
5. **Language finding**: a resumed `yield` block continues from the suspension point with its
   parameter still bound to the original argument, so an `if tick == n` ladder runs once and falls
   through. The first streaming probe did exactly that and emitted one segment instead of three; the
   byte count caught it. Streaming stages want straight-line yields.

## Open questions for the operator

- **Encoder strategy (item 4 above).** Blocks nothing today — the record layouts are identical either
  way — but it decides whether the directory leads or trails, so it wants settling before the crate.
- ~~The ECC plane is unexercised end to end.~~ **CLOSED**: the plane is implemented in
  `keleusma-wire`, and every single-bit fault across a protected payload (512 cases) is corrected and
  reported. The *prototype* artifacts in `secret/` still carry no ECC region, which now matters less
  since the Rust implementation exercises it.

## Step 2 landed: the `keleusma-wire` crate exists

Mechanism only, as resolved. It provides framing, a triplicated prologue and region directory,
fixed-stride record tables, byte pools, CRC-32, and the majority vote. It has **no dependency on the
Keleusma runtime** and hardcodes no schema — region kinds, record strides, and field offsets are all
the caller's. That is what makes it usable elsewhere, which was the stated point.

Written to be transliterable to Keleusma (step 6): no recursion, static loop bounds, no allocation on
the read path, fixed-size records, unrolled place-value field access, no traits or generics in the
codec core, state in explicit structs.

**Verification**: 12 unit tests, 11 integration tests, 1 doctest. Clippy clean at `-D warnings` both
with and without default features. Builds for `wasm32v1-none` with and without `alloc`, so the
`no_std` claim is tested rather than declared. Three tests carry most of the weight:

- **1536 single-bit fault injections** across the protected header, each required to be both
  corrected by the vote **and** reported by `needs_scrub()`.
- **Every truncation** of a valid artifact is rejected, and every single-bit corruption anywhere is
  required not to panic.
- **Aliasing asserted by address.** The read path must return slices *into* the caller's buffer. A
  test that only checked values would not notice an owned decode creeping in, which is precisely how
  P10 would be lost silently — so the pointer range is asserted directly.

### Two findings from writing it

1. **The prologue had to be split from the directory** — a bootstrapping problem invisible until a
   real reader existed. Voting the header needs the block stride, which needs `region_count`, which
   would itself be inside the block being voted; a bit flip there would desynchronise the search for
   the copies meant to repair it. A fixed-size prologue at fixed offsets is votable with no prior
   knowledge. **This also withdraws the "block check must be a trailer" correction from earlier
   today**: once the directory is out of the block, the check covers only fixed-size fields known
   before the first write, so no back-patching arises. The split subsumes the trailer.
2. **A totality hole in my own bounds checks.** They were written `at + n <= len`, which overflows
   for `at` near `usize::MAX` and panics in a debug build — in the functions whose entire contract is
   totality. Found by testing the extreme offset rather than by review. Now a subtraction on the
   length, which cannot overflow.

### Encoder strategy: RESOLVED

**Operator chose option (a)** (one buffer per region, leading directory), 2026-08-04, which is what
the crate implements. Option (b) stays reachable without touching any record layout, should
single-pass emission ever be required.

## The ECC plane and the derive have landed too

**The parity plane is in**, which is what makes the crate differentiated rather than "another
container". (72,64) SECDED, one check byte per 64-bit word, held in a region parallel to the data.
`builder.protect(id, kind)` generates it; `view.verify_region(&r)` scans it. Correction returns a
**value** and never writes to the caller's buffer — an in-place corrector would have needed `&mut`
and the allocation-free read path would have died to deliver the fault tolerance.

**`#[derive(WireRecord)]` is in**, in a separate `keleusma-wire-derive` crate behind an off-by-default
`derive` feature. It generates offset constants, a stride, and a total codec, removing the
hand-rolled-offsets adoption barrier. Fields pack with **no implicit padding** (`{u8, i16, i64,
[u8;5]}` → 0/1/3/11), which `repr(C)` would not produce, so the offsets must be generated rather than
taken from the type.

**A gate hole was found and closed.** `release-gate.sh` runs `cargo test --workspace` at DEFAULT
features and documents five crates BY NAME, so the `derive` feature would never have been tested and
neither new crate's docs would ever have been built under `-D warnings` — the same shape of hole that
let the broken `src/selfhost/` intra-doc links survive four releases. Four steps added.

## Publication readiness: PREPARED, but hold until internal use

The crate is prepared — LICENSE, README with four compiled doctests, `#![forbid(unsafe_code)]`,
`#[non_exhaustive]` on the growable types, docs.rs metadata on both crates, and gate coverage.

**It should NOT be published yet, and the reason is concrete.** Nothing consumes it. Its only users
are its own tests, and the first real consumer always finds something: `Region` gained a `covers`
field the moment the second requirement (ECC) arrived, which post-1.0 would have been a breaking
change. Publishing now freezes an API that no workload has exercised.

Known gaps, none blocking internal use:
- **MSRV 1.85 is declared but never verified** — no build against that toolchain.
- **No fuzzing.** Totality is tested exhaustively for single-bit faults and truncation, which is
  strong but is not the same as a fuzzer against a parser of untrusted bytes.
- **No size or timing numbers.** The "addressing is a shift" and "no allocation" claims are
  structural; the second is verified by construction and by address, the first is not measured.

## Step 4 stage 1 landed: the flattened constant table

`src/wire_schema.rs` supplies the schema the container deliberately omits — region kinds, record
meanings, and the flattening of a `ConstValue` tree into fixed-size records. Five regions:
`STRING_POOL`, `NAMES`, `CONSTS`, `STRUCT_AUX`, `ENUM_AUX`. 16 tests, all passing.

**The design claim is now implemented, not just asserted.** A composite references a RANGE that lies
strictly after it, produced by breadth-first numbering with roots pinned to `0..n` (a chunk indexes
its constants positionally). That is what makes the table walkable by a single reverse linear sweep
with no stack. The decoder RE-VALIDATES the ordering up front rather than trusting the encoder that
produced its input, and a hand-corrupted backwards range is a test.

**Side tables rather than wider records.** A struct needs a type name, field names and values; an enum
a type name, variant, optional discriminant and payload. Widening every record to the worst case would
cost 32 bytes for an `Int` needing 8, so those two kinds reference small side tables and the constant
record stays two words.

**Field names are interned WITHOUT sharing**, unlike everything else, because a struct's names must
stay contiguous for `field_names_first + i` addressing; a repeated name returning an earlier index
would break the run. Two structs sharing field names is the test.

### The finding worth carrying: a test suite that was blind

`ConstValue`'s hand-written `PartialEq` **deliberately ignores the enum discriminant** (the `..` in
its `Enum` arm). So `assert_eq!` on a round trip cannot see whether the discriminant survived, and
every enum round-trip test was passing **vacuously** with respect to it — they would have passed with
the field dropped entirely. The tests now use a `deep_eq` helper that compares it explicitly, and the
`Some(0)` vs `None` distinction is asserted by destructuring rather than by `!=`. **Anyone testing
round trips of `ConstValue` must not use `==`.**

### Not done, and not claimed

- **`decode_constants` returns OWNED values.** This is the tooling and test path, the analogue of the
  existing `decode_aux`. The **borrowed in-place accessor the VM needs is not written**, and that is
  the surface where P10 is preserved or lost.
- **Nothing is wired into the loader.** The `rkyv` path is untouched; this is parallel infrastructure
  alongside `debug_meta` and `value_layout`.
- **The rest of the aux body** — struct templates, param types, enum layouts, signatures, native
  return shapes, the scalar header block — is not encoded yet. Those are flat vectors following the
  same mechanical pattern.

## Stage 2a landed: the borrowed accessor (`ConstTable`)

**The probe rewrote the requirement, which is why step 1a exists.** The recorded claim was "string
constants materialise as `KStr` aliasing the image, so the accessor must be borrowed." Reading the
live runtime showed the true requirement is narrower and more precise:

- `chunk_const` **does** alias the image for a **non-empty top-level** `StaticStr` — it takes
  `bytes.as_ptr()` and mints a `KString` over the immortal image. Confirmed, still true.
- An **empty** string is deliberately NOT aliased, so the runtime need not rest on a non-null
  guarantee for a zero-length pointer.
- A **composite's** string leaves are **already copied today** — they materialise owned and the flat
  packer moves them into the arena. Borrowing them buys nothing the runtime uses.
- `chunk_const_str` is a separate helper that copies (`.to_string()`); it is not the hot value path.

So the hard requirement is **exactly one accessor returning image-aliasing bytes**, not a
borrow-everything design. Over-constraining would have complicated the accessor for no gain;
under-constraining would have silently cost the one load-bearing property.

`ConstTable<'a>` is that accessor: parse-and-validate once, then total allocation-free reads —
`str_bytes` (the aliasing one), `str`, `tag`, `payload`, `range`, `name_bytes`, `struct_aux`,
`enum_aux`. `decode_constants` was **refactored onto it**, so the owned and borrowed readers share
one parse path and cannot drift on the ordering check.

**24 tests.** The aliasing is asserted **by address**, with an inline control proving the predicate
discriminates — an owned copy has the same value and a different address, so without the control the
assertion would prove nothing.

## CORRECTION: stage 2b is NOT one mechanical increment (probed 2026-08-05)

The line previously here — "the remaining aux-body fields are flat vectors of scalars following the
same mechanical pattern, so they are lower-risk" — is **wrong**, and it was written into this channel
one increment earlier without being checked. A probe of the actual types:

| Type | Actual shape |
|---|---|
| `StructTemplate` | `type_name: String`, `field_names: Vec<String>` |
| `EnumLayout` | `type_name: String`, `variants: Vec<EnumVariantDisc>` (name + disc each), `min_payload` |
| `ChunkSignature` | `params: Vec<WireShape>`, plus `ret` and `resume` shapes |
| `WireShape` | a tagged union: `Top`, `Scalar { kind }`, `Composite { kind, size }` |
| `DataLayout` | **three** nested `Vec`s of structs (`slots`, `shared_layout`, plus persistent placement) |
| `WireChunk::debug_pool_bytes` | `Option<Vec<u8>>` — variable-length per chunk, needs a pool and a range |

Every one needs the same table-plus-range treatment the constant table got. **Stage 2b is four or
five separate increments, not one.** Ordered smallest-first by the loop's policy:

1. **`WireShape`** — genuinely fixed-size once tagged; one small record. The others depend on it.
2. **`ChunkSignature`** — three shape references plus a range into the shape table.
3. **`StructTemplate`** — a type name plus a contiguous run of field names; reuses the `NAMES`
   machinery and the `field_names_first + i` addressing already built and tested for struct constants.
4. **`EnumLayout`** — structurally the same as (3) with a discriminant per sub-name.
5. **`DataLayout`** — the largest; three parallel tables.
6. **The scalar header block and `debug_pool_bytes`** — the header is genuinely flat; the debug pool
   is opaque bytes into a region, deliberately kept unparsed so stripping stays a single assignment.

## Stage 2b increment 1 DONE: shapes and signatures

`WireShape` and `ChunkSignature` landed together, because a shape table with no consumer is dead code.
**35 tests.**

The probe confirmed the claim this time: `WireShape`'s widest variant carries a `u8` and a `u32`, so
the whole tagged union fits **one word** — no side table, unlike struct and enum constants.

**The same contiguity-versus-sharing tension as field names, resolved the same way.** A parameter run
must be contiguous so `params_first + i` addresses it, so parameters are appended **unshared**;
`ret` and `resume` are single references and may be **interned**. `Top` dominates real modules (every
non-Stream chunk resumes with it), so sharing the singles is worth having. A test pins both halves:
three unshared parameter entries, singles collapsing onto one of them.

**No forward-ordering rule applies here.** A shape references no other shape, so the recursion the
constant table had to linearise simply does not arise — worth stating, because carrying that rule over
by analogy would have added a check with nothing to check.

Two things fixed before they could matter:
- **The encoders are now composable.** `add_constant_regions` / `add_signature_regions` take an
  existing builder; the `encode_*` functions are thin wrappers. The aux body will eventually be ONE
  artifact carrying every region, and building that in now costs nothing where retrofitting would mean
  rewriting each encoder.
- **A hole in my own validation.** The bounds check read `ret >= shapes.len().max(1)`, so with an
  empty shape table a signature referencing shape 0 would pass and leave the accessors returning
  `None` rather than being total. Plain bounds now.

## Stage 2b increment 2 DONE: struct templates, enum layouts, and a SHARED name interner

`StructTemplate` and `EnumLayout` landed together. **44 tests.**

**The probe forced an architectural change.** `add_constant_regions` already claimed `STRING_POOL` and
`NAMES`, and the container **rejects duplicate region kinds** — so templates and enum layouts, which
also reference names, could not declare them again. Composability at the *builder* level was not
enough: the shared state is the **interner**.

`SchemaBuilder` now owns it. Each `add_*` contributes records and interns names; `finish` emits the
pool and name table once, after every contributor has run. A type name mentioned by both a constant
and a template is stored once **and comparable by index**, which a per-concern encoder could never
have achieved. There is a test building constants, signatures, templates and layouts into ONE
artifact and reading each back, asserting the shared name resolves to the same index from both sides.

Enum variants get their own table rather than riding the name run the way struct fields do, because a
bare run of names cannot carry the discriminants.

## Stage 2b increment 3 DONE: the constant table is multi-contributor

**The probe found a fourth vector and it changed the plan.** `DataLayout` was recorded as having
three nested vectors; it has **four**, and the fourth is `private_init: Vec<ConstValue>` — a forest of
constant *trees*, not scalars.

That matters beyond `DataLayout`. `encode_constants` pinned roots at `0..n`, which models **one**
chunk's pool — but a module has one pool **per chunk**, so the constant table had to become
multi-contributor regardless. Splitting that out first rather than bundling it into `DataLayout`.

`add_constant_pool` returns a `(first, count)` range; flattening is **deferred to `finish`** so every
pool's roots are concatenated and flattened once. Roots occupy the table's prefix in add order and
children are numbered after **all** of them, which keeps the forward-ordering invariant intact while
letting each contributor address its run as `first + i`. There is a test asserting the invariant
survives across pools — if children were numbered per-pool, a later pool's root would collide with an
earlier pool's child and the reverse sweep would read an uncomputed value.

Also pinned: an artifact with no constants emits **no** constant regions, so `ConstTable::parse`
reports absent rather than empty. 49 tests.

## Stage 2b increment 4 DONE: the data-segment layout

Four regions — `DATA_SLOTS`, `SHARED_LAYOUT`, `PRIVATE_COMPOSITE`, `DATA_INIT` — plus a constant
range for `private_init`, which rides the shared table rather than a parallel copy of the flattening
machinery. **57 tests.**

**`Option<DataLayout>` is encoded by region PRESENCE.** An absent `DATA_SLOTS` region means `None`; an
empty one means `Some` with no slots. Collapsing those would make a module with no `data` block
indistinguishable from one whose data block is empty, which are different programs. Both directions
are pinned by a test.

Every data record is **one word**, and every tag is numbered from one so a zeroed record is invalid
rather than reading as a well-formed shared slot.

## Stage 2b increment 5 DONE: per-chunk ranges for templates and parameter types

**Another probe finding: `struct_templates` is per-CHUNK.** Increment 2 built a module-level template
table with no ranges — incomplete rather than wrong, and it would have failed the moment a second
chunk appeared. Templates now defer and concatenate exactly as constants do, with
`add_struct_template_pool` returning a range. Field-name runs stay contiguous through the deferred
interning because a template's names are interned consecutively.

`param_types` is a per-chunk `Vec<TypeTag>` of **one-byte** values, so it is a **byte pool**, not a
record table — a whole-word record per tag would waste seven eighths of the region.

**A distinction drawn deliberately, not by reflex.** `LayoutTable` now treats absent template and
enum regions as **empty**, whereas `DataLayoutTable` treats an absent region as **`None`**. That is
not an inconsistency: `Option<DataLayout>` is semantically meaningful (a module with no `data` block
differs from one whose block is empty), while "no struct templates" has only one reading. A module
with templates but no enums is ordinary and must parse; a test pins it.

64 tests.

## STAGE 2b COMPLETE (2026-08-05): the whole aux body encodes

Increment 6 closed it — the chunk table, natives, scalar header, and debug pool. **74 tests.**
Every field of `WireAuxBody` and `WireChunk` now has a place in the schema.

A chunk record is six words and holds only fixed-size data: name index, four ranges (constants,
templates, parameter types, debug), op offsets, counts, and a block tag. Natives pair each name with
its return shape in **one record**, because `native_return_shapes` is parallel to `native_names` and
separate regions would let the pair fall out of step.

**`ABSENT` (`u32::MAX`) is the optional-index sentinel** — used for `entry_point`, a native's
return shape, and a chunk's debug pool. A sentinel rather than a parallel flag because these index
tables the container already bounds far below four billion entries, and a flag would have to be kept
in step with the field it describes. It also keeps `None` distinct from `Some(empty)` for the debug
pool, which is a release build versus a debug build that emitted nothing.

### A bug caught before landing, of a class that has now bitten twice

`add_natives` and `add_signatures` **both declared `kind::SHAPES`**, and the container rejects a
duplicate region — so calling both failed with `DuplicateRegion`. It survived a full increment
because the only test exercised natives *without* signatures.

This is the identical shape of defect as the `NAMES` collision in increment 2: **a region is shared
state, and a per-contributor table collides.** The shape table now lives in `SchemaBuilder` and is
emitted once at `finish`, like names. Two regression tests were added — one for this pair
specifically, and `every_add_method_can_be_called_together`, which exercises **every** contributor in
one builder so the next `add_*` that claims a taken region fails there rather than in whichever
combination nobody happened to test.

## Stage 2 COMPLETE: the whole aux body round-trips (2026-08-05)

`encode_aux_body` / `decode_aux_body` drive every `add_*` together — the first consumer that exercises
the shared-state design end to end rather than one table at a time. **80 tests.**

Per-chunk data is contributed first so each chunk record carries the ranges the contributions
returned; a chunk cannot describe a range it never wrote. A dedicated test asserts ranges do not
bleed between chunks, which is the failure the whole range design exists to prevent.

**A real compiled module round-trips**, and the test asserts its own coverage so it cannot quietly
become vacuous. Measured, not assumed: that corpus yields 3 chunks, 3 constants, 3 parameter types
and 3 signatures — but **zero struct templates and zero natives**, which are covered only by the
hand-built case. Worth knowing before treating "a real module round-trips" as blanket assurance.

The pre-gate checks caught two defects again: unresolved `[WireAuxBody]` doc links (the two-doc-scope
problem, third occurrence) and a test using `keleusma::lexer/parser/compiler`, which live behind the
`compile` feature and so broke the runtime-only build. Both classes are invisible to targeted tests.

## Corpus differential: the codec meets real compiler output (2026-08-05)

`tests/wire_corpus.rs` round-trips **all ten self-hosted stage sources** — the largest real Keleusma
programs that exist, `parse.kel` alone being 6022 lines. Measured coverage: **287 chunks, 2192
constants, 287 signatures, 10 data layouts**. Runs in **2.45 s**.

**It found a quadratic in the encoder within minutes of existing.** `Names::intern` was a linear scan,
justified by a comment I wrote saying "the name count per module is small, and a map would pull in
hashing for no measurable benefit at this size." The stage sources declare **thousands of data slots
each** — 16913 in one — and every slot name is interned. Encoding went from under a second to over
nine minutes as the count grew. Replaced with a `BTreeMap` (no hasher, so `no_std` is unaffected):
**782 s → 2.45 s** for the full corpus.

Also fixed while chasing it: `decode_aux_body` decoded each chunk's constant pool separately, and each
call re-walked the whole table — quadratic in chunk count. `decode_constant_pools` now does one sweep
for every range.

### What this cost me, and the lesson

I guessed the cause **three times** before measuring: first "the two biggest files dominate" (wrong —
all eight others still timed out), then "it must be the build" (wrong — the build is 1 s), then
"it's the quadratic decode" (real, but not the main cost). Each wrong guess cost a ten-minute timeout.
The per-stage instrumentation that actually found it took one run.

The corpus was also briefly split with the two largest stages behind `#[ignore]` to dodge the cost.
That split is **removed** — it would have hidden the two most valuable inputs behind a flag nobody
passes, to work around a defect that no longer exists.

### Coverage caveat, asserted rather than implied

The corpus emits **zero struct templates**. "The real corpus round-trips" therefore says nothing about
the template table, which is covered only by hand-built cases. The test asserts `total_templates == 0`
with a message telling whoever sees it fail to update this note.

## Step 5, increment 1 DONE: `AuxView`, the runtime's read surface

**The probe corrected the plan again.** I had sketched increment (a) as "the encoder wired behind
`module_to_wire_bytes` with rkyv still authoritative". That is not a real increment: emitting both
encodings changes the artifact and would force a `BYTECODE_VERSION` bump, which is a stop. So the
first genuine step is the accessor.

**The VM's read surface is much smaller than the 59-reference count implies.** Enumerated from the
archived call sites: per-chunk `constants`, `struct_templates` and `local_count`; `word_bits_log2` and
`float_bits_log2`; `schema_hash`; `shared_data_bytes`; `data_layout`; `enum_layouts`. That is the
whole of it.

`AuxView` parses **once** and holds the sub-tables. Each individual table calls `WireView::parse`
itself — right for tooling, which touches a table once; wrong for the runtime, which reads constants
repeatedly during execution and would re-walk the directory every time.

It also presents **chunk-relative** indices, because a chunk addresses its own pool from zero. Getting
that mapping wrong would have each chunk reading whatever constants sit at its indices — in bounds,
so a wrong answer rather than a fault. A test pins that a chunk cannot reach past its own pool.

`chunk_const_str_bytes` is the image-aliasing accessor, asserted **by address** with a control that
the predicate rejects a copy. 85 tests.

## Randomised input testing DONE — and the vacuity check earned its keep twice

`tests/wire_fuzz.rs` closes the "no fuzzing" gap flagged as a pre-publication blocker. Fixed-seed
xorshift, no new dependency, no nightly, 2.6 s. Four generators cover what the exhaustive tests
structurally cannot: **multi-byte** corruption, **wholly random** bytes, **light payload perturbation
under a valid header**, and **random truncation plus extension**.

Plus one claim stronger than totality: appending bytes to a valid artifact must not change what it
decodes to. The directory bounds every read, so trailing bytes are inert — if that fails, some reader
is deriving a length from the buffer size instead of the directory.

**The vacuity check is the part worth keeping.** A `count_parsing` assertion asks how many generated
inputs actually reach the readers rather than dying at framing. It failed twice:

| Generator | Inputs reaching the readers |
|---|---|
| Keep the 48-byte prologue, randomise the rest | **0 / 2000** |
| Keep the whole header, randomise 25% of payload | **4 / 2000** |
| Keep the whole header, change 1–4 payload bytes | **1581 / 2000** |

The first failed because the **directory is triplicated and voted too**, so randomising past byte 48
corrupts all three copies. The second because the decoder validates ordering, name indices, block tags
and ranges — heavy corruption trips one before any reader runs, which is correct behaviour that also
makes the test useless. Without that assertion I would have committed a fuzz suite exercising the
magic-number check and nothing else, passing forever.

## THE STOP IS RESOLVED: operator authorised `BYTECODE_VERSION` 1 → 2 (2026-08-06)

Reason given: the wire-format substrate itself has changed. **Publication remains held** — push, do
not publish.

## Cutover increment 1 DONE: resolve once, reconstruct cheaply

Probing the cutover found the design question that actually matters. `Vm::archived()` is an
`unsafe rkyv::access_unchecked` over a byte range — effectively free — and `chunk_const` calls it on
**every `LoadConst`**. Replacing it with a validating parse per access would be a hot-path
regression, so the port is not mechanical.

`AuxOffsets::resolve` walks the directory and validates **once**, yielding plain byte ranges that
carry **no borrow** — so a caller can store them beside the bytecode image without a self-referential
struct, which is the reason the obvious "cache an `AuxView`" approach does not work.
`AuxView::from_offsets` then rebuilds by slicing: a handful of bounds checks, no directory walk, no
revalidation.

The test that matters asserts the fast and slow paths **answer identically** across every read. If
they diverged, the runtime would return different values from the same bytes depending on which path
it took. Aliasing is re-asserted on the fast path too, so it cannot quietly become a copying path.

`keleusma-wire` gained `RecordTable::from_bytes` and `Pool::from_bytes` — legitimate mechanism-level
operations ("view these bytes as a table"), which keeps schema knowledge out of the VM.

90 schema tests.

## Cutover proper: STARTED, on a LOCAL RED BRANCH `feat/wire-cutover-proper`

**`v0.2.3` is untouched and green at `435a3b2`.** The in-progress work is one local commit,
`d3d459a`, which is **red by construction** and **not pushed** — the pre-push hook runs the full
gate and a red branch cannot pass it, and bypassing that hook is prohibited. The branch is durable
in the local repository; nothing is lost, but nothing is on origin either.

### What `d3d459a` already does

- `module_to_wire_bytes` (both sites, plain and signed) builds the aux body with `encode_aux_body`.
- The cold loader path decodes with `decode_aux_body`, and the **8-byte-aligned scratch copy is
  gone** — the v2 format is byte-addressed, so the decode reads the slice where it lies. That also
  removes the class of bug the copy existed to prevent (unaligned decode on a 32-bit target).
- **`BYTECODE_VERSION` is 2.**

### Why it is red, and the warning that matters most

322 lib tests fail. `Vm::archived()` is still `rkyv::access_unchecked` and now reinterprets the v2
format as an rkyv archive, reading garbage.

**The build is GREEN. The compiler does not catch any of this** — `access_unchecked` type-checks
against any byte range. Every error in this port is invisible until runtime, so *do not* treat a
clean `cargo build` as progress. The oracles are the test suite, the corpus round-trip, and VM
execution of the ten self-hosted stages.

### The remaining work, in order

1. **Add the accessors `AuxView` still lacks**, enumerated from the call sites:
   `op_record_count(chunk)`, `native_count()` / `native_name_bytes(idx)`,
   `template_field_count(chunk, template)`, `enum_min_payload(index)`,
   `enum_variant_count(index)`.
2. **Store `AuxOffsets` on the `Vm`**, resolved once at construction, and replace
   `fn archived(&self) -> &ArchivedWireAuxBody` with `fn aux(&self) -> AuxView<'_>` built via
   `AuxView::from_offsets`. The offsets carry no borrow, which is why this works where caching an
   `AuxView` does not.
3. **Port the 26 `archived()` call sites** in `src/vm.rs`. Line numbers as of `d3d459a`:
   1206 `chunk_const` (**hot**, and the P10 accessor), 1268 `chunk_op_count`,
   1275 `chunk_local_count`, 1283/1291 word width, 1297 float width, 1306 `enum_variant_layout`,
   1400 `chunk_count`, 1406 `native_name`, 1415 `chunk_const_str`, 1432 `struct_template`,
   2330 `private_composite_pool_offset` (binary-searches the private-composite table).
4. **Port the zero-copy entry** at `src/bytecode.rs:3886` (`rkyv::access::<ArchivedWireAuxBody>`),
   and the alignment guard just above it at 3879, which the v2 format no longer needs.
5. **Update `CLAUDE.md`**: it states `BYTECODE_VERSION` stays 1 under the no-public-adoption policy.
   The operator authorised 2 on 2026-08-06 because the substrate itself changed. The accepted
   hazard that text records — an old artifact being accepted-then-mis-read — is now *resolved* for
   v1 artifacts, since they are rejected on the version check.
6. **Do not expect the rkyv dependency to go away.** Six uses of `rkyv::util::AlignedVec` remain for
   buffer alignment, unrelated to the aux archive. I said "drop the dependency" earlier; that was
   wrong.

### The oracle for this cutover

`tests/wire_corpus.rs` already round-trips all ten self-hosted stages. After the port, the same
corpus must still round-trip **and** the VM must still execute those stages. That is a real
differential, not a smoke test, and it is why the corpus was built before the runtime was touched.

## Publication

**Still held.** The operator said "push, but do not yet publish" on 2026-08-06. Neither crate is
published. Publishing is irreversible and outward-facing: confirm before any attempt.
