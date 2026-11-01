# Build Output on the Scratch Volume, 2026-10-31

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked the kaizen line on 2026-10-31 to move this worktree's build output to
the scratch volume, to check whether an orphaned scratch copy from 30 October is still
needed, and to propose how Keleusma should adopt the template's scratch handling in
`scripts/worktree.sh`. The request pointed to V0.3.X's notice of sequence 190, the
template's `CLAUDE.md` section "Build Output on a Scratch Volume" and the header of the
template's `scripts/scratch.sh`. This report gives the receipt for the move, the finding
on the orphan and the proposal. Nothing in the proposal is carried out.

## 1. The move of this worktree's `target`

**Facts.** Nothing had written under `target` in the 24 hours before the move. Its newest
entry dated from 2026-10-24. The sandbox refuses `ps` and `pgrep`, so the absence of a
running build rests on file times, not on a process list. The command was the one the
operator gave, run from inside this worktree in the sandbox, at 21:42 UTC.

```sh
SCRATCH_BASE=/Volumes/Scratch ~/projects/project-template/scripts/scratch.sh link target
```

| Measure | Before | After |
|---|---|---|
| `target` | 3.3 GB, a directory | a link to `/Volumes/Scratch/keleusma/kaizen/target`, 3.3 GB there |
| Files with more than one hard link | 0 | not applicable |
| Data volume used, available | 698 GiB, 89 GiB | 694 GiB, 89 GiB |
| Scratch volume used, available | 105 GiB, 89 GiB | 109 GiB, 89 GiB |
| `git status --porcelain` | clean | clean |

The move took 11 seconds and met no refusal. The two volumes share one APFS container,
so its free space did not change. The check that followed built through the link, with
no `CARGO_TARGET_DIR` override. It ran `cargo fmt --check`, clippy with `-D warnings` on
`ci_shard_coverage` and `gate_ci_correspondence`, and nextest on both, 14 tests passing,
in 37 seconds, with no refusal. The scratch copy grew to 3.4 GB, and no real `target`
reappeared in the tree. nextest reported its test binary at the path in the tree,
`.../kaizen/target/debug/deps/...`, not at the path on scratch, so the path-scoped `pkill`
pattern in `PARALLEL_DEVELOPMENT.md` still matches. That was observed for nextest only.

**A larger store stays put.** This line has built with `CARGO_TARGET_DIR` under its
sandbox temporary directory, because `sccache` fails in the sandbox. Those directories
hold 8.5 GB on the data volume, under `/private/tmp`, which `scratch.sh` does not manage.
The operator's request named only `target`, so they were not moved. Section 3 covers
them.

## 2. The orphaned copy of `kaizen-test-duration`

`/Volumes/Scratch/keleusma-worktrees/kaizen-test-duration/target` holds 1.7 GB, all of it
Cargo output: `debug/` with its `build`, `deps`, `examples` and `incremental`
directories, 28 MB of `doc/`, an empty `tmp/`, and Cargo's own lock and fingerprint
files. It holds no nextest JUnit report, no `cargo-timings` page and no log. Its newest
file dates from 03:48 on 2026-10-24. No file has more than one hard link.

**Fact.** No tracked document cites a path inside it. The worktree's branch,
`test/kaizen-split-stage-artifact`, is kept on GitHub under the tag
`archive/test-kaizen-split-stage-artifact`, whose tree this line compared with the
original commit `e7a59d20` and found identical. The work done in that worktree, the
`opt-level = 1` change of pull request 493 and the per-stage split, is recorded in the
interline checkpoint and in its pull requests and CI runs, not in this directory.
**Inference.** Nothing in the copy is still needed, and everything in it can be rebuilt
from the tagged commit. Deleting it is the operator's decision.

**Another finding.** The copy sits under `keleusma-worktrees/`, not `keleusma/`. The
current `scratch.sh` names the repository folder after the main checkout and would have
used `/Volumes/Scratch/keleusma/kaizen-test-duration`. The template's `rm` cleanup would
not have found this copy either, so the sweep proposed below is needed in any case.

**Correction, 2026-10-31.** This report first inferred that an earlier version of the
script had named the folder after the worktree's parent directory. The operator
corrected the record. The administrator session moved the copy by hand during the
trial of 30 October and named the folder after the parent directory. No version of the
script did that.

