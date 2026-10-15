# Order 3 bounds contracts

## Scope

This document fixes the first bounds-contract increment within V0.2.3-WIP.
BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1. The
public instruction count remains 66. Development artifacts must be rebuilt after
contract changes. No publication is authorized.

BoundsCheck, indexed writes and the explicit bounds trap are specified below.
Flat Word and Byte array reads are also specified below. GetDataIndexed still
needs its total read contract. Non-first-pass GetIndex forms remain explicit proof
boundaries. These remaining internal bounds faults are open Order 3 requirements. This
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
and resource consumers must agree with the new contract. Indexed write lowering is implemented in both compilers. Indexed read lowering
remains part of the following increment. Do not infer support
for source syntax that the first-pass compiler does not yet admit.

## SetDataIndexed

The input stack ends with [value, index]. The instruction consumes both and returns
[index, b0, b1, b2, b3, flag], using the portable suffix above. A valid index writes
exactly one slot and returns flag zero. A negative index, exact upper bound or empty
span performs no write and returns flag one. The value is consumed on either outcome.
The span must fit the declared data segment before any access occurs. Invalid spans,
and wrong index types remain errors. Valid accesses retain the existing storage
contract checks.

Stack growth is four and shrink is zero, matching two inputs and six outputs. The
depth requirement is two and net effect plus four. All outgoing entries are Words.
The nominal cost estimate is eight units. No new opcode or operand encoding is used.

Both bare compiler lowerings compare the flag with Word one and emit the explicit
bounds trap on failure. The success path discards the remaining five diagnostic
Words. Operand evaluation order and source fault locations remain observable.
Direct bytecode can inspect the flag and handle failure without emitting a Trap.

## GetIndex for Word and Byte

Flat Word and Byte operands consume [array, index] and return
[value, index, b0, b1, b2, b3, flag]. An invalid index returns zero of the specified
element kind and flag one without reading an element. A valid index returns the
element and flag zero. The diagnostic bound is the actual element count. Empty
arrays are valid and always yield an invalid-index outcome. Type, representation,
stride and arena-lifetime failures remain contract errors.

The requirement is two and net effect plus five. Growth is five, shrink zero and
nominal cost eight. Other GetIndex operand forms retain their previous single-result
contract and remain outside the first-pass trap-free proof. They must be reported as
unsupported by that proof, never silently treated as total.

Bare lowering compares flag with one, traps on invalid, and discards the five
remaining diagnostic Words on success. Handled Word and Byte reads store the result,
index, limbs and flag in seven temporaries after evaluating the operands. Fully
covered invalid_index outcomes omit the fallback trap. A deterministic unreachable
value-and-break trailer keeps the virtual dispatch loop structurally bounded.

## GetDataIndexed

The input stack ends with [index, fallback]. The index is Word and fallback is
Word, Byte or Unit. The instruction consumes both. A valid index reads the selected
slot and an invalid index uses the supplied fallback without accessing storage.
It returns [value, index, b0, b1, b2, b3, flag]. Flag zero means a read occurred and
flag one means fallback. The complete span must fit the declared segment. The empty
span at base zero is valid even when the module has no data slots.

First-pass compilers supply Word zero or Byte zero for their corresponding element
kind. Other source kinds use Unit, and bare failure still reaches an explicit trap.
Fallback operands cannot carry text or composites. Their type is checked at runtime.
This keeps the new outcome compatible with the existing text analysis boundary.

The requirement is two and net growth five. Shrink is zero and nominal cost eight.
The output value remains an unknown shape in the typed verifier, with six Word
entries following it. No existing shape or host-contract check is removed. The
opcode and its two u24 wire operands remain unchanged.
