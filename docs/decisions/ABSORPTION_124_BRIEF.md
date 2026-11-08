# Absorption 124

> **Navigation** [Decisions](./README.md) | [Absorption 123](./ABSORPTION_123_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `1b7b01a5`, which is pull request 667 merged into
`v0.3.0`, and upstream `545087c5` on 2026-10-30, with 7 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-124`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **666** widens definite initialization, on the operator's option (b) of
  2026-10-30 as V0.2.X records it. The compiler writes every local on every
  path, and `verify_init`'s locals rule is a must-analysis with its
  `verify_depth.kel` twin. Every private slot gets its type's zero as its
  load-time initializer, a composite its zero body and `Text` the empty static
  string, and the virtual machine moves each composite initializer into the
  persistent pool at construction. A slot still initialized to `Unit` must be
  written on every path before a read.
- **668** is V0.2.X's handoff at `d8dae4cc`.

## Merge and effect

The interline checkpoint conflicted and was regenerated at 170 messages, and
none of upstream's 169 identifiers is missing. `docs/process/REVERSE_PROMPT.md`
was rebuilt by `tmp/rebuild_rp.sh` against `5ea94b3a`, as upstream's 249 lines
followed by this line's 726. `CLAUDE.md` keeps this line's routing paragraph.
`corpus_fingerprint` reported `codegen.kel`, `parse.kel`, `verify_depth.kel`
and `wire.kel` modified, which were re-pinned.

**The kind census.** 0 of 192,597 corpus instructions need a run-time kind,
with 31 static, 0 dynamic and 2 refused coroutines. `14_frame_log.kel`, whose
private slot started as `Unit`, was the last dynamic coroutine. Question 9's
aim is met for the corpus, and question 26 is withdrawn.

**The native private image.** `region::private_init_image` refused every
module whose private initializers held a composite, because it placed only
scalars. It now extends over the composite pool and the initialisation words
when a module has composite slots. Each composite slot whose initializer is
not `Unit` has its packed initializer at its pool offset, zero padded to its
pool entry, and its initialisation word set as a write sets it, the kind tag
in the low bits and the length above bit 32. A first version wrote `1`, which
the coroutine route decodes as the `Unit` tag, and a coroutine read of a
private enum slot then took the slot's scalar word as a body address and
crashed with `SIGSEGV`. The test that found it now passes. A `Text` private
slot's empty static string cannot be placed in a static image, so a module
holding one is refused natively.

Ten native tests failed in both float configurations.

| Test | Cause | Now |
|---|---|---|
| `host_contract_completeness`, `memory_read_census`, `private_init_image`, three tests | The image refused composite initializers | The image covers the pool and the words, and each test states the new layout. The census header counts two host-boundary rows a zeroed buffer satisfies, not three |
| `private_init_image::the_image_does_not_mark_a_composite_slot_as_written` | Its premise reversed | Replaced by `a_composite_slot_starts_written_with_its_types_zero`, which also reads the slot before its first write on both sides |
| `indexed_composite_slot::the_reference_faults_on_an_unwritten_sibling_element` | The sibling reads its zero | Replaced by `an_unwritten_sibling_element_reads_as_its_zero`, compared with the virtual machine |
| `corpus_differential::a_trapping_programs_native_side_dies_with_sigtrap` | Two guard subjects no longer fault | Both rows retired with their reason. Verified source cannot reach the initialisation-word guard any more, and it stays as defense in depth |
| `retcon_bytecode::private_composite_slots_preserve_unit_values_without_eager_faults` | The enum slot starts as `E::A` | Expected `[1, 1, 1, 1]`, on both sides |
| `retcon_bytecode::composite_reply_kinds_must_match_the_consuming_operation` | A private tuple slot written from an array-declared reply now has an admissible kind | A lowering is accepted only with its run-time kind guard, recognised by its `kind_valid` comparison |
| `static_kind_lowering::the_kind_census_over_the_corpus` | No corpus coroutine is dynamic | The corpus is pinned at 0, and a synthetic subject proves the census can report a need |
| `static_kind_lowering::a_dynamic_coroutine_is_refused_only_when_static_kinds_are_requested` | No corpus subject | The same synthetic subject |
| `stage_differential::wire_emission_joins_agree_with_the_vm_and_the_driver` | For `14_frame_log.kel` each join run alone leaves 26 bytes of the struct name `Frame` zero in NAMES and STRING_POOL | Native code still agrees with V0.2.X's driver entry on every join. The reference comparison for this subject is narrowed to that gap, pinned at 26 bytes, and V0.2.X was asked in sequence 170 |

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` before the test changes, which found the ten
failures, and after them.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 864 passed, 1 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | passed |

The one failure in each suite was `comment_citations`, which found a backticked
LLVM value name in a new test comment. The comment was reworded, and
`comment_citations` and `retcon_bytecode` were rerun and passed. The
documentation readers were rerun after this section was written. The root
workspace suite was not run locally. The hosted run on the pull request
covers it.
