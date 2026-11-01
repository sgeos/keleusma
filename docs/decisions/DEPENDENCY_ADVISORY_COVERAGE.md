# Dependency Advisory Coverage

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: found and repaired 2026-09-27, session 67. One related finding is **recorded and
not repaired** in this increment. No language, instruction-set, or wire-format change.

---

## The advisory database had never been consulted

`cargo audit` was installed on a developer machine and had **never been run against this
tree**: no continuous-integration job, no step in `scripts/release-gate.sh`, no mention in
`scripts/` at all. Its first run reported three advisories.

| advisory | title | fixed in |
|---|---|---|
| RUSTSEC-2026-0233 | Crafted archives can cause a **use-after-free during deserialization** | >= 0.8.17 |
| RUSTSEC-2026-0234 | Insufficient archive validation causes out-of-bounds reads in archives containing hash tables | >= 0.8.17 |
| RUSTSEC-2026-0235 | Insufficient archive validation causes out-of-bounds reads in archives containing `Rc`/`Arc` | >= 0.8.17 |

All three are against `rkyv 0.8.16`, and `keleusma 0.2.2` is **published to crates.io**
carrying it. The oldest advisory is dated 2026-05-11, so they had accumulated unnoticed for
roughly four months.

**The fix is lockfile-only.** The manifest already requires `"0.8"`, so `cargo update -p rkyv`
moves the lock to 0.8.18 and the scan clears: 102 dependencies, exit 0. Pinning an exact
version would block the next patch and is the opposite of what an advisory check wants.

## Were they reachable here? Measured, and the answer is no

This question is answered separately from the fix, because the fix is correct either way and
overstating severity is its own failure.

**No code path invokes rkyv's archive validation or deserialization.** Reading only code
positions, with comments stripped:

- `src/wire_format.rs` contains **no `rkyv::` use at all**. Two derives remain and no
  `#[rkyv(...)]` attribute.
- `src/bytecode.rs`'s only code-position `rkyv::` uses are **trait bounds inside derive
  attributes** — `rkyv::ser::Writer`, `rkyv::rancor::Source`, `rkyv::validation::ArchiveContext`
  — not call sites.
- There is no `rkyv::deserialize`, `rkyv::access`, or `rkyv::from_bytes` call anywhere in
  `src/`.

The vulnerable code is rkyv's validation and deserialization path, which nothing reaches.
`Module::from_bytes` and `Module::view_bytes` both delegate to
`crate::wire_format::module_from_wire_bytes`, the wire-format v2 reader.

This confirms `CLAUDE.md`'s statement that "rkyv no longer encodes or reads the auxiliary
body, though the dependency remains for six unrelated `AlignedVec` buffer-alignment uses."

## RECORDED AND NOT REPAIRED: seven comments say the opposite

Establishing the above took two increments **because the source says otherwise**, in the
present tense, in the two most load-bearing files.

| site | claim |
|---|---|
| `src/wire_format.rs:1272` | "`ops` vector out of the rkyv-archived body" |
| `src/wire_format.rs:1395` | "Bytes of the rkyv-archived auxiliary body." |
| `src/wire_format.rs:1595` | "framing header, opcode stream, operand pool, rkyv-archived …" |
| `src/wire_format.rs:1707` | "The aux body is rkyv-archived and **requires 8-byte** [alignment]" |
| `src/wire_format.rs:2356` | "deserializes the rkyv-archived auxiliary …" |
| `src/bytecode.rs:4192` | `Module::from_bytes`: "the auxiliary body's rkyv archive supplies the rest of the module" — the site that contradicts `Module::validate_bytes` a few lines below |
| `src/bytecode.rs:4159` | "every other Module field is rkyv-archived in the [aux body]" |

**This is security-relevant, not cosmetic.** Anyone assessing an rkyv advisory against this
project reads these comments and concludes the auxiliary-body path is affected. They misled the
assessment above for two increments before the code settled it.

**And one of them contradicts a public API's documented contract.**
`src/wire_format.rs:1707` asserts an 8-byte alignment requirement; `Module::validate_bytes`'s
own documentation says "**No alignment requirement.** The wire format v2 auxiliary body is
byte-addressed … Earlier revisions required an 8-byte-aligned body because the body was an rkyv
archive accessed in place; that constraint, and the aligned scratch copy hosts needed to satisfy
it, are gone." A host implementer reading the first would provision alignment the second says is
unnecessary.

**Not repaired here deliberately.** Seven comments in the two files that carry the bytecode
format each need individual judgement about what the correct statement is, and rushing that at
the end of a long investigation is how a wrong replacement gets written into a load-bearing
file. Some neighbouring rkyv comments are **accurate** — `bytecode.rs:1746` says a section
"carries no rkyv archive" and `bytecode.rs:2226` notes the derives will retire — so a blanket
edit would damage correct prose. The distinction between a live claim and a historical note
applies here exactly as it does to a changelog.

### Correction to the table above, 2026-09-27

**This table first listed `src/bytecode.rs:3556` and omitted `src/bytecode.rs:4192`.** Both
entries were wrong, and the count of seven was right.

`bytecode.rs:3556` is **accurate**: its full sentence reads "the rkyv archive of the full `Module`
is no longer produced or consumed". A grep clipped it mid-sentence and inverted its meaning, and
the table then carried an accurate site as stale. `bytecode.rs:4192` was identified while
investigating and left out when the table was written.

**The population was counted seven, then six, then seven.** That is the same class as a counting
grep that reported zero shellcheck findings earlier in the same session: trusting a pattern over
reading the sentence. Recorded rather than silently renumbered, and posted on the pull request
that introduced the table.

All seven are corrected in the increment that followed, and `bytecode.rs:3556` is deliberately
untouched.

## Also open: the derives may be vestigial

If nothing calls rkyv's archive path, the `Archive`/`Deserialize`/`Serialize` derives on the
bytecode types may be dead weight. `AlignedVec` is still used, so the dependency itself cannot
simply be dropped. Whether the derives can go is a separate question with a real payoff —
removing an unused serialization surface removes the advisory exposure entirely rather than
tracking it — and it is the operator's, since it touches the published bytecode types.
