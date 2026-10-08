//! Private scalar fields use value slots, not packed target-word storage.
#![cfg(feature = "self-host")]

#[test]
fn wire_stream_state_is_priced_as_private_value_slots() {
    use keleusma::bytecode::{VALUE_SLOT_SIZE_BYTES, Value};
    let wire = include_str!("../src/selfhost/kel/wire.kel");
    let debug = include_str!("../src/selfhost/kel/wire_debug.kel");
    assert_eq!(core::mem::size_of::<Value>(), 32);
    assert_eq!(VALUE_SLOT_SIZE_BYTES, 32);
    for (stage, name, field, count, bytes) in [
        (wire, "framing", "mode", 4, 128),
        (wire, "opstream", "pool", 8, 256),
        (wire, "opread", "pending", 9, 288),
        (wire, "sigstream", "count", 12, 384),
        (wire, "nm", "native_at", 37, 1184),
        (wire, "templates", "count", 7, 224),
        (wire, "ck", "debug", 7, 224),
        (wire, "initialization", "active", 7, 224),
        (wire, "fl", "module", 18, 576),
        (debug, "sort", "mode", 15, 480),
        (debug, "emit", "phase", 9, 288),
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

#[test]
fn module_forest_scratch_is_priced_at_the_declared_word_width() {
    let stage = include_str!("../src/selfhost/kel/wire.kel");
    let declarations: Vec<_> = ["forest:", "forest_queue:", "forest_sizes:"]
        .into_iter()
        .map(|name| {
            stage
                .lines()
                .find(|line| line.trim_start().starts_with(name))
                .unwrap()
        })
        .collect();
    let source = format!(
        "require word >= 64; shared data scratch {{ {} }} fn main() -> Word {{ scratch.forest[0] = 1; 0 }}",
        declarations.join("\n")
    );
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(&source).unwrap()).unwrap();
    let module = keleusma::compiler::compile(&ast).unwrap();
    assert_eq!(
        module.data_layout.as_ref().unwrap().shared_layout.len(),
        8190 + 1365 * 2
    );
    assert_eq!(module.shared_data_bytes, 87_360);
}
