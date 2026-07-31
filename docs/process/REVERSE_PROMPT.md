# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-30 (session 36)

## Headline — the planned tuple-in-tuple increment was NOT NEEDED; the construct already works

The operator confirmed tuple-in-tuple as the next increment. Before writing stage code, a differential
probe established that **the self-hosted pipeline already compiles tuple-in-tuple byte-identically**.
The handoff's premise (a Gap requiring a per-frame accessor variant in the codegen emit-DFS across
parse/reconstruct/codegen) is **false**. No `.kel` change was written, and none was needed.

The increment was therefore redirected to what genuinely remained: **pinning the previously unguarded
support against regression**, and **correcting the frontier map** that misdirected the plan.

- **Boundary 56 -> 65 Ok** (2 Gap / 1 RefRejects unchanged), pinned by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`.
- Nine new boundary cases plus `self_host_compiles_tuple_in_tuple_equality`.
- **Zero product-code change**: no `.kel`, opcode, record, node kind, or `BYTECODE_VERSION` change.

## Verification

- **The control matters more than the result.** The same probe was pointed at the two known Gaps
  (`float_arith`, `generic_fn`); they correctly reported DIVERGE and PANIC. This rules out the obvious
  failure mode, since `self_host_compile` starts from `compile_src(src)` and replaces chunk bodies, so a
  skipped replacement would report identity trivially. It does not skip: every function chunk's ops,
  constants, and `local_count` are replaced unconditionally, and `parse_functions` runs `parse.kel` over
  the WHOLE source including signatures.
- Verified byte-identical (ops, constant pool, and `local_count`): nested element in first, last, and
  both positions; three levels of nesting; a `Byte` leaf (which shifts the following outer element's
  flat offset); `!=`; a struct beside a nested tuple (a mixed subtree); array-of-tuple; and nested
  element ACCESS (`a.1` resolving to flat offset 16, not 8, which pins the layout itself).
- The boundary test and the new byte-identity test both pass. The FULL `scripts/release-gate.sh` result
  is recorded in the commit message; see the git log for the final state.

## Concern — an unexplained mechanism (stated rather than hidden)

The mechanism by which `parse.kel` represents a nested tuple parameter type was **not localized**.
Reading the stage suggests it cannot: `step_tuple_type` (~1457) is a flat state machine handling only
`Ident` and `RParen` (the inner `(` is ignored, the inner `)` would end the whole scan), it has a single
definition, there is no `tup_etuple` table analogous to `tup_estruct`, and no paren-depth state was
found. That reading predicts `a.1` lowering to offset 8; the measured output is 16, so the reading is
wrong somewhere and the correct explanation was not found. The behavior is established by the project's
stated oracle with working controls, but **anyone extending the tuple layout should re-derive the real
mechanism first** rather than trusting the flat-scanner reading.

## Corrected frontier map (measured, same probe and control)

**Already supported but still UNPINNED** — free boundary cases, not attempted here because they fall
outside the authorized tuple-in-tuple scope:
- array-of-array (`[[Word;2];2] == [[Word;2];2]`)
- enum tuple payload (`enum E { A(Word, Word), B }`)

**Genuinely still Gaps** (all measured DIVERGE), roughly by increasing effort:
- array-of-array nested in a struct
- enum with an array payload
- enum with a deep struct payload
- `struct { t: (P, Word) }` (a tuple-of-struct inside a struct)
- `struct { i: I }` where `I` holds an enum, and the same where `I` holds an array
- array-of-deep-struct; array of tuple-of-struct
- enum containing a struct containing an enum

So **deeper array/enum nesting and mixed subtrees involving array/enum remain real work**;
tuple-in-tuple and mixed subtrees involving tuples do not.

## Environment note — the OS update broke the Rust linker

`Xcode.app` fails to load `CoreDevice`/`Mercury` after the update, so `xcrun` cannot find `clang` and
every Rust link fails. The standalone Command Line Tools are intact. Every cargo and git command this
session was prefixed `DEVELOPER_DIR=/Library/Developer/CommandLineTools`. The durable fix needs sudo and
is the operator's to run:

```
sudo xcode-select -s /Library/Developer/CommandLineTools
```

Until that is run, the pre-push gate hook also needs the prefix (`DEVELOPER_DIR=... git push`).

## Next step — pick a REAL gap, or pause

The cheap adjacent win is pinning array-of-array and the enum tuple payload (two boundary cases, no
product code). Beyond that, the smallest genuine capability gap looks like **enum with an array payload**
or **`struct { t: (P, Word) }`**, but note this session's lesson: **probe with a control BEFORE planning**.
A conservative admission deferral is not evidence of a gap — the path it defers to may already be
correct, which is exactly what happened here.
