//! Native calls, differentiated against the virtual machine.
//!
//! # The observable is the CALL SEQUENCE, not the return value
//!
//! The 999 native call sites in the `piano_roll` family are side effects:
//! `host::set_waveform`, `host::play`, `host::set_adsr`. Measured, **zero of
//! them carry a return shape**, and there are 1643 `PopN` against them, so the
//! results are overwhelmingly discarded. Comparing only a final integer would
//! leave the thing these modules exist to do entirely unchecked — a lowering
//! that dropped every call, or reordered them, or passed arguments backwards,
//! would return the right number.
//!
//! So both sides log `(name, args)` in order and the logs are compared.
//!
//! # Argument order is the trap, and it is deliberately order-sensitive here
//!
//! `Vm::run` drains its stack as `stack[len - n..]`, delivering arguments in
//! DECLARATION order; the emitter pops (which reverses) and reverses back. A
//! two-argument native returning `a * 10 + b` catches a swap. Returning `a + b`
//! would not, and neither would any one-argument native — and 68 of the corpus
//! sites take one argument.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Op, Value};
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::cell::RefCell;

// The recorded call sequence for whichever side is running.
//
// Thread-local rather than a global mutex: the JIT calls back on the calling
// thread, and `cargo test` runs these in parallel, so a shared log would
// interleave two tests' calls and fail nondeterministically.
thread_local! {
    static LOG: RefCell<Vec<(String, Vec<i64>)>> = const { RefCell::new(Vec::new()) };
}

fn log_call(name: &str, args: &[i64]) {
    LOG.with(|l| l.borrow_mut().push((name.to_string(), args.to_vec())));
}

fn take_log() -> Vec<(String, Vec<i64>)> {
    LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

// String arguments are logged separately and DECODED on both sides.
//
// The two representations are not comparable as raw operands: the virtual
// machine marshals an owned `String` into the native, while the lowering passes
// the address of a constant global. Comparing the decoded text is the only
// comparison that means anything, and it is also the one that matters — a
// pointer that is merely non-null proves nothing about what it points at.
thread_local! {
    static STRLOG: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

fn log_str_call(name: &str, s: &str) {
    STRLOG.with(|l| l.borrow_mut().push((name.to_string(), s.to_string())));
}

fn take_strlog() -> Vec<(String, String)> {
    STRLOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
}

/// Decode the `{ i64 len, [n+1 x i8] bytes }` a static string global holds.
///
/// # Safety
/// `p` must be an address the lowering produced for a static string.
unsafe fn decode_static_str(p: i64) -> String {
    let base = p as *const u8;
    let len = unsafe { core::ptr::read_unaligned(base as *const i64) } as usize;
    let bytes = unsafe { core::slice::from_raw_parts(base.add(8), len) };
    String::from_utf8_lossy(bytes).into_owned()
}

#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__name(p: i64) -> i64 {
    let s = unsafe { decode_static_str(p) };
    log_str_call("host::name", &s);
    s.len() as i64
}

// The host side of the native ABI, as the linker sees it.
//
// **Exported under the mangled name rather than bound through
// `add_global_mapping`.** The mapping route compiled and then segfaulted on the
// first call, because the execution engine resolved the declaration to nothing
// and jumped to it. Exporting the symbol makes the engine resolve it the way a
// real link would, from the process symbol table, which also makes this test
// assert something it otherwise would not: that `native_symbol`'s output is a
// name an ordinary linker actually binds.
//
// The return values are deliberately asymmetric in their arguments so that a
// swap, a drop, or a duplicated argument changes the answer.
#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__one(a: i64) -> i64 {
    log_call("host::one", &[a]);
    a + 7
}
#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__two(a: i64, b: i64) -> i64 {
    log_call("host::two", &[a, b]);
    a * 10 + b
}
#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__three(a: i64, b: i64, c: i64) -> i64 {
    log_call("host::three", &[a, b, c]);
    a * 100 + b * 10 + c
}

