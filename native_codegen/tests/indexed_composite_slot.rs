//! **AN ARRAY-OF-COMPOSITE PRIVATE DATA FIELD, INDEXED.**
//!
//! # What was refused, and what changed
//!
//! `log.items[i]` on `private data log { items: [F; 3], .. }` was refused when the
//! direct case landed:
//!
//! > every element carries its own pool entry, and the stride is not proven
//! > uniform across the range here
//!
//! That was right at the time. Extrapolating `base + index * size` from the direct
//! case would have assumed a uniform stride nothing had checked.
//!
//! **`src/compiler.rs` places one pool entry per element in a single loop, each
//! advancing the running total by the same body size**, so the entries are
//! consecutive and uniformly spaced by construction. That is a fact about the
//! producer — and because it is checkable in the table this backend already
//! reads, it is CHECKED across the whole declared range rather than inherited
//! from the shape of the loop that built it. Two consecutive offsets prove
//! nothing about the third.
//!
//! # The property that needed its own subject
//!
//! **Every element has its own initialisation word.** A lowering that used the
//! base slot's word for the whole field would answer where the reference faults,
//! and *every test below would still pass*, because they all write the element
//! they read. The subject that separates the two writes element 0 and reads
//! element 2; it lives in `corpus_differential.rs` as a trap subject, because
//! proving the native side FAULTS needs its own process.
//!
//! # What remains unsupported
//!
//! - **A SHARED array of composites.** The shared layout describes scalar slots;
//!   a body operand reaching one is still refused by the width leg.
//! - **A non-uniform or incomplete pool range** is refused with the property that
//!   failed named. **No compiler can currently produce one**, so those refusals
//!   are reasoned rather than driven — stated here rather than implied away.
//! - **Nesting deeper than one array level** is whatever the compiler's slot
//!   assignment makes it; no subject here exercises an array of arrays of
//!   composites.

use keleusma_native::{LowerOptions, module_refusals};

mod common;

/// Two elements written at constant indices and both read back.
const CONSTANT_INDICES: &str = "\
struct F { a: Word, b: Word }\n\
private data log { items: [F; 3], count: Word }\n\
fn main(t: Word, u: Word) -> Word {\n\
    log.items[0] = F { a: t, b: t + 1 };\n\
    log.items[2] = F { a: t * 10, b: 0 };\n\
    log.items[0].a + log.items[2].a + u\n\
}\n";

/// Every element written through a loop, then one read at a RUNTIME index.
const RUNTIME_INDEX: &str = "\
struct F { a: Word, b: Word }\n\
private data log { items: [F; 3], count: Word }\n\
fn main(t: Word, u: Word) -> Word {\n\
    for i in 0..3 { log.items[i] = F { a: i * 10 + t, b: 0 }; }\n\
    log.items[u].a\n\
}\n";

#[test]
fn constant_indices_agree_with_the_reference() {
    let refusals = module_refusals(&common::build(CONSTANT_INDICES), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the indexed composite path must lower: {refusals:?}"
    );
    let (vm, nat) = common::vm_and_native_two_arg(CONSTANT_INDICES, 5, 1);
    assert_eq!(vm, 56, "5 + 50 + 1; if this moves the subject has changed");
    assert_eq!(nat, vm, "native={nat} vm={vm}");
}

/// **A RUNTIME index, so the address is computed rather than folded.**
///
/// With constant indices alone, an emitter that mis-scaled the stride could still
/// agree by accident on element 0, whose offset is the base.
#[test]
fn a_runtime_index_agrees_with_the_reference() {
    for u in 0..3i64 {
        let (vm, nat) = common::vm_and_native_two_arg(RUNTIME_INDEX, 5, u);
        assert_eq!(
            vm,
            u * 10 + 5,
            "the reference must see element {u}'s own body"
        );
        assert_eq!(nat, vm, "element {u}: native={nat} vm={vm}");
    }
}

/// **An unwritten sibling reads as its type's zero, on both sides.**
///
/// Until absorption 124 the reference faulted on a read of an element that a
/// sibling's write did not cover, and this test pinned that fault as the
/// counterpart of a trap subject in `corpus_differential.rs`. V0.2.X's widening
/// gives every element its type's zero at load, so the reference now reads that
/// zero, and native code must read the same zero from the installed image. Each
/// element keeps its own initialisation word, so the write to element 0 still
/// does not stand in for element 2.
#[test]
fn an_unwritten_sibling_element_reads_as_its_zero() {
    const SIBLING: &str = "\
struct F { a: Word, b: Word }\n\
private data log { items: [F; 3], count: Word }\n\
fn main(t: Word, u: Word) -> Word {\n\
    log.items[0] = F { a: t, b: 0 };\n\
    log.items[2].a * 100 + log.items[0].a + u\n\
}\n";
    let m = common::build(SIBLING);
    assert!(
        module_refusals(&m, LowerOptions::default()).is_empty(),
        "the subject must lower"
    );
    for (t, u) in [(4, 1), (-7, 3)] {
        let (vm, nat) = common::vm_and_native_two_arg(SIBLING, t, u);
        assert_eq!(vm, t + u, "the reference reads element 2 as the zero of F");
        assert_eq!(nat, vm, "t = {t}, u = {u}: native={nat} vm={vm}");
    }
}

/// The direct form still lowers, so nothing above was bought by loosening it.
#[test]
fn the_direct_form_is_unaffected() {
    const DIRECT: &str = "\
struct F { a: Word, b: Word }\n\
private data log { latest: F, count: Word }\n\
fn main(t: Word, u: Word) -> Word {\n\
    log.latest = F { a: t, b: t + 1 };\n\
    log.latest.a + u\n\
}\n";
    let (vm, nat) = common::vm_and_native_two_arg(DIRECT, 5, 1);
    assert_eq!(vm, 6);
    assert_eq!(nat, vm, "native={nat} vm={vm}");
}
