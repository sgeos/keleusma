# Brief — put the bare `for` in the construct-support boundary

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-25. The refusal landed in #273; this makes the gap **measured**
rather than merely refused.

## Why the boundary and not somewhere else

`self_hosted_construct_support_boundary` is this project's canonical answer to
*what does the self-hosted compiler support*. It classifies 95 constructs as
`SOk` / `Refuses` / `Diverges` / `RefRejects`, and its counts are pinned in the
handoff's check block.

**It contains exactly one `for` case and it is the counted form.** A reader
consulting the table to learn whether loops are supported gets a `SOk` and no
indication that the bare form is not lowered at all. The gap is recorded in
`tests/selfhost_bare_for.rs`, which is the right place for the diagnosis and the
wrong place for the *inventory*.

**A construct absent from the boundary is unverified by construction.** That
sentence is already in this tree, written about the bare form, in the file that
records why the gap went unmeasured — and the boundary still does not contain
it.

## What the classification should be, and it is now decidable

Before #273 the bare form panicked with a message about a missing chunk name.
The classifier catches a panic and files it `Refuses`, so it would have been
counted correctly for the wrong reason: an honest gap by accident of a
misleading abort.

**After #273 the refusal names the construct**, so `Refuses` is the truthful
classification rather than a lucky one. Expect **90 SOk / 2 Refuses / 3 Diverges
/ 1 RefRejects**.

## The specific wrong turns

**Do not assume the count.** The classifier runs the shipping entry point and
catches. Run it and read what it says before writing a number — the handoff's own
rule, and the one this session has broken twice by subtracting instead of
deriving.

**Read the gap pin's message before touching it.** The pin asserts the boundary
carries no such case, and is written to fail when that stops being true — which
this increment causes. **Its subject changes; it is not a test to delete
quietly**, and its NAME must change with its subject. Keeping a name that
asserts absence on a test that checks a verdict is how a test comes to measure
something other than what it says, which is the defect three sibling pins were
retired for in #273.

Its own failure message states the resolution: *if the bare form is not
supported, that case's verdict should say so rather than the table implying
coverage it does not have.* Follow it.

**Do not let the new case be satisfied by the old one.** That pin locates the
table by searching for `ctrl/for_limit`, then filters lines containing `for ` and
`in 0..`. A new case must be distinguishable from the counted one by the
classifier AND by any test reading the table as text.

**Do not report a boundary count without re-deriving it.** The counts are pinned
in the handoff and in the test. Both move together or neither does.

## The failure this session has paid for

Two exact figures written by inference rather than measurement, both in files
whose subject is measurement. A prediction made and missed. **Derive it, then
write it down**, in that order.
