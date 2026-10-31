# The native region supplement, as data for V0.2.X

> **Navigation** [Decisions](./README.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

**Purpose.** The operator answered question 22 on 2026-10-31 that V0.2.X folds
the native region supplement into `auto_arena_capacity_for`, and V0.2.X
confirmed it in its own session and asked in sequence 174 for the formula as
data. This record states it, gives a reference implementation in terms of
`keleusma` types alone, and lists the figure for every corpus module with an
entry, measured at `630e68f0`. Native code is the authority. If root `src`
and this record disagree, `native_codegen/src/region.rs` decides, and a
differential against `keleusma_native::region::host_arena_supplement_bytes`
pins agreement.

## The formula

All arithmetic is in `u32`, saturating, and `align(n)` rounds up to a multiple
of eight, the backend's body alignment, saturating to `u32::MAX`.

1. **A flat site's size.** `NewComposite(Flat { byte_size, .. })` reserves
   `byte_size`. `Const(i)` reserves the packed length of the chunk's constant
   `i` when that constant is a `Tuple` or an `Array` whose members, recursively,
   are only `Int`, `Fixed`, `Byte`, `Bool`, `Tuple` and `Array`, packed with no
   padding, eight bytes for `Int` and `Fixed` and one for `Byte` and `Bool`. Any
   other constant reserves nothing. `Boxed` reserves nothing.
2. **A chunk's own sites.** Walk the ops in order. Each site is placed at the
   running offset, and the offset then becomes `align(offset + size)`. The
   chunk's site bytes are the final offset.
3. **Stream locals.** For a `Stream` chunk, `align(local_count * 8)`, and zero
   otherwise.
4. **Stream spill.** For a `Stream` chunk, `align(64 * 8)`, which is 512, the
   backend's `MAX_STACK` of 64 words, and zero otherwise.
5. **A chunk's total.** `total = align(sites + locals + spill)`. Then for each
   `Call(callee, _)` in op order, `total = align(total) + total(callee)`,
   recursively, with a depth guard of 64 returning zero beyond it and an
   unknown callee contributing zero.
6. **The copy at the yield**, question 24. Only for the entry, and only when
   all of these hold: the entry is a `Stream` chunk; it has a yield-escape
   hazard, a `NewComposite(Flat)` inside an `Op::Loop` scope whose ops also
   contain a `Yield`; its signature's `ret` is `Flat { size, .. }` with
   `size > 0`; `verify::max_resumes_before_reset(module, entry)` is `Ok(n)` with
   `n > 0`; and, in a clone with every entry `Yield` replaced by `Len`, every
   `Scope::Iteration` verdict of `confine::module_confinement` for the entry is
   `Confined`. Then `offset = align(total(entry))` and the copy occupies
   `align(8 + size * n)` bytes from it.
7. **The figure.** With an entry, `offset + align(8 + size * n)` when step 6
   applies and `total(entry)` otherwise. Without an entry, zero.

## Reference implementation

```rust
use keleusma::bytecode::{
    BlockType, Chunk, ConstValue, Module, NewCompositeOperand, Op, WireShape,
};

fn align(n: u32) -> u32 {
    n.checked_add(7).map_or(u32::MAX, |x| x & !7)
}

fn const_len(c: &ConstValue) -> Option<u32> {
    match c {
        ConstValue::Int(_) | ConstValue::Fixed(_) => Some(8),
        ConstValue::Byte(_) | ConstValue::Bool(_) => Some(1),
        ConstValue::Tuple(xs) | ConstValue::Array(xs) => xs
            .iter()
            .try_fold(0u32, |a, x| a.checked_add(const_len(x)?)),
        _ => None,
    }
}

fn site_size(chunk: &Chunk, op: &Op) -> Option<u32> {
    match op {
        Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) => {
            Some(u32::from(*byte_size))
        }
        Op::Const(i) => match chunk.constants.get(usize::from(*i))? {
            c @ (ConstValue::Tuple(_) | ConstValue::Array(_)) => const_len(c),
            _ => None,
        },
        _ => None,
    }
}

fn sites(chunk: &Chunk) -> u32 {
    chunk
        .ops
        .iter()
        .filter_map(|op| site_size(chunk, op))
        .fold(0u32, |next, size| align(next.saturating_add(size)))
}

fn total(module: &Module, chunk: usize, depth: usize) -> u32 {
    if depth > 64 {
        return 0;
    }
    let Some(c) = module.chunks.get(chunk) else {
        return 0;
    };
    let stream = c.block_type == BlockType::Stream;
    let locals = if stream {
        align(u32::from(c.local_count).saturating_mul(8))
    } else {
        0
    };
    let spill = if stream { align(64 * 8) } else { 0 };
    let mut t = align(sites(c).saturating_add(locals).saturating_add(spill));
    for op in &c.ops {
        if let Op::Call(idx, _) = op {
            t = align(t).saturating_add(total(module, usize::from(*idx), depth + 1));
        }
    }
    t
}

fn has_hazard(c: &Chunk) -> bool {
    let mut open = Vec::new();
    for (i, op) in c.ops.iter().enumerate() {
        match op {
            Op::Loop(_) => open.push(i),
            Op::EndLoop(_) => {
                if let Some(s) = open.pop() {
                    let body = &c.ops[s + 1..i];
                    if body.iter().any(|o| matches!(o, Op::Yield))
                        && body.iter().any(|o| {
                            matches!(o, Op::NewComposite(NewCompositeOperand::Flat { .. }))
                        })
                    {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn yield_copy_bytes(module: &Module, entry: usize) -> Option<(u32, u32)> {
    use keleusma::confine::{Confinement, Scope};
    let c = module.chunks.get(entry)?;
    if c.block_type != BlockType::Stream || !has_hazard(c) {
        return None;
    }
    let WireShape::Flat { size, .. } = module.signatures.get(entry)?.ret else {
        return None;
    };
    let n = keleusma::verify::max_resumes_before_reset(module, entry).ok()?;
    if size == 0 || n == 0 {
        return None;
    }
    size.checked_mul(n)?.checked_add(8)?;
    let mut clone = module.clone();
    for op in &mut clone.chunks[entry].ops {
        if matches!(op, Op::Yield) {
            *op = Op::Len;
        }
    }
    let confined = keleusma::confine::module_confinement(&clone)
        .get(entry)
        .is_some_and(|v| {
            v.iter()
                .filter(|s| matches!(s.scope, Scope::Iteration { .. }))
                .all(|s| matches!(s.verdict, Confinement::Confined))
        });
    if !confined {
        return None;
    }
    Some((
        align(total(module, entry, 0)),
        align(8u32.saturating_add(size.saturating_mul(n))),
    ))
}

pub fn native_region_supplement(module: &Module) -> u32 {
    let Some(entry) = module.entry_point else {
        return 0;
    };
    match yield_copy_bytes(module, entry) {
        Some((offset, bytes)) => offset.saturating_add(bytes),
        None => total(module, entry, 0),
    }
}
```

The hazard test above restates `region::yield_escape_hazards` for the yes or
no it needs. That function pairs every flat site in any `Loop` scope containing
a `Yield`, counting dispatch scopes as iterating, which over-approximates in
the safe direction.

## Expected figures

Measured at `630e68f0` with `keleusma_native::region::host_arena_supplement_bytes`
over `common::corpus()`, whose sources are listed by `corpus_sources` in
`native_codegen/tests/common/mod.rs`. The second column is the figure, and the
third is `total(entry)` before the copy, which differs only for
`13_telemetry_stream.kel`, whose copy is 4 slots of 16 bytes and a cursor word,
72 bytes at offset 648.

| Module | Figure | Before the copy |
|---|---|---|
| `01_arithmetic.kel` | 0 | 0 |
| `02_struct_field.kel` | 16 | 16 |
| `03_enum_match.kel` | 24 | 24 |
| `04_for_in.kel` | 32 | 32 |
| `05_pipeline.kel` | 0 | 0 |
| `06_multiheaded.kel` | 0 | 0 |
| `07_refinement.kel` | 0 | 0 |
| `08_method_dispatch.kel` | 0 | 0 |
| `09_big_numbers.kel` | 96 | 96 |
| `10_multbyte.kel` | 192 | 192 |
| `11_signed.kel` | 0 | 0 |
| `12_sensor_window.kel` | 80 | 80 |
| `13_telemetry_stream.kel` | 720 | 648 |
| `14_frame_log.kel` | 600 | 600 |
| `15_pixel_blend.kel` | 80 | 80 |
| `analyze.kel` | 528 | 528 |
| `codegen.kel` | 520 | 520 |
| `external_native_witness.kel` | 0 | 0 |
| `fixed_arithmetic.kel` | 32 | 32 |
| `fixed_conversions.kel` | 0 | 0 |
| `float_witness.kel` | 0 | 0 |
| `frame.kel` | 528 | 528 |
| `interp.kel` | 4032 | 4032 |
| `lexer.kel` | 520 | 520 |
| `opcode_witness.kel` | 0 | 0 |
| `parse.kel` | 520 | 520 |
| `piano_roll_0.kel` | 1872 | 1872 |
| `piano_roll_1.kel` | 1872 | 1872 |
| `piano_roll_2.kel` | 560 | 560 |
| `piano_roll_3.kel` | 1080 | 1080 |
| `piano_roll_4.kel` | 760 | 760 |
| `piano_roll_5.kel` | 584 | 584 |
| `piano_roll_6.kel` | 696 | 696 |
| `piano_roll_7.kel` | 536 | 536 |
| `piano_roll_8.kel` | 920 | 920 |
| `piano_roll_9.kel` | 1016 | 1016 |
| `reconstruct.kel` | 528 | 528 |
| `refused_witness.kel` | 48 | 48 |
| `rogue_ai_boss.kel` | 696 | 696 |
| `rogue_ai_chaser.kel` | 48 | 48 |
| `rogue_ai_fast.kel` | 48 | 48 |
| `rogue_ai_hunter.kel` | 744 | 744 |
| `rogue_ai_idle.kel` | 24 | 24 |
| `rogue_ai_ranged.kel` | 72 | 72 |
| `rogue_ai_sleeper.kel` | 48 | 48 |
| `rogue_ai_smart.kel` | 72 | 72 |
| `rogue_ai_tracker.kel` | 728 | 728 |
| `rogue_ai_wander.kel` | 48 | 48 |
| `rogue_bestiary.kel` | 0 | 0 |
| `rogue_book_keeping.kel` | 24 | 24 |
| `rogue_combat.kel` | 64 | 64 |
| `rogue_consume.kel` | 0 | 0 |
| `rogue_descend.kel` | 40 | 40 |
| `rogue_dungen.kel` | 112 | 112 |
| `rogue_game.kel` | 560 | 560 |
| `rogue_gear.kel` | 0 | 0 |
| `rogue_item_potion.kel` | 320 | 320 |
| `rogue_item_scroll.kel` | 400 | 400 |
| `rogue_move_resolve.kel` | 0 | 0 |
| `rogue_pickup.kel` | 0 | 0 |
| `rogue_player_ai.kel` | 120 | 120 |
| `rogue_scroll_apply.kel` | 0 | 0 |
| `template_scan.kel` | 568 | 568 |
| `verify_datalayout.kel` | 520 | 520 |
| `verify_depth.kel` | 520 | 520 |
| `verify_structural.kel` | 520 | 520 |
| `verify_typed.kel` | 520 | 520 |
| `verify_types.kel` | 520 | 520 |
| `verify_yield.kel` | 520 | 520 |
| `wire.kel` | 520 | 520 |
| `wire_debug.kel` | 520 | 520 |
| `wire_schema.kel` | 520 | 520 |

## What a fold changes for hosts

A host that today adds this figure to `auto_arena_capacity_for` would count it
twice after the fold. This line will remove the separate figure from the
generated native header, `<prefix>_region_bytes`, only once V0.2.X's sizing
includes it, and will say so in the interline channel, so the two never apply
together. Whether the native region and the virtual machine's arena are one
allocation for a native host is a question of the host ABI that the fold does
not settle. This line reads the operator's principle, that the figure is one
an allocation will work with, as covering the native host's region
allocation.
