//! Indexing a struct field that is itself an array, `s.items[i]`.
//!
//! This shape was previously misrouted: the compiler assumed any
//! `identifier.field[index]` was a `data`-segment indexed access and
//! rejected a struct-typed receiver with "unknown data block". The fix
//! takes the data-segment route only when the base identifier is
//! actually a data block; a struct field falls through to the general
//! `Op::GetIndex` path, the same lowering as binding the field to a
//! local and indexing that.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run(src: &str) -> i64 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected finished integer, got {:?}", other),
    }
}

#[test]
fn struct_array_field_direct_index() {
    let src = "struct S { items: [Word; 3] }\n\
               fn get(s: S) -> Word { s.items[1] }\n\
               fn main() -> Word { get(S { items: [10, 20, 30] }) }";
    assert_eq!(run(src), 20);
}

#[test]
fn struct_array_field_multidimensional_index() {
    // A nested array field indexed to a scalar through two levels.
    let src = "struct G { grid: [[Word; 2]; 2] }\n\
               fn get(g: G) -> Word { g.grid[1][0] }\n\
               fn main() -> Word { get(G { grid: [[1, 2], [3, 4]] }) }";
    assert_eq!(run(src), 3);
}

#[test]
fn struct_array_field_checked_index() {
    // The checked-index construct over a struct field also resolves.
    let src = "struct S { items: [Word; 3] }\n\
               fn get(s: S, i: Word) -> Word { s.items[i] { ok(x) => x, invalid_index(_) => 0 } }\n\
               fn main() -> Word { get(S { items: [10, 20, 30] }, 2) }";
    assert_eq!(run(src), 30);
}

#[test]
fn data_segment_indexed_read_unaffected() {
    // The genuine data-segment indexed read must still take the
    // data-indexed path and return the constant element.
    let src = "const data cfg { table: [Word; 3] = [7, 8, 9] }\n\
               fn main() -> Word { cfg.table[1] }";
    assert_eq!(run(src), 8);
}
