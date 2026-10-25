//! The Order 4 corpus. See docs/decisions/ORDER4_HOSTED_RUNTIME_PLAN.md.
//!
//! `BOUNDARY_OK` copies the 107 cases that `self_hosted_construct_support_boundary` in
//! tests/selfhost_codegen.rs classified as byte-identical between the two compilers at
//! ec536b28 on 2026-10-25. It is a copy, not a link, so a case added there is not added here.
//! The harness renames an entry function `f` to `main` so the module has an entry point, and
//! compiles every case through the shipping self-hosted compiler rather than
//! trusting that classification, and counts the cases each stage reached.

/// The boundary cases classified `Ok`, as (label, source).
pub const BOUNDARY_OK: &[(&str, &str)] = &[
    ("scalar/unit_literal", "fn f() -> Word { let x = (); 7 }"),
    ("scalar/hex_literal", "fn f() -> Word { 0xFF + 0xdeadBEEF }"),
    ("scalar/hex_literal_upper_prefix", "fn f() -> Word { 0XAB }"),
    (
        "scalar/binary_literal",
        "fn f() -> Word { 0b1010 + 0B1101 }",
    ),
    (
        "scalar/word_arith",
        "fn f(a: Word, b: Word) -> Word { a + b - a * b }",
    ),
    (
        "scalar/word_divmod",
        "fn f(a: Word, b: Word) -> Word { a / b + a % b }",
    ),
    (
        "scalar/word_cmp",
        "fn f(a: Word, b: Word) -> bool { a < b }",
    ),
    ("scalar/word_neg", "fn f(a: Word) -> Word { 0 - a }"),
    (
        "scalar/byte_arith",
        "fn f(a: Byte, b: Byte) -> Byte { a + b }",
    ),
    (
        "scalar/byte_sub_mul",
        "fn f(a: Byte, b: Byte) -> Byte { a - b * a }",
    ),
    ("scalar/word_unary_neg", "fn f(a: Word) -> Word { -a }"),
    (
        "scalar/byte_cmp",
        "fn f(a: Byte, b: Byte) -> bool { a < b }",
    ),
    ("op/shift_lsl", "fn f(a: Word) -> Word { a lsl 2 }"),
    ("op/shift_asr", "fn f(a: Word) -> Word { a asr 2 }"),
    ("op/shift_lsr_const", "fn f(a: Word) -> Word { a lsr 2 }"),
    (
        "op/shift_lsr_var",
        "fn f(a: Word, k: Word) -> Word { a lsr k }",
    ),
    ("op/byte_shift", "fn f(a: Byte) -> Byte { a lsr 1 }"),
    (
        "op/bitwise_band",
        "fn f(a: Word, b: Word) -> Word { a band b }",
    ),
    ("op/bnot_word", "fn f(a: Word) -> Word { bnot a }"),
    ("op/bnot_byte", "fn f(a: Byte) -> Byte { bnot a }"),
    ("literal/string", "fn f() -> Word { let s = \"hi\"; 1 }"),
    (
        "nested/array_of_array_literal",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; 1 }",
    ),
    (
        "nested/array_of_array_index",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] }",
    ),
    (
        "nested/array_of_array_split_index",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; let b = a[0]; b[1] }",
    ),
    ("cast/word_to_byte", "fn f() -> Byte { 7 as Byte }"),
    ("cast/byte_to_word", "fn f(a: Byte) -> Word { a as Word }"),
    (
        "cast/round_trip",
        "fn f() -> Word { let b = 7 as Byte; b as Word }",
    ),
    (
        "cast/widened_bytes_add",
        "fn f(o: Byte, n: Byte) -> Word { (o as Word) + (n as Word) }",
    ),
    (
        "cast/widened_bytes_mul",
        "fn f(o: Byte, n: Byte) -> Word { (o as Word) * (n as Word) }",
    ),
    (
        "cast/widened_bytes_unparenthesized",
        "fn f(o: Byte, n: Byte) -> Word { o as Word + n as Word }",
    ),
    (
        "cast/widened_byte_plus_literal",
        "fn f(o: Byte) -> Word { (o as Word) + 1 }",
    ),
    (
        "cast/widened_byte_shift",
        "fn f(o: Byte) -> Word { (o as Word) lsl 1 }",
    ),
    (
        "cast/narrowed_words_add",
        "fn f(a: Word, b: Word) -> Byte { (a as Byte) + (b as Byte) }",
    ),
    (
        "cast/narrowed_words_band",
        "fn f(a: Word, b: Word) -> Byte { (a as Byte) band (b as Byte) }",
    ),
    (
        "cast/literal_plus_widened_byte",
        "fn f(o: Byte) -> Word { 1 + (o as Word) }",
    ),
    (
        "cast/narrowed_word_shift",
        "fn f(a: Word) -> Byte { (a as Byte) lsl 1 }",
    ),
    ("bool/true_literal", "fn f() -> bool { true }"),
    ("bool/false_literal", "fn f() -> bool { false }"),
    ("bool/literal_bound", "fn f() -> bool { let b = true; b }"),
    (
        "bool/literal_condition",
        "fn f() -> Word { if true { 1 } else { 2 } }",
    ),
    ("bool/not", "fn f(a: bool) -> bool { not a }"),
    (
        "bool/andalso",
        "fn f(a: bool, b: bool) -> bool { a andalso b }",
    ),
    (
        "bool/orelse",
        "fn f(a: bool, b: bool) -> bool { a orelse b }",
    ),
    ("bool/xor", "fn f(a: bool, b: bool) -> bool { a xor b }"),
    (
        "bool/and_eager",
        "fn f(a: bool, b: bool) -> bool { a and b }",
    ),
    ("bool/or_eager", "fn f(a: bool, b: bool) -> bool { a or b }"),
    (
        "prec/or_vs_and",
        "fn f(a: bool, b: bool, c: bool) -> bool { a or b and c }",
    ),
    (
        "prec/or_vs_xor",
        "fn f(a: bool, b: bool, c: bool) -> bool { a or b xor c }",
    ),
    (
        "prec/and_vs_cmp",
        "fn f(x: bool, y: Word, z: Word) -> bool { x and y < z }",
    ),
    (
        "ctrl/if_else",
        "fn f(a: Word) -> Word { if a > 0 { 1 } else { 2 } }",
    ),
    (
        "ctrl/match_int",
        "fn f(k: Word) -> Word { match k { 0 => 10, _ => 20 } }",
    ),
    (
        "ctrl/for_limit",
        "private data acc { s: Word }\nfn f(n: Word) -> Word { acc.s = 0; for i in 0..n limit 8 { acc.s = acc.s + i; } acc.s }",
    ),
    (
        "ctrl/for_bare",
        "private data acc { s: Word }\nfn f(n: Word) -> Word { acc.s = 0; for i in 0..n { acc.s = acc.s + i; } acc.s }",
    ),
    ("ctrl/loop_yield", "loop main(r: Word) -> Word { yield r }"),
    (
        "data/indexed_write_bare",
        "private data d { x: [Word; 4] }\nfn f(i: Word) -> Word { d.x[i] = 7; d.x[0] }",
    ),
    (
        "data/indexed_write_handled",
        "private data d { x: [Word; 4] }\nfn f(i: Word) -> Word { d.x[i] = 7 { ok(o, n) => o + n, invalid_index(j) => j } }",
    ),
    (
        "data/indexed_write_default_trap",
        "private data d { x: [Byte; 4] }\nfn f(i: Word) -> Byte { d.x[i] = 7 as Byte { ok(_, n) => n } }",
    ),
    (
        "comp/struct_ctor",
        "struct P { x: Word }\nfn f() -> P { P { x: 1 } }",
    ),
    (
        "comp/struct_field",
        "struct P { x: Word }\nfn f(p: P) -> Word { p.x }",
    ),
    ("comp/tuple_ctor", "fn f() -> (Word, Word) { (1, 2) }"),
    ("comp/tuple_field", "fn f(t: (Word, Word)) -> Word { t.0 }"),
    ("comp/array_lit", "fn f() -> [Word; 3] { [1, 2, 3] }"),
    (
        "comp/array_index",
        "fn f(a: [Word; 3], i: Word) -> Word { a[i] }",
    ),
    ("comp/enum_ctor", "enum E { A, B }\nfn f() -> E { E::A() }"),
    (
        "comp/enum_match",
        "enum E { A, B }\nfn f(e: E) -> Word { match e { E::A() => 1, E::B() => 2 } }",
    ),
    (
        "eq/struct",
        "struct P { x: Word }\nfn f(a: P, b: P) -> bool { a == b }",
    ),
    (
        "eq/tuple",
        "fn f(a: (Word,Word), b: (Word,Word)) -> bool { a == b }",
    ),
    (
        "eq/enum",
        "enum E { A, B }\nfn f(a: E, b: E) -> bool { a == b }",
    ),
    (
        "eq/enum_literal",
        "enum E { A, B }\nfn f(e: E) -> bool { e == E::A() }",
    ),
    (
        "eq/array",
        "fn f(a: [Word;3], b: [Word;3]) -> bool { a == b }",
    ),
    (
        "eq/array_of_struct",
        "struct P { x: Word }\nfn f(a: [P;2], b: [P;2]) -> bool { a == b }",
    ),
    (
        "eq/array_of_enum",
        "enum E { A, B }\nfn f(a: [E;2], b: [E;2]) -> bool { a == b }",
    ),
    (
        "eq/array_of_tuple",
        "fn f(a: [(Word,Word);2], b: [(Word,Word);2]) -> bool { a == b }",
    ),
    (
        "eq/array_of_array",
        "fn f(a: [[Word;2];2], b: [[Word;2];2]) -> bool { a == b }",
    ),
    (
        "eq/nested_struct_in_struct",
        "struct I { v: Word }\nstruct S { i: I, w: Word }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_struct",
        "struct S { t: (Word,Word), w: Word }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/array_in_struct",
        "struct S { a: [Word;2], w: Word }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/enum_in_struct",
        "enum E { A, B }\nstruct S { e: E, w: Word }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/tuple_of_struct",
        "struct P { x: Word }\nfn f(a: (P, Word), b: (P, Word)) -> bool { a == b }",
    ),
    (
        "eq/tuple_of_deep_struct",
        "struct I { v: Word }\nstruct M { i: I }\nfn f(a: (M, Word), b: (M, Word)) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple",
        "fn f(a: ((Word, Word), Word), b: ((Word, Word), Word)) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple_last",
        "fn f(a: (Word, (Word, Word)), b: (Word, (Word, Word))) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple_both",
        "fn f(a: ((Word, Word), (Word, Word)), b: ((Word, Word), (Word, Word))) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple_3level",
        "fn f(a: (((Word, Word), Word), Word), b: (((Word, Word), Word), Word)) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple_byte",
        "fn f(a: ((Word, Byte), Word), b: ((Word, Byte), Word)) -> bool { a == b }",
    ),
    (
        "eq/tuple_in_tuple_neq",
        "fn f(a: ((Word, Word), Word), b: ((Word, Word), Word)) -> bool { a != b }",
    ),
    (
        "eq/struct_in_nested_tuple",
        "struct P { x: Word }\nfn f(a: ((Word, Word), P), b: ((Word, Word), P)) -> bool { a == b }",
    ),
    (
        "eq/array_of_tuple",
        "fn f(a: [(Word, Word); 2], b: [(Word, Word); 2]) -> bool { a == b }",
    ),
    (
        "tuple/nested_elem_access",
        "fn f(a: ((Word, Word), Word)) -> Word { a.1 }",
    ),
    (
        "eq/array_of_array",
        "fn f(a: [[Word; 2]; 2], b: [[Word; 2]; 2]) -> bool { a == b }",
    ),
    (
        "eq/enum_tuple_payload",
        "enum E { A(Word, Word), B }\nfn f(a: E, b: E) -> bool { a == b }",
    ),
    (
        "eq/struct_tuple_of_struct",
        "struct P { x: Word }\nstruct S { t: (P, Word) }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/struct_tuple_of_deep_struct",
        "struct Q { z: Word }\nstruct P { q: Q }\nstruct S { t: (P, Word) }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_tuple_subfield",
        "struct P { u: (Word, Word) }\nstruct S { p: P }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_tuple_subfield_3level",
        "struct P { u: (Word, Word) }\nstruct Q { p: P }\nstruct S { q: Q }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/tuple_elem_struct_with_tuple",
        "struct P { u: (Word, Word) }\nstruct S { t: (P, Word) }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_array_subfield",
        "struct I { xs: [Word;2] }\nstruct S { i: I }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_array_subfield_sibling",
        "struct I { xs: [Word;2], w: Word }\nstruct S { i: I }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_enum_subfield",
        "enum E { A, B }\nstruct I { e: E }\nstruct S { i: I }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/nested_enum_subfield_payload",
        "enum E { A(Word), B }\nstruct I { e: E }\nstruct S { i: I }\nfn f(a: S, b: S) -> bool { a == b }",
    ),
    (
        "eq/array_of_tuple_of_struct",
        "struct P { x: Word }\nfn f(a: [(P, Word); 2], b: [(P, Word); 2]) -> bool { a == b }",
    ),
    (
        "eq/array_of_deep_struct",
        "struct I { v: Word }\nstruct M { i: I }\nfn f(a: [M; 2], b: [M; 2]) -> bool { a == b }",
    ),
    (
        "eq/array_of_3level_struct",
        "struct A { x: Word }\nstruct B { a: A }\nstruct C { b: B }\nfn f(a: [C; 2], b: [C; 2]) -> bool { a == b }",
    ),
    (
        "eq/enum_struct_payload",
        "struct P { x: Word }\nenum E { A(P), B }\nfn f(a: E, b: E) -> bool { a == b }",
    ),
    (
        "eq/2level_struct",
        "struct I { v: Word }\nstruct M { i: I }\nstruct O { m: M }\nfn f(a: O, b: O) -> bool { a == b }",
    ),
    (
        "eq/3level_struct",
        "struct A { x: Word }\nstruct B { a: A }\nstruct C { b: B }\nstruct D { c: C }\nfn f(p: D, q: D) -> bool { p == q }",
    ),
    (
        "eq/struct_arrayofstruct",
        "struct P { x: Word }\nstruct Q { ps: [P;2] }\nfn f(a: Q, b: Q) -> bool { a == b }",
    ),
];
