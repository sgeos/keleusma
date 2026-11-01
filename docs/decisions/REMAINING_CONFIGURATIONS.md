# The residual is closed: every gate test configuration run on this branch

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Measured 2026-09-02.**
Brief: [`REMAINING_CONFIGURATIONS_BRIEF.md`](./REMAINING_CONFIGURATIONS_BRIEF.md).
Origin: [`UNTESTED_COMBINATION_BRIEF.md`](./UNTESTED_COMBINATION_BRIEF.md).

---

## The result, per configuration, never summed

| configuration | exit | result |
|---|---|---|
| `keleusma` no default features | 0 | **297 passed, 0 failed, 99 binaries** |
| `keleusma` `signatures` | 0 | **2347 passed, 0 failed, 99 binaries** |
| `keleusma` `signatures,shell` | 0 | **2364 passed, 0 failed, 99 binaries** |
| `keleusma` `self-host` | 0 | **2518 passed, 0 failed, 99 binaries** |
| `keleusma-wire` all features | 0 | **57 passed, 0 failed, 5 binaries** |
| `keleusma-wire` no default features | 0 | **20 passed, 0 failed, 5 binaries** |
| detached `compiler/` | 0 | **86 passed, 0 failed, 9 binaries** |
| `keleusma` default features | 0 | **2720 passed, 0 failed, 116 binaries** (carried, measured earlier the same session) |

**A total across these is not a test count.** The same suite runs under several feature sets, so
summing counts most tests more than once.

## What it establishes, and what it does not

**Establishes**: the pairing of the `v0.2.3` line's `src/` with **this branch's corpus** — six added
`.kel` files, one modified, an updated index, read by six workspace tests — passes under every test
configuration the gate runs, not only under default features.

**Does not establish that the gate passed.** The gate also runs `fmt --check`, workspace `clippy -D
warnings`, `cargo doc -D warnings` at the docs.rs feature sets, the Markdown link check, and the
native step. **Running its test steps is not running it**, and calling this a gate pass would be the
scope deletion recorded seven times this session.

## ⚠ EVERY FIGURE MATCHES THE OTHER LINE'S GATE, AND THAT IS NOT EVIDENCE ABOUT THE CORPUS

297/99, 2347/99, 2364/99, 2518/99, 57/5, 20/5 and 86/9 are **identical** to the per-step figures the
`v0.2.3` line reported from their own gate on their own corpus.

**That is corroboration that the two trees behave alike. It is NOT evidence that the extra corpus
files went unexercised**, and a reader could easily take it that way. The corpus-reading tests iterate
a directory **inside a single `#[test]`**, so adding files changes the work without changing the
count. Identical numbers are the expected outcome whether or not the extra files were read.

## The prediction, resolved honestly

**Predicted: all green. All green.** No falsifier fired.

**This confirms an expectation rather than resolving a doubt**, which the brief said in advance would
be the case. **The value is that four `keleusma` configurations, both `keleusma-wire` ones, and the
detached subproject had never been run on this branch at all** — not in the confirmation.
