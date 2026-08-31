# Brief: the lexical divergence census

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief, self-directed. Not an operator ruling.

## The finding this generalises

On 2026-08-30 the string ABI increment found two defects by writing a test that asserted a
contract, not by reading code:

1. `lex_string` in the reference compiler pushed each scanned byte as `c as char`, so every byte
   at or above `0x80` was re-encoded. A six-byte literal baked as eleven bytes. **Every non-ASCII
   string literal in the language was silently wrong.**
2. The self-hosted driver's `unescape_string` handled four escapes where the reference handles
   six, so a literal carrying `\r` or `\0` compiled to different bytes under the two pipelines.

Both were invisible to the byte-identity oracle for one reason, and it is the reason that matters.
The first statement of it was that no `.kel` source contains a non-ASCII literal or an escape
sequence. **The measurement came back stronger: every double quote in all twelve stage sources is
inside a line comment, so the corpus contains ZERO STRING LITERALS.**

Not "no escapes" -- nothing. Every property of the string-literal path is unwitnessed by the
oracle: escapes, non-ASCII content, interning and deduplication, the empty literal, and the
constant pool's string tag. The two defects above were therefore not near-misses in otherwise
covered code; they were in a region the oracle has never once exercised. The oracle is a strong
instrument over the inputs it is given and says nothing whatever about inputs it is not.

In defect 1 the self-hosted pipeline was CORRECT and the reference was wrong. That direction is
worth holding onto: the reference is not the definition of right, it is one of two implementations.

## The goal

Establish, by measurement, where the reference compiler and the self-hosted pipeline disagree on
inputs the corpus does not contain, beginning with the lexical surface. For each divergence found:
fix it if one side is plainly wrong, and pin it if the correct answer is not this line's to choose.

The deliverable is a census that is **checked**, not a document that asserts one. A census whose
claims are prose is the artefact class this tree keeps having to repair.

## Prior failures to avoid, each paid for already

**A guard that finds nothing must fail, not pass.** The tail-row coverage assertion counted
statement forms and stayed green when two of six kinds were deleted, because those corpus cases
ended in a node neither side could type and both readings produced the identical unknown row. A
guard written specifically to prevent vacuous coverage was itself vacuous. Every assertion in this
census needs a non-vacuity check in both directions: it must fail if the probe set empties, and it
must fail if the thing it discriminates stops discriminating.

**A probe that fails to compile proves nothing and looks like silence.** A divergence probe that
the self-hosted subset refuses is not evidence of agreement. Refusals must be counted and reported
separately from agreements, or the census will read "no divergence" when it means "no measurement".

**A spike must match the crate type of the thing it sizes.** The borrowed-argument impl family
compiled in a binary-crate spike and produced 44 coherence errors in the library, because coherence
has no downstream to defend against in a binary. That cost an hour today.

**Do not restate a set the other side defines.** The escape pin derives its six-member set by
scanning all 128 ASCII bytes and asking the reference what it accepts. Had it restated the set, it
would have gone stale the first time an escape was added, silently. Derive every axis from the
implementation rather than from this brief.

**Read what CONSUMES the data.** Three wrong sizings on this line came from reasoning about a table's
shape instead of reading its two consumers. Before claiming an axis cannot diverge, find the code
that reads it.

**Do not fix a divergence by deciding a language question.** Where the two sides differ and neither
is obviously wrong, the answer may be an operator call. Pin the divergence in the failing direction
with a message that names the choice, and record it. Silently picking one is how a ruling gets made
by an implementation detail.

## Axes worth probing, derived not assumed

Enumerate the lexical surface from `src/lexer.rs` itself rather than from this list, which is a
starting point and is not claimed complete:

- string literals: non-ASCII content, every escape the reference accepts, escapes the reference
  rejects, an empty literal, a literal that is only escapes, adjacent literals, duplicate literals
  and whatever interning does with them
- integer literals: decimal, hexadecimal, binary, boundary magnitudes, leading zeros
- comments: line comments in every position, a comment containing a quote or a backslash, a comment
  at end of file with no trailing newline
- identifiers and keywords at their length and shape boundaries
- whitespace forms, including a file with no trailing newline

## Measured outcome, 2026-08-31

The census runs 49 probes across six axes against the SHIPPING driver and reports **49 agree, 0
diverge, 0 refused, 0 rejected by the reference**, in roughly two minutes.

**A clean result of that shape is not evidence until the instrument is shown to discriminate.** Two
positive controls drawn from the construct-support boundary, which records their outcomes
independently, are checked before the census runs: a generic function, which the subset refuses,
and float arithmetic, which compiles on both sides and produces different bytes. Both report as
recorded, so the classifier can see a refusal and a silent miscompile. Without the second control
in particular, a `classify` that always returned agreement would produce identical output.

## What "done" is not

It is not "I looked at the lexer and it seems fine". It is not a count of probes written. It is a
census whose agreement claims are executed against both pipelines, whose refusals are counted apart
from its agreements, and whose non-vacuity is asserted rather than assumed.
