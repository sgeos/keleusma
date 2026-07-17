//! Conformance gate for the first slice of the self-hosted A.2.1 typed operand-stack verifier
//! (`compiler/kel/verify_typed.kel`, driven by `selfhost::typed_reject_*_via_kel`).
//!
//! The stage reconstructs the flat shape of each operand-stack entry by abstract interpretation
//! and validates every compiler-baked flat field/array offset against the composite's known size
//! (audit B1/B2), for the STRAIGHT-LINE prefix of a chunk checked in isolation. It is sound but
//! incomplete: it defers (accepts) at the first control-flow op or native call. Two oracles
//! bound it, against the reference `verify_typed`:
//!
//!   * POSITIVE: the five stage sources and small valid programs are accepted by the reference
//!     `typed_check_module`; the stage must reject none of their chunks (a spurious reject fails
//!     here). A valid program is always accepted regardless of control flow, since the reference
//!     accepts it and the stage's deferral only forgoes checks.
//!   * NEGATIVE: hand-built STRAIGHT-LINE chunks whose flat access is out of bounds are rejected
//!     by the stage, and the reference `typed_check_chunk` (isolation) rejects the same chunk.
//!     The violation is placed before any control flow, so the stage's straight-line prefix
//!     covers it.

use keleusma::bytecode::{
    Chunk, ConstValue, Module, NewCompositeOperand, Op, StructField, WireShape,
};
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::verify_typed::{typed_check_chunk, typed_check_module};
use keleusma_selfhost::selfhost::{
    compile_src, typed_reject_chunk_via_kel, typed_reject_module_via_kel,
};

// Host scalar widths (64-bit Word/Float): the module's declared widths for the base program.
const WB: usize = 8;
const FB: usize = 8;

fn read_stage(rel: &str) -> String {
    std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn base_module() -> Module {
    compile_src("fn main(r: Word) -> Word { r }")
}

/// A chunk (over the Func base) with `ops` and `constants` substituted, to feed both the stage
/// and the reference isolation check.
fn chunk_with(ops: Vec<Op>, constants: Vec<ConstValue>) -> Chunk {
    let mut c = base_module().chunks[0].clone();
    c.ops = ops;
    c.constants = constants;
    c
}

// ---- POSITIVE: reference-accepted modules must not be rejected -----------------------------

fn assert_module_accepted(label: &str, src: &str) {
    let m = compile_src(src);
    assert!(
        typed_check_module(&m, WB, FB).is_ok(),
        "{label}: the reference typed check must accept this module for the oracle to bind"
    );
    assert!(
        !typed_reject_module_via_kel(&m),
        "{label}: the typed stage must not reject a reference-accepted module"
    );
}

#[test]
fn stage_sources_are_typed_accepted() {
    for stage in [
        "kel/lexer.kel",
        "kel/parse.kel",
        "kel/reconstruct.kel",
        "kel/codegen.kel",
        "kel/analyze.kel",
    ] {
        assert_module_accepted(stage, &read_stage(stage));
    }
}

#[test]
fn valid_composite_programs_are_typed_accepted() {
    assert_module_accepted(
        "struct-field",
        "struct P { x: Word, y: Word }\n\
         fn main() -> Word { let p = P { x: 1, y: 2 }; p.x + p.y }",
    );
    assert_module_accepted(
        "nested-struct",
        "struct Q { p: (Word, Word), z: Word }\n\
         fn g(q: Q) -> Word { q.z }\n\
         fn main() -> Word { g(Q { p: (1, 2), z: 3 }) }",
    );
    assert_module_accepted(
        "enum-match",
        "enum E { A(Word), B }\n\
         fn f(e: E) -> Word { match e { E::A(v) => v, E::B => 0 } }\n\
         fn main() -> Word { f(E::A(5)) + f(E::B) }",
    );
}

// ---- POSITIVE (chunk level): a well-formed straight-line flat access is accepted ------------

#[test]
fn in_bounds_flat_field_is_accepted() {
    // Build a 16-byte struct of two Ints, read the field at offset 8 (need 8 + 8 = 16 <= 16).
    let ops = vec![
        Op::Const(0),
        Op::Const(0),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: 2,
            byte_size: 16,
        }),
        Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }),
    ];
    let chunk = chunk_with(ops, vec![ConstValue::Int(1)]);
    assert!(
        !typed_reject_chunk_via_kel(&base_module(), &chunk),
        "in-bounds flat field must be accepted by the stage"
    );
    assert!(
        typed_check_chunk(&chunk, WB, FB).is_ok(),
        "in-bounds flat field must be accepted by the reference"
    );
}

