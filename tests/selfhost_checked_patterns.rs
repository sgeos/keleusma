//! Signed pattern payloads must not collide with the parser transport sentinel.
#![cfg(feature = "self-host")]

#[test]
fn negative_one_patterns_preserve_complete_artifacts() {
    for source in [
        "fn main() -> Word { -1 / 0 { ok(v) => v, zero_divisor(-1) => 7, zero_divisor(_) => 8 } }",
        "fn main() -> Word { (1 lsl 63) - 1 { underflow(-1, l) => l, underflow(_, _) => 9, ok(v) => v } }",
        "fn main() -> Word { (bnot 0 lsr 1) + 1 { overflow(0, -1) => 7, overflow(_, _) => 8, ok(v) => v } }",
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|error| panic!("{source}\n{error}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}

#[test]
fn byte_bindings_keep_their_kind_in_arm_arithmetic() {
    for source in [
        "fn main() -> Byte { (255 as Byte) + (1 as Byte) { ok(v) => v + (1 as Byte), overflow(v) => v + (2 as Byte) } }",
        "fn main() -> Byte { (7 as Byte) / (0 as Byte) { ok(v) => v, zero_divisor(n) => n / (1 as Byte) { ok(v) => v } } }",
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|error| panic!("{source}\n{error}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}
