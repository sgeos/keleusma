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
//! composites actually are.
//!
//! # ⚠ THE FIGURE THIS PARAGRAPH CARRIED WAS STALE AND HAD NO PRODUCER
//!
//! The superseded text read: *"0 of 239 construction sites are slot-homed and 239
//! are temporaries (`spike_composite_shape.rs`)"*. **That spike no longer reports
//! a 239 at all** — it reports chunks returning and taking composites — so the
//! number was carried rather than measured, and the handoff simultaneously quoted
//! **256 in 35 chunks** without either saying which population it meant.
//!
//! **Current, with its population named: 256 sites across 35 chunks of the
//! FOUR-ROOT corpus's 69 compiling modules**, agreed independently by the planner
//! and by a raw scan of the instruction stream (`site_count_agreement.rs`). A
//! three-root walk reports a different number about a different set.
//!
//! **The "0 slot-homed" half is NOT re-derived here and is not restated as
//! fact.** It needs its own walk, and asserting it on the strength of the stale
//! sentence it came from would be the same error one level down.
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
///
/// # ⚠ NO SITE IS EVER REUSED, AND THAT IS NOT ONLY A COST
///
/// Every static construction site gets its own offset. Nothing here consults
/// liveness, escape, aliasing, or confinement — the words do not appear in this
/// file — so the region is `sites x size` where the runtime's verified figure is
/// `peak_live x size`. **That difference is the measured arena bound gap: 11 of
/// 71 corpus modules demand more arena than the verified heap figure, 24 to 96
/// bytes each, and the gap is unbounded in static site count.**
///
/// It has always been recorded as a cost. **It is also the reason a wrong
/// confinement verdict cannot miscompile anything on this line**, which is worth
/// stating in the same place.
///
/// A planner that reuses a slot needs to be RIGHT that the previous occupant is
/// dead. This one never reuses, so it never needs a verdict at all — not a
/// correct one, not a conservative one, not any. **The conservatism that costs
/// the bytes is what buys the immunity.**
///
/// **AND THAT NON-REUSE IS NOW ENFORCED, not merely stated here.**
/// `region_nonreuse.rs` fails if any chunk plans two sites into overlapping
/// storage — on RANGES, not offsets, since distinct offsets can still overlap.
/// It was prose in four places and asserted in none until 2026-08-27, in a
/// codebase where this line had spent three iterations arguing for a reusing
/// planner. **The guard covers WITHIN-chunk reuse only**; cross-chunk collision
/// is the separate recorded defect in `composite_return_aliasing.rs`.
///
/// # WHOEVER CLOSES THE GAP IS BUYING BOTH HALVES
///
/// That is the point of putting this here rather than in a decision document.
/// Overlapping offsets for mutually exclusive or confined sites is the obvious
/// win and it **takes on an exposure that does not exist today**: from that
/// commit onward, a confinement verdict that is wrong in the unsafe direction is
/// a miscompile rather than a wasted byte.
///
/// Two facts about the verdicts that would be consumed, both established by
/// measurement on the `v0.2.3` line rather than argued here:
///
/// 1. **A `Confined` verdict is sound to trust; an `Escapes` verdict is only an
///    upper bound.** Their escape count fell from 12 to 10 when callee summaries
///    landed, and those two were **wrong rather than merely unestablished**. A
///    conservative default hides false positives exactly as well as it hides
///    gaps, and there is no third value to record one in.
/// 2. **Neither line can yet say "confined to the CALLER'S iteration" about a
///    region a helper built and returned.** Their per-chunk scoping reports such
///    a site as escaping in the callee and the caller carries no site at all.
///    Sound, and permanently pessimistic. A planner that overlaps on confinement
///    gets nothing for this shape today.
///
/// So the gap is real and closing it is right, but **it is not free and it is not
/// only about bytes.** See `docs/decisions/NATIVE_BOUNDS_TRANSFER.md` and the
/// `v0.3.0` handoff's Workstream E notes.
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

