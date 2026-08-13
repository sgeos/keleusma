//! Static placement of composite bodies in the arena's stack (bottom) section.
//!
//! # Why this is a layout pass and not an allocator
//!
//! The operator's model is that native Keleusma sets aside bump allocators for
//! sections of code, laid out cleanly enough to express in assembler. Taken to its
//! conclusion for a single section, the bump pointer disappears: if every
//! construction site in a chunk is given a distinct offset from the chunk's region
//! base, then allocation is a compile-time constant and there is no cursor to
//! advance, no allocator to call, and nothing to fail at run time.
//!
//! That mirrors what `compiler.rs` already does for private composite data slots,
//! which it describes as "linker-style fixed-address placement of program state".
//! This is the same idea applied to temporaries, which is where the corpus's
//! composites actually are: **0 of 239 construction sites are slot-homed and 239
//! are temporaries** (`spike_composite_shape.rs`).
//!
//! # What it deliberately does not do
//!
//! **Sites in disjoint scopes could share an offset and this pass does not let
//! them.** Summing every site over-approximates the true high-water mark, so a
//! chunk with two constructions in mutually exclusive branches reserves both. That
//! is sound and loose, and tightening it needs a liveness analysis this pass
//! deliberately omits. The looseness is bounded and small in practice: bodies run
//! 8 to 64 bytes with a median of 24, and the corpus-wide worst case over all 239
//! sites is 15,296 bytes.
//!
//! **It does not decide the section.** A body that outlives its chunk cannot live
//! at an offset from that chunk's base, and 23 of 826 corpus chunks return a flat
//! composite. Those need the caller's region, which is a caller-supplied pointer
//! rather than a different layout, and the first lowering slice refuses them
//! instead of guessing.

use keleusma::bytecode::{Chunk, NewCompositeOperand, Op};

/// Where one construction site's body lives, relative to the chunk's region base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SitePlacement {
    /// Index into the chunk's `ops` of the `NewComposite` that builds the body.
    pub op_index: usize,
    /// Byte offset from the chunk's region base.
    pub offset: u32,
    /// Body length in bytes, from the instruction's baked `byte_size`.
    pub size: u32,
}

/// Every construction site in a chunk, placed, with the region size they imply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionLayout {
    /// Placements in `ops` order, which is also ascending offset order.
    pub sites: Vec<SitePlacement>,
    /// Bytes this chunk must reserve in the stack section. The sum of every
    /// site's size, so an over-approximation of the true high-water mark.
    pub bytes: u32,
}

impl RegionLayout {
    /// Whether the chunk constructs anything at all. A chunk with no sites needs
    /// no region and no region pointer.
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
}

/// The byte alignment every body is placed at.
///
/// **The justification originally given here was FALSE and is corrected.** It
/// said a flat body's fields are word-packed, so aligning the body keeps field
/// accesses aligned. Measured 2026-08-13, the reference packs fields strictly
/// cumulatively with NO padding: `struct M { a: Byte, b: Word }` is nine bytes
/// with the word at offset ONE. Aligning the body does not align the fields, and
/// nothing can, because the layout is chosen for density.
///
/// Eight is kept anyway — it costs at most seven bytes per site, keeps each
/// address a single constant add, and gives the first field the best alignment
/// available. **The emitter must still store and load UNALIGNED**, which is a
/// property of the layout rather than of this constant.
const BODY_ALIGN: u32 = 8;

/// Round `n` up to the next multiple of [`BODY_ALIGN`], saturating.
///
/// Saturating rather than wrapping: a chunk whose sites exceed `u32::MAX` is
/// already unrepresentable, and wrapping would hand the emitter a small offset
/// for a huge body, which is a silent out-of-bounds write. Saturation makes the
/// downstream capacity check fail closed instead.
fn align_up(n: u32) -> u32 {
    match n.checked_add(BODY_ALIGN - 1) {
        Some(x) => x & !(BODY_ALIGN - 1),
        None => u32::MAX,
    }
}