## 3. Proposal

### Vendor `scratch.sh` into `scripts/`

Recommended. Copy the template's `scripts/scratch.sh` of v1.0.8 into `scripts/` as it
is, and record its source and version in its header. The reasons follow.

- `worktree.sh` calls `scripts/scratch.sh` by a path relative to the tree, as the
  template's does. A path into `~/projects/project-template` holds on one machine only,
  and a fresh clone elsewhere would fail where it now succeeds.
- The `script checks` CI job runs `shellcheck scripts/*.sh`, so a vendored copy is
  linted on every relevant pull request. The template's copy, outside the repository, is
  not. This line ran shellcheck on both template scripts on 2026-10-31 and found
  nothing.
- The cost is drift from the template. A later template fix has to be carried over,
  the way the Dependabot setup was adopted from the template on 2026-10-28.

Calling the template's copy instead avoids the drift and depends on a checkout this
repository does not control. It is not recommended.

### Changes to `worktree.sh new`

1. After `git worktree add`, when `SCRATCH_BASE` is set, run `scripts/scratch.sh link`
   inside the new tree, with the directories of the next section. Without
   `SCRATCH_BASE`, nothing changes.
2. **Refuse a name whose scratch folder already exists.** For a tree with no `target`,
   `scratch.sh link` creates the scratch folder if it is missing and links to it if it is
   present. A new tree whose name matches a stale or foreign scratch folder would
   therefore share that build output, and two trees could build into one directory at
   once. Keleusma names trees by the leaf of the branch, so `feat/x` and `fix/x` already
   collide. The trees under `.worktrees/` and under `../keleusma-worktrees/` also share
   one scratch namespace, because `scratch.sh` uses only the tree's base name. No two
   current trees share a base name, which this line checked on 2026-10-31. `new` should
   check for `$SCRATCH_BASE/keleusma/<leaf>` before `git worktree add` and stop if it
   exists.
3. Print the tree's path before linking, so that a failed link leaves a tree the caller
   can find, with its output in the tree as before.

### Changes to `worktree.sh rm`

1. After `git worktree remove` succeeds, delete `$SCRATCH_BASE/keleusma/<leaf>` when it
   exists. Keep Keleusma's refusal to remove a dirty tree and to delete an unmerged
   branch, so that build output is deleted only after the tree has been removed.
2. Do the same in the branch where the tree is already gone and `rm` only prunes. That
   is the path that left the orphan.
3. Delete only that one folder, resolved from the same names `scratch.sh` uses, and
   refuse an empty `SCRATCH_BASE` or leaf, so that the deletion cannot widen.

### A sweep for orphans

Add `scripts/scratch.sh orphans`, or the same check in `worktree.sh list`, to print each
folder under `$SCRATCH_BASE/keleusma/` that matches no current tree, with its size. It
reports and deletes nothing, because the operator decides deletions. It would show the
copy of section 2 only if it also read the old `keleusma-worktrees/` layout, so the first
run should name that folder by hand.

### Nested package targets

Keleusma has detached packages with their own target directories, `compiler/target`,
`fuzz/target`, `examples/rtos/target`, `keleusma-lsp/target` and `keleusma-wasm/target`,
and on `v0.3.0` also `native_codegen/target`, which held 23 GB in V0.3.X's tree.

- **Name them in the repository, not only in a machine profile.** Keep a list in
  `worktree.sh` and pass `scratch.sh link` only the entries whose package exists in the
  tree, tested by its `Cargo.toml`. `scratch.sh link` on a missing directory creates the
  parent folders and a link, so passing `native_codegen/target` to a `v0.2.3` tree would
  create an empty `native_codegen/` there.
- `SCRATCH_DIRS` in the machine's profile then stays a fallback for an existing tree,
  where `scratch.sh link` with no arguments manages only the root `target` by default.
- **This line's temporary target directories.** The kaizen line's habit of an isolated
  `CARGO_TARGET_DIR` under the sandbox's temporary directory keeps 8.5 GB on the data
  volume. **Inference.** The isolation is not needed for `sccache`, which an empty
  `RUSTC_WRAPPER` already avoids, as the check of section 1 showed. The line should build in its linked `target` from now on and record the
  change in its state file. Deleting the old directories is the operator's decision.