/// One `NewComposite` site that this pass places at a fixed offset, together
/// with a `Yield` that can carry the value it builds out to the host while a
/// later iteration overwrites that offset in place.
///
/// See [`yield_escape_hazards`] for what is and is not detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YieldEscapeHazard {
    /// Index into the chunk's `ops` of the construction site.
    pub site_op: usize,
    /// Index into the chunk's `ops` of a `Yield` in the same scope.
    pub yield_op: usize,
    /// The enclosing `Loop` scope, as a half-open `[lo, hi)` range over `ops`.
    pub scope: (usize, usize),
}

/// Half-open `[lo, hi)` bodies of every `Loop` scope, innermost and outermost
/// alike, paired with the scope's exit label.
///
/// Nesting is handled with a stack, so a site inside a `match` inside a `for`
/// appears in BOTH scopes. That is what makes the enclosing-loop question
/// answerable without a separate nesting walk.
fn loop_scopes(ops: &[Op]) -> Vec<(usize, usize, u16)> {
    let mut open: Vec<(usize, u16)> = Vec::new();
    let mut out = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::Loop(a) => open.push((i, *a)),
            Op::EndLoop(_) => {
                if let Some((s, a)) = open.pop() {
                    out.push((s + 1, i, a));
                }
            }
            _ => {}
        }
    }
    out
}

/// Op indices of every `Flat` construction site that lies inside some `Loop`
/// scope, and whose offset is therefore rewritten on each iteration.
///
/// This is the population the cross-iteration question is asked of. It says
/// nothing about escape — [`yield_escape_hazards`] answers that for the
/// single-chunk case, and the interprocedural case needs a call graph this
/// function deliberately does not build.
///
/// Every `Loop` scope counts, including the ones the compiler emits for `match`
/// and multi-clause dispatch, for the reason given on [`yield_escape_hazards`]:
/// a real loop containing `break` cannot be told from a `match` by that test.
pub fn loop_body_sites(chunk: &Chunk) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (lo, hi, _exit) in loop_scopes(&chunk.ops) {
        for (k, op) in chunk.ops[lo..hi].iter().enumerate() {
            if matches!(op, Op::NewComposite(NewCompositeOperand::Flat { .. }))
                && !out.contains(&(lo + k))
            {
                out.push(lo + k);
            }
        }
    }
    out.sort_unstable();
    out
}

