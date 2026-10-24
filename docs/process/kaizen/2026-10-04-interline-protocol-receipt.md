# Interline Protocol Installation Receipt

Measured on 2026-10-04 on `feat/kaizen-interline-protocol`, based on `2bc6b352`.
The operator explicitly requested protocols for communication in both directions
between Codex and Claude Code lines. The implementation and
[protocol](../INTERLINE_PROTOCOL.md) satisfy that transport scope locally.
This receipt does not claim a live model-to-model exchange.

## Implemented and installed

The client uses Python standard-library SQLite transactions. It has no external
service or network listener. This bounded local requirement does not adopt the
earlier proposal for a general messaging service based on the Model Context
Protocol. The versioned handoffs retain decisions and deliverables.

The shared store is `/Users/bsechter/projects/rust/keleusma/secret/interline`.
It contains the installed client, protocol and database. Local Git exclusions
keep the store and client configurations outside every tracked branch.
Source files are versioned in this increment. Installation was performed through
`scripts/install_interline.py` with explicit worktree and line arguments.

| Worktree | Default line | Configurations |
|---|---|---|
| Primary `keleusma` checkout | `v0.2.3` | Codex and Claude Code |
| `keleusma-worktrees/arena-composites` | `v0.3.0` | Codex and Claude Code |
| `keleusma-worktrees/proofs` | `proofs` | Codex and Claude Code |
| `keleusma-worktrees/kaizen` | `kaizen` | Codex and Claude Code |
| `/private/tmp/keleusma-kaizen-delta-20260928` | `kaizen` | Codex and Claude Code |

An explicit operator assignment takes precedence over a worktree default.
The installed reminder states this to avoid reassigning a kaizen session that
was launched from the primary checkout.

## Verification

The final Python suite passed fourteen cases. It starts separate client command
processes against temporary databases. Both Codex to Claude Code and Claude Code
to Codex routes are exercised with explicit reads, receipts and linked replies.
The harness labels are test inputs, not evidence that either model was launched.

Coverage includes concurrent identical retries, concurrent distinct sends,
conflicting retries, incorrect recipient acknowledgments, invalid reply routes,
unknown lines, malformed identifiers, invalid encoding, size limits, literal
shell-looking text, store corruption, missing stores, schema mismatch, bounded
inbox paging and non-destructive initialization. Installation tests preserve
unrelated hooks, check repeatability, refuse conflicting routes and configuration
symlinks, and execute both generated hook events with representative input.

```sh
python3 -B scripts/test_interline.py -v
```

The existing documentation link, comment citation and claimed-count guards were
run through their previously compiled standalone Rust binaries. They passed
fourteen cases in total. The test sources are unchanged. Their manifest root is
this isolated worktree and the binaries were compiled with the compile feature.
These are document checks, not a Cargo build or runtime verification.

```sh
tmp/delta/final/documentation_links --test-threads=1
tmp/delta/final/comment_citations --test-threads=1
tmp/delta/final/claimed_counts --test-threads=1
git diff --cached --check
```

The installed copies were compared with the source. Every installed hook command
was invoked directly with representative event input. The registered line list
and empty production inboxes were checked. No test traffic was inserted into
the production store and no peer acknowledgment was fabricated. The three peer
worktrees and kaizen integration worktree retained clean tracked state.

## Unverified activation and limits

Codex requires hook review through `/hooks`. The installer cannot substitute for
that client trust decision. Claude Code configuration discovery must be confirmed
in its running session. Direct execution proves the hook command works, not that
a live client discovered and trusted it.

Sibling-worktree sessions require a write grant to the shared store, available
through `--add-dir` in both inspected clients, or normal explicit approval.
The current session can access the store, but other live session sandboxes were
not probed. A running session may need restart or resume to load its configuration.

No live peer wakeup, consumption or reply has been verified. There is no automatic
wakeup mechanism. The protocol requires polling at defined work boundaries.
No full runtime gate, remote continuous integration, push, merge or publication
was performed. The local database is not replicated or backed up by Git.
Line identities are self-reported within a same-user trust boundary.
