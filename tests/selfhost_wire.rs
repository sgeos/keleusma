//! Differential tests for `src/selfhost/kel/wire.kel`, the wire format written
//! in Keleusma (step 6 of the wire-format programme).
//!
//! Slice 1 covers CRC-32/ISO-HDLC. The oracle is `keleusma_wire::crc32`, which
//! is the same algorithm and polynomial as the runtime's own `bytecode::crc32`;
//! both Rust implementations are independently pinned to the published check
//! value `crc32("123456789") == 0xCBF43926` (`keleusma-wire/src/crc.rs` and
//! `src/vm.rs`), so agreement here is agreement with a third-party constant
//! rather than with whichever implementation happened to be written first.
//! `bytecode::crc32` is `pub(crate)` and therefore unreachable from an
//! integration test, which is why the oracle is spelled the other way.
//!
//! The suite carries controls in BOTH directions, because a differential
//! against a known-good reference is exactly where a check that cannot fail
//! hides:
//!
//! - **must-not-fire** — over a corpus with asserted coverage, the Keleusma
//!   implementation and the oracle agree and the comparison stays quiet;
//! - **must-fire** — the same harness pointed at a deliberately mutated source
//!   must report divergence. Two independent mutations are used, since a single
//!   one could in principle be neutral.
//!
//! An assertion that never fires is indistinguishable from one that always
//! succeeds, so the must-fire cases are encoded here rather than run once by
//! hand and described in a commit message.

// Drives the reference front end and constructs a `Vm`, so it needs `compile`
// and `verify`. `wire.kel` declares `require word >= 64`, so it is not
// compilable at all under the narrow-word configurations and the whole file is
// gated out of them rather than failing there.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

/// The Keleusma source under test, embedded so the test cannot drift from a
/// stale copy on disk.
const WIRE_KEL: &str = include_str!("../src/selfhost/kel/wire.kel");

/// `wire.bytes` capacity, matching the array declared in the source. A buffer
/// longer than this is expected to trap rather than be truncated.
const CAPACITY: usize = 4096;

/// Build a `Vm` over `src` with a persistent region sized for its private data.
///
/// Omitting the persistent sizing makes every module with a `private data` block
/// fail at `Vm::new` for a reason unrelated to the construct under test, which
/// reads exactly like a language restriction. It is not one.
fn vm_for(src: &str) -> Vm<'static, 'static> {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&module);
    // Leaked so the returned `Vm` can outlive this frame; the test binary is
    // short-lived and this keeps each case independent.
    let arena = Box::leak(Box::new(Arena::with_capacity(
        DEFAULT_ARENA_CAPACITY + need,
    )));
    arena
        .resize_persistent(need)
        .expect("arena persistent region");
    Vm::new(module, arena).expect("verify")
}

/// Seed `buf` into the shared block and run `main`, returning the checksum.
///
/// `set_shared` addresses SLOTS, not byte offsets: `len` is slot 0 and
/// `bytes[i]` is slot `1 + i`. Getting that wrong shifts the input silently and
/// yields a plausible wrong checksum, which is how the first draft of this
/// harness failed.
fn run_crc_on(
    vm: &mut Vm<'static, 'static>,
    buf: &[u8],
    declared_len: i64,
) -> Result<i64, VmError> {
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(declared_len))?;
    for (i, byte) in buf.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(*byte))?;
    }
    match vm.call_with_shared(&mut shared, &[Value::Int(0)])? {
        VmState::Finished(Value::Int(n)) => Ok(n),
        other => panic!("unexpected VM state: {other:?}"),
    }
}

/// Compile `src` and checksum `buf` through it.
fn crc_in_keleusma(src: &str, buf: &[u8]) -> i64 {
    let mut vm = vm_for(src);
    run_crc_on(&mut vm, buf, buf.len() as i64).expect("run")
}

