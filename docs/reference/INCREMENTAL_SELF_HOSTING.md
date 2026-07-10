# Incremental Self-Hosting by Backward Migration

A method for turning a compiler written in a host language into one that compiles
itself, one stage at a time, without a big-bang rewrite and without giving up a working
reference at any point. It applies equally to rewriting any staged pipeline in a new
implementation language.

The method is a backward variant of the moving-seam incremental rewrite, sometimes
called the strangler pattern, specialized for compilers by porting the emit boundary
first, because that is where self-hosting efforts most often stall.

This document states the method on its own terms. A worked example follows at the end,
and the reader is assumed to have their own compiler in mind rather than any particular
one.

## When this applies

Check four preconditions before adopting it.

1. **A working reference implementation exists.** The existing compiler keeps running
   throughout and serves as the equivalence oracle that every intermediate state is
   checked against. Without a reference there is nothing to validate against, and the
   method does not apply.
2. **The pipeline has clean stage boundaries.** The compiler is a sequence of stages,
   for example tokenize, parse, analyze, generate, and emit, each consuming the previous
   stage's output. Those boundaries are where the seam moves.
3. **The boundaries can be bridged.** You can convert one stage's output in the host
   language into the input the next stage expects in the target language. This is what
   the adapters do.
4. **The backend is the real risk.** Producing a valid target artifact at the emit
   boundary is the hardest and least certain part. This is what justifies going
   backward. If instead your frontend is the risk, reverse the direction.

## The method

### Go backward, last stage first

Port the stages in reverse pipeline order. The last stage, the one that emits the target
artifact, is ported first. The first stage, usually the lexer, is ported last.

The reason is risk, not convenience. Frontends are well understood and low risk. The
wall is the backend. Self-hosting efforts commonly reach a state where the new compiler
can read its own source but cannot yet emit a valid artifact, and they stall there.
Porting the emit boundary first retires that risk before you invest in the easy stages.

### Move a single adapter seam

At any moment exactly one adapter sits at the frontier between the still-host-language
upstream and the already-target-language downstream. The upstream stages run unchanged
and produce their output as before. The adapter converts that output into the input the
first target-language stage expects. The downstream stages, already ported, chain
directly to one another.

Each time you port the next stage backward, it takes over the adapter's job and the
adapter disappears, and a new adapter appears one position further upstream. The seam
moves through the pipeline from back to front, and there is never more than one adapter
to maintain.

### Treat adapters as throwaway prototypes

Separate an adapter's output from its implementation. The output is the data contract
the two adjacent stages agree on, and it is permanent. The implementation, the
conversion from the host representation to the target one, is disposable. It is a
prototype of the output behavior that the not-yet-written upstream stage will eventually
produce for real, and writing that upstream stage is precisely what retires the adapter.

This framing sets the right quality bar. An adapter needs to be good enough to let you
build and validate the stage below it, not perfect, not general, and not efficient. When
an adapter is getting expensive to fix, that is often the signal to stop and write the
real upstream stage that will replace it. The goal is stages, not adapters.

### Validate against the reference at every step

At each intermediate state the pipeline is part target-language and part host-language,
and you check its output against the reference implementation on a test corpus.

Choose the strongest equivalence check that does not shackle you to an implementation
detail. Comparing the two compilers' final serialized bytes is the strongest, but it
forces the new compiler to reproduce the old one's serialization exactly, which is often
not worth it and can block progress. Comparing the logical artifact just below
serialization, the same instructions and tables and layout, is nearly as strong and far
more practical. Comparing observable runtime behavior on the corpus is the weakest and
is only as strong as the corpus is broad. Prefer logical-artifact equivalence, with
behavioral equivalence as a backstop.

### Keep a deferral ledger

Correctness during migration is a judgment call, and the ledger is what keeps it honest.
For each corpus program, either the current chain processes it correctly, or its
deviation is recorded in a ledger entry that names the specific upstream stage that will
correct it.

When that upstream stage lands, re-run the deferred cases and confirm each is resolved. A
deviation that its responsible stage does not fix was never an adapter limitation. It is
a real bug in the stage you already wrote, and the ledger is what surfaces it. Without
the ledger and the recheck, the judgment call becomes a place for stage bugs to hide
until the end.

### Switch modes when the stages are complete

Recognize two engineering modes. During migration you defer. You prefer building the next
stage to perfecting a throwaway adapter, and you record every deferral in the ledger.
Once all stages are ported and the adapters are gone, you switch from deferred work to
bug fixing. You drive the ledger to empty, and the fully self-hosted pipeline must
process the corpus correctly with no adapter left to defer to.

An empty ledger and a passing corpus under the fully self-hosted pipeline is the
completion gate. Every intermediate deferral is a debt against it.

### Decompose a large stage inside-out

A stage is rarely one increment, and the generate-and-emit stage in particular tends to
be the largest. Migrate it by the same backward discipline one level down. Have its input
adapter deliver fully-processed input at first, so the first increment is only the emit
boundary, then push the earlier work into the stage increment by increment, with the
adapter delivering progressively-less-processed input each time. The compiler that hosts
the reference was not written in one pass, and the port will not be either.

## A worked example, Keleusma

Keleusma is a total functional language whose compiler is a three-stage streaming
pipeline, lexer then parser then codegen, where the codegen stage fuses analysis,
specialization, code generation, and emit. It is being self-hosted by this method.
Codegen is ported first, behind an adapter that feeds it declarations from the
host-language front, then the parser, then the lexer. The emit boundary is a
host-provided primitive, so the new compiler produces a logical module that the host
serializes, which sidesteps reproducing the reference's byte format and lets the gate be
logical-artifact equivalence rather than byte equivalence. The applied plan, with the
stage table and the language-specific prerequisites, is in the project's compiler
milestones document.

## Prior art and lineage

The moving-seam incremental rewrite is the strangler pattern. Self-hosting and
bootstrapping are an old tradition in language implementation. The specific contribution
here is small. Run the seam backward so the emit boundary is retired first, treat the
adapters explicitly as throwaway prototypes of the stages that supersede them, and hold
correctness with a deferral ledger that is reconciled at every step and driven to empty
at completion.
