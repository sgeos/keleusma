//! The bundled text operations over `Text<N>`, compiler intrinsics behind the
//! `text` cargo feature, called as methods on the value.
//!
//! # What is pinned
//!
//! Each operation is run end to end under strict verification and its result
//! compared with a model computed in Rust from the specification in
//! `docs/decisions/TEXT_CAPACITY_TYPE.md`. The model never reads compiler
//! output, so agreement is evidence about the lowering and not a restatement
//! of it. The inputs are an exhaustive product over a fixed list of strings
//! and capacities rather than a sample, so a failure reproduces exactly.
//!
//! **Malformed operands are pinned too.** A `Text<N>` that script code builds
//! always has a length in `[0, N]` and zero bytes past it, but a host or a
//! hand-built module can supply any body. Every operation must give the
//! specified result for every body, reading the effective length
//! `min(max(len, 0), N)` and treating each byte at or past it as zero. The
//! malformed operands here are built by replacing the pooled constant of a
//! literal cast in a compiled module, and by a host entry argument.
//!
//! The cost claims are pinned too. Every module must pass `verify()`, the
//! worst-case execution time must be finite and must grow linearly with the
//! capacity, the exact instruction counts and costs recorded in the design
//! record must hold, and no operation may emit `Op::Len`, a call or a native
//! call.
//!
//! The method surface is resolved by receiver type. A method of the same name
//! on any other type still resolves to its trait impl, which is pinned here and
//! in the build without the feature.
//!
//! # What is not pinned here
//!
//! The refusal of the operations in a build without the feature lives in
//! `tests/text_ops_without_feature.rs`, which compiles only in that build.

#![cfg(all(feature = "compile", feature = "verify", feature = "text"))]