/// Replace `from` with `to`, requiring the anchor to occur exactly once.
///
/// Without the count assertion a mutation whose anchor has moved silently
/// produces the ORIGINAL source, and the must-fire test then compares a correct
/// implementation against the oracle and reports "no divergence" as though the
/// check were too strict. The failure would be in the mutation, not the check.
fn mutate(src: &str, from: &str, to: &str) -> String {
    let n = src.matches(from).count();
    assert_eq!(
        n, 1,
        "mutation anchor `{from}` must occur exactly once in wire.kel, found {n}; \
         update the anchor rather than the assertion"
    );
    src.replace(from, to)
}

/// Deterministic xorshift64, so the corpus is reproducible across runs and
/// machines and a failure can be re-examined.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 24) as u8
    }
}

/// The differential corpus: named edge cases first, then pseudorandom buffers.
///
/// Coverage is asserted rather than assumed by
/// [`the_corpus_covers_what_it_claims_to`], because "the corpus passes" says
/// nothing if the corpus is all short low-valued buffers.
fn corpus() -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = vec![
        ("empty".into(), Vec::new()),
        ("one zero byte".into(), vec![0x00]),
        ("one high byte".into(), vec![0xFF]),
        ("one byte at the sign boundary".into(), vec![0x80]),
        ("published check vector".into(), b"123456789".to_vec()),
        ("all zeroes, 64".into(), vec![0x00; 64]),
        ("all ones, 64".into(), vec![0xFF; 64]),
        ("ascending 0..=255".into(), (0..=255u8).collect()),
        ("descending 255..=0".into(), (0..=255u8).rev().collect()),
        ("at capacity".into(), vec![0xA5; CAPACITY]),
    ];

    // Pseudorandom buffers over a spread of lengths, including lengths either
    // side of the eight-bit and word boundaries the place-value writers in later
    // slices will care about.
    let mut rng = Rng(0x0123_4567_89AB_CDEF);
    for len in [
        1usize, 2, 3, 7, 8, 9, 15, 16, 17, 31, 63, 100, 255, 256, 511, 1000,
    ] {
        let buf: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        out.push((format!("random, len {len}"), buf));
    }
    out
}

// --- The corpus's own coverage -------------------------------------------

#[test]
fn the_corpus_covers_what_it_claims_to() {
    let c = corpus();
    assert!(c.len() >= 20, "corpus is too small: {}", c.len());
    assert!(
        c.iter().any(|(_, b)| b.is_empty()),
        "no empty buffer, so the zero-length path is untested"
    );
    assert!(
        c.iter().any(|(_, b)| b.iter().any(|&x| x >= 0x80)),
        "no byte at or above 0x80, so `Byte as Word` zero-extension is untested"
    );
    assert!(
        c.iter().any(|(_, b)| b.len() >= 1000),
        "no long buffer, so the loop is only exercised over short inputs"
    );
    assert!(
        c.iter().any(|(_, b)| b.len() == CAPACITY),
        "nothing at the array capacity, so the `limit` boundary is untested"
    );
    // Distinct inputs must give distinct checksums, or the harness is ignoring
    // its input and every later agreement is vacuous.
    let a = keleusma_wire::crc32(&[0x00]);
    let b = keleusma_wire::crc32(&[0x01]);
    assert_ne!(a, b, "the oracle itself does not discriminate single bytes");
}

// --- must-not-fire: the correct source agrees with the oracle ------------

#[test]
fn the_published_check_value_is_reproduced() {
    // The standard CRC-32/ISO-HDLC check constant. Agreement pins the
    // polynomial, the reflection, and both the initial and final XOR at once,
    // against a value neither implementation here produced.
    let got = crc_in_keleusma(WIRE_KEL, b"123456789");
    assert_eq!(
        got as u32, 0xCBF4_3926,
        "wire.kel disagrees with the published CRC-32/ISO-HDLC check value"
    );
}