/// Sites whose fixed offset can be overwritten while the host still holds the
/// value built there.
///
/// # The defect this detects
///
/// [`plan_chunk_region`] gives each site ONE offset for the life of the chunk,
/// so a site in a loop body writes the same bytes on every iteration. After
/// B28 a composite is `FlatComposite::Arena(ArenaHandle<[u8]>)` — a pointer and
/// length, not a copy — and the epoch it carries is advanced only by a `RESET`.
/// **An overwrite in place advances nothing**, so a host holding iteration `n`'s
/// handle calls `resolve`, succeeds, and reads iteration `n+1`'s bytes. The
/// failure is a silently wrong value rather than a `Stale` error, which is why
/// it must be refused at compile time instead of caught at run time.
///
/// Established against the runtime by the `v0.2.3` line and recorded in
/// `docs/proofs/COMPOSITE_REGION_REUSE.md` §4.1.1.
///
/// # What this over-approximates, and in which direction
///
/// Every relaxation here reports MORE hazards than exist, never fewer, so a
/// caller that refuses on a non-empty result cannot be argued into emitting the
/// defect:
///
/// - **Every `Loop` scope counts as iterating.** `Op::Loop` is a break-scope
///   marker that the compiler also emits for `match` and for multi-clause
///   dispatch, and those run once. Excluding them would be the natural
///   precision win and it is NOT taken: a real loop containing `break` is
///   indistinguishable from a `match` by that test, and excluding it would drop
///   a genuinely iterating scope.
/// - **Any `Yield` in the scope counts**, including one textually before the
///   site, because control returns to the top of the body and reaches it on the
///   next iteration.
/// - **No data flow is traced** from the site to the `Yield`. A site whose value
///   demonstrably never reaches the yield is still reported.
///
/// # What it does NOT detect, stated rather than implied
///
/// **The interprocedural case is open.** A composite built in a loop body,
/// returned to a caller, and yielded THERE is a hazard this function cannot
/// see, because it reads one chunk. Callers must not read an empty result as
/// "this module is free of the defect"; it means "no single-chunk instance".
/// Bounding the residual is the `Call` disqualifier in
/// `tests/loop_composite_census.rs`.
pub fn yield_escape_hazards(chunk: &Chunk) -> Vec<YieldEscapeHazard> {
    let mut out: Vec<YieldEscapeHazard> = Vec::new();
    // `loop_scopes` completes an inner scope before the outer one that contains
    // it, so the FIRST report for a site is its innermost enclosing scope. One
    // site enclosed by several scopes is ONE hazard, not one per scope; without
    // this a cost census would count the same defect twice and overstate what
    // refusing costs.
    for (lo, hi, _exit) in loop_scopes(&chunk.ops) {
        let body = &chunk.ops[lo..hi];
        let Some(yield_at) = body.iter().position(|o| matches!(o, Op::Yield)) else {
            continue;
        };
        for (k, op) in body.iter().enumerate() {
            if !matches!(op, Op::NewComposite(NewCompositeOperand::Flat { .. })) {
                continue;
            }
            let site_op = lo + k;
            if out.iter().any(|h| h.site_op == site_op) {
                continue;
            }
            out.push(YieldEscapeHazard {
                site_op,
                yield_op: lo + yield_at,
                scope: (lo, hi),
            });
        }
    }
    out
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

    /// A site enclosed by BOTH an inner `match` scope and an outer loop that
    /// yields is one defect, and must be reported once.
    ///
    /// Built from ops directly because the shape needed -- a construction inside
    /// a nested scope, with the `Yield` in the outer one -- is awkward to
    /// provoke from source, and the property under test is about the walk rather
    /// than about the compiler.
    #[test]
    fn a_site_under_nested_scopes_is_one_hazard_not_one_per_scope() {
        let c = chunk_with(vec![
            Op::Loop(1),    // 0  outer, iterating
            Op::Loop(2),    // 1  inner, match-like
            flat(8, 1),     // 2  the site
            Op::EndLoop(2), // 3
            Op::Yield,      // 4  reached on the next iteration
            Op::EndLoop(1), // 5
            Op::Return,     // 6
        ]);
        let h = yield_escape_hazards(&c);
        assert_eq!(h.len(), 1, "one site, one hazard, got {h:?}");
        assert_eq!(h[0].site_op, 2);
        assert_eq!(h[0].yield_op, 4);
        // The INNER scope has no `Yield`, so the hazard must be attributed to
        // the outer one. Attributing it inward would name a scope in which the
        // value cannot escape.
        assert_eq!(h[0].scope, (1, 5));
    }

    /// The predicate must be able to return nothing, or the dedupe test above
    /// proves only that it returns one of something.
    #[test]
    fn a_loop_without_a_yield_reports_no_hazard() {
        let c = chunk_with(vec![Op::Loop(1), flat(8, 1), Op::EndLoop(1), Op::Return]);
        assert!(yield_escape_hazards(&c).is_empty());
    }

    /// A chunk that constructs nothing reserves nothing, and must be
    /// distinguishable from one that constructs a zero-byte body.
    #[test]
    fn a_chunk_without_construction_needs_no_region() {
        let l = plan_chunk_region(&chunk_with(vec![Op::Return]));
        assert!(l.is_empty());
        assert_eq!(l.bytes, 0);
    }

    /// The single-site case, which was 80 of the corpus's 239 sites at 8 bytes
    /// when this test was written.
    ///
    /// **DATED, NOT PINNED, and deliberately not re-derived.** No instrument
    /// reports a corpus-wide site total, so this figure has no cheap re-derivation
    /// path. **Its role is motivation** -- it says why the single-site case is
    /// worth a test -- and the test is correct at any denominator. A reader who
    /// needs a current number must count, and should not assume this one is it.
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
        assert_eq!(Width::Scalar(8).bytes(), Some(8));
    }

    /// **The distinction the byte count alone cannot make.** An eight-byte
    /// nested body and a `Word` agree on `bytes()` and differ on everything the
    /// emitter must do: one is stored, the other copied from the address the
    /// operand holds. Storing a body would write the pointer into the parent
    /// while every downstream offset still looked correct.
    #[test]
    fn a_body_and_a_scalar_of_equal_size_are_distinguishable() {
        let scalar = Width::Scalar(8);
        let body = Width::Body(8);
        assert_eq!(scalar.bytes(), body.bytes(), "equal size is the hazard");
        assert!(!scalar.is_body());
        assert!(body.is_body());
        assert_ne!(scalar, body);
    }

    /// **THIS ASSERTION USED TO BUNDLE THREE TAGS UNDER ONE RATIONALE, and the
    /// rationale covered exactly one of them.**
    ///
    /// It read "a composite parameter's body length is not carried on its type
    /// tag" and then asserted `Composite`, `Float` AND `Fixed` were all unknown.
    /// That sentence is true of `Composite` and says nothing about the others.
    /// `Float` had a good reason living elsewhere; **`Fixed` had no stated
    /// reason anywhere**, and it was measurably wrong — the reference packs a
    /// `Fixed` field at eight bytes, exactly like a `Word`.
    ///
    /// **A bundle inherits the credibility of its best-justified member.** Each
    /// tag now carries its own reason, so a wrong one cannot hide behind a right
    /// one.
    #[test]
    fn a_composite_body_length_is_not_on_its_tag() {
        assert_eq!(width_of_tag(TypeTag::Composite), Width::Unknown);
    }

    /// `Float` stays unknown because this backend has NO float representation.
    ///
    /// Redundant with the module-level guard, which refuses a float by every
    /// route it can take, and kept deliberately as a second line: the guard is
    /// about admission, this is about what a width would mean if one arrived.
    #[test]
    fn a_float_tag_has_no_width_because_it_has_no_representation() {
        assert_eq!(width_of_tag(TypeTag::Float), Width::Unknown);
    }

    /// The tags whose packed width the signature really does state.
    ///
    /// **`Fixed` belongs here and did not until 2026-08-21.** A Q-format value
    /// is an `i64` of fixed-point bits and occupies a full slot; measured, the
    /// reference packs `struct { a: Fixed<16>, b: Fixed<16> }` at `byte_size:
    /// 16`, identical to a pair of `Word`s.
    #[test]
    fn scalar_tags_state_their_width() {
        assert_eq!(width_of_tag(TypeTag::Word), Width::Scalar(8));
        assert_eq!(width_of_tag(TypeTag::Fixed), Width::Scalar(8));
        assert_eq!(width_of_tag(TypeTag::Byte), Width::Scalar(1));
        assert_eq!(width_of_tag(TypeTag::Bool), Width::Scalar(1));
    }
}

