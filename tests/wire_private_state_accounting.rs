//! Private scalar fields use value slots, not packed target-word storage.
#![cfg(feature = "self-host")]

#[test]
fn wire_stream_state_is_priced_as_private_value_slots() {
    use keleusma::bytecode::{VALUE_SLOT_SIZE_BYTES, Value};
    let stage = include_str!("../src/selfhost/kel/wire.kel");
    assert_eq!(core::mem::size_of::<Value>(), 32);
    assert_eq!(VALUE_SLOT_SIZE_BYTES, 32);
    for (name, field, count, bytes) in [
        ("framing", "mode", 4, 128),
        ("opstream", "pool", 8, 256),
        ("sigstream", "count", 12, 384),
        ("nm", "native_at", 36, 1152),
    ] {
        let start = stage.find(&format!("private data {name} {{")).unwrap();
        let end = start + stage[start..].find('}').unwrap() + 1;
        let source = format!(
            "{}\nfn main() -> Word {{ {name}.{field} = 0; 0 }}",
            &stage[start..end]
        );
        let tokens = keleusma::lexer::tokenize(&source).unwrap();
        let ast = keleusma::parser::parse(&tokens).unwrap();
        let module = keleusma::compiler::compile(&ast).unwrap();
        assert_eq!(module.data_layout.as_ref().unwrap().slots.len(), count);
        assert_eq!(module.private_data_bytes, bytes);
        assert_eq!(
            keleusma::vm::required_persistent_capacity_for(&module),
            bytes as usize
        );
    }
}