#[test]
fn wire_kel_agrees_with_the_reference_across_the_corpus() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run");
        let want = keleusma_wire::crc32(&buf);
        assert_eq!(
            got as u32,
            want,
            "case `{name}` (len {}): wire.kel gave {got:#010x}, reference gave {want:#010x}",
            buf.len()
        );
    }
}

// --- must-fire: the same harness must report a broken implementation -----

/// Corpus cases on which a POLYNOMIAL mutation is undetectable, with the reason
/// each is inherent rather than a gap to close.
///
/// - `empty` folds no bytes, so no shift ever runs.
/// - `one high byte` is the single-byte case `0xFF`, and it is the ONLY one:
///   `0xFFFFFFFF xor 0xFF` is `0xFFFFFF00`, whose low eight bits are clear, so
///   all eight iterations take the else branch and the polynomial is never
///   consulted. Enumerating all 256 single-byte inputs confirms `0xFF` is unique
///   in this. Both the correct and the mutated source return `0xFF000000`.
const POLYNOMIAL_BLIND_CASES: &[&str] = &["empty", "one high byte"];

#[test]
fn a_mutated_polynomial_is_reported() {
    let mutant = mutate(WIRE_KEL, "0xEDB88320", "0xEDB88321");
    let mut vm = vm_for(&mutant);
    let mut agreed: Vec<String> = Vec::new();
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run") as u32;
        if got == keleusma_wire::crc32(&buf) {
            agreed.push(name);
        }
    }
    // Asserted as an exact set rather than a count. A case that starts agreeing
    // is a blind spot that must be explained and added deliberately, and a case
    // that stops agreeing means the reasoning above no longer holds.
    assert_eq!(
        agreed, POLYNOMIAL_BLIND_CASES,
        "the set of cases blind to a polynomial mutation changed; \
         explain the difference before editing this expectation"
    );
    // Guard against the whole corpus being blind, which the equality above would
    // also accept if `corpus()` were ever reduced to those two cases.
    assert!(
        corpus().len() > POLYNOMIAL_BLIND_CASES.len() + 10,
        "too few discriminating cases for this control to mean anything"
    );
}

#[test]
fn a_mutated_initial_value_is_reported() {
    // A second, independent mutation. `0xFFFFFFFF` appears twice in the source,
    // so the anchor includes the assignment to pick out only the seed.
    let mutant = mutate(WIRE_KEL, "crc.acc = 0xFFFFFFFF;", "crc.acc = 0xFFFFFFFE;");
    let mut vm = vm_for(&mutant);
    let mut agreed = Vec::new();
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run") as u32;
        if got == keleusma_wire::crc32(&buf) {
            agreed.push(name);
        }
    }
    // A changed seed shifts the result for every input, including the empty one.
    assert!(
        agreed.is_empty(),
        "the differential failed to report a mutated initial value on: {agreed:?}"
    );
}

#[test]
fn a_mutated_inner_iteration_count_is_reported() {
    // A third mutation, in the loop bound rather than a constant, so a defect in
    // the iteration structure is covered as well as one in the arithmetic.
    let mutant = mutate(WIRE_KEL, "for j in 0..8 {", "for j in 0..7 {");
    let mut vm = vm_for(&mutant);
    let mut agreed_on_nonempty = Vec::new();
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run") as u32;
        if got == keleusma_wire::crc32(&buf) && !buf.is_empty() {
            agreed_on_nonempty.push(name);
        }
    }
    assert!(
        agreed_on_nonempty.is_empty(),
        "the differential failed to report a shortened inner loop on: {agreed_on_nonempty:?}"
    );
}

// --- Properties the source's comments assert -----------------------------

