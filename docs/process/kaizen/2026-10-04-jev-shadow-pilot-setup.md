# Jev shadow pilot setup

The operator authorized a shadow pilot for V0.2.X and possible V0.3.X work.
The [operating guide](../JEV_SHADOW_PILOT.md) and harness-independent tools are
implemented and installed locally for both lines. V0.3.X participation remains
optional. The kaizen line owns the shared measurement protocol and checkpoint.

## Implemented behavior

The adapter records an immutable initial judgment, reviewed semantic input,
source digests and Git provenance before a model call. It checks that the evidence
occurs literally in a supplied source. Only the small semantic input and fixed
rubric reach TypeSafe. Results remain hidden until the task outcome and a blind
label have been recorded. Independent labels and self-review are distinguished.
The default report excludes synthetic samples and does not invent missing usage.

SQLite transactions serialize admission across concurrent lines and prevent
duplicate calls for a case. The shared limits are 500 attempted calls and one
million charged or reserved input tokens. Failed and interrupted requests retain
a conservative reservation. There are no automatic retries, automatic model calls
from hooks, model-based gates or omitted verification steps.

The claims rubric now explicitly treats a claim that a check passed as asserting
successful execution. This resolves the original spike's ambiguous case definition.
The pilot uses rubric `development-shadow-1`. Its results must not be pooled with
the earlier experimental rubric.

## Local installation

| Worktree | Enrolled line | Verified state |
|---|---|---|
| Primary checkout on `feat/v02-order1-next` | `v0.2.3` | Installed status command resolves the line and reports no development samples |
| `arena-composites` checkout on `v0.3.0` | `v0.3.0` | Installed status command resolves the line and reports no development samples |
| Project-local kaizen checkout | `kaizen` | Two explicitly synthetic setup samples recorded |

The shared installation is under `secret/jev-shadow/` in the primary checkout.
It includes the adapter, bounded service client, frozen configuration, operating
guide and example packets. Installed script, configuration and guide bytes were
compared with the worktree sources after the final local update.

The already-installed shared interline client was updated to advertise the pilot
guide. Before replacement, its bytes matched the prior tracked client. All six
existing Codex and Claude Code hook configuration files for the three worktrees
remained byte-identical. Both development-line reminder commands printed the
pilot guide path. No mailbox message was sent and no peer receipt was fabricated.
Hook execution by a future model session still depends on that client's existing
trust and permissions. Manual use works independently of the reminder.

The V0.2.X and V0.3.X tracked worktrees remain clean and on their original branches.
The setup does not merge kaizen history into either development line.

## Verification

| Check | Result |
|---|---|
| `python3 -B -m unittest discover -s scripts -p test_jev_shadow.py -v` | 23 passed |
| `python3 -B -m unittest discover -s scripts -p test_interline.py -v` | 19 passed |
| Existing documentation link and anchor guards | 2 passed |
| Existing comment citation guards | 6 passed |
| Existing claimed-count guards | 6 passed |
| Installed status command from each development worktree | Correct line, empty development sample collection |
| Installed reminder for each development line | Guide advertised without model invocation or sample content |
| Two live synthetic evaluations through the installed adapter | Both succeeded and validated as `jev-1.13.0` |

The pilot tests include simultaneous callers, budget exhaustion, duplicate calls,
interrupted requests, late responses after interruption, missing credentials,
malformed responses, nonfinite probabilities, unsupported model identifiers,
private paths, source mismatch, blind review, false assurance, immutable labels
and configuration mismatch. A fresh Git clone test restores a checkpoint without
the prior `tmp/` or `secret/` directories and reproduces its labelled comparison.

The document guards were run from direct Rust test builds against this worktree,
as documented in the [research report](./2026-10-04-jev-development-flow-spike.md).
No Rust runtime implementation changed. Full runtime, release and remote
continuous-integration gates were not run for this tooling increment.

## Live evidence and limits

The [tracked checkpoint](../jev-shadow/checkpoint.json) preserves the synthetic
requests, model answers, provider usage, timestamps, source references and task
outcome. Baselines and labels are deliberately marked as synthetic and not
independently adjudicated. They are setup evidence, not development accuracy data.

| Synthetic case | Returned choice | Confidence | Input tokens | Output tokens |
|---|---|---|---|---|
| Negated authentication error with an actual assertion failure | `compiler_or_test` | 0.82 | 512 | 59 |
| Successful execution claimed despite explicit nonexecution | `contradicts` | 1.00 | 504 | 44 |

Combined reported input usage was 1,016 tokens. At the configured published rate,
estimated service cost was USD 0.000042672. The default report correctly shows
zero development samples and no independently measured workflow benefit. Only
these synthetic excerpts were sent. The TypeSafe credential was not placed in process
arguments, subprocess environments, tracked files or service receipts.

The live calls preceded the final safe-error diagnostics and compact JSON
serialization changes. The final installed code passed the complete offline
pilot suite. The live observations establish the service path and response
contract, not execution of every final source line against the provider.

## Continuation

The development lines can now follow the installed guide and collect reviewed
samples during their ordinary work. No additional hook configuration is required.
The initial capture, preview and evaluation steps do not require repeatedly
asking the operator for permission. Blinding and explicit packet review are
workflow steps performed by the participating line.

At handoff, export, review and version the checkpoint through the kaizen owner.
A clone must contain this implementation and the latest reviewed checkpoint.
Installation reconstructs the store and the new worktree's enrollment. Old
absolute paths are not restored. Credentials still require private provisioning.
Unexported local observations cannot be recovered from a clone.

Work is on `feat/kaizen-jev-shadow-pilot`, based on `19893dc1`. Appendix B was read
before tracked changes. No push, merge or publication was performed. Remaining
uncertainties include actual participation, representative labels, real task
usage, missed sampling opportunities and whether a later controlled use of Jev
improves completed tasks. This setup makes those questions measurable without
treating a model judgment as authority.
