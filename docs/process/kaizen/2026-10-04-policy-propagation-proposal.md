# Gate and attribution policy propagation proposal

Prepared 2026-10-04 on `docs/kaizen-policy-propagation`, following the
[delta audit](./2026-10-04-process-delta-audit.md).

## Result

The [nine-file patch](./evidence/2026-10-04-policy-propagation.patch) reconciles
current-facing gate and attribution instructions with the existing operator policy.
It is a tested proposal for the V0.2.X owning line. It has not been applied to a
peer branch, pushed or merged. The original audit and its evidence are unchanged.

The substantive authority is the [operator gate-policy record][policy] at trunk
commit `89fc6aac87b94c4cad53392097386a857c51a8d1`, together with the actual-assistant
attribution convention in [the root instructions][instructions]. This increment
propagates those rules. It does not ask the operator to decide them again.

## Boundaries preserved

Commit once per prompt. Only feature branches may commit with recorded failures.
Version branches and `main` remain green. Relevant local checks may be targeted,
with exact commands, feature selections, exit codes, executed test counts and
omitted coverage recorded. Version-branch integration requires green remote
continuous integration for the candidate revision and relevant local evidence for
affected detached packages.

Full gates remain required before publication or merging into `main`, including
affected detached packages. Publication still needs explicit in-session operator
confirmation. Attribution names the assistant that actually contributed.

These are read policy statements. The judgment that the replacements below faithfully
propagate them is this review's inference. No new gate waiver or publication authority
is created by the patch.

## Exact replacement scope

The linked patch contains the old and new text with application context. This table
explains the decision behind each replacement.

| Surface | Conflict read in the source | Proposed replacement |
|---|---|---|
| `.github/PULL_REQUEST_TEMPLATE.md` | All changes must pass a full local workspace test, lint and documentation checklist | Require a scoped verification receipt, relevant local checks, green remote results for version integration and full gates at release checkpoints. Retain the user-visible-change and language-change checklist items. |
| `.cargo-husky/hooks/pre-push` | Comments call the local release gate mandatory before release-line merges and overstate the hook's coverage | Describe the configured routine checks and reference the settled integration boundary. All executable lines remain unchanged. |
| `CONTRIBUTING.md` | Contributors are instructed to fast-forward directly into `main`, run an unconditional broad local checklist and treat the detached compiler as absent from root continuous integration | Route features through the active version branch, identify the line-specific integration rules, select checks by affected scope and document the compiler job. Refer attribution to the actual-assistant convention. |
| `docs/process/AUTONOMOUS_IMPLEMENTATION_LOOP.md` | A Claude-only trailer, a full-local-gate merge guardrail, an overstated equivalence claim and a blanket hook-bypass prohibition conflict with other current instructions | Apply once-per-prompt attribution, remote version-branch gating and release checkpoint requirements. Explain the command-correspondence check's limits. Refer hook handling to the existing contributor procedure and require omissions to be recorded. |
| `docs/process/PROCESS_STRATEGY.md` | Its tiers require the full local gate for every merge and a fixed broad selection at every increment | Distinguish feature iteration, version integration and release checkpoints. Preserve risk-based checks and the remote matrix. |
| `docs/process/GIT_STRATEGY.md` | Its example hardcodes Claude, its coverage heading asserts equivalence and its merge-helper note suggests sufficient correctness on the linear line | Use an explicit attribution placeholder tied to the root convention, state the coverage guard's actual scope and describe the helper's missing remote-result check. |
| `docs/process/PARALLEL_DEVELOPMENT.md` | The active procedure calls the legacy helper the implementation of current merge policy and instructs a fast-forward merge | Use the governing line-specific merge procedure with candidate and target checks. Serialize full local gates when needed without making them universal. |
| `scripts/fast-check.sh` | Comments say the full self-host suite runs in the push hook and require the local release gate at integration | Describe the remote full-suite responsibility and scoped local evidence. All executable lines remain unchanged. |
| `scripts/merge-to-trunk.sh` | The entry commentary presents the helper as the current merge protocol | Add an explicit legacy-tooling notice explaining the missing remote-result check and incompatible V0.2.3 merge form. Preserve the executable behavior for separate review. |

The contributor paragraph about stage caching is also corrected because its former
absolute no-cache claim contradicted the fast-check path that the same paragraph
recommended. The replacement directs readers to the actual script and recorded
verification configuration rather than publishing another universal cache claim.

## Merge-helper limitation

Read evidence shows that the helper fetches, rebases, optionally runs the local
gate, checks whether the trunk moved and then offers a fast-forward merge and push.
There is no remote-check query on that path. Its success message cannot establish
the current integration requirements.

The patch therefore stops presenting this helper as the policy's implementation.
It does not repair the helper's executable logic. This distinction is material.
Changing merge automation requires a separate review of candidate identity, remote
check conclusions, line-specific merge forms and failure handling. The executable
helper was not run during this increment.

No repository-wide policy-consistency guarantee is claimed. Historical handoff and
journal entries remain historical. Release procedures and workflows are unchanged.
Unrelated stale counts and claims are outside this patch.

## Validation

The candidate is an exported copy of `89fc6aac` plus the patch. A second fresh
export accepted `git apply --check` and the actual application with exit zero.
All nine applied files were byte-compared with the tested candidate and matched.
The same patch passed a read-only application check in the primary checkout at
`563cc273`. Its nine target files are unchanged between those two source commits.

Direct Rust test compilation with edition 2024, the `compile` configuration and
`CARGO_MANIFEST_DIR` pointing at the candidate executed 26 cases. All passed with
zero failures or ignored cases. The selection was two documentation-link checks,
six citation checks, five gate-correspondence checks and thirteen count checks.
The gate checks include three explicit regression controls.

The three shell files passed syntax checks and ShellCheck. A comparison with the
baseline confirmed that all nonblank, noncomment lines are identical. No shell
control flow, hook selection or workflow behavior changes in this patch.

The kaizen proposal worktree also passed 14 document checks, comprising two link
checks, six citation checks and six count checks. The report's prose punctuation
scan and the tracked diff's whitespace check passed.

[Verification evidence](./evidence/2026-10-04-policy-propagation-verification.txt)
records the toolchain, commands, outputs and patch digest. Direct compilation
bypasses Cargo and dependency resolution. These results establish the selected
document checks, not a runtime or full integration gate. External links and remote
continuous-integration results remain unverified.

## To the V0.2.X owning line

Integrate the patch on a feature branch from the current version branch. Check its
application against that branch before applying it. The following commands assume
the patch has been made available at the shown local path.

```sh
git apply --check /path/to/2026-10-04-policy-propagation.patch
git apply /path/to/2026-10-04-policy-propagation.patch
cargo test --test documentation_links --test comment_citations --test gate_ci_correspondence --test claimed_counts
sh -n .cargo-husky/hooks/pre-push
bash -n scripts/fast-check.sh
bash -n scripts/merge-to-trunk.sh
shellcheck .cargo-husky/hooks/pre-push scripts/fast-check.sh scripts/merge-to-trunk.sh
git diff --check
```

Record the actual candidate and test outcomes, update the owning line's process
channels and require green remote checks before version-branch integration. The
direct-compilation evidence here does not replace that line's verification receipt.

The intended next kaizen check is whether the owning line accepts, revises or declines
these replacements, with the disposition recorded against this patch. A behavioral
repair of the merge helper remains a separately scoped candidate increment.

[policy]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/docs/process/GATE_POLICY.md
[instructions]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/CLAUDE.md