#[test]
fn the_accumulator_is_reset_so_repeated_calls_are_independent() {
    // The checksum state lives in a data block because locals are immutable, so
    // it persists across calls. `main` calls `crc_begin`, which must make a
    // second call on the SAME `Vm` give the same answer as the first. Without
    // that reset the second message would continue the first.
    let mut vm = vm_for(WIRE_KEL);
    let first = run_crc_on(&mut vm, b"123456789", 9).expect("run");
    let second = run_crc_on(&mut vm, b"123456789", 9).expect("run");
    let third = run_crc_on(&mut vm, b"different", 9).expect("run");
    let fourth = run_crc_on(&mut vm, b"123456789", 9).expect("run");
    assert_eq!(
        first, second,
        "a repeated call did not reset the accumulator"
    );
    assert_eq!(
        first, fourth,
        "an intervening message left state behind: {first:#x} then {fourth:#x}"
    );
    assert_ne!(first, third, "two different messages checksummed the same");
}

#[test]
fn the_result_stays_inside_the_thirty_two_bit_range() {
    // The source claims `acc` is always in [0, 2^32), which is why it carries no
    // masking. This observes the FINAL value only, so it is necessary and not
    // sufficient: it would catch a high bit that escaped and survived to the
    // end, and it would not catch one that was shifted away in between. The
    // sufficient evidence is agreement with the oracle across the corpus.
    let mut vm = vm_for(WIRE_KEL);
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run");
        assert!(
            (0..=0xFFFF_FFFFi64).contains(&got),
            "case `{name}`: checksum {got:#x} left the unsigned 32-bit range"
        );
    }
}

#[test]
fn arithmetic_and_logical_shift_agree_here_because_the_accumulator_is_never_negative() {
    // Documented in wire.kel: the range invariant makes `asr` and `lsr` compute
    // the same values in this function, so swapping them is NOT caught by the
    // differential. That is pinned here so a reader who notices the freedom
    // finds it recorded as understood rather than as an untested assumption.
    //
    // `lsr` remains the correct spelling: it is the operation the algorithm
    // calls for, and it stays correct if the invariant is ever weakened.
    let mutant = mutate(
        WIRE_KEL,
        "(crc.acc lsr 1) bxor 0xEDB88320",
        "(crc.acc asr 1) bxor 0xEDB88320",
    );
    let mutant = mutate(&mutant, "crc.acc lsr 1\n", "crc.acc asr 1\n");
    let mut vm = vm_for(&mutant);
    for (name, buf) in corpus() {
        let got = run_crc_on(&mut vm, &buf, buf.len() as i64).expect("run") as u32;
        assert_eq!(
            got,
            keleusma_wire::crc32(&buf),
            "case `{name}`: `asr` and `lsr` diverged, so the range invariant no longer holds \
             and wire.kel's comment is wrong"
        );
    }
}

// --- Hostile input -------------------------------------------------------

#[test]
fn a_length_beyond_the_capacity_traps_rather_than_truncating() {
    // A `len` larger than the array must not checksum a silently truncated
    // prefix. The loop's `limit` caps iteration at the capacity, so the indices
    // stay in bounds and the overrun then trips the cap.
    let mut vm = vm_for(WIRE_KEL);
    let buf = vec![0x5Au8; CAPACITY];
    let err = run_crc_on(&mut vm, &buf, (CAPACITY + 1) as i64)
        .expect_err("an over-long length must not succeed");
    let text = format!("{err:?}");
    assert!(
        text.contains("LoopLimitExceeded"),
        "expected a loop-limit trap, got {text}"
    );
}

#[test]
fn a_length_shorter_than_the_buffer_checksums_only_the_prefix() {
    // The declared length, not the array capacity, bounds the message. Seeding
    // trailing bytes that are not counted must not change the answer.
    let mut vm = vm_for(WIRE_KEL);
    let mut buf = b"123456789".to_vec();
    buf.extend_from_slice(b"IGNORED TAIL");
    let got = run_crc_on(&mut vm, &buf, 9).expect("run");
    assert_eq!(
        got as u32, 0xCBF4_3926,
        "bytes past the declared length were included in the checksum"
    );
}