/// Total region bytes a chunk needs INCLUDING every callee it can reach.
///
/// # Why the transitive figure, and why it terminates
///
/// A callee writes its own flat sites at offsets it plans from zero. If the
/// caller hands it the same region base it uses itself, the two collide — and
/// worse, two calls to ONE callee collide with each other, which is the
/// `10_multbyte.kel` defect: `p[0]` reads `r[0]`'s value.
///
/// So each call site is given a DISJOINT BLOCK of the caller's region, big
/// enough for everything the callee can reach. That is the authorised
/// caller-allocated return slot expressed through the pointer the caller
/// already passes: the callee never names an arena, it writes through an
/// address its caller chose.
///
/// The recursion terminates because the type checker rejects direct and mutual
/// recursion, so the call graph is acyclic — the same property the native stack
/// bound leans on. An unresolvable callee index contributes zero rather than
/// panicking; `lower_module` refuses such a module elsewhere.
pub fn region_total_bytes(
    module: &keleusma::bytecode::Module,
    chunk_index: usize,
    depth: usize,
) -> u32 {
    // Depth guard: acyclicity is a language property, and this is a library
    // walking bytecode that may not have come from the compiler.
    if depth > 64 {
        return 0;
    }
    let Some(chunk) = module.chunks.get(chunk_index) else {
        return 0;
    };
    let mut total = plan_chunk_region(chunk).bytes;
    for op in &chunk.ops {
        if let Op::Call(idx, _) = op {
            total = align_up(total).saturating_add(region_total_bytes(
                module,
                usize::from(*idx),
                depth + 1,
            ));
        }
    }
    total
}

