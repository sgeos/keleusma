# Absorption 83

Computed against native `99023f8f` and upstream `f213f421` on 2026-10-23,
with 11 unabsorbed commits measured after fetching. The separate conflict command
`git merge-tree --write-tree HEAD origin/v0.2.3` exited zero and produced tree
`82ce4fa65f5f730c4e58b6eb6de2e3f532a070c4`. Only after that command was the
prediction made that `FixedDiv` would require native outcome and stack changes.
The merge was prepared on `feat/v3-absorption-83` with `--no-ff --no-commit` so
the absorption, repair and handoff could share the prompt's single commit.

## Runtime reconciliation

Upstream changed `FixedDiv` from one result to a three-slot outcome. Zero
produces the numerator, Fixed zero and flag three. Nonzero division saturates
its quotient, produces Fixed zero in the high slot and reports flag zero even
when saturated. The compiler dispatches an uncaught zero outcome to `Trap`.

Before the repair, the existing saturation differential failed during lowering
with a stack-underflow diagnostic. The native emitter now substitutes a nonzero
divisor before division, selects the numerator on zero, and emits all three
slots. The coroutine analysis assigns Fixed, Fixed and Word to those slots.
Fraction counts at or above the word width remain refused.

Direct tests observe each slot without compiler dispatch and cover fraction
counts zero, sixteen and sixty-three. Coroutine tests receive Fixed replies
across direct and delegated suspension and check each outcome slot at both
optimization levels. Source-level tests cover handled division. Another test
covers homogeneous nested private writes across delegation, including either
index failing while the flattened offset could remain in range.

Three tests were added. No previous test name was removed. The handoff and
population guard now record 764 integration test functions in 157 files.
The changed corpus inputs are `parse.kel`, whose cast tracking now follows the
result type, and `verify_structural.kel`, whose FixedDiv trap classification
changed. The corpus fingerprint was refreshed only after the non-corpus run
re-derived its coverage and refusal figures. The two stage-coverage residuals
remain in the bounded brief and acceptance record.

## Local toolchain

The configured `/opt/local/libexec/llvm-22` was absent. A MacPorts installation
attempt stopped because `sudo -n` required a password. The sandbox also denied
the first fetch's Git metadata write and a network lookup. Their authorized
retries succeeded. No password was requested or recorded.

LLVM 22.1.8 was restored from the official MacPorts archive
`llvm-22-22.1.8_1.darwin_27.arm64.tbz2` into ignored
`tmp/llvm22-root/opt/local/libexec/llvm-22`. Its RIPEMD-160 signature verified
against the installed MacPorts public key. The relocated executable consumers
use `@executable_path/../lib/libLLVM.dylib` and were signed locally after that
change. Existing MacPorts libraries supply the other native dependencies.
`llvm-config`, `llc` and `llvm-readobj` report 22.1.8.

From the repository root, source `tmp/absorption-83/env.sh` before entering
`native_codegen`. It sets `LLVM_SYS_221_PREFIX`, `KEL_LLVM_BIN`,
`KEL_REQUIRE_LLVM_TOOLS=1` and `RUSTC_WRAPPER=`. It does not change tracked
configuration. Do not globally set `DYLD_LIBRARY_PATH` to the LLVM 22 directory.
That attempt made Rust load LLVM 22 where its compiler required LLVM 23 and
exited before tests. The executable relocation removed the need for that
variable. This local setup is absent from a fresh clone. Install LLVM 22.1
through the documented package-manager procedure there, or restore the signed
archive and equivalent executable paths.

## Verification

All commands below ran locally with the environment described above. Logs are
under ignored `tmp/absorption-83/`. The lowering checks ran on the uncommitted
absorption and repair tree with frozen inputs. Later handoff edits receive
separate documentation checks.

| Check | Result |
|---|---|
| Baseline native handoff tests before absorption | 23 passed, exit zero |
| Default non-corpus suite through `tools/frozen-run.sh cargo nextest run --no-fail-fast -E 'not binary(corpus_differential)'` | 781 run, 780 passed, one failed, zero skipped, exit 100, frozen |
| Cause of that failure | The fingerprint identified exactly the two absorbed stage-source changes. Coverage and refusal censuses passed. The fingerprint was then updated |
| Optimized default selection below | 159 passed, nine corpus tests excluded, exit zero, frozen |
| Optimized narrow-float selection below | 159 passed, nine corpus tests excluded, exit zero, frozen |
| Final formatting, lint and documentation checks | Formatting passed. All-target Clippy passed in both float configurations with warnings denied. Native documentation build passed with warnings denied. Root targeted checks passed 37 tests. Native handoff checks passed 24 tests. All exited zero |

The optimized selections used `KEL_OPTIMIZE=1 tools/frozen-run.sh cargo nextest
run --no-fail-fast` with the following expression. The narrow run additionally
used `--features narrow-float-32`.

```text
binary(differential) or binary(retcon_bytecode) or binary(corpus_fingerprint) or (binary(corpus_differential) and test(every_lowering_module_executes_or_is_exempt))
```

The default optimized run reported one passing test as `LEAK`,
`differential::a_flat_array_element_agrees_with_the_vm`. The command exited zero.
The cause is not established. The nine excluded corpus tests include mutation
sensitivity work, which this increment did not repeat. The broader non-corpus
suite was not repeated in narrow mode. No complete local gate is assembled
from these separate checks.

The root command used `RUSTC_WRAPPER= cargo test --locked --no-fail-fast` with
`--test documentation_links --test comment_citations --test gate_ci_correspondence
--test legacy_division_outcomes --test indexed_write_handlers`. It ran the default
feature configuration, not the self-host or full root matrix. The native handoff
command used `cargo test --locked --no-fail-fast` with `--test comment_citations
--test handoff_figures --test test_population_guard --test gate_record
--test shared_channel_discipline`. Native lint used `cargo clippy --locked
--all-targets -- -D warnings`, repeated with `--features narrow-float-32`.
Native documentation used `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`.
Formatting used `cargo fmt --all -- --check`.

These checks do not replace both complete native gates. No new full-gate record
was written. The committed record still names `ece800a5` and remains stale.
Hosted Native gate run `39988352001` passed both configurations at `4c19e026`
as recorded by the preceding handoff. No hosted check for this new feature
commit is claimed. Version integration remains pending remote verification.
Root `src/` and `tests/` were imported from upstream without local edits.
No opcode, bytecode version, publication or merge into `main` was introduced.

## Next session

Resume from the feature branch and its committed handoff. Inspect Git before
switching branches. Fetch and measure the upstream backlog again. Review this
repair and the reconciled acceptance matrix, run the remaining verification
appropriate to integration, then open the version pull request and integrate
only on green hosted checks under the repository policy. Publication is held.

The existing private self-assignment oracle limitation, foreign execution,
host packaging and native cost attestation remain open. Corpus mutation
sensitivity and a new complete local gate receipt are not implied by a passing
execution differential.
