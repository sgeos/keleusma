# One Binary, Selectable Phases

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: operator direction, 2026-08-17. The architecture is decided; the intermediate
representation's format is not, and the question that must be answered before designing it is stated
below.

> **The filename says `PIPELINE_THEN_MONOLITH` and the "then" is now wrong.** It reflects an earlier
> framing in which a pipeline was built first and collapsed into a monolith later. That framing is
> superseded: there is one architecture, and the pipeline and the monolith are two configurations of
> it. The name is kept so existing links resolve.

## The architecture

**One binary with selectable start and end phases.** Everything between them is fused and
demand-driven; the boundaries at the ends are serialised.

```sh
# The monolith. Nothing serialised internally.
cat input.kel | kelc --start=lex --end=emit > output.bin

# The full pipeline, one process per phase.
cat input.kel | kelc --start=lex --end=lex | kelc --start=parse --end=parse | ... > output.bin

# Any subset, fused where it is useful and cut where it is not.
cat input.kel | kelc --start=lex --end=reconstruct | kelc --start=codegen --end=emit > output.bin

# Or cut to a file, and resume later.
cat input.kel | kelc --start=lex --end=reconstruct > output.ir
cat output.ir  | kelc --start=codegen --end=emit    > output.bin
```

**There is no migration between a pipeline and a monolith, because they are the same program.** The
monolith is `--start=first --end=last`. The shell pipeline is N invocations with `start == end`. Same
binary, same code, same logical composition; only the fusion boundaries move.

## What the logical pipeline is

A stage consumes input units and produces output units; the next stage consumes those **as they
become available**. Fused, a unit is passed in memory. Cut, it is serialised. The composition is
identical either way, which is why the structure is permanent and only the transport is a choice.

The stages are already the right building block: all twelve are `loop main(resume) -> Word`, yielding
one unit per step, which is a generator. What defeats the composition today is the DRIVER, which
materialises between stages -- `br_lex` collects every token into a `Vec` before `parse.kel` sees one,
then seeds them into a 40,960-word shared array.

## Why this is worth more than the memory bound that motivated it

**The differential becomes bisectable, and that is the largest win.** The correctness signal for the
self-hosted compiler is byte identity against the reference. Today a divergence says *the artifact
differs*; it does not say where. With phase cuts, `--end=phase2` and `--end=phase3` bracket the
divergence to a phase. That converts the oracle from a detector into a locator.

**Side channels cannot hide.** Every boundary must be serialisable, so nothing passes between phases
through host state without appearing in the intermediate. The distinction this project keeps having to
state by hand -- a region COMPUTED by the stage against one merely FORMATTED from host-supplied
values -- becomes structural rather than something a reviewer must remember to ask about.

**Each phase is separately testable** against recorded inputs and outputs, rather than only through
the whole pipeline.

**No runtime cost when fused.** `--start=first --end=last` serialises nothing internally. The cost is
design cost, paid once.

## BOUNDARY FACTS: A STREAM IS NOT ALL A PHASE NEEDS

**A phase may depend on a whole-input property of an earlier phase's output**, which is not something a
stream can carry without destroying the stream.

The case in hand: `parse` needs the lexicographically sorted set of function names, because a resolved
call index must match the module's chunk order. That is a property of the LEXER's ENTIRE output.

### Why it cannot go in-band, which is the argument FOR a side channel

Carrying it as a header section of the lexer's output stream would force the lexer to see every token
before emitting its first one. **That destroys the streaming property of the very phase being
streamed.** A trailing section does not help either: the consumer needs the table BEFORE the tokens it
annotates.

So a side channel is not a convenience. It is what lets both phases stream, and that is a better
justification than the (real) precedent of `-fprofile-use`, dependency files, linker scripts and
`--sysroot`.

### The chosen shape: a pre-pass PHASE plus a file option

The pre-pass is an ordinary phase, so it needs no special case in the `--start`/`--end` model. Phase
zero produces the table; the rest consume it.

```sh
cat input.kel | kelc --start=lexchunk --end=lexchunk > chunk.ir
cat input.kel | kelc --start=lex --end=bytecode --chunk=chunk.ir > output.bin
```

### THE OPEN FORK: both phases read the SOURCE, so the input must be re-readable

Phase zero consumes the source to build the table; phase one needs the source again from the
beginning. The two-invocation form above works because the shell reopens the file each time. **A
single fused `--start=lexchunk --end=bytecode` reading from a PIPE cannot do that** -- once standard
input is drained it is gone.

Three ways out, and the choice decides whether the monolith is one command or necessarily two:

| option | consequence |
|---|---|
| **Buffer the source** | Fusion works from a pipe. Costs O(input) memory -- the smallest representation in the pipeline, but not O(1), in a design whose selling point is a bounded footprint |
| **Accept a file operand**, keeping standard input as the default | Fusion works by reopening; standard input remains for cut pipelines. Matches every compiler driver, so it surprises nobody |
| **Always split** at the pre-pass | No re-read and no buffering. The monolith is two commands |

**Unresolved.** The middle option looks best and costs least, but it is the operator's call. Note that
`--chunk` can only be OPTIONAL -- supplied if present, derived if absent -- under the first two: with
pure standard input there is nothing to derive it from a second time.

