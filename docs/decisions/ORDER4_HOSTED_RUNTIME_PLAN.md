# Order 4 hosted runtime plan

> **Navigation** [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md) | [Order 3 audit](./ORDER3_COMPLETION_AUDIT.md) | [Decisions](./README.md)

## Purpose and status

Order 4 is the first pass of Workstream D, a bytecode interpreter for Keleusma
written in Keleusma and run on the Rust virtual machine. This plan records the
operator rulings, the gate, the design and the slices. The interpreter is
`src/hosted/interp.kel`, its driver `src/hosted/mod.rs` behind the `self-host`
feature, and its harness `tests/hosted_runtime.rs`. Slices 1 to 4 landed on 2026-10-25.

## Operator rulings

Both rulings were given first-hand in the V0.2.X session on 2026-10-25.

1. **Gate.** The interpreter runs the modules the self-hosted compiler emits for
   a corpus of subset programs, including Stream, Yield and Reset, and its
   results match the Rust virtual machine on every case. Running the
   self-hosted compiler's own stage modules under the interpreter is not part of
   the first pass.
2. **Bound composition**, roadmap open decision 2. The interpreter is a `loop`
   that executes at most a fixed number K of interpreted instructions per tick
   and then yields. Its own per-tick time and memory bounds are therefore
   constants. A module is admitted only when its verified memory bound fits the
   interpreter's fixed capacities, and its verified time bound bounds the number
   of ticks.

## Measured starting point

Measured on 2026-10-25 at `ec536b28` by compiling the 107 `Ok` cases of
`self_hosted_construct_support_boundary`. Those cases are byte-identical between
the two compilers, so the reference compiler's output is the self-hosted
compiler's output.

- Every case is a single chunk. None calls another function.
- One case uses Stream, Yield and Reset. Five use the data segment. One uses a
  string.
- About fifty instruction forms occur. Control flow, locals and constants
  dominate, with flat composite construction and reads over arrays, structs,
  tuples and nested enums, checked arithmetic, and the Order 3 bounds trap.

The corpus is therefore too thin for the gate on its own. Calls, multi-iteration
streams and data persistence across Reset need a supplement.

Every instruction costs at least one cycle in `NOMINAL_COST_MODEL`, including
the structural ones. A module's verified nominal time bound for one stream
iteration is therefore also a bound on the number of instructions that
iteration executes. A custom cost model need not have that property, so
admission uses the nominal model.

## Gate

The gate is met when all of the following hold.

1. The interpreter source is a Keleusma program that the Rust verifier admits,
   with constant per-tick time and memory bounds.
2. The corpus is the 107 `Ok` boundary cases plus an Order 4 supplement. Each
   supplement program must itself be byte-identical between the two compilers,
   checked by the harness rather than assumed.
3. For every case and every argument vector in its fixture, the interpreter and
   the Rust virtual machine agree on the outcome class, which is finished,
   yielded or trapped with its kind, and on the value. Scalars compare exactly.
   Flat composites compare by body bytes.
4. For every streaming case the number of ticks per stream iteration is within
   the bound computed from the module's verified time bound and K, and the
   harness counts the ticks it observed.
5. A module that fails admission is refused by the host before execution, and
   capacity exhaustion inside the interpreter is a distinct status that a
   negative test reaches.

## Design

**Input.** The host decodes the verified module and marshals it into shared
tables, as the analysis and verification stages already receive their input.
The tables are an instruction table of one tag and up to three operands per
instruction, a chunk table, a constant pool of tagged words and the data layout.
Decoding wire bytes inside Keleusma is outside the first pass and is recorded as
a residual.

**State.** Private data holds the interpreter state across ticks. That is the
operand stack as parallel tag and word arrays, the frame stack, the program
counter and chunk, a byte heap for flat composite bodies with a top pointer,
and the interpreted data segment.

**Values.** A scalar is a tag and a word. A flat composite is a heap offset and
a size. Field and element reads compute a byte offset and read a scalar from
the heap. Reset returns the heap top to the mark taken at Stream, as the arena
does.

**Ticks.** Each tick executes at most K instructions. A tick ends early on
Yield, finish or trap, and reports a status with the yielded or returned value
in shared output. The host resumes the interpreter until the interpreted
program yields or finishes.

**Admission.** The host admits a module only after the Rust verifier accepts it,
every instruction form is in the supported set, and its operand-stack, frame and
heap requirements from the memory analysis fit the interpreter's capacities.
Admission is host-side in the first pass.

**Composed bound.** With W the module's verified nominal time bound for one
stream iteration, that iteration executes at most W instructions and so takes at
most the ceiling of W over K ticks, plus one for the yield. Each tick costs at
most the interpreter's verified per-tick time bound. Memory is the interpreter's
verified bound, which is constant because the capacities are fixed.

## Slices

