# The String Marshalling ABI: Option B, Received and Binding

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: Ruled and accepted. Not yet implemented.

## The ruling

The operator ruled that string marshalling follows Option B, stated as "make the two
embeddings agree". A string-taking native function must observe the same contract under
the virtual-machine embedding as under the native backend, rather than the two embeddings
presenting different ownership and representation semantics for the same declared
signature.

## Provenance, recorded because provenance is the point

Two events, and only the second one binds this line.

1. **2026-08-29.** The `v0.3.0` line committed `docs/decisions/ABI_RULINGS.md` on its own
   branch recording the ruling. That document was read off `origin/v0.3.0` by this line
   and deliberately not acted on, because a ruling read off another branch is not a ruling
   received, and their own record states the change is not implementable by their line
   since it alters marshalling in `src/`, which the `v0.2.3` line owns.
2. **2026-08-30.** The operator confirmed directly, in session, that the string ruling
   applies to the V0.2.X line as recorded. This document records that confirmation as the
   binding event. The float ruling remains as recorded on the other line's branch and is
   theirs to carry; nothing here restates it.

This file is named distinctly rather than duplicating the other line's `ABI_RULINGS.md`
path, because `v0.3.0` rebases onto `v0.2.3` and an add-add collision on the same path
would conflict on every sync.

## The verified technical claim beneath the ruling

Verified against this tree rather than trusted from the description. `String::from_value`
in `src/marshall.rs` clones an owned `String` out of a `StaticStr` value, so a
string-taking native receives an owned copy under the virtual-machine embedding, while
the native backend passes a length-and-bytes view. The source incompatibility between the
two embeddings is real, and it lives on this line's surface.

## Scope and cost, stated so the trade is on the record

Option B is an embedder-visible ABI change to the shipping crate's marshalling boundary.
Its benefit is realisable on the native backend, which lives on the `v0.3.0` line. Its
cost falls on existing embedders of this line, whose string-taking natives change
signature or semantics. The other line's record flags Option B as the most expensive of
the three options considered and anticipates revisiting strings later. The roadmap's
cross-cutting native ABI item requires the marshalling boundary to be specified rather
than merely implemented, so the implementing increment owes a specification of the agreed
contract alongside the code change.

## What implementation entails, scoped at the design level

The implementing increment must decide and record, at minimum, the agreed representation
a native observes, the ownership and lifetime rules at the boundary, the migration story
for existing embedders including whether a deprecation period applies, and the tests that
pin agreement between the two embeddings. Detail belongs to that increment, not to this
record.
