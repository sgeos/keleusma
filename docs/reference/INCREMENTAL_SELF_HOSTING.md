# Incremental Self-Hosting by Backward Migration

A method for turning a compiler written in a host language into one that compiles
itself, one stage at a time, without a big-bang rewrite and without giving up a working
reference at any point. It applies equally to rewriting any staged pipeline in a new
implementation language.

The method is a backward variant of the moving-seam incremental rewrite, sometimes
called the [strangler pattern][ref_strangler_fig], specialized for compilers by porting
the emit boundary first, because that is where self-hosting efforts most often stall.

This document states the method on its own terms, so that you can lift it out and apply
it to your own compiler. A worked example in one language follows at the end, and the
reader is assumed to have their own compiler in mind rather than any particular one.

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

The seam is easiest to see as a picture. The pipeline runs left to right, H marks a
stage still in the host language, T marks a ported target-language stage, and the bar is
the single adapter at the frontier.

```
start    H  H  H  H  H      all host, no adapter yet
step 1   H  H  H  H | T     emit stage ported, adapter in front of it
step 2   H  H  H | T  T
step 3   H  H | T  T  T
step 4   H | T  T  T  T
done     T  T  T  T  T      fully self-hosted, adapter gone
```

The two endpoints correspond to the two whole-compiler [tombstone diagrams][ref_tombstone_diagram]
of a bootstrap, the compiler written in the host language at the start and the compiler
written in the target language at completion, and the seam is the path between them.

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
and you check its output against the reference implementation on a test corpus. Assemble
the corpus from programs that exercise the language broadly, grow it whenever a deviation
escapes it, and include the compiler's own source, which is the largest and most
demanding program it will process.

Choose the strongest equivalence check that does not shackle you to an implementation
detail. Comparing the two compilers' final serialized bytes is the strongest, but it
forces the new compiler to reproduce the old one's serialization exactly, which is often
not worth it and can block progress. Comparing the logical artifact just below
serialization, the same instructions and tables and layout, is nearly as strong and far
more practical. Comparing observable runtime behavior on the corpus is the weakest and
is only as strong as the corpus is broad. Prefer logical-artifact equivalence, with
behavioral equivalence as a backstop.

Pin the reference for the duration of a migration. The reference is the oracle, and if it
changes underneath you the intermediate checks stop being comparable. Advancing the
reference to a newer version is a separate and deliberate step taken between migrations,
not during one.

### Keep a deferral ledger

Correctness during migration is a judgment call, and the ledger is what keeps it honest.
For each corpus program, either the current chain processes it correctly, or its
deviation is recorded in a ledger entry that names the specific upstream stage that will
correct it. In practice an entry records three things, the corpus program, the observed
deviation from the reference, and the responsible upstream stage.

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

One further gate belongs at the end, and it is specific to self-hosting rather than to
migration in general. Compile the compiler's own source with the reference and with the
self-hosted compiler and confirm the two artifacts agree, then compile the compiler with
itself and confirm the output is stable across successive generations. This is the
staged-bootstrap fixed-point check, the same reproducibility comparison a
[multi-stage bootstrap][ref_gcc_bootstrap] makes between its later stages. The compiler's
own source is the most demanding program it will process, and the fixed point is where a
subtle miscompilation that the corpus missed will surface.

### Decompose a large stage inside-out

A stage is rarely one increment, and the generate-and-emit stage in particular tends to
be the largest. A fused stage of this kind has no clean internal boundaries and so
appears to violate the second precondition, and the resolution is to impose the
boundaries as you go rather than to assume them. Migrate it by the same backward
discipline one level down. Have its input
adapter deliver fully-processed input at first, so the first increment is only the emit
boundary, then push the earlier work into the stage increment by increment, with the
adapter delivering progressively-less-processed input each time. The compiler that hosts
the reference was not written in one pass, and the port will not be either.

## Applying it to your compiler

The method reduces to a short procedure. Substitute your own stage names for the ones
below.

1. Confirm the four preconditions. If the frontend rather than the backend is your risk,
   run the seam forward instead of backward.
2. Pin the reference, and assemble a corpus that includes the compiler's own source.
3. Choose the equivalence check, preferring the logical artifact just below
   serialization over the serialized bytes.
4. Port the last stage, the one that emits, behind an adapter that feeds it the input the
   removed downstream stages would have produced. Validate against the reference.
