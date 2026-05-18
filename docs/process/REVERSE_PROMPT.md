# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-17
**Status**: Refactoring sweep applied. Net 79 lines deleted across the rogue example. Five surveyed refactors are all complete.

## Completed in this session round

| Refactor | Resolution |
|----------|------------|
| 5. Roll back the spanning-tree corridor pass | With the carve-floor-only fix in commit `4587871`, the simple chain `R[i] -> R[i+1]` is correctness-safe. The `connected` flag array, the two scratch slots, and the three helper functions are removed from `rogue_dungen.kel`. The corridor loop is back to its three-line form. |
| 4. Per-helper scratch slots in the dungen generator | Moot after the spanning-tree rollback. The scratch slots are gone with the helpers. |
| 3. Six-line dispatch shape on `AiPool` | Three new helpers in `ai.rs`: `call_pure` wraps the value-encoding, call, and error-format step; `call_pure_int`, `call_pure_ints`, and `call_pure_5` chain the appropriate unpacker. Every `dispatch_*` method drops from roughly twelve lines to roughly three. Saves about thirty lines across seven methods. |
| 1, 2. Script-loading duplication and position-coupled name list | New `AiModules::build` constructor takes a closure mapping a script filename to a `Module`. The startup path passes `compile_embedded` (which reads from an `EMBEDDED` lookup table of nineteen `(filename, include_str!)` pairs). The reload path passes `compile_disk` (which reads from `SCRIPT_DIR`). Both share the same field-by-name initialisation in the constructor, so adding a new script needs only two edits: a row in `EMBEDDED` and a field plus call in `AiModules::build`. The position-coupled `names` array and the `drain.next().unwrap()` chain are gone. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Line-count delta

```
 docs/guide/ROGUE.md                     |   4 +-
 examples/rogue/ai.rs                    | 155 +++++++++++++++----------
 examples/rogue/main.rs                  | 199 +++++++++++++-------------------
 examples/scripts/rogue/rogue_dungen.kel |  91 ++-------------
 4 files changed, 185 insertions(+), 264 deletions(-)
```

Net 79 lines deleted across the four files. The largest absolute reduction is in `main.rs` because the script-loading path was the most repetitive of the surveyed sites.

## Notes

- The `EMBEDDED` table in `main.rs` is the single source of truth for the example's nineteen script files. The dungen and game scripts are looked up by name (`compile_embedded("rogue_dungen.kel")`) rather than through a dedicated constant; this is consistent with the AI and item scripts and keeps the surface uniform.
- `AiModules::build` is the only constructor exposed for `AiModules`. The struct's public fields remain public so existing consumers of the type are unaffected.
- The `call_pure` helper takes a `&mut Vm<'static, 'static>` rather than `&mut Vm`. This is required because the `AiPool` stores virtual machines with `'static` lifetimes (the arenas are `Box::leak`-ed). Loosening the lifetime to a generic would not change call sites.

## Intended Next Step

Awaiting operator prompt. The example is in a clean state after the refactoring sweep. Remaining intentional deferrals are documented as manual exercises (Exercise 3.7 placeholder statuses, Exercise 3.6 per-monster shadowcast, the larger tier-two and tier-three exercises). No open bugs or stale TODOs that I can identify.
