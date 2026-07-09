# Keleusma: A Guide for New Programmers — Proposed Outline

> Draft for review. This file proposes the structure of a from-scratch
> Keleusma learning guide. It is a planning document, not guide content.
> Nothing here is final. The chapter list, the ordering, the music
> mapping, and the scope are all open to revision.

## 1. Audience and Goal

The guide teaches Keleusma to a reader who has never programmed before.
It is general documentation for a broad audience, addressed in three
concentric rings.

- Almost everyone listens to music and carries some intuition for it.
- Many aspiring programmers play an instrument.
- A smaller group composes.

The guide uses music as the on-ramp throughout the learner track. A
concept is first named in musical terms the listener already holds, then
translated into the programming idea, then made precise in Keleusma.
Readers who play or compose receive deeper payoff, but no step depends on
being able to read notation.

The end goal is a reader who understands the language well enough to
explain it on camera. Each chapter is sized to roughly one short video.
Video scripts are out of scope for this guide and will be drafted later.

## 2. Pedagogical Models

Two documents were reviewed as models. The decisions drawn from each are
recorded here so the choices are auditable.

**From "The Rust Programming Language".** The Rust book installs the
tooling, then presents one complete working program early, before the
systematic fundamentals, so the learner gains momentum and a mental
picture of a whole program. It then teaches fundamentals in order, places
the language's hardest distinctive idea in its own dedicated part, and
ends with a capstone project. This guide adopts that arc. Chapter 3 is a
complete program. The distinctive Keleusma ideas, namely the three
function categories and the yield model, get a dedicated part. The piano
roll is the capstone.

**From the Rhai Book.** Rhai is, like Keleusma, an embedded language. Its
book opens with "what Rhai is and what Rhai is not" before any code,
because an embedded language behaves unexpectedly if the reader assumes
it is general-purpose. It also separates language learning from engine
embedding into distinct parts for distinct audiences. This guide adopts
both moves. Chapter 1 sets expectations directly. Part IX is a separate
embedding track.

**Chapter format.** Every chapter is self-contained, demo-driven, and
free of forward references. Each one states a goal, develops one runnable
example, shows the output, and connects the idea to its subject. A
chapter that cannot be demonstrated by running something is a chapter
that needs rethinking.

**Two registers.** The learner track, Parts I through VIII, uses the
music on-ramp and teaches a reader new to programming. The embedding
track, Part IX, addresses a developer who already knows Rust and wants to
host Keleusma inside a Rust program. It drops the music metaphor and uses
plain technical prose. Its worked example is the piano roll, so it stays
musical in subject matter while not relying on music to explain ideas.

## 3. How Programs Run in This Guide

The learner track assumes the `keleusma` command-line tool and standalone
`.kel` script files run with `keleusma run`. This is the path that needs
no Rust host code. The eleven scripts in `examples/scripts/` are the seed
material for the feature chapters.

Actual sound is the exception. A plain script cannot emit audio. The
capstone part treats the piano roll example as a prepared playground. The
song scripts are `.kel` source embedded in `examples/piano_roll.rs`
through `include_str!`, so modifying a song means editing a `.kel` file
and rebuilding the host. Part VIII Chapter 28 covers that setup, and its
recipe must be written and tested against a real machine before release.

The embedding track in Part IX additionally assumes a Rust toolchain and
working familiarity with Rust.

## 4. Proposed Table of Contents

Forty chapters in ten parts. Chapter granularity is deliberately fine
because each chapter targets one short video.

### Part I — Setting Out

**Ch 1. What Keleusma Is, and What It Is Not.** Sets expectations. A small
language for describing things that run on a steady beat, embedded inside
a larger program. Names up front what is absent and why: no recursion, no
unbounded loops, no free-form input. Music hook: a score is not the
orchestra; it is precise instructions the orchestra follows.

**Ch 2. Installing Keleusma and the Interactive Prompt.** Tooling setup,
then first hands-on use through the REPL, closing with a saved script file
and a shebang aside for macOS and Linux. Draws on `GETTING_STARTED.md`.
Operating-system-general, with the install steps kept brief.

