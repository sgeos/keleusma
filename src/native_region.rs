//! The buffers a native compilation of a module needs, for a host that runs
//! native code, and the region term of the combined figure for a host that
//! keeps one arena for either route.
//!
//! # Which function a host calls
//!
//! - A host that runs the virtual machine calls
//!   [`crate::vm::auto_arena_capacity_for`], which excludes the native region.
//! - A host that runs only native code calls [`native_route_buffers_for`](crate::native_region::native_route_buffers_for),
//!   which reports the composite region, the private buffer and the shared
//!   buffer, and none of the virtual machine's terms.
//! - A host that keeps one arena for either route calls
//!   [`crate::vm::auto_arena_capacity_for_vm_and_native`], which adds
//!   [`native_region_supplement_bytes`](crate::native_region::native_region_supplement_bytes) to the virtual machine's figure.
//!
//! # Provenance and authority
//!
//! The V0.3.X line's native backend reserves a composite region for each
//! chunk, a spill area and locals for the stream entry, and on some streams a
//! copy of the yielded value. That backend lives in the detached
//! `native_codegen` package, which this crate cannot depend on. The operator
//! ruled on question 22, confirmed first-hand in the V0.2.X session on
//! 2026-10-31, that the runtime's sizing figure include this supplement, so
//! that its value works for native allocation. On the operator's later ruling
//! that the sizing function right-size an arena, recorded in
//! `docs/decisions/ARENA_SIZING_ROUTES.md`, the combined figure moved to
//! [`crate::vm::auto_arena_capacity_for_vm_and_native`] and
//! [`crate::vm::auto_arena_capacity_for`] became the virtual machine's figure
//! alone.
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
//! # The private buffer
//!
//! The backend's private buffer is the virtual machine's persistent figure,
//! [`crate::vm::required_persistent_capacity_for`], plus
//! [`native_persistent_supplement_bytes`](crate::native_region::native_persistent_supplement_bytes), the backend's
//! `persistent_supplement_bytes`. That supplement is one word for the resume
//! state of each `Stream` chunk and one initialisation word for each private
//! composite pool slot, each count rounded up to the body alignment.
//!
//! # What this module does not report
//!
//! The backend's frame slot and machine stack figures depend on its lowering
//! and on the target, and only the backend computes them. A host linking the
//! backend's static library reads them from the library's header. The figures
//! here are for the 64-bit host target the backend measured, and they mean
//! something only for a module the backend lowers. A module it refuses, such
//! as one with a `Text` private slot, gets figures here that size nothing.

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
/// This is the native route's whole arena term, which
/// [`native_route_buffers_for`] reports as its region. The virtual machine's
/// [`crate::vm::auto_arena_capacity_for`] does not include it, and
/// [`crate::vm::auto_arena_capacity_for_vm_and_native`] adds it once, so a host
/// sizing from the combined function must not add it again.
///
/// The arithmetic saturates at `u32::MAX`, which no host allocation of a
/// verified module reaches, so an overflow over-states rather than truncates.
pub fn native_region_supplement_bytes(module: &Module) -> u32 {
    let Some(entry) = module.entry_point else {
        return 0;
    };
    match yield_copy_bytes(module, entry) {
        Some((offset, bytes)) => offset.saturating_add(bytes),
        None => total(module, entry, 0),
    }
}

/// The persistent bytes a native compilation adds after the virtual machine's
/// persistent figure, the backend's `persistent_supplement_bytes`.
///
/// First the gap by which the backend's private image exceeds that figure,
/// [`native_private_gap_bytes`], zero for a module without a packed private
/// array. Then one eight-byte word for the resume state of each `Stream` chunk in the
/// module, whether or not it is the entry, and one eight-byte initialisation
/// word for each private composite pool slot, which records whether the slot
/// has been written. Each of the two counts is rounded up to the body
/// alignment, and the sum saturates at `u32::MAX`.
pub fn native_persistent_supplement_bytes(module: &Module) -> u32 {
    let streams = module
        .chunks
        .iter()
        .filter(|c| c.block_type == BlockType::Stream)
        .count();
    let composite_slots = module.data_layout.as_ref().map_or(0, |dl| {
        // One initialisation word per pooled private ELEMENT, which is what the
        // per-element placement table counted before the field layout.
        dl.fields
            .iter()
            .filter(|f| {
                f.visibility == crate::bytecode::SlotVisibility::Private && f.offset.is_some()
            })
            .map(|f| f.count as usize)
            .sum::<usize>()
    });
    let words = |n: usize| align(u32::try_from(n).unwrap_or(u32::MAX).saturating_mul(8));
    native_private_gap_bytes(module)
        .saturating_add(words(streams))
        .saturating_add(words(composite_slots))
}

