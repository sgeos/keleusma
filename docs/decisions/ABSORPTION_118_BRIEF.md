# Absorption 118

> **Navigation** [Decisions](./README.md) | [Absorption 117](./ABSORPTION_117_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `4326ac85`, which is pull request 634 merged into
`v0.3.0`, and upstream `4e0c2304` on 2026-10-30, with 25 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-118`
with `--no-ff --no-commit`.

- **Upstream pull request 628** is V0.2.X's second batch. By its notice in
  sequence 150, it answers this line's sequences 134 and 137. It adds
  `stage_executor::StageExecutor`, a crate-level trait, and
  `selfhost::with_stage_factory`, which offers each stage module to an
  installed `StageFactory` before the driver builds it. The framing stage's
  ten natives reach a returned executor through `register_native`. The same
  batch changes `Vm::resume` so that it no longer pushes its input after a
  `Reset`. It also requires a native's declarations to agree, refuses a
  whole-array store into a data field, compiles a `let` annotated with a
  generic instance type, and adds `Vm::ref_context_for` and
  `Vm::bytecode_image`. `keleusma native` delegates to `kel-native`.
- **Kaizen pull requests 633 and 630** add a survey of continuous integration
  practices and record Dependabot security updates with a runner audit.

## Merge and effect

Two files conflicted. The interline checkpoint was regenerated from the
mailbox store and holds 151 messages, and none of upstream's 150 identifiers
is missing. `docs/process/REVERSE_PROMPT.md` was rebuilt by
`tmp/rebuild_rp.sh` against `751fa668`, as upstream's new 112 lines followed
by this line's 720. `CLAUDE.md` keeps this line's routing paragraph.

No stage or corpus file changed, so `corpus_fingerprint` reported no digest
moved. Root code changed in the virtual machine and the self-hosted driver.
A probe merge of `1ebe86f7` on the stack before pull request 634 merged had
already passed the default native suite, so the `Vm::resume` change does not
alter this line's differentials in any test they run.

The executor boundary unblocks question 19 step 1. Implementing
`StageExecutor` natively needs an entry for each chunk the driver calls
through `call_function`, a bridge from the boxed native closures to the
native code's calls, and the conversion of a stage's yielded word to a
`VmState`. That is its own pull request.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on the merged tree. Every step passed.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 858 passed, 0 failed |
| Native suite, `narrow-float-32` | 858 passed, 0 failed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
