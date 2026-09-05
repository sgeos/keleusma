# The `InvalidBytecode` class, enumerated

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: First pass, measured against the tree. Written 2026-09-04.

## Why this census exists

`VmError::InvalidBytecode` means *this artefact should never have been produced*. It is **the class
`verify()` exists to exclude**. A module that verifies, loads, and then raises it is a hole in the
load-time guarantee rather than a bad program.

On 2026-09-04 exactly such a hole was found **by accident**, while removing an unrelated fallback:
checked indexing over a `Multiword` compiled, verified, took a memory bound, loaded, and trapped. It
was not predicted, not covered by any test, and reachable that day with nothing holding it shut.

**One hole found by accident implies nothing about how many remain.** The honest response to an
accidental find in a class nobody has enumerated is to enumerate the class.

## The question asked at each site

Not *can this fire* -- a corrupt or hand-built module can reach almost any of them, and the runtime
is right to keep every check. The question is narrower:

> **Can a module that a supported producer emitted, and that `verify()` accepted, reach this site?**

## The population, and the instrument's reach

Derived from source, not chosen by hand:

```sh
grep -rn "VmError::InvalidBytecode" src/ --include=*.rs
```

**50 matches, of which 46 are construction sites in production code.** The four excluded are named
rather than silently dropped: one doc comment referring to the variant, one match arm listing it
among unrecoverable errors, and two inside unit tests. They live in `src/vm.rs` (44) and
`src/marshall.rs` (2).

**A first draft of this table said 48, and mis-sized three groups.** The grep counts TEXT, and a doc
comment and a match arm read exactly like a construction site to it. Re-derived by classifying every
match rather than by adjusting the total, which is the same discipline this line applies to every
other figure it publishes.

**What this instrument would miss.** It matches the error CONSTRUCTED at the site. A site that
returns a pre-built error value, propagates one from a helper with `?`, or maps another error kind
into this one would not appear. One such conversion DOES exist and is included because the grep
happened to see it, which is evidence the class has members this scan cannot enumerate. The
population is a lower bound.

## Verdicts by group

Sites are grouped where they share one defect class. Probing one member of a class and saying so is
honest; probing every member individually is not a better use of the same effort.

| # | group | sites | verdict |
|---|---|---|---|
| A | flat scalar decode failure, converted from the codec error | 1 | not examined |
| B | float opcode without the `floats` feature | 2 | **REACHABLE -- see below** |
| C | `Fixed` fraction bits exceeding the word width | 5 | **defended**, by two checks that compose |
| D | composite operand form mismatch | 7 | **defended**, by boundary canonicalization |
| E | structural indices out of range | 9 | not examined |
| F | shared and private data-segment layout | 7 | host-contract; not examined |
| G | arena staleness after reset | 3 | not examined |
| H | the three "should never have been emitted" | 3 | **closed 2026-09-04** |
| I | operand-range and constant-kind checks | 6 | not examined |
| J | unregistered or invalid native index | 3 | host-contract; not examined |

The group sizes sum to 46, which is the population above; a table whose parts do not add to its
stated whole has been the tell for a miscount here before.

**Seventeen of forty-six sites carry an examined verdict.** The remaining twenty-nine are named by
group and explicitly marked as not examined. A census whose entries are unexamined opinions is worse than a
short one that says which sites were looked at.

## Group B is reachable, and it is a real deployment shape

**A module using floats verifies, loads, and traps on a runtime built without the `floats`
feature.** Measured with `--no-default-features --features verify`:

| step | result |
|---|---|
| `Module::from_bytes` | accepted |
| `verify()` | **accepted** |
| `Vm::new` | **loaded** |
| call | **`InvalidBytecode`** |

**Two independent reasons nothing catches it earlier**, and closing either would suffice.

1. **`verify()` has no `floats` gating at all** -- not one conditional in `src/verify.rs` mentions
   the feature, so the structural pass has no notion that a float opcode is inadmissible.
2. **The header width check cannot reject it.** Load admits when `got <= max_supported`, and
   `RUNTIME_FLOAT_BITS_LOG2` is **not** gated on the feature, so a build without floats still
   advertises the full width.

**Nothing here is corrupt.** The fixture is the ordinary output of the reference compiler, and
omitting floats is the point of the feature -- an embedded target is exactly where it is used.
Producing bytecode on one build and running it on another is the normal deployment shape for a
language that ships precompiled modules.

**Proportionality.** The trap is loud: a clean error at call time, not a wrong answer, a crash, or
memory unsafety. What is wrong is the LAYER. Exposure is to a host that builds without `floats` and
runs a module produced by a build that had them.

Pinned by `tests/float_opcode_without_floats.rs`. **Not repaired.**

### The recommended repair, and the evidence for it

Refuse a float opcode in `verify()` when the feature is absent, moving the refusal from run time to
load time. **Prototyped and measured**: about ten lines in the opcode scan, after which the pin fails
at its `verify()` step exactly as its message anticipates, and the float-free control still
compiles, verifies, loads and runs. The prototype was then reverted.

It was left undone for a reason that should be weighed rather than assumed away: **continuous
integration builds no configuration in which this test compiles.** Stated precisely, because the
loose version of it was wrong: CI DOES run `--no-default-features`, but bare, without `compile` or
`verify` — and the pin requires `verify` with `floats` absent, so it is configured out there. Every
other job is additive to the default features and therefore includes floats. An imprecise reason
outlives the finding it is attached to, so it is corrected rather than left standing.

### That objection is now discharged, and the decision is narrower than it was

