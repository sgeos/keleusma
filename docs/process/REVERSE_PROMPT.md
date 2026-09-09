# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-09 (session 65) — composite kinds withheld with witnesses, and the module-versus-runtime width skew audited clean on all three axes

## THE FOUR DECISIONS ARE STILL YOURS AND NONE HAS MOVED

They are the reason the large work is blocked, and nothing below decides any of them.

1. **How does a value ENTER a `Text<N>`?** It appears in every program anyone writes with the
   type. Open question 2 in [`TEXT_CAPACITY_TYPE.md`](../decisions/TEXT_CAPACITY_TYPE.md).
2. **Is the width bundle worth a breaking change?** 33 signatures, 14 public, published crate.
3. **Should `verify()` refuse float opcodes when the `floats` feature is absent?** **Evidence
   COMPLETE**: ten lines, prototyped, **zero new failures**, and the semantic worry is moot because
   the lexer refuses float literals in that build. Unlanded only because I said it was your call in
   a merged document, and a deferral is worth something only if honoured. **This is the cheap one.**
4. **Does any build configuration earn a continuous-integration job?** Cheaper than it looked on
   the WIDTH axis, unchanged on the FEATURE axis.

## THIRD INCREMENT: ONE PROPERTY, THREE AXES, AND A REFUTED HYPOTHESIS

The float result was not a finding about floats. **Three widths are carried independently — word,
float, address — and each has two possible authorities**: the module header the compiler baked every
offset from, and the runtime type parameter of `GenericVm`. The load check does not force agreement.
It refuses a module whose width is **wider** than the runtime and admits one that is **narrower**, so
a module compiled for a small target and run on a large host is supported — and is where a second
authority becomes visible. **That is how the opaque defect presented.**

**The existing skew tests cannot see it.** Every configuration in `composite_width_skew.rs` is
matched; a matched pair cannot expose a second authority because both readings coincide.

**Both remaining axes are clean**, including the address axis the original defect lived on, which had
no coverage in this form. Word: 3 of 6 catch the mutation. Address: 2 of 3. **Each mutation fails
only its own axis**, so the two tests are independent rather than one guard firing twice.

**A probable hypothesis was tested and refuted.** Two word cases survive the mutation. Boxing was the
likely explanation — a sibling test records that a boxed composite agrees on both runtimes — and it
is **excluded**: all four shapes are flat at both module widths. Constant folding is excluded too.
The cause is unestablished and the test says so, recording what was ruled out. Had I not checked, a
plausible wrong cause would have entered the tree, which is how two of the four `wire.kel` causes
were first diagnosed wrongly.

## SECOND INCREMENT: AN AUDIT'S SCOPE ARGUMENT WAS AN INSTANCE OF THE ERROR IT AUDITED

`FLAT_FIELD_WIDTH_AUDIT.md` justified auditing only the OPAQUE flat field with one sentence: every
other kind is a function of the word or the float width, **"so a site assuming a word is correct for
them."** That is **false for `Float`** — a float field is sized by the float width, selected
independently of the word, and a word assumption is correct only where the two happen to be equal.
**That is the exact coincidence that hid the opaque defect.** The scope was right; the argument for
it repeated the mistake being audited. Corrected in place.

**The excluded class was then measured, and it is clean** — six flat-composite shapes, in the
configuration capable of exposing the defect: a module declaring a **narrower** float than the
runtime provides, which the load check admits since it refuses only a wider one.

**Two earlier attempts proved nothing, and the second is worth more than the result.** Mis-sizing
the LAYOUT is an **equivalent mutation** — invisible in all six cases, because the compiler's
offsets and the runtime's strides both derive from it and move together. That coherence is precisely
what the opaque field lacked. The defect needs **two authorities**: taking the VM's float width from
the runtime type rather than the module header supplies one, and three of six cases then catch it
with a **silently wrong value** rather than a fault — the opaque defect's own signature.

The three that catch it are the three that read a field **positioned after** a float; the others
read the float itself and land in the same place either way. `tests/flat_float_field_width.rs`
asserts that property by name rather than inventorying constructs.

**"No defect found", never "no defect exists."** Six shapes are not the class.

## WHAT THIS SESSION DID: THREE VERDICTS, NO KINDS MOVED, AND THAT IS THE OUTCOME

The type channel's last extraction has four of eight kinds on the pipeline. Of the four that are
not, the branch pair was already withheld with a recorded reason. The **three composite kinds** —
field access, index access, struct literal — carried only a note saying the two representations
"disagree about what a node IS", which is a reason to look rather than a finding.

Each now has a **measured verdict and a witness test**. None moves.

| kind | verdict | why, in one line |
|---|---|---|
| 5, field on a value | **WITHHELD** | reconstruction refuses the program; there is no forest to extract from |
| 6, index on a value | **WITHHELD** | same |
| 7, struct literal | **WITHHELD** | the record does not say which struct is built, and the size it does carry is not a substitute |

**Kind 7's witness is a proof rather than a survey.** A struct of one `Word` and a struct of eight
`Byte`s are both eight bytes wide. Supplying one field to each produces a **byte-identical**
struct-literal record while the reference accepts the first and rejects the second with exactly the
field-count error. No function of that record can reproduce the verdict, so emitting a row would
either reject a correct program or lose the check. **Both directions unsound** — the branch pair's
shape.

## THE PREDICTION WAS WRITTEN FIRST AND WAS WRONG IN BOTH HALVES

| predicted | measured |
|---|---|
| kind 7 **moves**, the declared count is already on the wire | **wrong**; the count is not on the wire at all |
| kinds 5 and 6 do not move, blocked on a missing **annotation** record | verdict right, **mechanism wrong**: they are blocked on the pipeline refusing the whole program |

Recorded as missed rather than revised. The kind-7 miss came from reasoning that a sibling
extraction already returns declared field sets — true, for a struct you can NAME.

## A FINDING BESIDE THE SLICE: A PUBLIC EXTRACTION PANICS, AND IT IS BOUNDED

`expression_rows_from_pipeline` **panics** rather than returning on programs the reference merely
rejects. Measured over eight ill-typed programs: four panic, four return rows.

**Proportionality, bounded twice over.** `self_hosted_compile` compiles with the reference FIRST
and surfaces the reference's own error, so such a program never reaches the pipeline; and it wraps
the pipeline in `catch_unwind` besides. **The shipping compiler is not exposed.** Exposure is to
direct callers of the extraction API, which today are tests. Recorded on the function, not repaired
— making a public API total is a contract change and there is no caller needing it.

## WHAT WOULD UNBLOCK KIND 7, AND WHY I DID NOT START IT

A record naming the literal's struct. `parse.kel` **already resolves the identity** — the field
records preceding a literal carry INDICES, and only a resolved declaration yields an index — and
then discards it. That is a **record-stream change**, which is your call on this fork, and the same
class of decision as the `let`-binding-name record you ruled on earlier. Not begun.

## VERIFICATION, STATED AS RUN RATHER THAN AS INTENDED

- `selfhost_typecheck` **41 passed, 0 failed** with `--features self-host`.
- `comment_citations` and `claimed_counts` green.
- `cargo fmt --check`, and `clippy --tests --features signatures,shell,self-host -D warnings` clean.
- The three feature sets **without** `self-host` compile: `--features signatures`,
  `--features signatures,shell`, and `--no-default-features --features compile,verify`. Default
  features build clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` clean.
- **Both new guards were mutation-tested** and each fails on the assertion it was written to make.

**Not run**: the full workspace suite and Miri. CI is the gate for the merge.
