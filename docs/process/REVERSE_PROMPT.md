# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-10 (session 40, continued)

## Where things stand

**The driver has stopped copying and started computing.** Two of the three values it owed are done.

| | |
|---|---|
| `v0.2.3` | slices 1–11 merged; two docs commits local |
| `feat/selfhost-wire-driver` | slices 12–13 plus two plan corrections, **gate owed** |
| Machine | free — `wire-corpus@9eb623d` went **GREEN**, 12 steps, 0 failures |

`tests/selfhost_wire.rs` is **125 tests**. Keleusma now computes `STRING_POOL`, `NAMES` and an
input-to-index map from a (name, mode) sequence, and reorders a depth-first constant forest into
the breadth-first `CONSTS` table. Both are byte-identical to `encode_aux_body` on real compiled
modules.

### What is still NOT computed

- **The (name, mode) sequence** is a Rust model of the encoder's call order, restricted to chunk
  names and enum layouts and guarded by `assert_no_other_contributors`. Generating it from the AST
  is the driver's job and is not done.
- **`STATIC_STR`, `STRUCT` and `ENUM` constants**, which intern as they walk and so couple the
  flattener to the interner and the two side tables. That coupling is the next slice.
- **Per-chunk ranges** — `consts_first/count`, `templates_first/count`, `param_types_first/count`.
- **The dedup scan is LINEAR**, the shape that cost the reference 782 seconds on a mid-sized stage
  before it became a `BTreeMap`. Correct at ten names; **it must be replaced before a real stage
  drives it**, where the count reaches 395,804.

## The thing I would most want a reader to take from this stretch

**"The corpus cannot reach X" is a fact about the corpus. Whether a source can reach X is a
separate question, and it has to be asked separately.** I had written the second as though it
followed from the first, in a plan document, on the strength of a sound measurement. Asking it
properly overturned two conclusions in one day:

- The flattener does **not** need hand-built constant trees. `const data`, referenced from a
  function, emits real composite constants to depth 2 in about a kilobyte.
- **Five of the six DERIVE rows** in the emitter coverage matrix are reachable from ordinary source.
  The sixth, `STRUCT_TEMPLATES`, is settled by construction rather than sampling: its only non-flat
  type is `Text` under a narrow word, and this suite is gated out of narrow-word builds.

**The matrix still reads 14 REAL / 6 DERIVE**, because upgrading a row means rewriting its emitter
test and none of that is done. The achievable split is 19 / 1.

## Three defects I caught in my own work, since they generalise

- **A vacuity control caught that a passing test was four-fifths empty.** The flattener differential
  went green while the assertion that its cases distinguish breadth-first from depth-first failed:
  a composite in LAST position makes the two walks coincide, and four of five cases had that shape.
  **A corpus-level control is a different instrument from a must-fire mutation** — the mutation asks
  whether the check can report a defect, this asks whether the inputs can tell two answers apart.
- **A rule can be correct and untestable.** I implemented the interner's last-match semantics with a
  comment explaining it, then noticed it was invisible in both regions the slice emits. The fix cost
  half the name cap. **Prefer a lower cap to an untestable rule.**
- **A probe that reports absence must distinguish "not there" from "I could not read it."** Mine
  read a region at stride 16 where the stride is 8; `records()` failed and `map_or(0)` reported an
  empty region. The all-zero baseline made it look consistent.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB source
  produces a 16 MB artifact, paid three times over in parallel tables plus the pool they index.
- **The (72,64) SECDED plane is entirely unexercised** by the shipping encoder.
- **Gate cost.** The clean figure is still unmeasured; earlier readings were taken under contention.
- **MSRV 1.85 declared, never verified.**

## Parallel development

`v0.3.0` carries native code generation, gated `3d36feb` GREEN, and has adopted
`scripts/gate-status.sh`. Their measurement that matters here: **ten of eleven stage modules refuse
native lowering on `Stream`, not on composites**, so Order 1's native path is gated on
sub-coroutines. Their caveat stands — `lower_module` refuses on the first unsupported opcode, so
`Stream` is necessary, not provably sole. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries — there is no wake.

## Method rules this stretch paid for

- **Ask the reachability question separately from the corpus question.**
- **A green differential against a real oracle can still be weak evidence**, if the corpus cannot
  distinguish the answers. Assert that it can.
- **Order guards by what would otherwise TRAP.** `for k in 0..n limit 341` aborts the VM when the
  range exceeds the cap, so a malformed count must be rejected before it is used as a bound; a
  sticky flag would be set too late.
- **Read the call graph before sampling more inputs.** Thirteen probes said composite constants were
  unreachable; two greps found the path.
- **Check `$?` explicitly; never read success off output.**
