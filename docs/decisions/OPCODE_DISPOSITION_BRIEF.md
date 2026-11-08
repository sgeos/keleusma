# Brief — "63 of 66" reads as three missing opcodes, and none of the three is missing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02, night.**

---

## The present goals

| goal | state |
|---|---|
| **the disposition of the unlowered opcodes** | **this brief** |
| the verdictless opcodes | done and measured; verification in flight |
| the workspace staleness clause | done, and executable |
| `f16` | blocked, no oracle. The reference refuses `float_bits_log2` 3 and 4 at load |
| publication | held |
| absorption 48 | nothing unabsorbed |

## The problem is a reading, and the reading drives prioritisation

The ISA lowering census reports **63 of 66 lowered** over 74 corpus modules. **The natural reading is
that three opcodes need implementing.** That reading is wrong for all three, and it is the kind of
wrong that sends effort at work that should not be done.

| opcode | census row | actual disposition |
|---|---|---|
| `Reset` | UNPROVEN | **accepted.** Measured this session: 33 corpus modules emit it, 32 accepted, 1 refused, dispatched in none. It is consumed by the degenerate-stream SHAPE match, which the census does not instrument |
| `IsStruct` | NO CORPUS WITNESS | **no verdict available.** Zero witnesses, no probe. A reachability fact, not a support fact |
| `Len` | NAMED REFUSED | **refusing is CORRECT**, and lowering it would be a defect |

**So the count of opcodes whose support is missing is zero**, and the fraction says otherwise to
anyone who does not read the rows beneath it.

## Why lowering `Len` would be a defect rather than progress

`Op::Len` on a flat array returns `VmError::InvalidBytecode`. The reference compiler emits it from an
ordinary program, `for x in if c { a } else { b }`, because the static length analysis has no arm for
a conditional source.

**The reference side traps.** A backend that lowered `Len` would compute a length where the reference
errors, which manufactures divergence in the one signal this line treats as its correctness oracle.
Refusing is the behaviour that keeps the two sides comparable.

**And the repair is not this line's.** `src/vm.rs` and `src/verify.rs` belong to the `v0.2.3` line and
are read-only here. The existing hazard file already reports this; what is missing is that the census
does not connect its own `Len` row to that report.

## Prior failures this brief exists to avoid repeating

**A fraction read past its population** is this line's most frequent error, recorded as SCOPE_DELETION
and found again twice today: a seven-file figure read as the size of a coverage gap, and a
count-of-tests-reading-docs inflated by a grep that matched comments.

**An absent verdict read as a negative verdict.** Fixed this session for `Reset`, but the census output
itself still invites it.

**A guard that agrees with itself.** Twice today: a reach test green under a mutation that
mis-classified the source tree, and a negative assertion that would have held with a dead instrument.
Any check added here needs a case where it must fail.

## The wrong turns

**1. Do not implement `Len`.** It is the one row that looks most like actionable work and is the one
that must not be done. Record why, so the next reader does not treat it as a gap.

**2. Do not change the census's counting to make the fraction prettier.** Moving `Reset` into the
lowered column would hide that it is accepted by a route the instrument cannot see, which is a fact
worth keeping. **The remedy is disposition alongside the count, not a better count.**

**3. Do not claim the backend is complete.** "No opcode is known to be missing support" is a
statement about two censuses over their populations. `IsStruct` has no verdict at all, and a lowering
verdict is not a correctness claim in any case.

**4. Do not put the explanation only in prose.** The misreading happens at the census output. If the
disposition is not visible where the fraction is printed, the next reader repeats the inference.