**Ch 3. A Complete First Program: A Note of the Major Scale.** One full
program, end to end, that computes the frequency of a note of the major
scale through the equal-temperament formula. Uses functions, the `Word`
and `Float` types, a cast, an array with indexing, and a host math
native. The Rust-book "early complete program" move, chosen so a
musician's first program produces something musically real.

### Part II — The Building Blocks

**Ch 4. Values and Types.** `Word`, `Float`, `Fixed`, `Byte`, `bool`,
`Text`, `Unit`. Draws on `01_arithmetic.kel`. Music hook: a type is an
instrument's range, the set of notes it can actually play.

**Ch 5. Names and Bindings.** `let`, and the fact that a binding does not
change. Music hook: a named motif you refer back to.

**Ch 6. Functions.** Declaring `fn`, parameters, return values. Music
hook: a function is a reusable phrase.

**Ch 7. Making Decisions.** `if`/`else`, `match` expressions, comparison
operators, the boolean words (`and`, `or`, `xor`, `not`, `andalso`,
`orelse`), and the bit-level operators (`band`, `bor`, `bxor`, `bnot`,
`lsl`, `asl`, `lsr`, `asr`).

**Ch 8. Bounded Repetition.** `for`-in over ranges and arrays, and
`break`. Draws on `04_for_in.kel`. Music hook: a repeat sign with a known
number of bars.

**Ch 9. The Pipeline Operator.** `|>` for left-to-right composition.
Draws on `05_pipeline.kel`. Music hook: a signal chain, one pedal into
the next.

### Part III — Shaping Data

**Ch 10. Structs.** Declaration, construction, field access. Draws on
`02_struct_field.kel`. Music hook: a note bundles pitch, duration, and
velocity together.

**Ch 11. Enums.** Variants, including variants that carry data. Draws on
`03_enum_match.kel`. Music hook: an articulation is one of a fixed set.

**Ch 12. Tuples and Arrays.** Fixed-arity grouping and fixed-size
sequences. Music hook: a chord as a fixed group, a bar as a fixed
sequence of slots.

**Ch 13. Pattern Matching in Depth.** Destructuring structs, enums,
tuples; wildcards; exhaustiveness.

**Ch 14. Multiheaded Functions and Guards.** Several function heads with
the same name, plus `when` guards. Draws on `06_multiheaded.kel`. Music
hook: a different response prepared for each cue.

### Part IV — The Heart of Keleusma

**Ch 15. The Three Function Categories.** `fn`, `yield`, `loop`. The
central chapter. Music hook: a finished calculation, a phrase that pauses
for a cue and then ends, and the piece itself that grooves forever.

**Ch 16. Yield: Talking to the Host.** The coroutine model, the dialogue
type, `resume`. Music hook: the metronome tick, the handover point of one
beat.

**Ch 17. The loop Function.** A program that runs forever and must
produce on every cycle. Music hook: an ostinato, and the RESET cycle as
variation form.

**Ch 18. The Data Segment.** State that survives across beats. The
`shared`, `private`, and `const data` blocks. Music hook: the key
signature, the current bar, the tempo.

### Part V — The Verifier and the Guarantees

**Ch 19. Why Was My Program Rejected?** The conservative-verification
stance. Draws on `WHY_REJECTED.md`. Music hook: a player who cannot
promise to finish the bar on time is not allowed on stage.

**Ch 20. Time and Memory Budgets.** WCET and WCMU, and totality and
productivity stated as promises. Music hook: only so many notes fit in
one beat, and a song must keep its pulse.

### Part VI — Going Deeper

**Ch 21. Generics and Traits.** Type parameters, trait bounds, method
dispatch, and const generics (`<const n: Word>`, the turbofish
`f::<7>()`). Draws on `08_method_dispatch.kel`.

