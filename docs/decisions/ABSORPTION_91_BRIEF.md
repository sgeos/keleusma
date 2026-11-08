# Absorption 91

Computed against native `5324a734` and upstream `50b20ef6` on 2026-10-25,
with 4 unabsorbed commits measured after fetching. The merge base is
`ec536b28`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `485f7f0c81d423c548dba60a33f5ab6f7d7dcabb` with two conflicting
files, `docs/process/REVERSE_PROMPT.md` and
`docs/process/interline/checkpoint.json`. The merge was prepared on
`feat/v3-absorption-91` with `--no-ff --no-commit`.

## What was absorbed

Two upstream pull requests.

- Pull request 516, merged as `3272d46d`, is Kaizen's cleanup of home paths
  in process documents, with a refusal of such paths at pilot capture in
  `scripts/jev_shadow.py`.
- Pull request 517, merged as `50b20ef6`, is V0.2.X's soundness fix, which
  [`FLAT_TEXT_DECODE_SOUNDNESS.md`](./FLAT_TEXT_DECODE_SOUNDNESS.md) records.
  The type checker now requires a yielded value to have the declared return
  type. A flat `Text` field read at the host boundary is range-checked and
  UTF-8-checked before decoding. `RefContext` and `NativeCtx` gain a public
  `rodata` field.

No opcode, bytecode version or stage source under `src/selfhost/kel/` changed.
V0.2.X reported the merge in `v023-20261025-pr517-merged`, which was read and
acknowledged.

## Conflict resolution

Upstream rewrote its bounded reverse prompt to 50 lines. At the merge base the
file had 308 lines, and this line's side is those 308 lines, checked byte for
byte against the base, followed by 668 appended lines beginning at the heading
`APPENDED BY THE V0.3.X LINE`. The resolution keeps upstream's file and appends
this line's 668 lines unchanged, as absorptions 82 and 85 did. The checkpoint
was regenerated from the live store. After this line's two replies it holds 69 messages, one of them traffic between other lines.

## Effect on this line

`native_codegen` builds neither `RefContext` nor `NativeCtx` with a literal, so
the new field needed no change here. Measured on the staged merge in the default
configuration, the native suite passed 803 tests and failed 6.

**Five coroutine dialogue subjects no longer compiled.** Each yielded a value
whose type differs from its function's return type, which per-site dialogues
were written to exercise. That is now a compile error. Each subject was
rewritten in `retcon_bytecode.rs` to keep what it tests within the new rule.

| Test | Rewrite |
|---|---|
| `per_site_dialogues_distinguish_yields_replies_and_completion` | The helper returns `bool` to yield one, and the entry yields `Word` at both sites. The replies stay `Byte`, `Word` and `Float`, and every observed value is unchanged |
| `explicit_flat_replies_survive_reuse_and_delegated_entry_completion` | The pair yields move into a pair-returning helper the entry delegates to. The Boolean yield becomes a `Word` yield |
| `parameterless_dialogues_complete_with_each_scalar_kind` | Each case yields its declared type's zero rather than `false`, and the dialogue's yield shape follows |
| `native_snapshot_results_survive_calls_suspension_and_completion` | A Word-returning helper `emit` carries the scalar yields beside the pair completion. Native results stay live across the delegated suspensions, and the values and call counts are unchanged |
| `native_snapshot_completion_links_from_c_and_emits_for_tier_one_targets` | The same `emit` helper, with the site taken from its chunk and a `Word` reply |

**What the rewrites give up.** Within one chunk, yields of different shapes can
no longer come from source. The native lowering still accepts such bytecode
under explicit dialogues, and the typed verifier still admits it, because it
records a composite's kind and size and not its field layout. That path now
has no source-built subject here. Upstream records the same verifier limit as
a residual awaiting an operator ruling.

**The shared-channel guard's floor was recalibrated.**
`the_shared_channel_keeps_both_lines_material` required more than 8192 bytes
before this line's addendum, a figure set when upstream's document was 1988
lines. Upstream's file is now 2829 bytes and is kept first and entire, so the
correct resolution failed the old floor. The floor is now 1024 bytes, which
still fires if the addendum lands at the top.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` on the staged
merge after the repairs. Logs are under ignored `tmp/order-3/`. Figures are
read from each log's summary lines and its own captured status.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 809 passed, 0 failed, status zero |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 809 passed, 0 failed, status zero |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 16 passed, status zero |

The native suites ran before this table was written. The tests that read this
receipt and the handoff were rerun afterwards. These checks do not run the root
workspace suite, the self-host tier or the backend gate's optimised phase.

The hosted CI run on the absorption pull request is the version-branch
integration check. No new full-gate record was written. Publication is held and
no merge into `main` is authorized.
