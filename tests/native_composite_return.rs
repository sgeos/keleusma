#![cfg(all(feature = "compile", feature = "verify"))]
//! B37 residuals: unsignatured-native returns of text-bearing composites across
//! the composite kinds, complementing the tuple case in `flat_ref_tuple.rs`.
//!
//! A native builds its result with no arena, so a text field is a `StaticStr`
//! and the composite body is boxed. The compiler bakes flat construction and
//! flat access for the declared return type (text is a flat `(ptr, len)` field
//! at the host word width), so the native-result canonicalisation must promote
//! the `StaticStr` field to an arena `KStr` and pack the body flat, or the
//! access mismatches the body. These tests pin that behaviour for struct,
//! array, enum, and `Option<Text>` returns.

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use keleusma::bytecode::EnumBody;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

#[test]
fn native_struct_with_text_field_flattens_and_reads() {
    // A native returns a `Sensor { name: Text, id: Word }`. The body is boxed
    // with a `StaticStr` name; the compiler bakes flat struct field access.
    // The canonicalisation promotes `name` to a `KStr` and packs the struct
    // flat, so `s.id` and `s.name` read back. id 10 plus len("abcd") 4 is 14.
    let src = "use sensor() -> Sensor\n\
               use tlen(Text) -> Word\n\
               struct Sensor { name: Text, id: Word }\n\
               fn main() -> Word { let s = sensor(); s.id + tlen(s.name) }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("sensor", |_| {
        Ok(Value::struct_value(
            String::from("Sensor"),
            vec![
                (String::from("name"), Value::StaticStr(String::from("abcd"))),
                (String::from("id"), Value::Int(10)),
            ],
        ))
    });
    vm.register_fn("tlen", |s: String| -> i64 { s.len() as i64 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(14)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn native_array_of_text_flattens_and_indexes() {
    // A native returns `[Text; 2]` of `StaticStr`. Each element promotes to a
    // `KStr` and the array packs flat. len("ab") 2 plus len("cde") 3 is 5.
    let src = "use lines() -> [Text; 2]\n\
               use tlen(Text) -> Word\n\
               fn main() -> Word { let a = lines(); tlen(a[0]) + tlen(a[1]) }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("lines", |_| {
        Ok(Value::array(vec![
            Value::StaticStr(String::from("ab")),
            Value::StaticStr(String::from("cde")),
        ]))
    });
    vm.register_fn("tlen", |s: String| -> i64 { s.len() as i64 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(5)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn native_enum_largest_text_variant_flattens_and_matches() {
    // A native returns the `Note(Text)` variant of `Msg { Note(Text),
    // Code(Word) }`. `Note` carries the largest payload (a two-word Text),
    // so the body the canonicalisation packs with `min_payload == 0` happens
    // to equal the compiler's `word + payload_max` size and the match reads
    // back. len("hey") is 3. A smaller variant of the same enum is the
    // documented residual below.
    let src = "use note() -> Msg\n\
               use tlen(Text) -> Word\n\
               enum Msg { Note(Text), Code(Word) }\n\
               fn main() -> Word { let m = note(); match m { Msg::Note(s) => tlen(s), Msg::Code(n) => n } }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("note", |_| {
        Ok(Value::Enum(EnumBody::boxed(
            String::from("Msg"),
            String::from("Note"),
            vec![Value::StaticStr(String::from("hey"))],
        )))
    });
    vm.register_fn("tlen", |s: String| -> i64 { s.len() as i64 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(3)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
#[ignore = "B37 residual: simple EnumBody::boxed loses the variant discriminant and \
            padding, so a non-first, non-largest enum variant returned by an \
            unsignatured native silently misreads. A native can work around it with \
            EnumBody::boxed_with_layout(disc, min_payload); the signatured-native \
            direction supplies both automatically."]
fn native_enum_smaller_later_variant_known_limitation() {
    // Documents a residual of the flatten direction for non-Option enums.
    // `Code(Word)` is the second variant (discriminant 1) with a smaller payload
    // than the first variant `Note(Text)`. The simple `EnumBody::boxed`
    // constructor records discriminant 0 and no largest-variant padding hint,
    // whereas the compiler bakes access against discriminant 1 and a
    // `word + payload_max` body. The discriminant disagreement makes the match
    // silently select `Note`, so this returns 0 rather than 7 (it does not even
    // error). A native that supplies `boxed_with_layout(disc = 1,
    // min_payload = <Text size>)` packs the correct body; the signatured-native
    // direction recovers the enum type and does this automatically. Marked
    // ignore until one of those lands.
    let src = "use code() -> Msg\n\
               enum Msg { Note(Text), Code(Word) }\n\
               fn main() -> Word { let m = code(); match m { Msg::Note(_) => 0, Msg::Code(n) => n } }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("code", |_| {
        Ok(Value::Enum(EnumBody::boxed(
            String::from("Msg"),
            String::from("Code"),
            vec![Value::Int(7)],
        )))
    });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn native_option_text_return_matches() {
    // A native returns `Option<Text>`. `Option` is kept boxed by both the
    // compiler (baked boxed access) and the canonicalisation (the `Option`
    // arm), so the two agree without flattening. This pins that the boxed
    // path destructures correctly. len("hello") is 5.
    let src = "use getopt() -> Option<Text>\n\
               use tlen(Text) -> Word\n\
               fn main() -> Word { match getopt() { Option::Some(s) => tlen(s), Option::None => 0 } }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("getopt", |_| {
        Ok(Value::Enum(EnumBody::boxed(
            String::from("Option"),
            String::from("Some"),
            vec![Value::StaticStr(String::from("hello"))],
        )))
    });
    vm.register_fn("tlen", |s: String| -> i64 { s.len() as i64 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(5)),
        other => panic!("expected finished, got {:?}", other),
    }
}