5. Move the seam one stage upstream. The stage you just ported takes over the adapter's
   job, and a new adapter appears in front of the next unported stage. Validate again.
6. Repeat until the first stage is ported and no adapter remains, recording every
   deviation in the ledger and reconciling it at each step.
7. Drive the ledger to empty, confirm the fully self-hosted pipeline passes the corpus,
   and confirm the self-application fixed point is stable across generations.

If a single stage is too large to port in one increment, most often the emit stage,
apply the same procedure one level down inside it.

## A worked example, Keleusma

Note. This section is written prospectively and is to be revised to report the outcome,
the ledger driven to empty and the fixed point stable across generations, once the
self-host completes.

Keleusma is a total functional language whose compiler is a three-stage streaming
pipeline, lexer then parser then codegen, where the codegen stage fuses analysis,
specialization, code generation, and emit. It is being self-hosted by this method.
Codegen is ported first, behind an adapter that feeds it declarations from the
host-language front, then the parser, then the lexer. The emit boundary is a
host-provided primitive, so the new compiler produces a logical module that the host
serializes, which sidesteps reproducing the reference's byte format and lets the gate be
logical-artifact equivalence rather than byte equivalence. Because both the reference and
the new compiler hand the same host serializer the same logical module, equal logical
artifacts imply equal bytes here, so byte equivalence comes for free rather than being a
burden to avoid. The applied plan, with the stage table and the language-specific
prerequisites, is tracked in the project's self-hosting milestones.

One constraint is specific to Keleusma and does not generalize to the method. Because
Keleusma is total and resource-bounded, each ported stage must also clear Keleusma's own
verifier, being guaranteed termination and the worst-case execution-time and worst-case
memory-use bounds. A ported stage therefore has a second acceptance test beyond agreement
with the reference, and the accumulating symbol and type environment, which grows with the
number of top-level declarations, must fit within the stage's declared bounds.

## Prior art and lineage

The moving-seam incremental rewrite is Fowler's [strangler pattern][ref_strangler_fig].
[Self-hosting][ref_self_hosting] and [bootstrapping][ref_bootstrap] are an old tradition
in language implementation, and the reproducibility comparison used as the completion gate
is the one a [multi-stage bootstrap][ref_gcc_bootstrap] performs between its stages. A
separate line of work pursues bootstrapping with machine-checked correctness rather than
migration mechanics, the [CakeML][ref_cakeml] verified compiler being the sharpest
instance, and behind all of it sits Thompson's [trusting-trust][ref_trusting_trust]
argument for why the provenance of the seed matters. This method is deliberately about the
mechanics of migration, not about proving correctness.

The specific contribution here is small. Run the seam backward so the emit boundary is
retired first, treat the adapters explicitly as throwaway prototypes of the stages that
supersede them, hold correctness with a deferral ledger that is reconciled at every step
and driven to empty at completion, and close with the self-application fixed point.

## References

### Paper

- [Reflections on Trusting Trust][ref_trusting_trust], Ken Thompson, Communications of the ACM, 1984

[ref_trusting_trust]: https://dl.acm.org/doi/10.1145/358198.358210

### Reference

- [Strangler Fig Application][ref_strangler_fig], Martin Fowler
- [Self-hosting compilers][ref_self_hosting]
- [Bootstrapping compilers][ref_bootstrap]
- [Tombstone and T-diagrams][ref_tombstone_diagram]
- [GCC multi-stage bootstrap and stage comparison][ref_gcc_bootstrap]
- [Reproducible builds][ref_reproducible_builds]
- [CakeML verified bootstrapped compiler][ref_cakeml]

[ref_strangler_fig]: https://martinfowler.com/bliki/StranglerFigApplication.html
[ref_self_hosting]: https://en.wikipedia.org/wiki/Self-hosting_(compilers)
[ref_bootstrap]: https://en.wikipedia.org/wiki/Bootstrapping_(compilers)
[ref_tombstone_diagram]: https://en.wikipedia.org/wiki/Tombstone_diagram
[ref_gcc_bootstrap]: https://gcc.gnu.org/install/build.html
[ref_reproducible_builds]: https://reproducible-builds.org/
[ref_cakeml]: https://cakeml.org/

### Related

- [Keleusma, the reference implementation and the subject of the worked example][ref_keleusma]

[ref_keleusma]: https://github.com/sgeos/keleusma
