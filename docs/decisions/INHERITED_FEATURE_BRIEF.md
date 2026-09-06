# BRIEF — a dependency the backend relies on but never declares

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: working brief. Goals A and B of this increment; goal C (absorption 52) is separate
by design and must be measured alone.

---

## Goal A — the inherited float dependency

### The finding, verified rather than taken from the handoff

`native_codegen/Cargo.toml` declares:

```
keleusma = { path = "..", features = ["compile", "self-host"] }
```

`default-features` is not disabled, so the parent's `default = ["compile", "verify", "floats"]`
also applies. `self-host` implies `compile` and `verify`, so **those two are genuinely declared**.
**`floats` is not, and is not implied by anything.** It reaches this package solely because
defaults were never turned off.

### Why that is worth a ratchet and not a shrug

**The manifest states an intent, and the intent is wrong.** A reader concludes the backend needs
`compile` and `self-host`. It also needs `floats`: the float differential, the float composite
tests, the declared-width refusals and the ABI scope tests all rest on it.

**The realistic failure is a tidy-up, not a malicious edit.** Adding `default-features = false`
is exactly what someone would do on seeing an explicit feature list, and it is the kind of change
that looks like a clarification.

**Scope honestly, and this brief got the scope wrong on its first pass.** It said the float tests
would "fail loudly" without the feature. **Measured by mutation: the package does not COMPILE** —
`default-features = false` removes `ScalarKind::Float` and the build stops before any test runs.
That is louder still, and it means the behavioural probe cannot be what catches the realistic case.
**The declaration ratchet is the guard that does the work.** The defect is that the manifest
misdescribes what the package depends on. **Do not oversell this as a hidden hole.** The peer line's Group B
finding — a float module verifying, loading and then trapping on a no-floats runtime — is about
the RUNTIME and is a different, larger thing. Conflating them would be an overclaim.

### The repair, and the guard

Declare `floats` explicitly, so the manifest says what the package uses. Then a ratchet that fails
with a message naming the manifest line, rather than a bare lex error.

## Goal B — the deferred citation edit

The disposition strings in `isa_lowering_census.rs` should cite `RESET_UNPROVEN_BRIEF.md` and
`IS_STRUCT_DISPOSITION.md`, which already held the mechanisms those dispositions describe. It was
deliberately not made because it needed a verification run. **An unverified edit in a commit is
worse than a recorded intention** — that reasoning was right and the only thing that changed is
that the run is now affordable.

## Goal C — absorption 52

14 commits on `origin/v0.2.3`.

**MEASURE IT ALONE.** That discipline caught a conflation at absorptions 13, 14 and 15, and the one
time it was skipped the count caught the slip instead. **Absorption 40 was not measured alone and is
recorded as a failure**: edits landed while its run was in flight, and this suite contains tests that
READ SOURCE FROM DISK, so the attribution had to be argued. The point of the rule is that an
attribution never has to be argued.

**Record the prediction BEFORE merging**, and name the population as well as the pass count — a
prediction that names a population has to move when the population moves.

## Wrong turns, named

1. **Do not overstate goal A.** It is a manifest-accuracy defect with a loud failure mode, not a
   silent hole.
2. **Do not bundle absorption 52 with A or B.** The whole value of measuring alone is lost.
3. **Do not edit the tree while a measurement runs.** Done twice on this line, most recently in the
   previous increment.
4. **Count with `--no-fail-fast`**, and split runs at `corpus_differential`, which alone consumes
   most of the ten-minute background ceiling.
5. **A pin is the last edit to what it pins.** `corpus_fingerprint` was updated early last increment
   and had to be updated again.
6. **`src/` and `tests/` at the repository root belong to the `v0.2.3` line.** Absorb; do not edit.
