# `--features self-host` alone does not build, and it is one ungated line

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: reported by the V0.3.X line, 2026-09-06. **The repair is not this line's to make** —
`src/selfhost/mod.rs` belongs to the `v0.2.3` line and is read-only here. Every fact below was
measured for this document.

---

## The defect

```
cargo build -p keleusma --no-default-features --features self-host
error[E0599]: no variant, associated function, or constant named `Float` found for enum `ScalarKind`
   --> src/selfhost/mod.rs:337:26
337 |         5 => ScalarKind::Float,
```

**One site, and `floats` is exactly the missing piece**: `--features self-host,floats` builds with
zero errors. `ScalarKind::Float` sits behind the `floats` feature, and this line names it
unconditionally.

**It is the feature, not a combination.** `self-host` alone fails; `compile,self-host` fails for the
same single reason; **`compile,verify` builds cleanly**, which is the case the `v0.2.3` line already
repaired.

## Why nothing caught it

**Continuous integration runs `cargo nextest run --profile ci -p keleusma --features self-host`, which
is ADDITIVE to the default features.** `default = ["compile", "verify", "floats"]`, so floats is
present and the job is green.

**That is the `v0.2.3` line's own recorded pattern, in a fresh instance**: *"a job named for a feature
does not necessarily cover that feature."* Their [`FEATURE_COMBINATION_SWEEP.md`](./FEATURE_COMBINATION_SWEEP.md)
swept eleven configurations and stated that **the unswept space — the narrow selectors and
`self-host` — is far larger than the swept one.** `self-host` was named as unswept, and it is broken.

## How it was found, which is not by looking for it

The V0.3.X line declared `native_codegen`'s inherited `floats` dependency and mutation-tested the
declaration by adding `default-features = false`. **The build failed in `keleusma`, not in the
backend**, which is what made this visible. The intended subject was a manifest that under-described
itself; this is a different and larger thing, and the two are deliberately kept apart.

## Scope, stated rather than implied

**This is a BUILD failure, loud and immediate.** It is not the load-time hole recorded in
[`INVALID_BYTECODE_CENSUS.md`](./INVALID_BYTECODE_CENSUS.md) Group B, where a float-using module
verifies, loads, and then traps on a no-floats runtime. Conflating a configuration that cannot compile
with one that compiles and then misbehaves would overstate this.

**And it says nothing about whether `self-host` without floats is a configuration anyone wants.** The
self-hosted pipeline may legitimately require floats; if so the honest repair may be to declare that
dependency rather than to gate the line. **That is a design judgement for the owning line**, and this
document reports the fact rather than prescribing the fix.

## What the V0.3.X line did NOT do

It did not edit `src/`. It did not add a continuous-integration job — the `v0.2.3` line has already
recorded that per-push cost is the operator's call, and adding a job on their behalf would decide a
question they deliberately left open.