**Ch 22. Newtypes and Refinement Types.** Distinct named types and `where`
predicates. Draws on `07_refinement.kel`. Music hook: a part written so a
wrong note cannot be played.

**Ch 23. Handling Partial Operations.** The partial-operation construct
family. Checked arithmetic (`ok`/`overflow`/`underflow`/`zero_divisor`
over `Word`, `Byte`, `Float`, `Fixed<N>`, with `saturate_max`/
`saturate_min`), array indexing (`invalid_index`), newtype construction
(`invalid_newtype`), discriminant-to-enum (`payload_discriminant`/
`invalid_discriminant`), and native calls (`error`), framed by the
two-backend contract. Draws on `09_big_numbers.kel`, `10_multbyte.kel`,
`BIG_NUMBERS.md`, and `RUNTIME_FAULTS.md`.

**Ch 24. Information-Flow Labels.** Confidentiality labels, `classify`,
`declassify`. An advanced chapter placed late by design. Music hook: a
part marked "do not share with the bootleg recording."

### Part VII — Shipping a Program

**Ch 25. From Source to Bytecode.** Compiling, the wire format at a high
level, the shebang line.

**Ch 26. Signed Modules and Hot Code Swap.** Ed25519 signing and code
replacement at RESET. Draws on `11_signed.kel`. Music hook: a sealed and
signed score, and swapping to a new arrangement at the next downbeat
without stopping the band.

### Part VIII — The Capstone: Making Music

**Ch 27. The Piano Roll: How It Works.** The architecture of the example.
Draws on `PIANO_ROLL.md`.

**Ch 28. Setting Up Your Own Song Playground.** A tested recipe for a
cargo project that builds the piano roll host. Notes the SDL3 dependency
as the real setup cost, heaviest on Windows.

**Ch 29. Writing and Modifying a Song.** Editing a `.kel` song and
rebuilding. The first chapter in the guide that produces sound.

**Ch 30. A Tour of the Song Roster.** The ten songs as capability
demonstrations, including the experimental pieces that a standard
digital audio workstation could not sequence. Draws on the `SONG_*_SPEC`
files in `docs/extras/`.

### Part IX — Embedding Keleusma in a Rust Program

This part addresses a Rust developer who wants to host Keleusma inside a
larger program. It assumes Rust knowledge and uses plain technical prose.
The piano roll is its integrated worked example. A reader who has met the
piano roll as the learner-track capstone now sees the host behind it.

**Ch 31. Embedding Keleusma: Orientation.** Who this part is for. The
host and script split, and what the host owns. A minimal host that
compiles a tiny module, constructs a VM, calls it, and reads a value
back. Draws on the host section of `GETTING_STARTED.md`.

**Ch 32. Constructing a VM and Running a Module.** The compile pipeline,
the `Arena`, `Vm::new`, and the module lifecycle. Draws on `EMBEDDING.md`.

**Ch 33. Registering Native Functions.** The ergonomic `register_fn` path
and the `KeleusmaType` derive first, then the closure path for natives
that capture host state. The piano roll motivates the closure path,
because its natives share an `Arc<Mutex<...>>` voice table.

**Ch 34. The Coroutine Protocol from the Host Side.** `call`, `resume`,
the `Yielded` and `Reset` states, the dialogue type, and error recovery.
The piano roll tick loop is the worked example.

**Ch 35. Sizing the Arena and Reading the Bounds.** Arena capacity, WCMU,
and the relationship between the declared bounds and the arena. Draws on
`COOKBOOK.md`.

**Ch 36. Loading Precompiled and Signed Bytecode.** Loading from bytes,
the trust matrix for signed modules, and the `unchecked` constructors and
what they cost.

**Ch 37. Hot Code Swap from the Host.** `replace_module`, data-segment
migration, and Replace semantics. The piano roll song swap is the worked
example.

**Ch 38. Calibrated WCET and Cost Models.** The `CostModel`, the
`keleusma-bench` calibration crate, and how a host obtains measured cycle
tables for its hardware.

