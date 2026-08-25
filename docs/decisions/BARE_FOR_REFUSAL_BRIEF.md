# Brief — refuse the bare `for` by name, before supporting it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-25. The handoff names bare-`for` support in `parse.kel` as the
largest single Order 1 win. **This increment is not that.** It is the refusal
that should exist whether or not support ever lands, and it is a prerequisite
for landing it safely.

## Why the refusal first, and why it is not a consolation prize

`self_host_compile` on a bare `for` panics with ``no chunk named `acc` ``. Seven
iterations of diagnosis were needed to trace that through five layers to its
cause: phase 4 of the loop-header machine waits for the contextual `limit`
identifier, the bare form never supplies one, the header never reaches its body
phase, and the braces are attributed to the wrong block. **The message names
neither the construct nor the file.**

This project's parser names thirteen failure modes with their own causes,
precisely so a user learns what happened. The self-hosted front end has eleven
diagnostic codes and a clean channel for them — `perr(code, detail)` records the
first cause, and `step()` reports it INSTEAD of the record just computed, so the
host stops at the next record boundary rather than consuming a garbage stream.
**The machinery is already there; the bare form simply does not use it.**

## The measurement that scoped this

Bare `for` is **26 opcodes**; `for … limit` is **70**. Measured, not recalled —
the handoff's 24-against-68 is the same claim with a slightly different
harness. So support is a **second lowering**: a plain `Loop` with a `BreakIf`
and two frame slots, against `ForLimit`'s five slots, its in-body cap counter,
and its whole post-loop outcome analysis with a `Trap`.

That is a multi-file change across `parse.kel`, `reconstruct.kel` and
`codegen.kel`, plus a boundary case. **It is not this increment**, and saying so
plainly is better than starting it and leaving it half-landed.

## Where the refusal goes, and why not in the driver

**In `parse.kel`, at phase 4.** The phase machine already knows: when phase 4
sees `{` instead of the `limit` identifier, the input is unambiguously the bare
form. A driver-side pre-scan would be a syntactic proxy for a fact the parser
holds exactly.

It also becomes the first **unsupported-construct** code among eleven that are
all **capacity** limits. That distinction is worth stating in the code: a
capacity diagnostic tells a user to split their function, and this one tells
them the construct is not implemented.

## The specific wrong turns

**Do not assume the diagnostic reaches the host.** `perr` sets a field; it does
not stop the parse. The claim that the host refuses cleanly rests on `step()`
checking `perr_code` on every record, and that must be OBSERVED on this input,
not read off the source. The existing failure is a downstream panic, and a
diagnostic that arrives after it changes nothing.

**Do not let the refusal become the test's only subject.** A test that the bare
form is refused passes equally if EVERY input is refused. The `limit` form
compiling byte-identically is the control, and it is already written.

**Do not delete or weaken `tests/selfhost_bare_for.rs`.** It is a gap pin,
written to fail when the gap closes. This increment does not close the gap — it
changes how the gap REPORTS — so the pin's subject changes and the pin stays.
Read its message before touching it.

**Do not report a message that names the cause without naming the remedy.**
`for … limit N` is accepted and byte-identical today. A user who hits this wants
to know that.

**Do not renumber the existing codes.** They are matched by value in
`describe_parse_diagnostic` and pinned against the stage. Append.

## The failure this tree keeps repeating

Every significant correction this session came from running something, not from
reading it. Four instrument failures, all first outputs of freshly written
tools; two forwarded to another line unverified. **Write the probe, then the
assertion.** And when the output is a name or a code, go and touch what it
points at rather than re-reading it.