/// Run `src` on the virtual machine with the three natives registered, and
/// return `(result, call log)`.
fn vm_run(src: &str, args: &[i64]) -> (i64, Vec<(String, Vec<i64>)>) {
    let _ = take_log();
    let _ = take_strlog();
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");

    vm.register_fn("host::one", |a: i64| -> i64 {
        log_call("host::one", &[a]);
        a + 7
    });
    vm.register_fn("host::two", |a: i64, b: i64| -> i64 {
        log_call("host::two", &[a, b]);
        a * 10 + b
    });
    vm.register_fn("host::three", |a: i64, b: i64, c: i64| -> i64 {
        log_call("host::three", &[a, b, c]);
        a * 100 + b * 10 + c
    });
    vm.register_fn("host::name", |s: String| -> i64 {
        log_str_call("host::name", &s);
        s.len() as i64
    });

    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    let out = match vm.call(&vals).expect("vm run") {
        keleusma::vm::VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    };
    (out, take_log())
}

/// Lower `src`, bind the native symbols into the JIT, run it, and return
/// `(result, call log)`.
fn native_run(src: &str, args: &[i64]) -> (i64, Vec<(String, Vec<i64>)>) {
    let _ = take_log();
    let _ = take_strlog();
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let entry = m.entry_point.expect("entry point");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    lm.verify().expect("LLVM module verification");

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    // Force the exported host symbols to be retained by the linker. Without a
    // use, the test binary may drop them and the engine then resolves the
    // declaration to nothing and jumps to it — which is a segfault, not a
    // failed assertion.
    std::hint::black_box((
        kel_native_host__one as *const (),
        kel_native_host__two as *const (),
        kel_native_host__three as *const (),
        kel_native_host__name as *const (),
    ));

    let name = format!("kel_chunk_{entry}");
    let out = match args.len() {
        1 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&name) }
                .expect("symbol");
            unsafe { f.call(args[0]) }
        }
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&name) }
                .expect("symbol");
            unsafe { f.call(args[0], args[1]) }
        }
        n => panic!("harness does not drive {n}-argument entry points"),
    };
    (out, take_log())
}

/// Both sides must agree on the result AND on the whole call sequence.
fn assert_agrees(src: &str, args: &[i64]) {
    let (vr, vlog) = vm_run(src, args);
    let vstr = take_strlog();
    let (nr, nlog) = native_run(src, args);
    let nstr = take_strlog();
    assert_eq!(vr, nr, "return value disagrees for args {args:?}");
    assert_eq!(
        vlog, nlog,
        "native call SEQUENCE disagrees for args {args:?}; \
         the return value matched, which is why this is checked separately"
    );
    assert_eq!(
        vstr, nstr,
        "the STRING argument sequence disagrees for args {args:?}"
    );
    assert!(
        !vlog.is_empty() || !vstr.is_empty(),
        "both call logs are empty, so this test asserts nothing about native calls"
    );
}

#[test]
fn a_native_call_agrees_with_the_vm_on_result_and_sequence() {
    assert_agrees(
        "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b) }",
        &[3, 4],
    );
}

#[test]
fn argument_order_is_not_reversed() {
    // `two` returns `a * 10 + b`, so a swap gives 43 where 34 is correct. The
    // asymmetry is the whole point: `a + b` would pass either way.
    let (vr, _) = vm_run(
        "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b) }",
        &[3, 4],
    );
    assert_eq!(vr, 34, "the VM itself must see declaration order");
    assert_agrees(
        "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b) }",
        &[3, 4],
    );
}

#[test]
fn three_arguments_agree() {
    assert_agrees(
        "use host::three\nfn main(a: Word, b: Word) -> Word { host::three(a, b, 9) }",
        &[1, 2],
    );
}

#[test]
fn a_chained_native_result_feeds_the_next_call() {
    assert_agrees(
        "use host::one\nuse host::two\n\
         fn main(a: Word, b: Word) -> Word { host::one(host::two(a, b)) }",
        &[5, 6],
    );
}

#[test]
fn a_discarded_native_result_still_makes_the_call() {
    // The corpus shape: 1643 `PopN` against 999 calls. The result is dropped,
    // so only the log proves the call happened at all.
    assert_agrees(
        "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b); a }",
        &[8, 2],
    );
}

#[test]
fn calls_inside_control_flow_agree_on_both_paths() {
    let src = "use host::one\nuse host::two\n\
               fn main(a: Word, b: Word) -> Word { \
                 if a > b { host::one(a) } else { host::two(a, b) } }";
    assert_agrees(src, &[9, 1]);
    assert_agrees(src, &[1, 9]);
}

