# BRIEF — extract the argument marshalling that three gaps now share

**Filed 2026-09-23, against `6a74ca62`, backlog 0, suite 667.**

> ⚠ **STATUS.** Filed before its own work lands. The gap it states is open at filing
> by construction.

## Why now, when the same extraction was declined before

On 2026-09-23 I declined to extract `corpus_differential.rs`'s stub machinery,
writing that it was *"a tuned instrument, late in a session, for confirmatory
measurements."* **That judgement was correct on the evidence then**: it served one
beneficiary, the arena-touch census, whose remaining modules would have confirmed a
figure the static census already established.

**Three separate gaps now need the same capability**, each measured rather than
assumed:

| gap | blocked on |
|---|---|
| 11 streaming corpus modules | host natives registered (`host::song_name`, `host::run_player_turn`) |
| 3 streaming corpus modules | a `Composite` entry argument, built in the arena and passed as a handle |
| the lowered `Text` return | comparing a `Value::StaticStr` against an `i64` handle |

One missing capability, three beneficiaries. **That changes the arithmetic, and it
is a re-evaluation on new evidence rather than a reversal.**

## The pattern is one already applied successfully this session

`general_native_arena_extent` took a source string; the streaming census needed a
module. It was **refactored, not duplicated**: the body moved to
`native_arena_extent_of_module` and the original became a thin wrapper, with
`arena_high_water.rs` — the existing consumer — as the check that nothing drifted.
That worked, and it is the shape to repeat.

**Duplication is the wrong answer here**, and this line has said so: a second
implementation of a tuned instrument is a second thing to drift.

## ⚠ THE PRE-COMMITMENT THAT BOUNDS THIS

**If the extraction reaches beyond the stub registration itself — into the
differential's logging, its reference-argument masking, or its seed machinery —
STOP AND REPORT THE SHAPE.** Those are load-bearing and tuned: the masking exists
because without it *"the WHOLE MODULE was discarded"* over one field, and the seed
count was raised to 64 only after 4 and 24 were shown insufficient.

A refactor that spreads is how a tuned instrument gets quietly retuned.

## Wrong turns, named

1. **`corpus_differential.rs`'s own tests are the check.** It must pass unchanged
   after the move, exactly as `arena_high_water.rs` did. If it needs edits beyond
   calling the moved function, the move is wrong.
2. **Do not change what a stub returns.** The stubs log and return shaped values the
   differential compares; altering either changes what that instrument measures.
3. **`common/mod.rs` is already in the host-buffer census.** Adding to it needs no
   new census entry, but a new test FILE would — and that list is compared as an
   ORDERED sequence.
4. **Do not create probe files in `tests/` while a gate runs.** Eleventh catalogue
   row, mine from this session.
5. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment

- **Land the extraction alone, verified, before using it for anything.** Attaching
  a new consumer in the same increment makes a failure's attribution an argument.
- **If it lands, the first consumer is the three composite-argument modules**, not
  the eleven native ones — smaller, and it exercises the harder half of the
  capability.
- **If the extraction proves to need the differential's tuned parts, that is the
  result**, and it converts three "blocked on machinery" notes into one
  "blocked on a design question" note, which is more useful than a vague deferral.

---

# VERDICT — the marshalling is NOT separable, and the three gaps are reframed

**The pre-commitment's stop condition fired on reading, before any code moved.**

`run_vm` spans 258 lines and is entangled with thread-local state — `take_log`,
`SAW_REF_ARG`, `REF_POSITIONS` — the `NativeEntry` table, and `ExemptClass`.

**The decisive fact is what the stubs DO.** They log every native call, and `Run`
carries that log as a first-class field beside `results` and `shared`: **the native
call SEQUENCE is part of what the differential compares.** A stub extracted without
its logging would not serve that instrument; a stub extracted with it drags the
masking machinery along, and that masking exists because without it *"the WHOLE
MODULE was discarded"* over a single differing field.

So there is no "stub registration" to lift out. There is a comparison whose stubs
are one of its organs.

## What this changes about the three gaps

They were recorded as **blocked on machinery someone could extract**. They are
better described as **blocked on a design question**:

> Any harness that drives these modules needs its own semantics for what a native
> call MEANS in a comparison — what is logged, what is masked, what a non-scalar
> argument is allowed to look like. `corpus_differential` answers that for its own
> purpose. A second harness with a different purpose needs its own answer, not a
> borrowed one.

**That is a more useful note than "needs the machinery"**, because it says what a
future increment must decide rather than what it must copy. And it explains why the
earlier decline was right for a reason better than the one given at the time: not
merely that the instrument was tuned and the session late, but that **there was
never a separable part to take.**

## No code was changed

The brief above committed to stopping and reporting rather than letting a refactor
spread. It spread on the first read, so nothing moved.

---

# ⚠ CORRECTION TO THE VERDICT ABOVE, 2026-09-24 — IT WAS TOO STRONG

The verdict concluded there is *"no separable stub registration to lift out"* and
reframed all three gaps as blocked on a design question. **That is right for a
DIFFERENTIAL and wrong as stated**, because it applied a differential's requirement
to a consumer that does not have it.

**The arena-touch census does not compare native call sequences.** It measures how
many arena bytes a module touches. It needs natives only so the module RUNS. The
logging and masking that make `corpus_differential`'s stubs inseparable serve a
comparison the census does not perform.

And the reference side is weaker still than assumed: `native_calls.rs` records that
the `piano_roll` family's natives *"carry zero return shape"* with 1643 `PopN`
against 999 call sites, so **their results are overwhelmingly discarded**. A stub
that merely succeeds is sufficient for running them.

## What the census would actually need

- **Reference side**: a trivial stub per declared native. Small.
- **Native side**: the JIT resolves a declared native by SYMBOL. `native_calls.rs`
  binds hand-written `extern "C" kel_native_host__one/two/three` and relies on the
  linker retaining them; a corpus module declaring `host::song_name` needs
  `kel_native_host__song_name`, which does not exist in the process — and its own
  comment warns that the engine then *"resolves the declaration to nothing and jumps
  to it, which is a segfault, not a failed assertion."*
  The mechanism that closes this is `add_global_mapping`, binding each declared
  native to an arity-matched stub at run time.

## Why this correction matters more than the increment it unblocks

The unblocked work is **confirmatory**: eleven piano-roll modules whose static share
`region_composition.rs` already establishes. **The wrong claim is the liability.**
It was published in a commit message and in the handoff's pickup list, where a later
reader would take "blocked on a design question" as settled and not look again.

**The error has a name this session has used repeatedly**: a requirement belonging to
one instrument asserted as a property of the capability. The differential needs
logging; the census needs a function that returns. Reading `run_vm` answered the
first question and I reported it as the answer to both.
