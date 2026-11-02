# Absorption 131

> **Navigation** [Decisions](./README.md) | [Absorption 130](./ABSORPTION_130_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `72c4316b`, which is pull request 721 merged into
`v0.3.0`, and upstream `2a88d34a` on 2026-11-02, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-131`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **718** records V0.2.X's rulings of 2026-11-01.
- **719** sizes the arena by route, on the operator's ruling that
  `auto_arena_capacity_for` must right-size an arena. That function is again
  the virtual machine's figure alone, and gains a term for seeded private
  composite initializers. `auto_arena_capacity_for_vm_and_native` adds the
  native region supplement once. `native_route_buffers_for` returns the
  region, private and shared buffers of a host that runs a module only
  natively, and `vm_arena_terms_for` returns each virtual machine term.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph, and no
corpus fingerprint moved. V0.2.X's sequence 251 reported the change as the
answer to item 4 of this line's sequence 212, and named three documents here
that described the old sizing. `linkage.rs` and `region.rs` now name the
route functions, and so does the header of `region_supplement_formula.rs`,
which also described the old sizing. The formula record already carries
V0.2.X's supersession note.

`the_runtimes_route_figures_are_the_backends` checks, on every corpus module,
that `native_route_buffers_for` returns the backend's region figure, its
private figure of `required_persistent_capacity_for` plus
`persistent_supplement_bytes`, and the module's shared bytes, and that
`auto_arena_capacity_for_vm_and_native` is `auto_arena_capacity_for` plus the
region figure. The population moved from 848 to 849. Every other use of
`auto_arena_capacity_for` in the native tests sizes a virtual machine with
slack, as V0.2.X inferred, and none changed.

## Verification

Made with `RUSTC_WRAPPER=` at the records commit.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | FILL |
| Native suites, default features and `narrow-float-32` | FILL |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | FILL |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | FILL |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