The missing verification was supplied rather than handed over. With the repair applied, the full
`--no-default-features --features compile,verify` suite was run and compared against the same suite
unrepaired:

| | unrepaired | with the repair |
|---|---|---|
| new failures introduced | -- | **zero** |
| `tests/float_opcode_without_floats.rs` | passes, the hole open | fails at its `verify()` step, the pin firing as designed |

**And the semantic worry is moot for locally-compiled code.** The repair refuses a module CONTAINING
a float opcode, not one that executes it, so a module with unreachable float code would be refused.
That sounded like a capability loss until it was measured: **the LEXER refuses a float literal
without the feature**, so no float program can be compiled on such a build at all. The only artefacts
affected are ones compiled elsewhere and imported -- exactly the case where refusing at load is
unambiguously right.

What remains is a single semantic judgement for the operator, not an engineering risk.

### Getting that evidence required repairing the configuration itself

**`--no-default-features --features compile,verify` did not compile**, and two further tests failed
once it did. Five defects, all one class: float-dependent code with no `floats` gate.

| file | how it failed |
|---|---|
| `tests/selfhost_codegen.rs` | names `ScalarKind::Float`, a variant absent without the feature |
| `tests/selfhost_wire.rs` | its constant-kind match leaves the catch-all unreachable |
| `tests/multiword.rs` | one probe's source carries a float literal; refused at LEX |
| `tests/block_form_statements.rs` | the grammar's own example uses `Float`; refused at LEX |
| `tests/narrow_vm.rs` | a float-width helper is dead code there (warning only, left) |

**None of these was tolerated; every one was invisible.** The release gate's no-default step does not
add `compile,verify`, and continuous integration never omits floats, so **nothing anywhere built this
combination.** The configuration in which the hole lives was the configuration nothing exercised,
which is the whole reason the hole survived. It is now green at 105 binaries and 1863 tests.

Same family as the verify-without-floats build failure V0.2.2 repaired, which suggests the class
recurs and that a feature-combination sweep would be worth more than any single fix in it.

## Group C is defended, and by two checks that only work together

Worth recording because neither check alone is sufficient, and a future change to either reopens it.

- `verify()` rejects a `Fixed` fraction count at or beyond **the module's declared** word width.
- Load rejects a module whose declared word width **exceeds the runtime's** (`got <= max_supported`).

The runtime's own guard compares against the RUNTIME width. Without the second check, a module
declaring a 64-bit word with 32 fraction bits would pass verification and then trap on a 16-bit
runtime. With it, that module never loads there. **Removing or loosening the load-time width
comparison would reopen five sites at once.**

## Group H, for the record

`Op::Len` on a flat array, `Op::Len` on a flat tuple, and `Op::IsStruct` on a flat struct. All three
are addressed: the compiler has **no producer found** for `Op::Len`, and `Op::IsStruct`'s witness was
closed at two symmetry gaps. See [`OP_LEN_ROOT_REPAIR.md`](./OP_LEN_ROOT_REPAIR.md) and
`tests/opcode_reachability.rs`.

## What must not be concluded from this document

**No site here is claimed unreachable.** Group B is reachable; groups C and H are defended by
named checks; everything else is *not examined*, which is a statement about this census and not
about the code.

**Nothing should be deleted on the strength of it.** These are defences against corrupt artefacts,
and the wire format admits hand-built modules. This is a reachability record, not a deletion list.

## Group D is defended, and the defence is a third piece of code neither side reveals

The seven "operand form does not match" sites fire when the access form the compiler baked
disagrees with the representation the value has. The shape looked dangerous, and the code says so
from both ends: **the compiler bakes a FLAT access for a scalar-fielded struct**, while
`GenericValue::struct_with_widths` states plainly that a host-built composite is BOXED -- "no arena
here, so the no-arena path is boxed". The `GetField` dispatch has arms for flat-with-flat and
boxed-with-boxed and sends everything else to the refusal.

**The pairing does not occur, because a host-returned composite is canonicalized at the call
boundary** into an arena-resident flat body, after a companion pass restores a boxed enum's
discriminant and padding from the module's recorded layouts. The boxed body never reaches an access
site.

**This was verified by mutation, and the first mutation was aimed wrong.** Removing the
canonicalization on the ARGUMENT path changed nothing, because the return path is a different call
site. Removing it on the native-RESULT path produces exactly
`InvalidBytecode("GetField operand form does not match struct body")`. Had the census stopped at the
first mutation it would have recorded a mechanism that does not do the work attributed to it.

Seven shapes were driven through a native and none reached a refusal: a struct field, a struct
rebound then accessed, a tuple index, an index into an array of structs followed by a field, an enum
payload through a match, and two forms of the same struct case. **No witness found** -- not
unreachable.

The other route in is closed earlier. A composite-typed re-entrant `yield` reply is one of the two
shapes the typed pass defers on, and for a struct, tuple or array it is **refused at compile time**
by that pass rather than deferred to run time.

Both properties are pinned by `tests/native_composite_canonicalization.rs`, because the
canonicalization is load-bearing and invisible from either end: the compiler's baking and the
runtime's dispatch each look locally correct, and the code that reconciles them sits between them.
If it regresses, seven refusals open at once.

## Where the next pass should start

Group E, the nine structural-index sites, or group I, the six operand-range and constant-kind
checks. Both are places where `verify()` plausibly has a corresponding check and plausibly does not,
and neither has been looked at. Groups F and J are host-contract surfaces and are lower value: a
host that supplies a mis-sized buffer or an unregistered native has broken a stated contract, which
is the same class as the native array-length finding rather than a hole in the guarantee.