1. Host driver, table marshalling, the tick loop, scalar and control-flow
   instructions, and the differential harness over the scalar part of the corpus.
2. Flat composites, meaning construction, field, tuple and element reads,
   nested forms, enum tests and enum fields, over the equality part of the
   corpus.
3. The data segment, checked arithmetic and the trap kinds, including the
   bounds trap payload.
4. Calls and returns over the supplement.
5. Stream, Yield and Reset with fuel slicing and the tick-bound check over
   multi-iteration supplement cases.
6. The completion audit.

## Slice record

**Slice 1, 2026-10-25.** Scalars and control flow: constants, locals, the
operand stack, structured branches and loops, comparisons, checked Word and Byte
arithmetic through the language's own checked-arithmetic construct, wrapping
Byte arithmetic, bitwise operations, shifts, conversions and the trap kinds
including the bounds payload. The harness admitted 47 of the 107 corpus cases
and ran 698 argument vectors, 691 finishing and 7 trapping. Every run agreed with
the Rust virtual machine on outcome and value, and stayed within its instruction
and tick bounds. The other 60 cases were refused at admission, for composites,
the data segment, the stream case or a string constant, which later slices
address.

The corpus cases name their function `f`, which leaves a module without the entry
point `Vm::new` requires, and the boundary test only compares bytes. The harness
renames the entry to `main`, and the shipping self-hosted compile re-establishes
byte identity for each renamed program.

The instruction-count check found a defect outside Order 4. Seven runs exceeded
their verified time bound by one, because the time walk charged nothing for
`Else`, `EndIf` and `EndLoop` and omitted a counted loop's final header pass. The
operator ruled first-hand to fix it first. See
[WCET_STRUCTURAL_MARKERS.md](./WCET_STRUCTURAL_MARKERS.md).

**Slice 2, 2026-10-25.** Flat composites: construction, struct, tuple and enum
field reads in flat and nested forms, Word and Byte element reads and nested
element reads with the Order 3 bounds outcome and placeholder, and `IsEnum` by
discriminant. A composite is a body in a heap of one byte per Word, which the
host writes for composite arguments and reads for a composite result. Bodies are
immutable, so a nested read is a view and copies nothing. A string constant is
carried as an inert tag, used only as a name operand of `IsEnum`, which a flat
enum ignores. Arguments are generated from the declared types, rotated so
successive samples differ in every position, with every enum variant. The
harness admitted 101 cases and ran 971 argument vectors, 959 finishing and 12
trapping, all agreeing with the Rust virtual machine. 615 runs passed a
composite argument and 4 returned a composite. Five data-segment cases and the
stream case remain refused.

**Slice 3, 2026-10-25.** The data segment, meaning `GetData`, `SetData`,
`GetDataIndexed`, `SetDataIndexed` and `BoundsCheck` with their Order 3 outcome
tails, over private scalar slots. The host seeds each slot with the value the
Rust virtual machine gives it at load. That value is read from a wire round trip
of the module rather than from the module in memory, because the virtual machine
reads the encoded form, where an omitted initializer becomes `Unit`. Shared slots
and private composite slots are refused. The harness admitted 105 cases and ran
987 argument vectors, 971 finishing and 16 trapping, all agreeing with the Rust
virtual machine, 16 of them through the data segment. One of those reads a slot
it never wrote, so the seeded initial values agree. Two cases remain refused.
`ctrl/for_bare` has a loop with no static iteration bound, so no verified time
bound exists for it, and the conservative stance refuses it. `ctrl/loop_yield`
is the stream case of slice 5.

**Slice 4, 2026-10-25.** Calls and returns, with the callee's locals beyond its
arguments starting as `Unit` as in the Rust virtual machine. The per-chunk time
bound charges a call its dispatch and not the callee's body, so admission
computes a transitive instruction bound. A chunk with shallow bound `w`
executes at most `w` over the nominal cost of a call, which is 10, calls, and
each runs a callee within the largest callee bound `m`, so `w + (w / 10) * m`
bounds the chunk. The figure is computed bottom-up, and a cyclic call graph is
refused rather than assumed absent. It is loose and sound. A ten-program call
supplement covers argument passing, nested and repeated calls, a call in a
counted loop, composites passed to and returned from a callee, a trap in a
callee and calls in conditions. All ten compile byte-identically through the
shipping self-hosted compiler. The harness admitted 115 of the 117 cases and ran
1062 argument vectors, 1034 finishing and 28 trapping, all agreeing with the Rust
virtual machine, 75 of them across a call. A control shows a run through a
callee exceeding the entry's shallow bound while staying within the transitive
one.

## Not in the first pass

Native calls, Text operations, Fixed arithmetic, floats, boxed composites,
decoding wire bytes in Keleusma, interpreter-side admission, and running the
self-hosted stages under the interpreter.
