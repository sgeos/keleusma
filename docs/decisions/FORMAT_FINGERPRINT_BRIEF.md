# Brief: the format fingerprint, and the layout changes it exists to catch

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this, and why now

The operator has frozen `BYTECODE_VERSION` at 2 until a hardware compiler exists or the language
sees real adoption, and has stated the consequence plainly, that early releases will not be
compatible with one another. The version check is exact equality, so it rejects version 1 and
version 3 and admits every release that says 2. Under the freeze that is every release.

The operator authorized a fingerprint in the header's reserved word as a temporary, unpromised
mechanism, on the reasoning that it will probably work in practice because a version bump will
likely arrive before that word is properly assigned to anything. That framing is what makes it
cheap enough to build without defending.

The timing is the whole argument. Two layout changes are queued, `Opaque` moving to the address
width and `ScalarKind::Text` collapsing to one address once `Text<N>` lands. Built first, the
fingerprint moves on its own when each lands and is demonstrated against real changes. Built
afterwards, it is a detector written after the events it was meant to detect, pinned by a
synthetic test.

## What this is not

It is not cross-release compatibility and must not be described as such. It catches a release
that **sets** a bit or field an older reader did not know about. It does not catch a release that
**reinterprets** a field both readers already read, which is exactly the E5M2-to-E4M3 case at an
unchanged `float_bits_log2`. Only unfreezing the version catches that, and the operator has
declined, with the consequence named.

An earlier draft of this proposal was a fingerprint justified as forward compatibility. The
operator's question, what does the word actually do given the version check already rejects
1 and 3, collapsed it. The honest answer is that it distinguishes two releases that both say 2,
which is a version number under another name. It survives only as a best-effort mechanism the
operator has explicitly accepted as unguaranteed.

## The two design decisions that determine whether it works

**Derive it; do not hand-maintain it.** A constant someone bumps per release fails the way a frozen
version fails, by being forgotten. Hashing the canonical flat-layout table means it moves on its own.

**Evaluate at pairwise-distinct reference widths.** This is the trap. Hashed at 64/64/64, the
`Opaque` change from `word_bytes` to `addr_bytes` produces an identical number, because those
widths are equal there, so the fingerprint would miss precisely the change now in flight. Feed it
distinct values instead. They need not be supported configurations; the fingerprint needs
determinism, not runnability.

## The interaction that nearly made this expensive

Twelve Keleusma stage sources emit this header and are compared byte-for-byte against the Rust
reference. Changing what the reference writes could have broken every one.

It does not, because `wire.kel` writes the reserved field from an input rather than a literal. The
Keleusma side needs the right value seeded, not new logic. **Seed it from the same constant the
reference uses.** A duplicated literal would drift, and the drift would be invisible until someone
read both files.

The consequence is favourable and worth stating. If the fingerprint moves and the seed does not
follow, the byte-identity comparisons fail loudly. That answers the objection that a
hand-maintained value fails by being forgotten, because on the Keleusma side it cannot be
forgotten.

## Prior failures in this session, all one species

Three times, a check that passed in one configuration was reported as a conclusion about a wider
set.

- `feat/opaque-address-width` was recorded as compiling against every test target. It compiles
  under default features and fails under `self-host` at five sites and under `signatures,shell`
  **with test targets** at one more. A `cargo check` without `--tests` reports that last one clean.
- The float ladder documentation was reported as not existing after a search of remote branches.
  It existed, unpushed, in a peer's worktree. The search was sound and the conclusion was wider
  than the search.
- A background gate was read as green from a notification that reported the wrapper's exit code.
  The gate was three steps into nine.

**The rule this session keeps relearning: name the feature set and target selection whenever a
compile or test result is written down, and never let a green result stand for a set larger than
the one measured.**

## Specific wrong turns to avoid

**Do not gate the reserved word and the fingerprint against each other.** A fingerprint there is
non-zero by construction, so a zero-check on the same word would reject everything. The zero-check
survives only for the unused flag bits.

**Do not claim the oracle still passes without running it.** The self-hosted comparisons are
excluded from the routine push tier and run at the merge gate. A green push proves nothing about
them.

**Do not repair the six `addr_bytes` sites by inferring the argument.** Each call site has a
correct width and a plausible wrong one, and `word_bytes` and `addr_bytes` are equal on the
development host, so a wrong choice passes every test run here and is wrong on a 32-bit target.