/// The bytes by which the native backend's private image exceeds the virtual
/// machine's persistent figure, [`native_private_base_bytes`] less
/// [`crate::vm::required_persistent_capacity_for`], saturating at zero and at
/// `u32::MAX`.
///
/// Packed private storage made the virtual machine's figure smaller than the
/// backend's image, whose private elements each keep a word. The backend's
/// published contract sizes its private buffer as the virtual machine's figure
/// plus its persistent supplement, so the gap is carried in the supplement,
/// and that contract keeps sizing the same buffer it sized before packing.
#[must_use]
pub fn native_private_gap_bytes(module: &Module) -> u32 {
    let gap = native_private_base_bytes(module)
        .saturating_sub(crate::vm::required_persistent_capacity_for(module));
    u32::try_from(gap).unwrap_or(u32::MAX)
}

/// The buffers a host that runs `module` only as native code allocates, in
/// bytes, as [`native_route_buffers_for`] reports them.
///
/// Each field is one buffer the backend's entry takes, each aligned to the
/// backend's buffer alignment. One allocation carved into these, or separate
/// allocations, is the host's choice. None of the virtual machine's terms is
/// here, because native code keeps its operands in machine values, holds no
/// call frames or opaque registry in an arena, places composite bodies in the
/// region, and reads a composite host input through the host's own pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeRouteBuffers {
    /// The composite region, [`native_region_supplement_bytes`], the backend's
    /// `host_arena_supplement_bytes` and its header's region figure.
    pub region_bytes: usize,
    /// The private buffer, [`crate::vm::required_persistent_capacity_for`]
    /// plus [`native_persistent_supplement_bytes`], the header's private
    /// figure. The backend's private initialisation image is copied into its
    /// start.
    pub private_bytes: usize,
    /// The shared buffer, [`crate::vm::shared_data_bytes_for`], the same
    /// buffer the virtual machine borrows.
    pub shared_bytes: usize,
}

/// The persistent bytes the native backend lays its private image out
/// against, before its supplement.
///
/// The backend stores every private element in a word of its own, followed by
/// the composite pool, an image of its own choosing that no external consumer
/// observes. It places its stream resume state and its composite
/// initialisation words after this figure. Until packed private storage, S1
/// of the array wire-slot design, the figure was the virtual machine's own
/// [`crate::vm::required_persistent_capacity_for`], then 32 bytes per private
/// element plus the pool, which was never smaller than the backend's image.
/// Packed private storage made the virtual machine's figure smaller than the
/// backend's image for a packed array of any element narrower than a word and
/// for a private array of composites, so the backend keeps the element-slot
/// figure, and every native memory image and offset stays as it was.
///
/// The figure is the private element count times the bundled runtime's
/// `Value` size, plus the persistent composite pool. It saturates. On a
/// 64-bit target that size is pinned at 32 bytes by the `const` assertion
/// beside `Value` in `src/bytecode.rs`, so the figure cannot drift with a
/// host's layout there. That assertion is scoped to a 64-bit pointer width,
/// and on a 32-bit target the slot and therefore this figure are smaller.
#[must_use]
pub fn native_private_base_bytes(module: &Module) -> usize {
    let elements = module
        .data_layout
        .as_ref()
        .map_or(0, crate::bytecode::DataLayout::private_slot_count);
    elements
        .saturating_mul(core::mem::size_of::<crate::bytecode::Value>())
        .saturating_add(module.persistent_composite_bytes as usize)
}

/// The buffers for a host that runs `module` only as native code.
///
/// This is the native route's sizing function. It is deliberately not a
/// single arena capacity, so that it cannot be passed where
/// [`crate::vm::auto_arena_capacity_for`] is expected. A host that runs the
/// virtual machine calls that function instead, and a host that keeps one
/// arena for either route calls
/// [`crate::vm::auto_arena_capacity_for_vm_and_native`].
///
/// # Failure modes
///
/// It does not fail. Every figure is computed from the module alone, and the
/// region and supplement arithmetic saturates. It does not check that the
/// backend lowers the module, so the figures of a module the backend refuses
/// size nothing. The frame slot and machine stack are not reported, as the
/// module documentation explains. The formula is a copy of the backend's, and
/// the backend stays the authority if the two disagree.
pub fn native_route_buffers_for(module: &Module) -> NativeRouteBuffers {
    NativeRouteBuffers {
        region_bytes: native_region_supplement_bytes(module) as usize,
        private_bytes: crate::vm::required_persistent_capacity_for(module)
            .saturating_add(native_persistent_supplement_bytes(module) as usize),
        shared_bytes: crate::vm::shared_data_bytes_for(module),
    }
}
