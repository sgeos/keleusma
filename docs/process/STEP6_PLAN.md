# B28 item 2 step 6 -- delete `FlatComposite::Inline`, collapse `Value` 40 -> 32

Working note (AI to human). Captures the full investigation so step 6 can be
executed without re-deriving. Delete this file when step 6 lands.

## Goal

`FlatComposite` is `enum { Inline { bytes: Vec<u8>, epoch: u64 }, Arena(ArenaHandle<[u8]>) }`.
The `Inline` variant (a 24-byte `Vec` plus an 8-byte epoch) pins the enum at 40
bytes, which pins `GenericValue` at 40. Steps 1-5 removed the last `Inline`
producer that had to exist (the shared composite write now goes to the host
buffer). Deleting `Inline` collapses `FlatComposite` to a single arena handle
(plus a zero-size `Empty`), which should bring `Value` to 32. Pin it with a
`const` size assertion.

## Confirmed ground truth (the de-risking)

- The runtime native boundary uses `KeleusmaType::from_value_ctx`, which already
  resolves composites through `ctx.arena` (marshall.rs:646 for arrays, and the
  tuple/struct/enum impls likewise). The bare `from_value` reading
  `fc.as_bytes()` (marshall.rs:537, 725) is only a no-arena convenience path,
  not the runtime boundary. So the boundary is already arena-aware.
- `materialized()`/`to_inline()` exist to feed owned bytes to three no-arena
  readers: the shared packer (now resolves into the host buffer, step 3b),
  value equality (`eq_in_arena`, arena-aware), and the native boundary
  (`from_value_ctx`, arena-aware). All three are now arena-aware, so the owned
  copies are no longer needed.
- `FlatComposite` has a MANUAL `PartialEq` (Inline-by-content; any Arena pair ->
  false). After the collapse it becomes `Empty == Empty -> true`, else false.
  The body enums and `GenericValue` keep deriving `PartialEq` with unchanged
  runtime semantics (Arena pairs were already unequal under it); real content
  equality stays `eq_in_arena`.

## Design

`FlatComposite = enum { Empty, Arena(ArenaHandle<[u8]>) }`. `Empty` is a
zero-length, always-valid body (the `()`/Unit-only case). It niche-optimizes
away because `Arena` carries a `NonNull`, so the enum is the handle's 24 bytes.

## Inline producers to convert (lib code)

- `FlatComposite::zeroed` -- test-only; remove, rewrite tests onto `build_in_arena`.
- `FlatComposite::from_bytes` / `from_bytes_with_epoch` -- bytecode.rs:1023
  (`from_flat_nested_bytes`) and 1185. Convert to arena-direct (`build_in_arena`)
  or `Empty` for zero-length; check each caller has an arena.
- `build_in_arena(size == 0)` and `in_arena(empty)` -- return `Empty`.
- `to_inline` -- remove; consumers go arena-aware (below).
- `value_from_archived` const scratch -- builds `Inline` that `build_const_pool`
  relocates into a boxed pool body with an always-live handle; build into the
  box directly.

## Inline consumers to rework

- `materialized()` (bytecode.rs:1080) + ~11 callers (vm.rs 4038/4062/4105/4654/
  4659/4675, bytecode recursion, audio_natives:247). The Flat arm uses
  `to_inline`. Remove the `materialized()` calls where the value is already
  arena-resident and the downstream resolves (the SetData/SetDataIndexed
  RESET-survival is handled by `SetDataComposite`->persist for private and the
  host buffer for shared -- VERIFY the compiler emits `SetDataComposite` for
  every private composite write; this is the load-bearing invariant and the
  main UB surface; miri must exercise a `loop` that writes a composite data slot
  and RESETs).
- `from_value` composite arms (marshall.rs:537, 725) -- make them error pointing
  to `from_value_ctx` (or remove), since a no-arena reader cannot read an arena
  body. B36 already documents `from_value` as bundled-convenience.
- Inline-only methods (`as_bytes`, `as_bytes_mut`, `write_at`, `slice_at`,
  `len`, `is_empty`) -- construction scratch, replaced by `build_in_arena`'s
  fill closure; remove if unused after the producer conversion.
- `inline_bytes` -- keep: `Empty -> Some(&[])`, `Arena -> None` (the no-arena
  `PartialEq` at bytecode.rs:441).
- `resolve`/`byte_len`/`is_valid`/`eq_in_arena`/`ref_epoch`/`nested_view` --
  adapt to `{ Empty, Arena }`: `Empty` resolves to `&[]`, `byte_len` 0,
  `is_valid` true, `ref_epoch` 0.

## Recommended order (each ends green; the enum delete is the only non-incremental cut)

1. Add the `Empty` variant; handle it in every method (still `{ Inline, Empty,
   Arena }`, `Empty` unused). Green.
2. Convert empty producers (`build_in_arena(0)`, `in_arena(empty)`) to `Empty`. Green.
3. Const scratch builds into the box directly (no `Inline`). Green.
4. Rework `materialized`/`to_inline` consumers arena-aware; drop the now-needless
   `materialized()` calls; point `from_value` composite arms at `from_value_ctx`. Green.
5. Delete `FlatComposite::Inline` + the dead Inline-only methods; rewrite the
   `flat_value.rs` test module; add the `const` size assertion (`Value == 32`).
   Green + MIRI over flat_value and the SetData/RESET path.

## Verification

Four-gate (default, signatures, all-features, clippy, fmt) + `cargo doc -D
warnings` + the size assertion + `cargo +nightly miri test` over `flat_value`
and a VM test that writes a composite data slot across a RESET. Operator agreed
miri is required for this step (weak gate coverage of arena-handle UB).