#[test]
fn repeated_calls_preserve_their_order() {
    assert_agrees(
        "use host::one\nfn main(a: Word, b: Word) -> Word { \
           host::one(a); host::one(b); host::one(a + b) }",
        &[2, 3],
    );
}

/// **Must not fire.** The error-reify flag pushes a two-slot `(code, flag)`
/// result rather than one, so lowering it as an ordinary call would leave the
/// operand stack one short at every such site.
///
/// Measured at 0 of 999 corpus sites, so no source produces it. The flag is set
/// on real compiled bytecode instead, which is the same technique the typed
/// verifier's conformance corpus uses.
#[test]
fn the_error_reify_flag_is_refused_rather_than_mislowered() {
    let src = "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b) }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");

    let mut patched = 0usize;
    for c in m.chunks.iter_mut() {
        for op in c.ops.iter_mut() {
            match op {
                Op::CallVerifiedNative(i, n) => {
                    *op = Op::CallVerifiedNative(*i, *n | 0x80);
                    patched += 1;
                }
                Op::CallExternalNative(i, n) => {
                    *op = Op::CallExternalNative(*i, *n | 0x80);
                    patched += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        patched, 1,
        "expected exactly one native call site to patch; the test would be \
         vacuous if it patched none"
    );

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_module(&ctx, &lm, &m, LowerOptions::default())
        .expect_err("the error-reify flag must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("error-reify"),
        "the refusal must name the reason, got: {text}"
    );
}

/// **Must not fire.** Two natives whose names differ but whose symbols coincide
/// would bind both call sites to one host definition, and each site would look
/// correct in isolation.
#[test]
fn colliding_native_symbols_are_refused() {
    let src = "use host::two\nfn main(a: Word, b: Word) -> Word { host::two(a, b) }";
    let mut m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    // `host::two` and `host..two` both mangle to `kel_native_host__two`.
    //
    // Note which pair collides and which does NOT. `host_two` maps to
    // `kel_native_host_two`, a DIFFERENT symbol, because each separator
    // character becomes one underscore rather than the whole `::` collapsing to
    // one. Collapsing would read better and would make `host::two` and
    // `host_two` collide — a far likelier pair in a real program than this one.
    m.native_names.push("host..two".to_string());

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_module(&ctx, &lm, &m, LowerOptions::default())
        .expect_err("a symbol collision must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("kel_native_host__two"),
        "the refusal must name the colliding symbol, got: {text}"
    );
}

/// A static string literal reaches the native with its exact contents.
///
/// This is the shape all ten `piano_roll` modules open with:
/// `host::song_name("…")` in the init block, which is why every one of them
/// refused at its FIRST op and hid 999 native calls behind that refusal.
#[test]
fn a_static_string_reaches_the_native_intact() {
    assert_agrees(
        "use host::name\nfn main(a: Word, b: Word) -> Word { host::name(\"Harmonic Garden\"); a + b }",
        &[1, 2],
    );
}

/// The length is carried explicitly, so an interior NUL is not a truncation.
///
/// A `char*` ABI would report 5 here and both sides would agree on the WRONG
/// answer only if the virtual machine truncated too — it does not, so this
/// fails loudly against a C-string layout rather than passing quietly.
#[test]
fn an_interior_nul_is_not_truncated() {
    assert_agrees(
        "use host::name\nfn main(a: Word, b: Word) -> Word { host::name(\"abcde\u{0}fghij\"); a + b }",
        &[1, 2],
    );
}

/// Two distinct literals do not collapse onto one global.
#[test]
fn distinct_literals_stay_distinct() {
    assert_agrees(
        "use host::name\nfn main(a: Word, b: Word) -> Word { \
           host::name(\"first\"); host::name(\"second\"); host::name(\"first\"); a + b }",
        &[1, 2],
    );
}

/// An empty literal is a length of zero, not a null pointer.
#[test]
fn an_empty_literal_is_length_zero() {
    assert_agrees(
        "use host::name\nfn main(a: Word, b: Word) -> Word { host::name(\"\"); a + b }",
        &[1, 2],
    );
}

/// Multi-byte UTF-8 survives, which byte length rather than character count
/// makes observable: this string is 3 characters and 7 bytes.
#[test]
fn multibyte_utf8_survives() {
    assert_agrees(
        "use host::name\nfn main(a: Word, b: Word) -> Word { host::name(\"é日本\"); a + b }",
        &[1, 2],
    );
}
