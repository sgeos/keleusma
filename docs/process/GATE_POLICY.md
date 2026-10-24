# Operator gate-policy record

## Source

These quotations are from the operator messages in the active Codex conversation on 2026-09-28, following the legacy V0.3.X handoff review. They are direct instructions to the assistant, not proposals authored by the assistant. This file records the conversation excerpts in the repository because `PROMPT.md` has no active prompt and is read-only to the assistant. No conversation permalink or exported transcript is attached.

The operator first wrote the following.

> Commit once per prompt is OK, but only feature branches may commit on red.

> This project has gates that take a long time to run, so the goal is to run a relevant subset of gates to maintain velocity, while only running the full suite at important points. For example, a feature branch that does not work out can be iterated on until it is in good shape, or it can be discarded in a worst case scenario. This is not true for the V0.X.Y lines, or the main branch.

When the assistant asked whether every native-only feature merge required full checks, the operator clarified the following.

> Full gates absolutely need to run before publication or merging into the main branch. V0.X.Y branches should be kept green via remote CI. Local gates can remain targeted as long as there is high confidence that remote CI will come back green.

Here, CI means continuous integration.

## Effect on the recorded workflow

The clarification supersedes the assistant-authored per-integration full-native-gate requirement introduced during the handoff repair. Commit `cb91b987` still interpreted version-branch integration as a mandatory full-local-suite checkpoint. The operator clarification was applied in upstream `e3634edb` and native `aa4db05f`. This record supplies the source omitted from those commits.

Full gates are mandatory before publication or merging into `main`. Version branches require green remote continuous integration. Relevant local subsets may support version-branch integration, with exact commands, outcomes and omissions recorded. Root continuous integration does not exercise the detached native package, so green remote results alone do not establish native correctness. The local selection must account for native changes as well. A historical full-gate record remains historical until complete gates are rerun.

Publication still requires explicit in-session confirmation. This ruling does not authorize a publication, a merge on red, or any change to the bytecode version or opcode limit.
