# Order 3 bounds contracts

## Scope

This document fixes the first bounds-contract increment within V0.2.3-WIP.
BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1. The
public instruction count remains 66. Development artifacts must be rebuilt after
contract changes. No publication is authorized.

BoundsCheck and the explicit bounds trap are specified below. GetIndex,
GetDataIndexed and SetDataIndexed still need their total read and write contracts.
Their existing internal bounds faults remain an open Order 3 requirement. This
increment alone cannot establish trap freedom or justify check elision.

## Portable diagnostic suffix

Stack notation lists the bottom first. The diagnostic suffix is
[index, b0, b1, b2, b3, flag]. All six entries are Words. The index retains its
original signed value. The low eight bits of each bi encode the corresponding
little-endian byte of the unsigned 32 bit exclusive bound. Producers sign-extend
each byte when constructing a Word, so the encoding also fits an 8 bit runtime.
The flag is zero for an index in range and one otherwise.

Four byte limbs are intentional. Two 16 bit limbs would lose information on the
supported 8 bit Word runtime. The representation has no hidden virtual-machine
state, allocation, or dependency on an earlier failed access. Nested handlers
cannot overwrite another access's diagnostic payload.

## BoundsCheck

The immediate remains an unsigned 16 bit exclusive bound. The instruction requires
one Word index. It preserves that index and appends b0, b1, b2, b3 and flag. The
upper two bound limbs are zero. It performs no indexed data access and cannot raise a
language bounds fault. Negative indices and a zero bound produce flag one.

An empty operand stack and a non-Word operand remain contract failures. Runtime
checks for these conditions remain. The depth effect is requirement one and net
plus five. Stack growth is five and shrink is zero. The nominal cost estimate is
seven units, accounting for the five added payload slots relative to the old
predicate. This is an unmeasured estimate, not a hardware timing guarantee.

## Explicit bounds trap

TrapKind IndexOutOfBounds uses code eight. The stack payload is
[index, b0, b1, b2, b3], with no flag. The instruction consumes these five Words and
returns the existing IndexOutOfBounds error with the reconstructed bound. A bound
that cannot fit the host error representation is malformed bytecode and is refused.
Unknown trap codes remain malformed bytecode. Source fault locations name the Trap.

The depth effect is requirement five and net minus five. Stack growth is zero and
shrink is five. The nominal cost estimate is six units. Both depth verifiers must
check terminal operands before ending a path. Both typed verifiers must enforce
operand availability and floors of loop frames before termination. Their existing
shape widening remains unchanged and does not prove exact Word kinds. Runtime
payload type checks therefore remain mandatory.

## Bare lowering

After BoundsCheck, compare flag with Word one and enter a structured If only when
it matches. That branch emits Trap IndexOutOfBounds using the existing payload.
The successful path discards the four bound limbs and retains the original index
for stride arithmetic. The branch introduces no anonymous local slots.

The Rust compiler uses this lowering for per-dimension checks. Keleusma verifier
and resource consumers must agree with the new contract. Indexed read and write
lowering in Keleusma remains part of the following increment. Do not infer support
for source syntax that the first-pass compiler does not yet admit.
