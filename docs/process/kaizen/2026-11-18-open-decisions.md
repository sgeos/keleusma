# Decisions Ruled on 2026-11-18

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

On resuming on 2026-11-18 after a pause, this line put seven questions to the operator in its
session. They follow [the list of 2026-11-04](./2026-11-04-open-decisions.md) and the
questions raised since. This record gives them as presented, in brief, and the operator's
first-hand ruling.

## The questions as presented

1. **The CI toolchain.** Rust 1.100.0 reached the unpinned stable toolchain on 2026-11-12, and
   its Clippy has failed every pull request since. (a) Pin CI and bump deliberately, with a
   weekly advisory job on the latest stable, recommended. (b) Stay unpinned and fix lints as
   stable moves. (c) Pin through `rust-toolchain.toml` for every build.
2. Push this line's idle compaction result and open its pull request.
3. Delete the merged local branch `docs/kaizen-proof-ci-plan`.
4. **Where worktrees live.** (b) Inside the project under `keleusma/.worktrees/`, recommended
   because pydromon roles require it. (a) Beside it under `keleusma-worktrees/`.
5. **E4 of 2026-11-04.** Whether the interline checkpoint keeps other projects' messages.
6. Two questions the proofs line put: whether the heavy Kani harnesses, about 30 GB each, may
   be measured on this shared machine, and whether a run may pause its own processes when the
   machine is critical.
7. The signing key, which V0.2.3's release awaits, and the deferred K-series checks.

## Operator ruling, 2026-11-18

The operator ruled first-hand in this line's session, in these words.

> Keleusma should use the latest Rust toolchain. Getting everything up to date can be a Kaizen
> task. Create a PR to update to the latest toolchain and/or address PRs to fix CI in a
> sensible order.
> 1B. 2, yes. 3, yes. 4A. 5, yes for now; may revisit later given more free attention. 6, yes
> and yes. 7, defer until I can prepare the key.

| Item | Disposition |
|---|---|
| Toolchain | Keleusma follows the latest stable Rust, unpinned in CI, option (b). This line brings the code and the pinned detached packages up to date and fixes CI, then works the blocked pull requests in order |
| 2 | Pushed, with its pull request |
| 3 | Deleted |
| 4 | Option (a). Worktrees live beside the project, under `keleusma-worktrees/`, as `worktree.sh` already does. This line's own tree moves there between sessions. Pydromon roles, which require a worktree inside the project, are not available to Keleusma lines while this stands |
| 5 | Yes, for now. The checkpoint keeps other projects' messages, and the export hold of sequences 294 to 296 is lifted. **This line's reading**, which the ruling does not state: the disclosure-term scan still applies to every export, so a message that matches a term is exported with its body redacted, as the operator decided for sequences 193 and 194 on 2026-10-31, and the store itself is left unchanged |
| 6 | Yes, and yes. The heavy Kani harnesses may be measured on this machine, and a run may pause its own processes when the machine is critical. Relayed to the proofs line |
| 7 | Deferred until the operator can prepare the key. V0.2.3's publication still waits for it |
