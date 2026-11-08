# Absorption 88

Computed against native `badca87f` and upstream `ea6f3c8c` on 2026-10-24, with
8 unabsorbed commits measured after fetching. The merge base is `018ad48b`. The
separate conflict command `git merge-tree --write-tree HEAD origin/v0.2.3`
exited one and produced tree `f09d726ac04d8ec9b0034c3c4b54ff9ac2cdfa3a` with one
conflicting file, `docs/process/interline/checkpoint.json`, which was regenerated
from the live store. The merge was prepared on `feat/v3-absorption-88`, cut from
`feat/v3-absorption-87` before pull request 510 merged as `30cd340d`.

## What was absorbed

V0.2.X Order 3, first-pass trap analysis, merged upstream as pull request 506 at
`a20011dd` and announced in `v023-20261024-order3-complete`. For this backend,
two contracts changed and four stage sources changed. No opcode was added and
the bytecode version is unchanged.

- **Every native call is reified.** The compiler sets the reify flag on every
  call, so the call pushes the value and a zero flag, or a host error code and
  a one. A bare call follows with a flag test and `Trap(HostError)`, code 9.
- **`GetIndex` on a nested composite or boxed element is total.** An invalid
  index pushes a zero-filled placeholder of the element's size, then the index,
  the four bound limbs and the flag, the layout flat Word and Byte reads already
  had.
- The virtual machine now refuses, before execution, a module that calls an
  unregistered native.
- `analyze.kel`, `codegen.kel`, `verify_structural.kel` and `verify_typed.kel`
  changed.

## Effect on this line, measured

On the staged merge with the previous lowering, the default native suite ran
160 binaries and failed 53 of 802 tests. The failures were native calls, which
the lowering refused because of the reify flag, and nested indexed reads.

The lowering changes, in `native_codegen/src/lib.rs`:

- A reified native call pushes its result and a constant zero flag. **The
  native host ABI has no soft-failure channel.** A native is `i64 f(i64...)`,
  so a linked host function cannot report a soft host error. The `Trap(HostError)`
  arm lowers and is never taken. A host that needs soft failures needs an ABI
  carrying them, which is Workstream D's to define. This is a limit, not a
  divergence for any host this ABI can express.
- A nested `GetIndex` selects the address of a zero-filled module constant of
  the element's size for an invalid index and pushes the bounds suffix. The
  former trap guard is removed, because the compiler now dispatches the flag.

The remaining four failures were expected and are resolved as follows.

| Test | Cause | Resolution |
|---|---|---|
| `the_error_reify_flag_is_refused_rather_than_mislowered` | It pinned the old refusal | Replaced by `every_reified_native_call_lowers_and_agrees_with_the_vm`, which checks that the compiler sets the flag and that bare and handled calls agree with the virtual machine |
| `unused_chunks_do_not_change_coroutine_admission_or_site_identifiers` | The virtual machine now refuses an unregistered native at load | The test registers a native that panics if called. It sits in an unused chunk |
| `the_corpus_is_what_the_pinned_figures_were_measured_against` | Four stage sources changed | Re-pinned after the named censuses passed with their pinned figures |
| `q2_is_the_bytecode_bound_monotone_in_native_size` | Inversions fell from 18 to 17 | Measured on `v0.3.0` before the absorption and diffed. Exactly the eleven native-calling chunks moved. Re-pinned with the cause |

The population is unchanged at 158 files and 775 functions.

## Verification

Run locally with `RUSTC_WRAPPER=` on the final tree. Logs are under ignored
`tmp/absorption-85/`.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | clean |
| `cargo clippy --locked --tests -- -D warnings`, default and `--features narrow-float-32` | clean in both |
| Full native `cargo test --locked --no-fail-fast` with `KEL_REQUIRE_LLVM_TOOLS=1`, default | 160 binaries, 802 passed, status zero |
| The same with `--features narrow-float-32` | 160 binaries, 802 passed, status zero |
| Root `documentation_links` and `comment_citations` | 8 passed, status zero |

One full run was stopped and repeated after the resume stamp named the merge
commit of pull request 510, which is not an ancestor of this branch. The stamp
now names `badca87f`, the same tree. The self-host tier and the other root
feature sets were not run locally. Hosted CI is the integration check.
Publication is held and no merge into `main` is authorized.