### Documentation and gates

- **`.gitignore`.** The tracked root rule is `**/target/`, with a trailing slash, which
  matches a directory and not a link, and `examples/rtos/.gitignore` has `target/`. Today
  only the shared `.git/info/exclude`, which is local to this machine, ignores the links.
  Change both to forms without the slash, `**/target` and `target`, and the two retained
  lines for the language server and wasm packages with them, so that a fresh clone on
  another machine ignores a link too. The comment above the rule should say why.
- **`PARALLEL_DEVELOPMENT.md`.** In "Worktrees or separate clones", state that a tree's
  `target` may be a link to the scratch volume, that `cargo clean` deletes the link and
  not the copy, and that `scripts/scratch.sh link` must be run after a clean and before
  building. State that the path-scoped `pkill` pattern still matches, as observed for
  nextest. `GIT_STRATEGY.md` and `CLAUDE.md` only point to `worktree.sh` and need no
  change, which keeps `CLAUDE.md` short.
- **Tests.** Add a Python test under `scripts/`, which the `script checks` job runs, that
  builds a temporary repository with a temporary `SCRATCH_BASE` and checks that `new`
  links each existing package target, that `new` refuses an existing scratch folder,
  that `rm` deletes only its own folder and leaves a sibling's, that `rm` of a dirty
  tree deletes nothing, and that without `SCRATCH_BASE` nothing is linked. shellcheck
  already covers the scripts.
- **Changelog.** None. The change is process tooling and changes nothing users see.

### Costs and risks

- **Hard links and APFS clones break on the move.** A copy to another volume writes each
  linked or cloned file out in full. V0.3.X found no file with more than one hard link in
  its Cargo output, and this line found none in its own. `find` cannot detect APFS clones.
  The container's free space was unchanged across both moves, and a lost clone would
  have lowered it, so none was lost here. That is an inference from `df`.
- **A move across volumes is a copy and a delete, not a rename.** An interrupted move can
  leave part of the output on each side. `scratch.sh link` then refuses to merge them,
  which is safe, and the operator deletes one side.
- **The sandbox must allow writing under `SCRATCH_BASE`.** It does on this machine, and a
  machine that does not will refuse every build there.
- **A session started before the change** must restart before it builds through the
  links, by the guidance V0.3.X relayed from the operator's administrator session.

## Next step

The operator decides whether to adopt the proposal and whether to delete the orphaned
copy and this line's temporary target directories. When the proposal is approved, the
kaizen line makes the change on its own branch and then sends notices to the other
lines.

## Operator decisions, 2026-10-31

The operator's decisions reached this line in the session on 2026-10-31. The proposal
is approved as written, to be implemented in its own pull request for the operator to
merge, with notices to the other lines after it. The vendored script is to stay easy to
compare with the template's, because the administrator session will propose matching
fixes in `sgeos/project-template`. Deleting this line's old temporary target
directories is approved. The administrator session deletes the orphaned copy itself.

**The deletion.** At 22:03 UTC this line deleted seven folders under its sandbox
temporary directory, `kaizen-fuzz-target`, `kaizen-guard-target`, `kaizen-main-target`,
`kaizen-merge-target`, `kaizen-nodef-target`, `kaizen-target` and `kel-target-kaizen`,
8.0 GB in all. Their dependency files named this line's worktrees or carried its name.
Three folders there were kept. `target`, 535 MB, was built from another project, and
`arch-target`, 1.1 MB, and `kel-target-o1`, empty, show no sign of being this line's.
The 8.5 GB figure of section 1 had counted all ten.

**One departure from the proposal.** Section 3 said to record the vendored script's
source and version in its own header. To keep it byte-identical to the template's copy,
as the operator asked, the implementation records them in the header of `worktree.sh`
and in `PARALLEL_DEVELOPMENT.md` instead, and `scripts/scratch.sh` is unchanged.

**One addition.** Review of the implementation found that `rm` of a tree missing from
`../keleusma-worktrees/` would have deleted the scratch folder of a live tree with the
same leaf elsewhere, such as this line's tree under `.worktrees/`. `rm` now keeps a
folder that any current worktree's name still uses, and a test covers it. The data volume went from 698 GiB
used with 85 GiB available to 690 GiB used with 93 GiB available.
