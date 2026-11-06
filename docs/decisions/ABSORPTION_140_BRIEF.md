# Absorption 140

> **Navigation** [Decisions](./README.md) | [Absorption 139](./ABSORPTION_139_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `d0c18311`, which is pull request 743 merged into
`v0.3.0`, and upstream `cecd93db` on 2026-11-06, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-140`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **744** is option B and S1 of the array wire-slot design, on the
  operator's ruling of 2026-11-04. A data operand names a field.
  `GetDataIndexed` and `SetDataIndexed` take the field with an index
  selecting the element, `GetData` and `SetData` address element zero, and
  the verifier refuses a data span that crosses into another field. S1 packs
  private arrays at element width in the virtual machine, which reduces the
  corpus's private data by 74.8 percent as V0.2.X measured it. No opcode and
  no wire byte change.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and the
interline checkpoint is not touched.

**This line reviewed the native changes before the merge.** V0.2.X asked for
that review in sequence 310 and held its merge until this line accepted, in
sequence 311.

- `lib.rs` and `coroutine/types.rs` translate a data operand to the field's
  first element where operands are read, so the lowering and the analysis
  stay slot-indexed, as `data_view.rs` assumes.
- `region.rs` and `src/native_region.rs` keep the native element-slot image
  behind `native_private_base_bytes`, and fold `native_private_gap_bytes` into
  both copies of the persistent supplement, so `required_persistent_capacity_for`
  plus the supplement sizes the same buffer as before.
- In `retcon_bytecode.rs`, an initializer case this line wrote for Encoding F
  is removed, because under option B its indexed span crosses two fields. The
  crossing-span test asserts the verifier's refusal, and the native placement
  refusal stays as a backstop for unverified bytecode.

V0.2.X added the documentation line this line asked for, tying
`native_private_base_bytes` to the `Value` size assertion.

## Verification

Made with `RUSTC_WRAPPER=` at `c4703a11`.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 880 passed, 0 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | 62 and 124 passed |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