// ---- NEGATIVE: straight-line flat access out of bounds -------------------------------------

/// The stage must reject `ops`, and the reference isolation check must reject the same chunk.
fn assert_chunk_rejected(label: &str, ops: Vec<Op>, constants: Vec<ConstValue>) {
    let chunk = chunk_with(ops, constants);
    assert!(
        typed_reject_chunk_via_kel(&base_module(), &chunk),
        "{label}: the typed stage must reject this chunk"
    );
    assert!(
        typed_check_chunk(&chunk, WB, FB).is_err(),
        "{label}: the reference typed check must also reject this chunk"
    );
}

#[test]
fn flat_field_offset_out_of_bounds_is_rejected() {
    // Read a full word at offset 9 of a 16-byte body (need 9 + 8 = 17 > 16).
    assert_chunk_rejected(
        "field-oob",
        vec![
            Op::Const(0),
            Op::Const(0),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            }),
            Op::GetField(StructField::Flat {
                offset: 9,
                kind: ScalarKind::Int,
            }),
        ],
        vec![ConstValue::Int(1)],
    );
}

#[test]
fn nested_flat_field_out_of_bounds_is_rejected() {
    // Extract an 8-byte nested body at offset 12 of a 16-byte body (need 12 + 8 = 20 > 16).
    assert_chunk_rejected(
        "nested-oob",
        vec![
            Op::Const(0),
            Op::Const(0),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            }),
            Op::GetField(StructField::FlatNested {
                offset: 12,
                size: 8,
                variant: CompositeKind::Struct,
            }),
        ],
        vec![ConstValue::Int(1)],
    );
}

#[test]
fn newcomposite_packed_size_mismatch_is_rejected() {
    // Two Int elements pack to 16 bytes but the baked flat struct body claims 12
    // (NewCompositeSizeMismatch). All element sizes are known, so the check fires.
    assert_chunk_rejected(
        "newcomposite-size",
        vec![
            Op::Const(0),
            Op::Const(0),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 12,
            }),
        ],
        vec![ConstValue::Int(1)],
    );
}

#[test]
fn flat_field_on_a_scalar_is_rejected() {
    // A flat field access on a scalar operand is a type error (expected composite).
    assert_chunk_rejected(
        "field-on-scalar",
        vec![
            Op::Const(0),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ],
        vec![ConstValue::Int(1)],
    );
}

// ---- Frame-stack capability (slice 2a): checks past control flow, and height balance ---------

#[test]
fn flat_field_out_of_bounds_after_a_branch_is_rejected() {
    // An empty if/else, then construct a 16-byte struct and read a full word at offset 9 (17 >
    // 16). Slice 1 deferred at the If; the frame-stack interpreter checks the basic block after
    // the branch and rejects, as the reference does.
    assert_chunk_rejected(
        "flat-oob-after-branch",
        vec![
            Op::PushImmediate(1),
            Op::If(3),
            Op::Else(3),
            Op::EndIf,
            Op::Const(0),
            Op::Const(0),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            }),
            Op::GetField(StructField::Flat {
                offset: 9,
                kind: ScalarKind::Int,
            }),
        ],
        vec![ConstValue::Int(1)],
    );
}

// ---- Residuals: Call argument-vs-parameter check and exact composite-kind compatibility ------

