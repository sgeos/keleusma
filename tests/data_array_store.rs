//! A whole-array store into a data field is refused.
//!
//! # The defect
//!
//! An array field of a data block occupies one data slot per element, but
//! `d.a = [..]` lowered to a single `SetData` on the field's first slot. The
//! program compiled and verified. Measured on 2026-10-30 before the fix, a
//! private `[Word; 4]` read 0 from `d.a[3]` after `d.a = [1, 2, 3, 4]`, the
//! first element read back as the whole array and then faulted in arithmetic,
//! and a shared array faulted with `UnsupportedWidth`. A pykeleusma finding.
//!
//! # The rule
//!
//! A whole-array read of a data field is already refused, because of the same
//! layout. The write is now refused to match. Element writes and struct fields
//! are unchanged.
//!
//! # What these tests do NOT establish
//!
//! They do not add an element-wise lowering, which would make the form work.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn compiled(source: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = keleusma::lexer::tokenize(source).map_err(|e| format!("{e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("{e:?}"))?;
    keleusma::compiler::compile(&program).map_err(|e| e.message)
}

fn run(source: &str) -> Value {
    let module = compiled(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, &[]).expect("runs") {
        VmState::Finished(v) => v,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn a_whole_array_store_into_a_data_field_is_refused() {
    for source in [
        "private data d { a: [Word; 4] }\nfn main() -> Word { d.a = [1, 2, 3, 4]; d.a[3] }",
        "private data d { a: [Word; 4] }\nfn main() -> Word { let x = [5, 6, 7, 8]; d.a = x; d.a[3] }",
        "shared data d { a: [Word; 4] }\nfn main() -> Word { d.a = [1, 2, 3, 4]; d.a[3] }",
        "private data d { a: [Byte; 3] }\nfn main() -> Byte { d.a = [1 as Byte, 2 as Byte, 3 as Byte]; d.a[2] }",
        "private data d { a: [Word; 2], b: Word }\nfn main() -> Word { d.b = 5; d.a = [1, 2]; d.b }",
    ] {
        let message = compiled(source).expect_err(source);
        assert!(
            message.contains("is an array; assign its elements through"),
            "{source}: {message}"
        );
    }
}

#[test]
fn element_stores_and_struct_fields_still_work() {
    assert_eq!(
        run(
            "private data d { a: [Word; 4] }\nfn main() -> Word { d.a[0] = 1; d.a[3] = 4; d.a[0] * 10 + d.a[3] }"
        ),
        Value::Int(14)
    );
    assert_eq!(
        run(
            "struct P { x: Word, y: Word }\nprivate data d { p: P }\nfn main() -> Word { d.p = P { x: 3, y: 4 }; d.p.y }"
        ),
        Value::Int(4)
    );
}
