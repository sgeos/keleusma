# Book Example Verification

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief, session 67. Scope is the verification of Keleusma code printed in
`book/src`. No language, instruction-set, or wire-format change is contemplated, and
nothing here needs operator authorisation.

---

## The claim this work is about

`book.yml` calls `book/ci/verify_examples.py` "the freshness guarantee that motivated
keeping the book in the main repo: every example with a stated output is executed and
compared on each CI run, so the book cannot silently drift from the implementation."

The qualifier "with a stated output" is doing far more work than a reader would expect,
and the phrase "on each CI run" is false on the branch where the work happens.

## What is measured, on the tree of this brief

Derived by walking `book/src/*.md` with the verifier's own block rule.

| quantity | value |
|---|---|
| bare fenced blocks, which the verifier treats as Keleusma | 175 |
| assertions the verifier makes | 51 |
| blocks containing `fn main` | 50, of which 26 carry a matched output claim |
| blocks containing `loop main` and no `fn main` | 8, none reachable by the verifier |
| complete programs, therefore | 58, of which 26 are verified |
| output claims on blocks the verifier skips for lacking `fn main` | 9 |

The 51 assertions **pass** against the current CLI. This work is not repairing a red
signal. It is enlarging a signal that covers rather less than its description.

### Two structural holes, independent of coverage

**The verifier has not run in two months and has never run on the version branch.**
`book.yml` triggers on `push` and `pull_request` to `main` only. Its last run was
2026-07-24. `book/src` has been modified on `v0.2.3` repeatedly since, including two
corrections of a wrongly stated mechanism in session 66. The guarantee is real and it is
attached to the wrong branch.

**The verifier cannot report a collapse.** `checked` is printed and never compared against
a floor. A reworded chapter, a changed fence convention, or a bad `SRC` argument yields
`checked 0 claimed examples; 0 failure(s)` and exit 0. This is the first row of the
companion table in `CLAUDE.md`: a whole family producing zero cases behind a healthy
aggregate.

## The confirmed defect

`book/src/WHY_REJECTED.md` quotes, as the diagnostic for a first-class function
reference, text that does exist in `src/compiler.rs`. On the trigger the chapter prints,
the compiler never reaches it; the type checker refuses earlier with
`type error: undefined identifier`. Verified by supplying the definition the example
omits, which does not change the outcome.

The chapter's stated purpose is to let a reader match an error they have seen to an
explanation. A quoted diagnostic that the documented trigger cannot produce fails at
exactly that purpose. Whether the compiler site is reachable by some other trigger is a
separate question and is **not** claimed here either way.

## What to build

Four properties, in no required order.

1. **The verification runs where the work happens.** Cheapest correct form is to invoke it
   from the `docs-links` job of `ci.yml`, which already builds the CLI to run a Keleusma
   script. That job runs on every branch. `book.yml` keeps its release-binary run for
   `main`.
2. **A collapse fails.** Floors on the number of blocks discovered and the number of
   assertions made, phrased so that a rewording which loses the extraction is a failure
   rather than a silence.
3. **Every complete program has a determined, executed status.** An expectation written
   beside the block as an HTML comment, `<!-- verify: ... -->`, invisible in the rendered
   book and not extracted into `book/po`. Statuses are `accept` (runs, exit zero),
   `compile` (compiles, cannot run standalone), `reject` with an optional diagnostic
   substring, and `skip` with a mandatory reason.
4. **The gap cannot silently regrow.** A Rust test in the main suite deriving the census
   and requiring that every complete program is either output-claimed or marked. It runs on
   every branch in five feature sets with no Python and no built binary.

## Why an HTML comment rather than a manifest or a fence info string

A separate manifest keyed by file and line drifts on every edit and its drift is silent in
the direction that matters. A fence info string would change the block's rendered
highlighting and would break the verifier's own rule that a bare fence is Keleusma. An
HTML comment sits with the code it describes, cannot drift from it, renders as nothing, and
has precedent in `book/src/INSTRUCTION_SET.md`. `book/po/ja.po` contains no HTML comment,
so the marker is not translated material.

## Prior failures this work must not repeat

**The instrument manufacturing a contradiction.** `23_big_numbers.md` presents a block
introduced by "Change `main` to ..." which calls a function defined earlier in the chapter.
Run standalone it fails with `undefined function add_checked`. That is **not** a book
defect; it is a fragment the harness fed to a compiler as though it were a program. Session
66 paid for this class twice, once with a language server driven down a closed pipe. Every
refusal must be read for its reason before it is called a finding.

**A crude classifier used as an oracle.** The census inferred "rejection context" from
words like *error* and *refuse* in surrounding prose. It flagged the **Rewrite** program of
a rejected-then-corrected pair, which is supposed to compile. That heuristic is adequate for
counting and unacceptable for a pass or fail, which is why the expectation must be written
down rather than inferred.

**A negative with no control.** The rejection chapters exist to show programs the
implementation refuses. A harness that refuses everything, through a wrong flag or a
missing binary, satisfies every such assertion. The book already contains the control:
these chapters pair a rejected program with its rewrite. Requiring the rewrite to be
**accepted** in the same run is what makes the refusals mean something.