/// **The arena bytes a host must ADD when running this module natively.**
///
/// # Why this exists, and what it closes
///
/// `keleusma::vm::auto_arena_capacity_for` is the documented way for a host to
/// size an arena, and it returns the sum of exactly four terms: operand-stack
/// bytes, call-frame bytes, the module's auxiliary arena bytes, and the
/// verifier's `max_heap_bytes`. **None of them is this backend's composite
/// region.** Measured over the corpus, eleven modules demand MORE than
/// `max_heap_bytes` — the only term that could plausibly have covered composite
/// bodies — so it does not bound this pool. The virtual machine puts composite
/// bodies in the arena's TOP region; the backend takes its region from the
/// BOTTOM section and gives every call site a DISJOINT block.
///
/// This function publishes the missing figure so a host can close the gap today,
/// with no runtime change.
///
/// # THIS IS A WEAKER GUARANTEE THAN THE RUNTIME RETURNING IT
///
/// A figure the host must remember to ADD is not the same as one the sizing
/// function includes. **A host that calls only `auto_arena_capacity_for` is
/// still under-provisioned for native execution**, and nothing here changes
/// that. Whether the runtime should absorb this term is an arena-accounting
/// question that belongs to the operator; this crate publishing what it needs
/// does not settle it.
///
/// # What the figure is rooted at
///
/// The module's ENTRY chunk, transitively through calls, because that is the
/// call a host scopes a region for. A module with no entry point requires
/// nothing here, since there is no call to scope.
///
/// Bytes, taken from the same planner the lowering itself uses. There is
/// deliberately no second region model: a parallel one would measure the model.
pub fn host_arena_supplement_bytes(module: &keleusma::bytecode::Module) -> u32 {
    match module.entry_point {
        Some(entry) => region_total_bytes(module, entry, 0),
        None => 0,
    }
}

/// Per-call-site region offsets for one chunk, in instruction order.
///
/// The chunk's own flat sites occupy `[0, plan_chunk_region(chunk).bytes)`;
/// each `Op::Call` then receives a disjoint block sized by
/// [`region_total_bytes`] of its callee. Returned as `(op_index, offset)` so the
/// emitter can hand the callee `region_base + offset`.
pub fn plan_call_site_regions(
    module: &keleusma::bytecode::Module,
    chunk_index: usize,
) -> Vec<(usize, u32)> {
    let Some(chunk) = module.chunks.get(chunk_index) else {
        return Vec::new();
    };
    let mut next = align_up(plan_chunk_region(chunk).bytes);
    let mut out = Vec::new();
    for (op_index, op) in chunk.ops.iter().enumerate() {
        if let Op::Call(idx, _) = op {
            out.push((op_index, next));
            next = align_up(next.saturating_add(region_total_bytes(module, usize::from(*idx), 0)));
        }
    }
    out
}
