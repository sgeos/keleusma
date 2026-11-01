# Brief — close the residual this line named rather than leave it stated

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G15** run the remaining gate configurations on this branch | **unblocked, and the subject of this brief** |
| `f16` | no oracle — the reference refuses widths 3 and 4 at load |
| `Text<N>`, `Opaque` | the `v0.2.3` line's |
| publication | held |

## Why now, when the last iteration chose not to

The previous iteration recorded this residual and wrote: *"Recording it beats half-closing it in the
last hour of a long session."* **That was a judgement about capacity, not about value.** The residual
is now the only unblocked work, so closing it fully is the right call where half-closing it was not.

## What is actually unverified

[`UNTESTED_COMBINATION_BRIEF.md`](./UNTESTED_COMBINATION_BRIEF.md) established that `src/` and
`tests/` are **byte-identical** to `origin/v0.2.3` while the corpus is not — six added `.kel` files,
one modified, and an updated index, read by six workspace tests. The pairing *their source with this
corpus* was then run and passed.

**Under default features only.** The gate runs eight test steps; **one has been run here.** The
remaining seven:

- `keleusma` with no default features
- `keleusma` with `signatures`
- `keleusma` with `signatures,shell`
- `keleusma` with `self-host`
- `keleusma-wire` with all features, and with none
- the detached `compiler/` subproject

## The prediction, and why the expected answer is "green"

**Predicted: all configurations pass.** The corpus-reading tests are not feature-gated, so default
features already exercised the files that differ, and `src/selfhost/kel` — which the `self-host`
configuration reads — is byte-identical to `origin/v0.2.3`.

**Falsifier**: any failure in any configuration. That would mean this branch breaks something the
other line's gate covers on their corpus, and it becomes a release blocker rather than a branch
curiosity, since the plan is to back-merge this line before publication.

**A green result is worth less than it looks and should be reported as such**: it confirms an
expectation rather than resolving a doubt. **The value is in the four configurations that have never
run here at all**, not in the confirmation.

## The wrong turns

**1. Run the gate's own commands, not approximations.** A configuration verified by a
similar-but-different invocation is a different measurement wearing the same name.

**2. Report per-step, never a total.** The same suite runs under several feature sets, so summing
counts most tests more than once. That is the gate's own recorded rule.

**3. Read the tool's status, not the pipeline's.** Both directions have bitten here: `tee` yields
green on a red tree, a trailing `grep` yields red on a green one.

**4. Do not run them concurrently.** They contend, and the perf canary's contended reading sits three
per cent from its own regression row.

**5. Do not report this as "the gate passed."** The gate includes `fmt`, `clippy`, docs, markdown
links and the detached subprojects. Running its test steps is not running it, and saying otherwise
would be the scope deletion this session has recorded seven times.