**A wrong-reason refusal read as a confirmation.** `WHY_REJECTED.md` shows a program that
fails on an unregistered native rather than on the property the chapter is about. A refusal
that never reaches the documented check is indistinguishable from a passing assertion, which
is the second row of the companion table in `CLAUDE.md`. This is why `reject` takes an
optional diagnostic substring, and why the substring is worth supplying wherever the
chapter quotes one.

**A count claim already at its tolerance.** `CLAUDE.md` states 112 integration test files;
the tree has 121, and `the_instructions_do_not_misstate_the_integration_test_counts` allows
10. This work adds a test file, so the stated figures must be re-derived rather than
nudged, in both places the document states them.

**A guard run before the last edit.** Several of session 65's lies came from this. Any
figure quoted in a commit message or a channel must come from a run that started after the
final edit to its subject.

## What is deliberately not in scope

Fragments. 101 bare blocks contain no `main` and cannot be compiled standalone without a
wrapper that would invent a context the book does not state. Counting them is useful and
guessing their intent is not. The census records the number; the assertions do not touch
them.

The Japanese translation. `book/po/ja.po` translates prose. A translated output claim would
not match the extraction, which means the translation is unasserted. That is a real gap and
a separate decision, since it turns on whether claims should be marked untranslatable.

---

## Findings

Recorded on 2026-09-26. Each was produced by executing the book's own code, and each
is separated from the harness limitations beneath it.

### Defects in the guide, repaired

**The productivity rule's FAQ entry described a rule the implementation no longer has.**
`FAQ.md` stated that the structural check is purely lexical, that it does not chase calls,
and that a `loop` whose only `yield` sits in a called function is rejected with
`loop body has no yield on at least one path`. The program it printed to demonstrate that
**compiles**. `compute_always_yielding` in `src/verify.rs` is a fixpoint over the call
graph, and the structural pass's own diagnostic says "directly or by delegating to an
always-yielding function". The entry was wrong in the costly direction: it advised
restructuring code that needs none. Corrected in place, with the correction dated and the
superseded claim quoted, since a reader who acted on the old text deserves to see that it
moved.

**Two chapters pinned an exact byte count of compiled output.** `17_loop_function.md`
claimed 2372 bytes and the tool writes 3676; `25_from_source_to_bytecode.md` claimed 2400
against 3684. Both were stale, presumably since the wire format v2 replacement. The
numbers are removed rather than updated: the size of a compiled artefact is not a stable
property of a program, so a figure written into the guide is a promise that the format will
not change.

**A quoted diagnostic was unreachable, and the implementation was fixed rather than the
book.** `WHY_REJECTED.md` quotes the first-class-function-reference diagnostic. That text
exists at `src/compiler.rs:8961` and the compiler never ran, because the type checker
refused first with `undefined identifier`, whose site carried the comment
"Bare function name reference. Report unknown." The name **is** defined, so that message
sent a reader looking for a missing definition. The type checker now emits the accurate
wording when the identifier resolves to a known function, and reports an undefined
identifier otherwise; both directions are exercised. This makes the guide's quotation true
by improving the diagnostic rather than by documenting a poor one.

### An observation left for the operator, not acted on

**`run --print-memory` reports a bound for a program `run` refuses.** On the recursive
program of `19_why_rejected.md`, plain `run` fails structural and resource verification
while `--print-memory` exits zero and prints a transient figure equal to the whole default
arena. Two readings are available and the evidence does not separate them. It may be
deliberate: verification compares the arena capacity against the worst-case bound, so a
host must be able to learn the size **before** a capacity is fixed, and running the check
first would refuse a large program before it could be provisioned. Or the saturating figure
may be a fallback standing in for a bound that was never computed. The first reading does
not cover this case, since the refusal here is recursion in the topological sort rather
than capacity. **Not changed**, because which reading is intended decides the fix.

### Harness limitations, which are NOT findings about the book

Recorded because each one arrives looking exactly like a defect.

- **"Change the program to ..." blocks print only a replacement entry point.** Run
  standalone they fail on definitions they no longer have. Two such blocks, in
  `22_newtypes_and_refinement.md` and `23_big_numbers.md`, are assembled by the `prelude`
  form the way the prose tells the reader to assemble them. The first then reproduces its
  chapter's quoted refinement diagnostic exactly, which is a verification the book has
  never had.
- **Programs that need a host.** `EMBEDDING.md` and one `WHY_REJECTED.md` example call
  natives that only an embedding registers, and `26_signed_modules_and_hot_swap.md` declares
  a module that will not load without a key. All three compile and verify; none can be run
  by the command-line tool. They are checked to the point the tool can reach.
- **Stream programs do not terminate.** Eight `loop main` programs cannot be run to
  completion and are checked with `run --print-memory`, which compiles, verifies, and exits.
- **A prose-based classifier is not an oracle.** A first census inferred "this is meant to
  be rejected" from words like *error* and *refuse* nearby. It flagged the **Rewrite** half
  of a rejected-then-corrected pair, which is supposed to compile. Every expectation is
  written down for this reason.