**Ch 39. A Full Host, End to End.** A walkthrough of the complete
`examples/piano_roll.rs`, partitioned so the reader sees which code
embeds Keleusma and which code is ordinary audio synthesis. Folds in the
remaining embedding patterns from `COOKBOOK.md`.

### Part X — Where to Go Next

**Ch 40. Further Reading.** Pointers to the roguelike example, the
specification documents, and the architecture documents. Draws on
`ROGUE.md`.

## 5. Mapping to Existing Repository Material

| Existing file | Used by |
|---|---|
| `docs/guide/GETTING_STARTED.md` | Ch 2, Ch 31 |
| `examples/scripts/01`–`11` | Ch 3, 4, 8, 9, 10, 11, 14, 21, 22, 23, 26 |
| `docs/guide/WHY_REJECTED.md` | Ch 19 |
| `docs/guide/BIG_NUMBERS.md` | Ch 23 |
| `docs/guide/PIANO_ROLL.md` | Ch 27–29 |
| `docs/extras/SONG_*_SPEC.md` | Ch 30 |
| `docs/guide/EMBEDDING.md` | Ch 32–37 |
| `docs/guide/COOKBOOK.md` | Ch 35, Ch 39 |
| `examples/piano_roll.rs` | Ch 33, 34, 37, 39 |
| `docs/guide/FAQ.md` | Woven throughout as sidebars |
| `docs/guide/ROGUE.md` | Ch 40 |

**Reconcile note.** The nine reference pages above
(`GETTING_STARTED.md`, `EMBEDDING.md`, `COOKBOOK.md`, `PIANO_ROLL.md`,
`ROGUE.md`, `WHY_REJECTED.md`, `FAQ.md`, `BIG_NUMBERS.md`, and
`LLM_USAGE.md`) have been copied into the guide directory alongside the
chapters, so the linear course is a strict superset of the prior
in-repository material. The course chapters cover the same ground at a
learner-level depth and forward to the reference pages for completeness.
See [README.md](./README.md) for the combined index.

## 6. Deferred or Out of Scope

- Video scripts. To be drafted after the guide content is approved.
- The roguelike as a worked tutorial. Referenced in Ch 40, not taught, so
  the guide's two worked examples, the script-side songs and the
  host-side piano roll, stay coherent.
- The exhaustive host API surface. The embedding part teaches the common
  path. Full API reference remains the job of the rustdoc and the
  embedding reference documents.

## 7. Proposed Music-to-Keleusma Concept Map

A starting point for the music framing in the learner track. A reviewer
with a music background should correct or extend this table.

| Music idea | Keleusma idea | Chapter |
|---|---|---|
| A short phrase you reuse | A function | 6 |
| Working out a chord's notes, a calculation that finishes | An atomic `fn` | 15 |
| A phrase that pauses for a cue, resumes, and eventually ends | A `yield` function | 15, 16 |
| The piece itself, a groove that repeats and sounds a beat every cycle | The `loop` function | 15, 17 |
| The metronome tick, the moment a beat is handed over | A `yield` | 16 |
| Key signature, current bar, tempo, things carried across beats | The data segment | 18 |
| A song must keep its pulse and cannot freeze | The productivity guarantee | 20 |
| Only so many notes fit in one beat | The WCET budget | 20 |
| An instrument's playable range | A type | 4 |
| A player who cannot promise to finish the bar on time is cut | The verifier rejecting unbounded programs | 19 |
| A sealed, signed score from the composer | A signed bytecode module | 26 |
| Swapping arrangements at the next downbeat without stopping | Hot code swap at RESET | 26 |
| Variation form, the subject re-entered with changes | The RESET cycle | 17 |

## 8. Open Questions for Review

1. **The first complete program.** Chapter 3 computes major-scale
   frequencies. Is that a satisfying first program, or is there a better
   musical starting point?
2. **The concept map in Section 7.** A reviewer with a music background
   should check it.
3. **Information-flow labels (Ch 24).** Advanced for a first language.
   Keep it as a late optional chapter, or move it to further reading?
