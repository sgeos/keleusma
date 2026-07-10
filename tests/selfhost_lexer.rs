// The self-hosted compiler is a full-width host tool. Its byte-level and
// op-encoding arithmetic overflows a narrow declared word, and the compiler
// rejects a target wider than the runtime, so these tests are meaningful only on
// a 64-bit runtime, not under the `narrow-word-*` (embedded-target) feature
// configs.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! Regression test for the self-hosted compiler's Stage 1 lexer
//! (`compiler/kel/lexer.kel`, increment 1). It compiles the lexer on the
//! current runtime, drives it over a source held in shared data, and checks the
//! streamed token encoding. Guards that the lexer keeps compiling and tokenizing
//! as the runtime evolves toward V0.3.0.
use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

#[test]
fn self_hosted_lexer_increment_1() {
    let src = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify");

    let input = b"let x = 42";
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(input.len() as i64))
        .expect("len");
    for (i, &byte) in input.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(byte))
            .expect("byte");
    }

    // Collect non-PENDING tokens as (kind, value) until EOF.
    let mut tokens: Vec<(i64, i64)> = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..64 {
        match st {
            VmState::Yielded(Value::Int(t)) => {
                let (kind, value) = (t % 16, t / 16);
                if kind != 0 {
                    tokens.push((kind, value));
                    if kind == 1 {
                        break; // EOF
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    eprintln!("tokens (kind,value) = {:?}", tokens);
    // IDENT len3 "let", IDENT len1 "x", PUNCT '=' , INT 42, EOF
    assert_eq!(
        tokens,
        vec![(2, 3), (2, 1), (4, b'=' as i64), (3, 42), (1, 0)]
    );
}