use keleusma::Arena;
use keleusma::bytecode::{ConstValue, Module, Op, TupleBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// Byte width of this build's word, derived from the crate rather than
/// restated, so a narrow-word selector does not make the expectations wrong.
fn word_bytes() -> usize {
    keleusma::target::Target::host().word_bits() as usize / 8
}

/// The flat body of a `Text<cap>` holding `bytes`, built from the
/// specification and not from compiler output. It always satisfies the
/// invariant, so a result compared against it is checked for the invariant as
/// well as for its content.
fn body(bytes: &[u8], cap: usize) -> Vec<u8> {
    assert!(bytes.len() <= cap, "a model text must fit its capacity");
    let wb = word_bytes();
    let mut out = (bytes.len() as u64).to_le_bytes()[..wb].to_vec();
    out.extend_from_slice(bytes);
    out.resize(wb + cap, 0);
    out
}

/// A Keleusma string literal for `s`, escaping what the lexer requires.
fn lit(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn compile_src(src: &str) -> Module {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    compile(&program).unwrap_or_else(|e| panic!("compile failed for {src}: {}", e.message))
}

/// Compile end to end and return the message. A lex or parse failure panics,
/// so that a fixture with a syntax error cannot pass as a refusal.
fn compile_err(src: &str) -> String {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    match compile(&program) {
        Ok(_) => panic!("expected a refusal, but this compiled: {src}"),
        Err(e) => e.message,
    }
}

/// Run `main` under strict verification and return its finished value, with a
/// flat tuple result copied out while the arena is alive.
#[derive(Debug, PartialEq)]
enum Outcome {
    Body(Vec<u8>),
    Word(i64),
    Bool(bool),
}

fn run_module(module: Module, what: &str) -> Outcome {
    keleusma::verify::verify(&module).unwrap_or_else(|e| panic!("verify() refused {what}: {e:?}"));
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    match vm
        .call(&[])
        .unwrap_or_else(|e| panic!("main must run for {what}: {e:?}"))
    {
        VmState::Finished(Value::Tuple(TupleBody::Flat(fc))) => {
            Outcome::Body(fc.resolve(&arena).expect("live body").to_vec())
        }
        VmState::Finished(Value::Int(n)) => Outcome::Word(n),
        VmState::Finished(Value::Bool(b)) => Outcome::Bool(b),
        other => panic!("unexpected result for {what}: {other:?}"),
    }
}

fn run(src: &str) -> Outcome {
    run_module(compile_src(src), src)
}

fn run_body(src: &str) -> Vec<u8> {
    match run(src) {
        Outcome::Body(b) => b,
        _ => panic!("expected a flat text body for {src}"),
    }
}

fn run_word(src: &str) -> i64 {
    match run(src) {
        Outcome::Word(n) => n,
        _ => panic!("expected a Word for {src}"),
    }
}

fn run_bool(src: &str) -> bool {
    match run(src) {
        Outcome::Bool(b) => b,
        _ => panic!("expected a Bool for {src}"),
    }
}

/// Inputs for the exhaustive products. The list covers the empty text, one
/// byte, an interior NUL, a two-byte UTF-8 character, and a text that fills a
/// small capacity exactly.
const SAMPLES: &[&str] = &["", "a", "ab", "a\0b", "\u{e9}", "abcd"];

/// The smallest capacity a sample fits, and one with slack beyond it.
fn caps_for(s: &str) -> [usize; 2] {
    let n = s.len().max(1);
    [n, n + 3]
}

// --- Concatenation ---

#[test]
fn concat_matches_the_model_over_every_pair_and_capacity() {
    let mut reached = 0usize;
    for a in SAMPLES {
        for b in SAMPLES {
            for ca in caps_for(a) {
                for cb in caps_for(b) {
                    let src = format!(
                        "fn main() -> Text<{}> {{ ({} as Text<{ca}>).concat({} as Text<{cb}>) }}",
                        ca + cb,
                        lit(a),
                        lit(b)
                    );
                    let joined = [a.as_bytes(), b.as_bytes()].concat();
                    assert_eq!(run_body(&src), body(&joined, ca + cb), "{src}");
                    reached += 1;
                }
            }
        }
    }
    // Counted, so that an emptied sample list cannot pass as a clean run.
    assert_eq!(reached, SAMPLES.len() * SAMPLES.len() * 4);
}

#[test]
fn concat_nests_and_its_capacity_is_the_sum() {
    let src = "fn main() -> Text<9> { \
               let a = \"ab\" as Text<2>; \
               let b = \"c\" as Text<3>; \
               a.concat(b).concat(\"de\" as Text<4>) }";
    assert_eq!(run_body(src), body(b"abcde", 9));
    // An unannotated binding of a result keeps its capacity for later use.
    let src = "fn main() -> Word { \
               let r = (\"ab\" as Text<2>).concat(\"c\" as Text<3>); \
               let s = r.slice(1, 9); \
               s.len() + s.capacity() }";
    assert_eq!(run_word(src), 2 + 5);
    let msg = compile_err("fn main() -> Text<4> { (\"ab\" as Text<2>).concat(\"cd\" as Text<3>) }");
    assert!(msg.contains("Text<4>") && msg.contains("Text<5>"), "{msg}");
}

#[test]
fn a_const_generic_capacity_is_resolved_by_monomorphization() {
    let src = "fn twice<const n: Word>(t: Text<n>) -> Text<n + n> { t.concat(t) }\n\
               fn main() -> Text<6> { twice::<3>(\"xy\" as Text<3>) }";
    assert_eq!(run_body(src), body(b"xyxy", 6));
    let src = "fn size<const n: Word>(t: Text<n>) -> Word { t.len() * 10 + t.capacity() }\n\
               fn main() -> Word { size::<5>(\"xy\" as Text<5>) }";
    assert_eq!(run_word(src), 25);
    // A result passed to a generic function instantiates it at the result type.
    let src = "fn id<T>(x: T) -> T { x }\n\
               fn main() -> Text<5> { id((\"ab\" as Text<2>).concat(\"c\" as Text<3>)) }";
    assert_eq!(run_body(src), body(b"abc", 5));
    let src = "fn id<T>(x: T) -> T { x }\n\
               fn main() -> Word { id((\"ab\" as Text<2>).len()) }";
    assert_eq!(run_word(src), 2);
}

// --- Length and capacity ---

#[test]
fn len_is_the_live_byte_count_and_capacity_is_the_type() {
    for s in SAMPLES {
        for cap in caps_for(s) {
            let t = format!("({} as Text<{cap}>)", lit(s));
            assert_eq!(
                run_word(&format!("fn main() -> Word {{ {t}.len() }}")),
                s.len() as i64,
                "{s:?}"
            );
            assert_eq!(
                run_word(&format!("fn main() -> Word {{ {t}.capacity() }}")),
                cap as i64,
                "{s:?}"
            );
        }
    }
    // Large capacities do not fit an immediate and go through the pool.
    assert_eq!(
        run_word("fn main() -> Word { (\"a\" as Text<40>).capacity() }"),
        40
    );
    assert_eq!(
        run_word("fn main() -> Word { (\"abc\" as Text<40>).len() }"),
        3
    );
}

// --- Equality ---

#[test]
fn eq_matches_the_model_over_every_pair_and_capacity() {
    let mut reached = 0usize;
    let mut equal = 0usize;
    for a in SAMPLES {
        for b in SAMPLES {
            for ca in caps_for(a) {
                for cb in caps_for(b) {
                    let src = format!(
                        "fn main() -> bool {{ ({} as Text<{ca}>).eq({} as Text<{cb}>) }}",
                        lit(a),
                        lit(b)
                    );
                    let want = a == b;
                    assert_eq!(run_bool(&src), want, "{src}");
                    reached += 1;
                    equal += usize::from(want);
                }
            }
        }
    }
    assert_eq!(reached, SAMPLES.len() * SAMPLES.len() * 4);
    // Both outcomes were reached, so a lowering that always answers one value
    // cannot pass.
    assert_eq!(equal, SAMPLES.len() * 4);
}

#[test]
fn eq_distinguishes_a_prefix_and_a_trailing_nul() {
    for (a, b) in [("ab", "abc"), ("a", "a\0"), ("\0", "")] {
        let src = format!(
            "fn main() -> bool {{ ({} as Text<4>).eq({} as Text<4>) }}",
            lit(a),
            lit(b)
        );
        assert!(!run_bool(&src), "{a:?} and {b:?} must differ");
    }
}

// --- Slicing ---

/// The clamped slice of a well-formed content, from the specification.
fn slice_model(s: &[u8], start: i64, end: i64) -> Vec<u8> {
    let len = s.len() as i64;
    let st = start.clamp(0, len);
    let en = end.clamp(st, len);
    s[st as usize..en as usize].to_vec()
}

#[test]
fn slice_matches_the_clamped_model() {
    let bounds: &[i64] = &[i64::MIN + 1, -1, 0, 1, 2, 3, 4, 5, 100, i64::MAX];
    let mut reached = 0usize;
    for s in ["", "abcd", "a\0b"] {
        let cap = s.len() + 2;
        for &start in bounds {
            for &end in bounds {
                let src = format!(
                    "fn main() -> Text<{cap}> {{ ({} as Text<{cap}>).slice({start}, {end}) }}",
                    lit(s)
                );
                assert_eq!(
                    run_body(&src),
                    body(&slice_model(s.as_bytes(), start, end), cap),
                    "{src}"
                );
                reached += 1;
            }
        }
    }
    assert_eq!(reached, 3 * bounds.len() * bounds.len());
}

// --- Malformed operands ---

/// A `Text<N>` operand in any state, with any length word and any `N` bytes.
#[derive(Clone, Copy, Debug)]
struct Raw {
    len: i64,
    bytes: &'static [u8],
}

impl Raw {
    /// The content the specification reads from this body. The effective
    /// length is `min(max(len, 0), N)` and the content is the bytes below it.
    fn content(self) -> Vec<u8> {
        let l = self.len.clamp(0, self.bytes.len() as i64) as usize;
        self.bytes[..l].to_vec()
    }

    fn constant(self) -> ConstValue {
        ConstValue::Tuple(vec![
            ConstValue::Int(self.len),
            ConstValue::Array(self.bytes.iter().map(|&b| ConstValue::Byte(b)).collect()),
        ])
    }
}

/// Length words for a malformed operand. They cover the most negative word, a
/// negative length, the empty and partial lengths, the full capacity, one
/// past it, and the most positive word. Each is representable in a word of
/// this build, because the extremes are derived from the word width.
fn bad_lengths(cap: usize) -> Vec<i64> {
    let bits = (word_bytes() * 8) as u32;
    let min = if bits >= 64 {
        i64::MIN
    } else {
        -(1i64 << (bits - 1))
    };
    let max = if bits >= 64 {
        i64::MAX
    } else {
        (1i64 << (bits - 1)) - 1
    };
    let cap = cap as i64;
    vec![min, -1, 0, 1, cap - 1, cap, cap + 1, max]
}

/// Bytes of a four-byte body. Every byte is nonzero, so any length below four
/// leaves nonzero padding past it.
const FOUR: &[u8] = &[b'a', b'b', 0xff, b'z'];
/// Bytes of a three-byte body, with a byte that is not valid UTF-8 alone.
const THREE: &[u8] = &[b'x', 0x80, b'y'];

/// Compile `src`, whose text operands are literal casts of the distinct
/// placeholders `"1"`, `"2"` and so on in order, and replace each placeholder's
/// pooled body with the matching malformed operand.
///
/// A placeholder is found by its pooled shape and its single content byte, so
/// a placeholder that did not reach the constant pool fails the replacement
/// count rather than leaving the module well formed.
fn run_with(src: &str, operands: &[Raw]) -> Outcome {
    let mut module = compile_src(src);
    let main = module
        .chunks
        .iter_mut()
        .find(|c| c.name == "main")
        .expect("main chunk");
    let mut replaced = 0usize;
    for cv in main.constants.iter_mut() {
        let ConstValue::Tuple(items) = cv else {
            continue;
        };
        let [ConstValue::Int(1), ConstValue::Array(bytes)] = items.as_slice() else {
            continue;
        };
        let Some(ConstValue::Byte(tag)) = bytes.first() else {
            continue;
        };
        let index = usize::from(tag.wrapping_sub(b'1'));
        let Some(raw) = operands.get(index) else {
            continue;
        };
        assert_eq!(
            bytes.len(),
            raw.bytes.len(),
            "placeholder capacity in {src}"
        );
        *cv = raw.constant();
        replaced += 1;
    }
    assert_eq!(replaced, operands.len(), "placeholders replaced in {src}");
    run_module(module, src)
}

#[test]
fn len_of_a_malformed_text_is_the_effective_length() {
    let mut reached = 0usize;
    for len in bad_lengths(FOUR.len()) {
        let raw = Raw { len, bytes: FOUR };
        let got = run_with("fn main() -> Word { (\"1\" as Text<4>).len() }", &[raw]);
        assert_eq!(got, Outcome::Word(raw.content().len() as i64), "{raw:?}");
        let got = run_with(
            "fn main() -> Word { (\"1\" as Text<4>).capacity() }",
            &[raw],
        );
        assert_eq!(got, Outcome::Word(4), "{raw:?}");
        reached += 1;
    }
    assert_eq!(reached, 8);
}

#[test]
fn concat_of_malformed_texts_is_the_concat_of_their_contents() {
    let mut reached = 0usize;
    for la in bad_lengths(FOUR.len()) {
        for lb in bad_lengths(THREE.len()) {
            let a = Raw {
                len: la,
                bytes: FOUR,
            };
            let b = Raw {
                len: lb,
                bytes: THREE,
            };
            let got = run_with(
                "fn main() -> Text<7> { (\"1\" as Text<4>).concat(\"2\" as Text<3>) }",
                &[a, b],
            );
            let want = [a.content(), b.content()].concat();
            assert_eq!(got, Outcome::Body(body(&want, 7)), "{a:?} {b:?}");
            reached += 1;
        }
    }
    assert_eq!(reached, 64);
}

#[test]
fn eq_of_malformed_texts_compares_their_contents() {
    let mut reached = 0usize;
    let mut equal = 0usize;
    // The second operand differs from the first only past its second byte,
    // so two bodies that agree on a short effective length are equal however
    // their padding differs.
    const OTHER: &[u8] = &[b'a', b'b', 0x01, 0x02];
    for (cb, other) in [(4usize, OTHER), (3, &OTHER[..3])] {
        for la in bad_lengths(FOUR.len()) {
            for lb in bad_lengths(cb) {
                let a = Raw {
                    len: la,
                    bytes: FOUR,
                };
                let b = Raw {
                    len: lb,
                    bytes: other,
                };
                let got = run_with(
                    &format!("fn main() -> bool {{ (\"1\" as Text<4>).eq(\"2\" as Text<{cb}>) }}"),
                    &[a, b],
                );
                let want = a.content() == b.content();
                assert_eq!(got, Outcome::Bool(want), "{a:?} {b:?}");
                reached += 1;
                equal += usize::from(want);
            }
        }
    }
    assert_eq!(reached, 128);
    // Equal pairs exist only at effective lengths zero, one and two, and both
    // outcomes must be reached for the comparison to mean anything.
    assert!(equal > 0 && equal < reached, "{equal} of {reached}");
}

#[test]
fn slice_of_a_malformed_text_slices_its_content() {
    let bounds: &[i64] = &[i64::MIN + 1, -1, 0, 1, 2, 3, 4, 5, i64::MAX];
    let mut reached = 0usize;
    for len in bad_lengths(FOUR.len()) {
        let t = Raw { len, bytes: FOUR };
        for &start in bounds {
            for &end in bounds {
                let got = run_with(
                    &format!("fn main() -> Text<4> {{ (\"1\" as Text<4>).slice({start}, {end}) }}"),
                    &[t],
                );
                let want = slice_model(&t.content(), start, end);
                assert_eq!(got, Outcome::Body(body(&want, 4)), "{t:?} {start} {end}");
                reached += 1;
            }
        }
    }
    assert_eq!(reached, 8 * bounds.len() * bounds.len());
}

#[test]
fn a_malformed_host_argument_reads_as_its_effective_content() {
    // The same rule holds for a body a host builds and passes to an entry,
    // which is the path the design record names for a malformed value.
    let module = compile_src(
        "fn main(t: Text<4>) -> Word { \
         t.len() * 1000 + t.concat(\"\" as Text<1>).len() * 100 + t.slice(0, 9).len() }",
    );
    keleusma::verify::verify(&module).expect("verify");
    let wb = word_bytes();
    let mut reached = 0usize;
    for len in bad_lengths(FOUR.len()) {
        let need = required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("arena");
        let mut raw = len.to_le_bytes()[..wb].to_vec();
        raw.extend_from_slice(FOUR);
        let arg = keleusma::flat_value::FlatComposite::build_in_arena(&arena, raw.len(), |dst| {
            dst.copy_from_slice(&raw);
            Ok(())
        })
        .expect("arena space")
        .expect("filled");
        let mut vm = Vm::new(module.clone(), &arena).expect("strict construction");
        let got = vm
            .call(&[Value::Tuple(TupleBody::Flat(arg))])
            .unwrap_or_else(|e| panic!("length {len}: {e:?}"));
        let l = Raw { len, bytes: FOUR }.content().len() as i64;
        match got {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, l * 1101, "length {len}"),
            other => panic!("length {len}: {other:?}"),
        }
        reached += 1;
    }
    assert_eq!(reached, 8);
}

// --- Composition with the rest of the language ---

#[test]
fn a_result_crosses_a_yield_and_lives_in_a_struct() {
    let src = "struct S { n: Word, t: Text<5> }\n\
               fn main() -> Text<5> { \
               let s = S { n: 1, t: (\"ab\" as Text<2>).concat(\"c\" as Text<3>) }; \
               s.t }";
    assert_eq!(run_body(src), body(b"abc", 5));
    // A field read is a receiver like any other.
    let src = "struct S { n: Word, t: Text<5> }\n\
               fn main() -> Word { let s = S { n: 1, t: \"abc\" as Text<5> }; s.t.len() }";
    assert_eq!(run_word(src), 3);
    let module = compile_src(
        "loop main(e: Word) -> Text<4> { \
         let n = yield (\"h\" as Text<2>).concat(\"i\" as Text<2>); n }",
    );
    keleusma::verify::verify(&module).expect("verify");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    match vm.call(&[Value::Int(0)]).expect("run") {
        VmState::Yielded(Value::Tuple(TupleBody::Flat(fc))) => {
            assert_eq!(fc.resolve(&arena).expect("live"), body(b"hi", 4));
        }
        other => panic!("expected a yielded flat tuple, got {other:?}"),
    }
}

#[test]
fn labels_flow_through_an_operation() {
    // A labelled operand labels the result, so the labelled length cannot be
    // returned where an unlabelled Word is declared.
    let msg = compile_err("fn f(t: Text<4>@Secret) -> Word { t.len() }\nfn main() -> Word { 1 }");
    assert!(msg.contains("Secret"), "{msg}");
    compile_src("fn f(t: Text<4>@Secret) -> Word@Secret { t.len() }\nfn main() -> Word { 1 }");
    // A labelled argument labels the result too, not only the receiver.
    let msg = compile_err(
        "fn f(t: Text<4>, u: Text<4>@Secret) -> bool { t.eq(u) }\nfn main() -> Word { 1 }",
    );
    assert!(msg.contains("Secret"), "{msg}");
}

// --- Method resolution against user methods of the same name ---

#[test]
fn a_user_type_keeps_its_own_method_of_the_same_name() {
    // The operations are resolved by receiver type, so a trait method named
    // `len` or `eq` on a struct still resolves to the impl.
    let src = "struct S { a: Word }\n\
               trait Size { fn len(s: S) -> Word; fn eq(s: S, o: S) -> bool; }\n\
               impl Size for S { fn len(s: S) -> Word { s.a + 40 } \
               fn eq(s: S, o: S) -> bool { s.a != o.a } }\n\
               fn main() -> Word { let s = S { a: 2 }; \
               let t = \"abc\" as Text<4>; \
               if s.eq(S { a: 2 }) { 0 } else { s.len() * 10 + t.len() } }";
    assert_eq!(run_word(src), 423);
}

#[test]
fn a_method_on_text_n_that_is_not_an_operation_is_refused() {
    let msg = compile_err("fn main() -> Word { (\"a\" as Text<2>).length() }");
    assert!(msg.contains("no method `length`"), "{msg}");
}

#[test]
fn a_trait_impl_on_text_n_does_not_take_over_an_operation() {
    // The receiver type decides, so the bundled operation answers even when a
    // trait impl for the same capacity declares a method of the same name.
    let src = "trait Size { fn len(t: Text<4>) -> Word; }\n\
               impl Size for Text<4> { fn len(t: Text<4>) -> Word { 99 } }\n\
               fn main() -> Word { (\"abc\" as Text<4>).len() }";
    assert_eq!(run_word(src), 3);
}

#[test]
fn the_free_function_spelling_is_no_longer_an_operation() {
    // `text::len` is an ordinary path again. Without a `use` it is an
    // undefined function, as any other undeclared path is, and with one it is
    // a host native call, so a host native may again be imported under the
    // `text` path. Neither is lowered in place.
    let msg = compile_err("fn main() -> Word { text::len(\"ab\" as Text<4>) }");
    assert!(msg.contains("undefined function"), "{msg}");
    let module = compile_src(
        "use text::upper\nfn greet(name: Text) -> Text { text::upper(name) }\n\
         fn main() -> Word { 1 }",
    );
    assert!(
        module.native_names.iter().any(|n| n == "text::upper"),
        "{:?}",
        module.native_names
    );
}

// --- Cost ---

/// The ops of `main` in a module.
fn main_ops(src: &str) -> Vec<Op> {
    let module = compile_src(src);
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main chunk");
    main.ops.clone()
}

#[test]
fn no_operation_emits_len_or_a_call() {
    for src in [
        "fn main() -> Word { (\"ab\" as Text<4>).len() }",
        "fn main() -> Word { (\"ab\" as Text<4>).capacity() }",
        "fn main() -> Text<8> { (\"ab\" as Text<4>).concat(\"c\" as Text<4>) }",
        "fn main() -> bool { (\"ab\" as Text<4>).eq(\"c\" as Text<4>) }",
        "fn main() -> Text<4> { (\"ab\" as Text<4>).slice(0, 1) }",
    ] {
        let ops = main_ops(src);
        assert!(!ops.is_empty(), "{src}");
        for op in &ops {
            assert!(
                !matches!(
                    op,
                    Op::Len
                        | Op::CallVerifiedNative(..)
                        | Op::CallExternalNative(..)
                        | Op::Call(..)
                ),
                "{src} emitted {op:?}"
            );
        }
        let module = compile_src(src);
        assert!(module.native_names.is_empty(), "{src} registered a native");
    }
}

/// The instruction count, worst-case execution time and arena worst-case
/// memory of an operation alone, excluding the evaluation of its operands and
/// the `Return` of `main`.
///
/// Every operand in a measured source is a literal cast or a Word literal of
/// at least 1000, so each is exactly one `Op::Const` whose pooled value is a
/// text body or such a Word. Those loads and the final `Return` are
/// subtracted at their own nominal cost. Operands load no arena memory, so the
/// arena figure is the operation's own.
fn op_cost(src: &str) -> (usize, u32, u32) {
    let module = compile_src(src);
    keleusma::verify::verify(&module).expect("verify");
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main chunk");
    let wcet = keleusma::verify::wcet_whole_chunk(main).expect("a static WCET");
    let (_, heap) = keleusma::verify::wcmu_whole_chunk(main).expect("a static WCMU");
    let is_operand = |op: &Op| match op {
        Op::Const(i) => matches!(
            main.constants[usize::from(*i)],
            ConstValue::Tuple(_) | ConstValue::Int(1000..)
        ),
        _ => false,
    };
    let excluded: Vec<&Op> = main
        .ops
        .iter()
        .filter(|op| is_operand(op) || matches!(op, Op::Return))
        .collect();
    let excluded_cost: u32 = excluded.iter().map(|op| op.cost()).sum();
    (main.ops.len() - excluded.len(), wcet - excluded_cost, heap)
}

/// The per-operation formulas recorded in the cost table of
/// `docs/decisions/TEXT_CAPACITY_TYPE.md`, checked at six capacities so that a
/// change to the lowering or to the cost model cannot leave the table stale.
#[test]
fn the_static_cost_matches_the_recorded_formulas() {
    let wb = word_bytes() as u32;
    let mut reached = 0usize;
    for n in [1usize, 2, 4, 8, 16, 32] {
        let n32 = n as u32;
        let t = |tag: &str| format!("(\"{tag}\" as Text<{n}>)");
        let len = op_cost(&format!("fn main() -> Word {{ {}.len() }}", t("1")));
        let capacity = op_cost(&format!("fn main() -> Word {{ {}.capacity() }}", t("1")));
        let concat = op_cost(&format!(
            "fn main() -> Text<{}> {{ {}.concat({}) }}",
            2 * n,
            t("1"),
            t("2")
        ));
        // An asymmetric pair separates the coefficient of `A` from that of `B`.
        let concat_wide_a = op_cost(&format!(
            "fn main() -> Text<{}> {{ {}.concat(\"2\" as Text<1>) }}",
            n + 1,
            t("1"),
        ));
        let eq = op_cost(&format!(
            "fn main() -> bool {{ {}.eq({}) }}",
            t("1"),
            t("2")
        ));
        // Only the smaller capacity enters the cost of equality.
        let eq_wide_b = op_cost(&format!(
            "fn main() -> bool {{ {}.eq(\"2\" as Text<{}>) }}",
            t("1"),
            2 * n
        ));
        let slice = op_cost(&format!(
            "fn main() -> Text<{n}> {{ {}.slice(1001, 1002) }}",
            t("1")
        ));
        // Instructions `27A + 16B + 47`, WCET `46A + 29B + 63`, arena
        // `2(A + B) + wb`.
        let concat_formula = |a: u32, b: u32| {
            (
                (27 * a + 16 * b + 47) as usize,
                46 * a + 29 * b + 63,
                2 * (a + b) + wb,
            )
        };
        let eq_formula = |m: u32| ((14 * m + 41) as usize, 22 * m + 49, 0);
        assert_eq!(len, (17, 20, 0), "len at {n}");
        assert_eq!(capacity, (2, 2, 0), "capacity at {n}");
        assert_eq!(concat, concat_formula(n32, n32), "concat at {n}");
        assert_eq!(concat_wide_a, concat_formula(n32, 1), "concat at {n}, 1");
        assert_eq!(eq, eq_formula(n32), "eq at {n}");
        assert_eq!(eq_wide_b, eq_formula(n32), "eq at {n}, {}", 2 * n);
        assert_eq!(
            slice,
            ((16 * n32 + 61) as usize, 29 * n32 + 78, 2 * n32 + wb),
            "slice at {n}"
        );
        reached += 1;
    }
    assert_eq!(reached, 6);
}

#[test]
fn the_static_cost_is_finite_and_linear_in_the_capacity() {
    let wcet = |src: &str| -> u32 { op_cost(src).1 };
    for op in ["concat", "eq"] {
        let at = |n: usize| {
            let ret = if op == "eq" {
                "bool".to_string()
            } else {
                format!("Text<{}>", 2 * n)
            };
            wcet(&format!(
                "fn main() -> {ret} {{ (\"a\" as Text<{n}>).{op}(\"b\" as Text<{n}>) }}"
            ))
        };
        let (w4, w8, w16) = (at(4), at(8), at(16));
        assert!(w4 < w8 && w8 < w16, "{op}: {w4} {w8} {w16}");
        // Linear: doubling the capacity adds the same amount each time.
        assert_eq!(w16 - w8, 2 * (w8 - w4), "{op}: {w4} {w8} {w16}");
    }
}

// --- The self-hosted compiler ---

/// The stages under `kel/` do not mirror the operations. A program using one
/// necessarily names a `Text<N>`, because the literal cast is the only way to
/// make an operand, so the shipping entry refuses it as unsupported before any
/// stage runs. This lives here rather than in a `selfhost_*` binary because the
/// workspace job, the only one that enables `text` together with `self-host`,
/// does not run those binaries.
#[cfg(feature = "self-host")]
#[test]
fn a_bundled_text_operation_is_outside_the_self_hosted_subset() {
    let src = "fn main() -> Word { (\"ab\" as Text<2>).concat(\"c\" as Text<2>).len() }";
    match keleusma::selfhost::self_hosted_compile(src, &keleusma::target::Target::host()) {
        Err(keleusma::selfhost::SelfHostError::Unsupported { detail }) => {
            assert!(detail.contains("Text<N>"), "{detail}");
        }
        other => panic!("expected an unsupported refusal, got {other:?}"),
    }
}

// --- Refusals ---

#[test]
fn an_argument_of_the_wrong_type_is_refused() {
    for (src, needle) in [
        (
            "fn main() -> Text<4> { (\"ab\" as Text<4>).slice(true, 1) }",
            "expects Word",
        ),
        (
            "fn main() -> bool { (\"ab\" as Text<4>).eq(\"ab\") }",
            "argument 0 expects a capacity-bounded text",
        ),
        (
            "fn main() -> Text<8> { (\"ab\" as Text<4>).concat(3) }",
            "argument 0 expects a capacity-bounded text",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{src}: {msg}");
    }
}

#[test]
fn a_receiver_that_is_not_a_text_n_does_not_reach_an_operation() {
    // Static text and a Word have no such method, so the ordinary method
    // resolution refuses them.
    for src in [
        "fn main() -> Word { \"ab\".len() }",
        "fn main() -> Word { let n = 3; n.len() }",
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("no method `len`"), "{src}: {msg}");
    }
}

#[test]
fn an_arity_mismatch_is_refused() {
    for (src, needle) in [
        (
            "fn main() -> Word { (\"a\" as Text<2>).len(\"b\" as Text<2>) }",
            "expects 0 argument",
        ),
        (
            "fn main() -> bool { (\"a\" as Text<2>).eq() }",
            "expects 1 argument",
        ),
        (
            "fn main() -> Text<2> { (\"a\" as Text<2>).slice(0) }",
            "expects 2 argument",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{src}: {msg}");
    }
}

#[test]
fn an_operation_has_no_outcome_arms_to_handle() {
    // The operations are total. A guarded form over one is refused by the
    // general rule, which admits arithmetic and native calls only, so no
    // operation-specific refusal is needed for the method form.
    let msg = compile_err(
        "fn main() -> Word { (\"a\" as Text<2>).len() { ok(v) => v, host_error(c) => c } }",
    );
    assert!(msg.contains("construct currently guards only"), "{msg}");
}

#[test]
fn the_comparison_operators_stay_refused_on_text_n() {
    // The ruling makes `eq` the comparison surface. The operators stay
    // refused, because the general composite rule would admit them and they
    // faulted at runtime when measured.
    for src in [
        "fn main() -> bool { let t = \"a\" as Text<2>; t == t }",
        "fn main() -> bool { let t = \"a\" as Text<2>; t < t }",
    ] {
        compile_err(src);
    }
}

#[test]
fn a_result_capacity_past_the_flat_body_limit_is_refused() {
    // A flat body is at most `u16::MAX` bytes, the length word included.
    let half = (usize::from(u16::MAX) - word_bytes()) / 2 + 1;
    let msg = compile_err(&format!(
        "fn main() -> Text<{}> {{ (\"\" as Text<{half}>).concat(\"\" as Text<{half}>) }}",
        2 * half
    ));
    assert!(msg.contains("larger than the largest capacity"), "{msg}");
}
