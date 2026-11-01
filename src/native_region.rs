//! The arena bytes a native compilation of a module needs beyond what the
//! virtual machine needs, so that one sizing figure serves both routes.
//!
//! # Provenance and authority
//!
//! The V0.3.X line's native backend reserves a composite region for each
//! chunk, a spill area and locals for the stream entry, and on some streams a
//! copy of the yielded value. That backend lives in the detached
//! `native_codegen` package, which this crate cannot depend on. The operator
//! ruled on question 22, confirmed first-hand in the V0.2.X session on
//! 2026-10-31, that [`crate::vm::auto_arena_capacity_for`] fold this
//! supplement in, so that its value works for native allocation.
//!
//! This module reimplements the formula that V0.3.X sent as data on
//! 2026-10-31, its `NATIVE_REGION_SUPPLEMENT_FORMULA.md`, which that line
//! checks against `keleusma_native::region::host_arena_supplement_bytes` on
//! every corpus module with an entry. **The native backend stays the
//! authority.** If the two disagree, this copy is wrong. A change to the
//! backend's region plan must be mirrored here, and
//! `tests/native_region_supplement.rs` pins figures V0.3.X measured so that
//! a drift in this copy fails.
//!
//! # The formula
//!
//! All arithmetic is `u32` and saturating. `align(n)` rounds up to a multiple
//! of eight, the backend's body alignment.
//!
//! 1. A flat `NewComposite` reserves its byte size. A `Const` of a tuple or
//!    array whose members, recursively, are only `Int`, `Fixed`, `Byte`,
//!    `Bool`, tuples and arrays reserves its packed length, eight bytes for
//!    `Int` and `Fixed` and one for `Byte` and `Bool`. Anything else reserves
//!    nothing.
//! 2. A chunk's sites are placed in op order, each at the running offset,
//!    which then becomes `align(offset + size)`.
//! 3. A `Stream` chunk adds `align(local_count * 8)` for its locals and 512
//!    for the backend's spill of 64 words.
//! 4. Each `Call` adds the callee's total after aligning, recursively, with a
//!    depth guard of `MAX_CALL_DEPTH`.
//! 5. When the entry is a stream that builds a flat composite in a loop that
//!    also yields, returns a flat value, has a bounded number of resumes
//!    before a reset, and whose iteration sites are all confined once its
//!    yields are disregarded, the backend copies the yielded value at the
//!    yield. That copy adds `align(8 + size * resumes)` after the total.
//!
//! # Over-provisioning
//!
//! A host that runs only the virtual machine receives this supplement too, so
//! its arena is larger than the virtual machine needs by exactly this figure.
//! The operator accepted that in choosing one figure over two. A host that
//! formerly added the native figure itself must stop, or it counts the
//! supplement twice.

use crate::bytecode::{BlockType, Chunk, ConstValue, Module, NewCompositeOperand, Op, WireShape};

/// The backend's body alignment in bytes.
const ALIGN: u32 = 8;

/// The backend's spill reservation for a stream, `MAX_STACK` of 64 words.
const STREAM_SPILL_BYTES: u32 = 64 * 8;

/// Calls nested deeper than this contribute nothing, as in the backend. The
/// surface language forbids recursion, so a verified module stays far below
/// it, and the guard only keeps a malformed module from recursing without
/// bound.
pub const MAX_CALL_DEPTH: usize = 64;

fn align(n: u32) -> u32 {
    n.checked_add(ALIGN - 1)
        .map_or(u32::MAX, |x| x & !(ALIGN - 1))
}

/// The packed length of a constant the backend places as a flat site, or
/// `None` for a constant it does not place.
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

/// The region bytes of chunk `chunk` and every chunk it calls.
fn total(module: &Module, chunk: usize, depth: usize) -> u32 {
    if depth > MAX_CALL_DEPTH {
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
    let spill = if stream { align(STREAM_SPILL_BYTES) } else { 0 };
    let mut t = align(sites(c).saturating_add(locals).saturating_add(spill));
    for op in &c.ops {
        if let Op::Call(idx, _) = op {
            t = align(t).saturating_add(total(module, usize::from(*idx), depth + 1));
        }
    }
    t
}

/// Whether a loop body both builds a flat composite and yields, the hazard
/// that makes the backend copy the yielded value.
fn has_hazard(c: &Chunk) -> bool {
    let mut open = alloc::vec::Vec::new();
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

/// The offset and size of the copy at the yield, when the backend makes one.
fn yield_copy_bytes(module: &Module, entry: usize) -> Option<(u32, u32)> {
    use crate::confine::{Confinement, Scope};
    let c = module.chunks.get(entry)?;
    if c.block_type != BlockType::Stream || !has_hazard(c) {
        return None;
    }
    let WireShape::Flat { size, .. } = module.signatures.get(entry)?.ret else {
        return None;
    };
    let n = crate::verify::max_resumes_before_reset(module, entry).ok()?;
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
    let confined = crate::confine::module_confinement(&clone)
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

/// The arena bytes a native compilation of `module` needs beyond the virtual
/// machine's figure, or zero for a module without an entry.
///
/// [`crate::vm::auto_arena_capacity_for`] adds this, so a host that sizes an
/// arena from that function needs no separate native term.
pub fn native_region_supplement_bytes(module: &Module) -> u32 {
    let Some(entry) = module.entry_point else {
        return 0;
    };
    match yield_copy_bytes(module, entry) {
        Some((offset, bytes)) => offset.saturating_add(bytes),
        None => total(module, entry, 0),
    }
}