#[test]
fn call_argument_shape_mismatch_is_rejected() {
    // `main` passes a scalar to `f`, but `f`'s parameter is mutated to a flat composite: a
    // scalar-vs-flat argument mismatch. Only seeding (the callee parameter shape) catches it.
    let mut m = compile_src(
        "fn f(x: Word) -> Word { x }\n\
         fn main() -> Word { f(3) }",
    );
    let fidx = m
        .chunks
        .iter()
        .position(|c| c.name == "f")
        .expect("f chunk");
    m.signatures[fidx].params[0] = WireShape::Flat {
        kind: CompositeKind::Struct.to_tag(),
        size: 16,
    };
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "the reference must reject the argument shape mismatch"
    );
    assert!(
        typed_reject_module_via_kel(&m),
        "the stage must reject the argument shape mismatch"
    );
}

#[test]
fn call_argument_kind_mismatch_same_size_is_rejected() {
    // `main` constructs a 16-byte struct and passes it to `f`, whose parameter is mutated to a
    // 16-byte ARRAY: same size, different composite kind. Only exact composite-kind tracking
    // (not size-only flat compatibility) catches this.
    let mut m = compile_src(
        "struct P { x: Word, y: Word }\n\
         fn f(p: P) -> Word { p.x }\n\
         fn main() -> Word { f(P { x: 1, y: 2 }) }",
    );
    let fidx = m
        .chunks
        .iter()
        .position(|c| c.name == "f")
        .expect("f chunk");
    m.signatures[fidx].params[0] = WireShape::Flat {
        kind: CompositeKind::Array.to_tag(),
        size: 16,
    };
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "the reference must reject the same-size different-kind argument"
    );
    assert!(
        typed_reject_module_via_kel(&m),
        "the stage must reject the same-size different-kind argument"
    );
}

// ---- Seeding (slice 2b): a composite-parameter field access caught only with seeding --------

#[test]
fn seeded_composite_param_field_out_of_bounds_is_rejected() {
    // `f` takes a 16-byte struct parameter and reads a field. Seeding makes the parameter's Flat
    // shape known, so a mutated out-of-bounds field offset is caught -- which the isolation check
    // (all-Top locals) defers. This is the flagship seeded check (Phase 2b).
    let mut m = compile_src(
        "struct P { x: Word, y: Word }\n\
         fn f(p: P) -> Word { p.x }\n\
         fn main() -> Word { f(P { x: 1, y: 2 }) }",
    );
    // Bump `f`'s flat field offset out of bounds (offset 12 + a word 8 = 20 > 16).
    let mut mutated = false;
    for c in &mut m.chunks {
        for op in &mut c.ops {
            if let Op::GetField(StructField::Flat { offset, .. }) = op {
                *offset = 12;
                mutated = true;
            }
        }
    }
    assert!(mutated, "expected a flat GetField in the compiled program");

    // The seeded reference and the seeded stage both reject.
    assert!(
        typed_check_module(&m, WB, FB).is_err(),
        "the seeded reference typed check must reject the out-of-bounds parameter field"
    );
    assert!(
        typed_reject_module_via_kel(&m),
        "the seeded stage must reject the out-of-bounds parameter field"
    );
    // Without seeding the parameter is Top, so the isolation check defers (does not reject) --
    // confirming the rejection comes from the seeding, not from an unrelated check.
    let fchunk = m
        .chunks
        .iter()
        .find(|c| {
            c.ops
                .iter()
                .any(|o| matches!(o, Op::GetField(StructField::Flat { .. })))
        })
        .expect("the chunk with the flat field");
    assert!(
        !typed_reject_chunk_via_kel(&m, fchunk),
        "the isolation check must defer the parameter field access (proving seeding is required)"
    );
}

#[test]
fn if_else_branch_height_mismatch_is_rejected() {
    // The then-arm leaves one value; the else-arm leaves none. The arms rejoin at different
    // operand heights (audit B3/B4). Pass 1/2 and the depth pass accept it (the depth pass maxes
    // the arms); only the typed height-balance join rejects.
    assert_chunk_rejected(
        "branch-height-mismatch",
        vec![
            Op::PushImmediate(1),
            Op::If(4),
            Op::Const(0),
            Op::Else(4),
            Op::EndIf,
            Op::Return,
        ],
        vec![ConstValue::Int(1)],
    );
}