### REQUIRED WHICHEVER WAY THAT GOES: fingerprint the sidecar

`--chunk=` naming a table built from a DIFFERENT input produces a byte-plausible wrong artifact. Under
the always-split form the two invocations are always separate, so this is not an edge case -- it is
the ordinary way to hold the tool wrong.

**Stamp the sidecar with a fingerprint of the input it was derived from and verify it on load.** A
hash of the source or of the token stream. Cheap, and it converts a silent wrong artifact into a
refusal naming both files. Without it the option is pointed at exactly the property the project exists
to prove.

### One decision to take once rather than N times

**Enumerate the whole-input facts before fixing the sidecar's format.** The chunk table is the only one
known, and only because something asked for it; nothing has yet put the question to each phase.

If the enumeration turns up more, one sidecar with SECTIONS beats `--chunk= --syms= --enums=`: one
file, one fingerprint, one correspondence to check, and the pre-pass runs once regardless of how many
facts it yields. That is what an object file's symbol table is. Genuinely a judgement call, and it
depends on an enumeration nobody has done.

## Two properties to build in from the start

**A phase tag and a version stamp in every intermediate.** `--start=codegen` handed output meant for
`parse`, or two invocations of different binary versions, must fail LOUDLY. Without it a wrong pipe
order produces a plausible wrong artifact, which is the exact failure mode this project's verification
stance exists to prevent.

**Intermediates are explicitly UNSTABLE.** Stamped to the binary version, mismatches refused. They are
a composition and debugging surface, not a compatibility promise. Saying so now costs nothing; saying
it after someone depends on one costs a great deal.

## WHY TURBO PASCAL IS CITED, which is not as a template

**It is the strongest prior art that self-hosting under a hard memory bound is POSSIBLE.** A complete
self-hosted Pascal compiler in tens of kilobytes, and fast. That is an existence proof, and it is cited
because the question it answers -- *can this be done at all* -- gates everything else.

Once the answer is "yes", the Turbo Pascal solution is ADAPTED to this project's actual state rather
than copied. Its choices answered its own constraints: a single unit, no separate compilation, no
proof obligation, and a machine where crossing a process boundary was not something you did. Keleusma
has different constraints and a verifier Turbo Pascal did not have.

**It is also not an instance of this architecture.** Turbo Pascal bounded memory by materialising no
intermediate at all; this bounds it by making each stage's working set small. Both work. They disagree
about whether an intermediate should be cheap to write or cheap to hold, and the terms
"Turbo Pascal-style streaming" and "a filter pipeline" were used interchangeably in the conversation
that produced this document when they pull in different directions.

## What is already true, measured rather than assumed

- **All twelve stages are `loop main(resume) -> Word` coroutines**, stepped one yield at a time.
- **The lexer is already a filter** in everything but plumbing: byte in, token out, one at a time.
- **The parser never moves its cursor by more than one token.** Measured over five sources including a
  whole real stage by `the_parser_never_jumps_more_than_one_token`, reading the EXECUTED cursor rather
  than counting assignments in the source. A one-token pushback slot suffices.
- **`parse.kel` already emits a record stream**, so its output is a stream in the literal sense.
- **Bounded working set is forced by the language**, not achieved by discipline.
- **The largest artifact is 39,216 bytes**, under the 65,536-byte stage window, after the all-default
  initialiser elision took the corpus body from 712,936 to 103,544. A final phase can therefore buffer
  an entire artifact, so the container's LEADING directory survives and
  [`WIRE_FORMAT_V2_WORD_ORIENTED.md`](./WIRE_FORMAT_V2_WORD_ORIENTED.md)'s choice does not need
  reopening. Its trailing-directory alternative was implemented and works, and stays available as a
  last resort.

## Unverified, and to be measured before building on

- **Whether `reconstruct` and `codegen` buffer per FUNCTION or per PROGRAM.** Their shape suggests per
  function, and "the shape suggests" was wrong four times in the session that produced this document.
  The measurement is the one already used for the parser: instrument the executed reach rather than
  read the source.
- **Which phases have whole-input dependencies.** The chunk table is the only one known. Until each
  phase is asked, the sidecar's format cannot be fixed without risking a fourth fact arriving late.

## The first increment, which needs no shell and no format

**Fuse `lexer` into `parse` so the parser pulls a token on demand**, in-process, with no intermediate
collection. Both stages are measured, the parser is known to need only a one-token window, and success
removes the largest remaining residency.

**The chunk-table pre-pass is the obstacle and its answer is the classical one**: two passes. Pass one
streams the lexer and collects only function names -- bounded, a name table and nothing else. Pass two
streams the lexer again, fused into the parser, materialising no token stream. Running the lexer twice
is how a single-pass compiler has always handled a forward reference it cannot settle on first sight.

## Sequencing, stated because agreement on the architecture is not agreement on the priority

This work does not touch the type checker's input path or the composite constant case, which are what
stand between here and Order 1. That is a deliberate operator choice, not an oversight.

## Precedent

[`../reference/RELATED_WORK.md`](../reference/RELATED_WORK.md) section 11. The phase-selection shape is
standard -- `opt` with pass selection, `clang -cc1 -emit-*`, `gcc -fdump-*`. What appears to be without
precedent is stages written in the language being compiled, each carrying a proven worst-case memory
bound.
