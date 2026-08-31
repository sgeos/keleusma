# BRIEF — which lowering arms exist and are exercised by nothing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this class, and why now

**Nested float composite bodies were ACCEPTED and unverified**, and I found that by accident, one
increment after writing them down as "still absent". A refusal is loud. **An accepted path that no
test executes ships a plausible wrong number**, and this backend's whole correctness argument is a
differential — which is worth exactly as much as the paths it actually drives.

That was one instance. **This asks how many more there are, deliberately rather than by luck.**

## The unit is finer than an opcode, and that is the point

The tree already accounts for opcodes: 63 of 66 lower, `Len` refused, `Reset` never visited, one
without a corpus witness. **That granularity would not have caught the nested case**, because
`GetField` was already counted as covered while its `Float` arm had no witness at all.

So the unit here is the **(opcode, scalar kind) pair** the lowering actually branches on —
`GetField` × `Float`, `GetIndex` × `Byte`, and so on — plus the shared-slot kind tags. An arm the
lowering has and the corpus never produces is a candidate; a candidate with no hand-written test
either is the finding.

## What good looks like

A table, over a stated population, of the kind-discriminated arms the lowering carries, marked by
whether the corpus reaches them. **Every entry not reached by the corpus is then resolved
explicitly**: covered by a named hand-written test, or unexercised and recorded as such.

The deliverable is the table and the honest residue. **Closing the gaps is a separate decision** that
the table makes cheap to take.

## Prior failures to avoid repeating

- **Do not derive the arm list by parsing the lowering's source text.** That is a model of the code
  that can be wrong, and it is the fourth-instrument-error shape. Enumerate the kinds from the type
  that defines them and the occurrences from compiled modules.
- **A clean result is evidence about the instrument first.** If the sweep reports every arm covered,
  establish that it CAN report an uncovered one before believing it.
- **State the population on every figure.** This line has quoted 91 modules where there were 67.
- **Do not conflate "the corpus does not reach it" with "nothing tests it".** They are different
  claims and the second is the one that matters; the hand-written tests are a second population and
  must be consulted, not assumed empty or assumed sufficient.
- **Do not append a filter to a command whose status you intend to read.** Fired again this session.
- **Check the binary count, not just the pass count.** A SIGTERM produced a plausible "398 passed, 0
  failed" this session and only the short binary count betrayed it.

## The wrong turn most likely here

**Turning the census into a coverage-closing spree.** The value is the honest list. Writing a test
for every gap in the same increment would bury which gaps were real, and an arm that is genuinely
unreachable deserves a recorded refusal rather than a contrived witness.

## Outcome, written after the measurement

**Population: 69 modules, 1074 chunks.** Five families carry a scalar kind — the flat struct, tuple,
enum and array reads, plus the shared-slot layout tags — over eight kinds, so forty combinations.

**The corpus reaches eight of forty.** `Int` in all four read families and in shared slots, `Fixed`
in the struct and array reads, and `Byte` in shared slots. **Everything else is corpus-silent**, which
is a far larger residue than the opcode-level accounting suggests: `GetField` and `GetIndex` are both
counted as covered opcodes while six of their eight kind arms have no corpus witness.

### The finding that justified the census

**THE CORPUS NEVER PRODUCES A `Byte` OR `Bool` COMPOSITE FIELD OR ELEMENT READ AT ALL.** Zero, in
every one of the four read families. The lowering has a `Byte | Bool` arm that ZERO-extends, and the
tree already records the hazard in its neighbour: *changing the narrow load from zero-extension to
sign-extension left every other test passing.*

`GetField × Byte` turned out to be covered after all, by
`differential::a_byte_field_zero_extends_like_the_vm`, which uses 200 precisely because
sign-extension would read it as −56. **`GetIndex × Byte` was covered by nothing**, and that gap is
closed here with one witness rather than left recorded, because the hazard is known and the arm is
reachable from ordinary source.

### The attribution table was wrong on its first draft, and that is the honest half of this

It listed `GetField × Byte` as unexercised. **Corpus silence is not coverage**, and the second
population has to be READ rather than assumed — which is exactly the distinction this census exists
to keep, applied to the census itself. Six combinations now resolve to named tests; **twenty-six are
unexercised**, most of them kinds this backend refuses outright, where a contrived witness would be
worse than an honest gap.

**The table is hand-maintained in one direction only, and says so.** A row that the corpus starts
reaching fails an assertion; a row that a new test starts covering needs a human. That asymmetry is
recorded rather than hidden.