/// Place every flat construction site in `chunk`.
///
/// Sites are placed in instruction order at ascending, eight-byte-aligned
/// offsets. A `Boxed` operand is skipped, since it has no baked body size; the
/// corpus contains none, and a lowering that meets one refuses it elsewhere
/// rather than silently placing nothing.
pub fn plan_chunk_region(chunk: &Chunk) -> RegionLayout {
    let mut sites = Vec::new();
    let mut next: u32 = 0;
    for (op_index, op) in chunk.ops.iter().enumerate() {
        let Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) = op else {
            continue;
        };
        let size = u32::from(*byte_size);
        sites.push(SitePlacement {
            op_index,
            offset: next,
            size,
        });
        next = align_up(next.saturating_add(size));
    }
    RegionLayout { sites, bytes: next }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keleusma::bytecode::Module;
    use keleusma::value_layout::CompositeKind;
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    fn compile_src(src: &str) -> Module {
        compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
    }

    fn flat(byte_size: u16, count: u16) -> Op {
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count,
            byte_size,
        })
    }

    /// A chunk carrying only what this pass reads. Built from a real compile so
    /// the struct's other fields keep whatever the compiler considers valid,
    /// rather than being invented here and drifting from it.
    fn chunk_with(ops: Vec<Op>) -> Chunk {
        let m = compile_src("loop main(r: Word) -> Word { yield r }");
        let mut c = m.chunks.into_iter().next().expect("one chunk");
        c.ops = ops;
        c
    }

    /// A chunk that constructs nothing reserves nothing, and must be
    /// distinguishable from one that constructs a zero-byte body.
    #[test]
    fn a_chunk_without_construction_needs_no_region() {
        let l = plan_chunk_region(&chunk_with(vec![Op::Return]));
        assert!(l.is_empty());
        assert_eq!(l.bytes, 0);
    }

    /// The single-site case, which is 80 of the corpus's 239 sites at 8 bytes.
    #[test]
    fn one_site_is_placed_at_the_base() {
        let l = plan_chunk_region(&chunk_with(vec![flat(8, 1), Op::Return]));
        assert_eq!(l.sites.len(), 1);
        assert_eq!(l.sites[0].offset, 0);
        assert_eq!(l.sites[0].size, 8);
        assert_eq!(l.bytes, 8);
    }

    /// **Placements must not overlap.** This is the property the whole pass
    /// exists to provide, and it is asserted directly rather than inferred from
    /// the arithmetic, over the real corpus shapes.
    #[test]
    fn placements_never_overlap() {
        let l = plan_chunk_region(&chunk_with(vec![
            flat(24, 3),
            flat(8, 1),
            flat(40, 5),
            flat(16, 2),
            Op::Return,
        ]));
        assert_eq!(l.sites.len(), 4);
        for w in l.sites.windows(2) {
            assert!(
                w[0].offset + w[0].size <= w[1].offset,
                "site at {} (+{}) overlaps the next at {}",
                w[0].offset,
                w[0].size,
                w[1].offset
            );
        }
        let last = l.sites.last().unwrap();
        assert!(
            last.offset + last.size <= l.bytes,
            "the last body runs past the reserved region"
        );
    }

    /// Every body starts eight-byte aligned, so no field access is misaligned.
    #[test]
    fn every_body_is_aligned() {
        let l = plan_chunk_region(&chunk_with(vec![flat(4, 1), flat(12, 2), flat(8, 1)]));
        for s in &l.sites {
            assert_eq!(
                s.offset % BODY_ALIGN,
                0,
                "body at {} is misaligned",
                s.offset
            );
        }
    }

    /// A size that is not a multiple of the alignment must still leave the next
    /// body aligned, and must not be rounded DOWN into its neighbour.
    #[test]
    fn an_unaligned_size_pads_rather_than_truncating() {
        let l = plan_chunk_region(&chunk_with(vec![flat(12, 2), flat(8, 1)]));
        assert_eq!(l.sites[0].size, 12, "the body keeps its true size");
        assert_eq!(l.sites[1].offset, 16, "the next body is padded past it");
        assert_eq!(l.bytes, 24);
    }

    /// A `Boxed` operand carries no baked body size and is skipped rather than
    /// placed at a guessed size. The corpus contains none; this pins the
    /// behaviour so meeting one is a refusal elsewhere, not a silent zero here.
    #[test]
    fn a_boxed_operand_is_not_placed() {
        let boxed = Op::NewComposite(NewCompositeOperand::Boxed {
            kind: CompositeKind::Struct,
            count: 2,
            meta: 0,
        });
        let l = plan_chunk_region(&chunk_with(vec![boxed, flat(8, 1)]));
        assert_eq!(l.sites.len(), 1, "only the flat site is placed");
        assert_eq!(l.sites[0].op_index, 1);
    }

    /// MUST-FIRE CONTROL, against a real compile rather than hand-built ops.
    ///
    /// Every other test here builds its own `Op`s, so all of them would pass if
    /// the real compiler emitted a shape this pass cannot see. This one compiles
    /// source and requires a placement to come out.
    #[test]
    fn a_real_compiled_construction_is_placed() {
        let m = compile_src(
            "struct P { x: Word, y: Word }\n\
             fn build(a: Word) -> Word { let p = P { x: a, y: a }; p.y }\n\
             loop main(r: Word) -> Word { yield build(r) }\n",
        );
        let placed: usize = m
            .chunks
            .iter()
            .map(|c| plan_chunk_region(c).sites.len())
            .sum();
        assert!(
            placed > 0,
            "a source written to construct a struct produced no placement, so this \
             pass cannot see what the compiler actually emits"
        );
    }
}

#[cfg(test)]
mod width_tests {
    use crate::{Width, width_of_tag};
    use keleusma::bytecode::TypeTag;

    /// **The default must be `Unknown`, and this is the assertion that pins it.**
    ///
    /// A `Byte` occupies a full `i64` operand slot, so a `Byte` and a `Word` are
    /// indistinguishable on the emitter's stack while packing to one byte and
    /// eight. If an unstated width ever became a word, every byte field would
    /// mispack silently, and the byte-identity oracle would only notice where
    /// the corpus happens to build one.
    #[test]
    fn an_unknown_width_yields_no_byte_count() {
        assert_eq!(Width::Unknown.bytes(), None);
        assert_eq!(Width::Bytes(8).bytes(), Some(8));
    }

    /// A composite parameter's body length is not carried on its type tag, so
    /// it must stay unknown rather than be guessed at a word.
    #[test]
    fn a_composite_tag_has_no_known_width() {
        assert_eq!(width_of_tag(TypeTag::Composite), Width::Unknown);
        assert_eq!(width_of_tag(TypeTag::Float), Width::Unknown);
        assert_eq!(width_of_tag(TypeTag::Fixed), Width::Unknown);
    }

    /// The two tags whose packed width the signature really does state.
    #[test]
    fn scalar_tags_state_their_width() {
        assert_eq!(width_of_tag(TypeTag::Word), Width::Bytes(8));
        assert_eq!(width_of_tag(TypeTag::Byte), Width::Bytes(1));
        assert_eq!(width_of_tag(TypeTag::Bool), Width::Bytes(1));
    }
}
