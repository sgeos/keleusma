# A C host calling a Keleusma protection policy

A worked answer to a question a firmware team actually asks: **can I let a customer change this rule?**

```sh
cd native_codegen && examples/motor_policy/run.sh
```

## The problem

Every motor drive derates on temperature and trips on overcurrent. The thresholds are tuned per
deployment, and **the people who most want to change them are furthest from the firmware team.**

That is exactly where a host would like field-updatable logic and cannot take the risk. An unbounded
loop or an allocation inside a control loop is not a slow response, it is a safety incident.

## What Keleusma changes about that

The policy is **total**, so it terminates by construction. Its memory is **statically bounded**, so it
cannot exhaust the controller. And the verifier **rejects a policy whose bound cannot be proved**,
which is the guarantee the firmware team needs before accepting a field-updatable rule at all.

The build prints the bounds rather than claiming them, and **keeps two different kinds of claim
apart**:

```
TWO DIFFERENT KINDS OF CLAIM, KEPT APART:

MEMORY, which describes THIS OBJECT. The backend's provisioning is
checked against these figures by the bound-transfer tests.
  worst case over all chunks: stack 352 B, heap 24 B
  shared segment 58 B, preallocated by the host and never grown
  NOTHING GROWS AT RUN TIME: the host supplies every region up front.

BYTECODE COST, from the verifier's virtual-machine cost model:
  chunk 0 (derate_for  )     19 cost units
  chunk 1 (main        )    134 cost units
  ^ NOT A BOUND ON NATIVE EXECUTION TIME. It counts bytecode under the
    interpreter's cost model. No measurement in this project relates it
    to the machine code emitted above, and that code may call
    compiler-runtime routines with no bytecode counterpart at all.
```

**The point is not that the policy runs. It is that its worst-case MEMORY was known before it ran.**

> ⚠ **An earlier version of this file quoted both figures under one heading, `PROVEN BOUNDS`.** The
> memory half transfers to the emitted object and is measured against it; the cost half is a bytecode
> count under the interpreter's model and **nothing in this project relates it to native execution**.
> Presenting them together invited the second to be read as the first. The figure is kept because it
> is true about the bytecode; only its subject is now stated. See
> [`NATIVE_WCET_ASYMMETRY.md`](../../../docs/decisions/NATIVE_WCET_ASYMMETRY.md).

## The contract

`policy.h` is **generated from the compiled module**, not written by hand, so its offsets cannot drift
from the code they describe. It is the C-header form of an interface to a separately compiled
procedure.

```c
#define KEL_SHARED_BYTES 58
#define KEL_IO_ZONE_TEMP_0_OFFSET   0  /* Fixed: int64_t of Q-format bits; divide by 2^F for units */
#define KEL_IO_ZONE_DERATE_0_OFFSET 32
#define KEL_IO_TRIPPED_OFFSET       56 /* bool: 0 or 1, one byte */
```

**A `Fixed` value carries its bits and not its scale.** The header states the scale, exactly as a C
header states the layout of anything compiled separately. This policy uses `Fixed<8>`, so one unit is
1/256.

## What the host must supply

The entry takes the two integer arguments and **three trailing pointers**: the shared buffer, the
private region, and the composite region.

**The private region must be word-aligned.** `host.c` declares it as `int64_t` for that reason. A
`char` array satisfies the type and violates the alignment, and the failure presents as a bus fault
rather than a wrong answer.

## What this example does NOT show

**Streams do not lower.** `Stream` and `Reset` are refused by this backend, so the policy is a plain
function called once per control cycle rather than a coroutine. A streaming design would not build.

**The native artefact can demand more region memory than the runtime's verified figure.** The planner
gives every construction site its own offset and never overlaps them, so for some programs its arena
demand exceeds the interpreter's. That is a real gap, measured, and it is not closed here.

**`Text` and `Opaque` shared slots are refused**, and a `Float` slot needs an eight-byte float.

## How it is kept honest

`tests/example_motor_policy.rs` builds this example, links it with the system C compiler, runs the
binary, and compares **the shared buffer byte for byte** against the same policy on the virtual
machine.

The buffer rather than the printed summary, deliberately: a wrong offset or a wrong width writes the
right number to the wrong place and leaves the summary unchanged. That defect class has been found
twice in this package, so the oracle is aimed at it — and shifting a slot offset in the lowering does
make the test fail, which is how the check was shown to be capable of firing.
