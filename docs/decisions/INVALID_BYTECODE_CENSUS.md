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
| D | composite operand form mismatch | 7 | not examined |
| E | structural indices out of range | 9 | not examined |
| F | shared and private data-segment layout | 7 | host-contract; not examined |
| G | arena staleness after reset | 3 | not examined |
| H | the three "should never have been emitted" | 3 | **closed 2026-09-04** |
| I | operand-range and constant-kind checks | 6 | not examined |
| J | unregistered or invalid native index | 3 | host-contract; not examined |

The group sizes sum to 46, which is the population above; a table whose parts do not add to its
stated whole has been the tell for a miscount here before.

**Ten of forty-six sites carry an examined verdict.** The remaining thirty-six are named by group and
explicitly marked as not examined. A census whose entries are unexamined opinions is worse than a
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

It is left undone here for a reason that should be weighed rather than assumed away: **continuous
integration does not run this feature set.** The three sets it runs are default, `signatures,shell`
and `self-host`, all of which include floats. The repair and its test would be exercised only by the
release gate's `--no-default-features` step, so landing it wants a deliberate local run of that
configuration rather than a reflexive merge.

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

**The next pass should start at group D.** The typed operand-stack verifier runs in a sound
defer-on-unknown mode: an operand whose flat shape it cannot reconstruct defers to a retained
runtime guard rather than a load-time rejection. The seven "operand form does not match" sites are
precisely where a deferred shape meets a runtime refusal, which is the same shape as the two holes
already found.
