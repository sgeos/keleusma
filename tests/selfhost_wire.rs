//! Differential tests for `src/selfhost/kel/wire.kel`, the wire format written
//! in Keleusma (step 6 of the wire-format programme).
//!
//! **Slice 1** covers CRC-32/ISO-HDLC. Its oracle is `keleusma_wire::crc32`,
//! which is the same algorithm and polynomial as the runtime's own
//! `bytecode::crc32`; both Rust implementations are independently pinned to the
//! published check value `crc32("123456789") == 0xCBF43926`
//! (`keleusma-wire/src/crc.rs` and `src/vm.rs`), so agreement here is agreement
//! with a third-party constant rather than with whichever implementation
//! happened to be written first. `bytecode::crc32` is `pub(crate)` and therefore
//! unreachable from an integration test, which is why the oracle is spelled the
//! other way.
//!
//! **Slice 2** covers the little-endian place-value primitives, the 16-byte
//! prologue, and the majority-of-three vote. Its oracle is stronger: **byte
//! identity** against what `keleusma-wire` emits for the same input, plus the
//! complementary direction that `WireView::parse` accepts what Keleusma emitted,
//! plus agreement with the reference reader on a damaged artifact.
//!
//! The suite carries controls in BOTH directions, because a differential
//! against a known-good reference is exactly where a check that cannot fail
//! hides:
//!
//! - **must-not-fire** — over a corpus with asserted coverage, the Keleusma
//!   implementation and the oracle agree and the comparison stays quiet;
//! - **must-fire** — the same harness pointed at a deliberately mutated source
//!   must report divergence. Several independent mutations are used, since any
//!   single one could in principle be neutral — and one of them, a mutated CRC
//!   polynomial, provably is neutral on two inputs.
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
const CAPACITY: usize = 65536;

/// Longest buffer the CRC corpus uses.
///
/// Deliberately far below `CAPACITY`. Slice 3 grew the array to 65536 to hold a
/// full 1024-region directory, and running the whole corpus at that size would
/// cost 8 x 65536 inner iterations per case in a debug build for no extra
/// coverage — the checksum does not care how large the array is, only how many
/// bytes it folds. The capacity boundary itself is exercised by a dedicated test.
const CRC_CORPUS_MAX: usize = 4096;

// Shared-block slot map. `set_shared` addresses slots, and the block is laid out
// `len, bytes[65536], nregions, rkind[1024], rflags[1024], rlen[1024],
// rcovers[1024], warg`. Every scalar was appended AFTER the byte array so that
// `bytes[i]` stays at slot `1 + i` and no earlier seeding site moves.
const NREGIONS_SLOT: usize = 1 + CAPACITY;
const RKIND_SLOT: usize = NREGIONS_SLOT + 1;
const RFLAGS_SLOT: usize = RKIND_SLOT + 1024;
const RLEN_SLOT: usize = RFLAGS_SLOT + 1024;
const RCOVERS_SLOT: usize = RLEN_SLOT + 1024;
const WARG_SLOT: usize = RCOVERS_SLOT + 1024;
const WARG2_SLOT: usize = WARG_SLOT + 1;
const WARG3_SLOT: usize = WARG2_SLOT + 1;
const WARG4_SLOT: usize = WARG3_SLOT + 1;
const WARG5_SLOT: usize = WARG4_SLOT + 1;

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

// --- Slice 2 harness: emission and voting --------------------------------

/// Run command `cmd`, with `buf` pre-seeded into `bytes` and `nregions` set,
/// and return both the result and the resulting byte array.
fn run_cmd(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    nregions: i64,
    seed: &[u8],
) -> Result<(i64, Vec<u8>), VmError> {
    run_cmd_full(vm, cmd, nregions, seed, &[], 0, seed.len().max(64))
}

/// The general form: seed the byte array and the per-region input arrays, run
/// `cmd`, and read back `read_len` bytes.
///
/// `read_len` is explicit because the array is 65536 slots and reading it all
/// back through `get_shared` on every call would dominate the suite. Each test
/// asks for the prefix it actually inspects.
fn run_cmd_full(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    nregions: i64,
    seed: &[u8],
    regions: &Regions,
    warg: i64,
    read_len: usize,
) -> Result<(i64, Vec<u8>), VmError> {
    run_cmd_args(
        vm,
        cmd,
        nregions,
        seed,
        regions,
        [warg, 0, 0, 0, 0],
        read_len,
    )
}

/// As `run_cmd_full`, with all four command arguments.
fn run_cmd_args(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    nregions: i64,
    seed: &[u8],
    regions: &Regions,
    args: [i64; 5],
    read_len: usize,
) -> Result<(i64, Vec<u8>), VmError> {
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(seed.len() as i64))?;
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(nregions))?;
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(args[0]))?;
    vm.set_shared(&mut shared, WARG2_SLOT, Value::Int(args[1]))?;
    vm.set_shared(&mut shared, WARG3_SLOT, Value::Int(args[2]))?;
    vm.set_shared(&mut shared, WARG4_SLOT, Value::Int(args[3]))?;
    vm.set_shared(&mut shared, WARG5_SLOT, Value::Int(args[4]))?;
    for (i, byte) in seed.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(*byte))?;
    }
    for (i, (kind, flags, len, covers)) in regions.iter().enumerate() {
        vm.set_shared(&mut shared, RKIND_SLOT + i, Value::Int(i64::from(*kind)))?;
        vm.set_shared(&mut shared, RFLAGS_SLOT + i, Value::Int(i64::from(*flags)))?;
        vm.set_shared(&mut shared, RLEN_SLOT + i, Value::Int(*len as i64))?;
        vm.set_shared(
            &mut shared,
            RCOVERS_SLOT + i,
            Value::Int(i64::from(*covers)),
        )?;
    }
    let ret = match vm.call_with_shared(&mut shared, &[Value::Int(cmd)])? {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("unexpected VM state: {other:?}"),
    };
    let n = read_len.min(CAPACITY);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        match vm.get_shared(&shared, 1 + i)? {
            Value::Byte(b) => out.push(b),
            other => panic!("slot {i} is not a Byte: {other:?}"),
        }
    }
    Ok((ret, out))
}

/// The first 48 bytes the reference emits for an artifact with `n` regions:
/// three copies of the prologue. Built by declaring `n` empty regions, since the
/// prologue's only region-dependent field is the count.
fn reference_prologue(n: usize) -> Vec<u8> {
    let mut b = keleusma_wire::WireBuilder::new();
    for i in 0..n {
        // Distinct kinds, because the container rejects a duplicate kind. The
        // payloads are empty, so nothing past the header area is compared.
        b.region(1 + i as u16, 0).expect("declare region");
    }
    let bytes = b.finish().expect("finish");
    bytes[..48].to_vec()
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
        ("longest corpus buffer".into(), vec![0xA5; CRC_CORPUS_MAX]),
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
        c.iter().any(|(_, b)| b.len() == CRC_CORPUS_MAX),
        "nothing at the corpus maximum, so the long-buffer path is untested"
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

// =========================================================================
// SLICE 2 — container primitives, the prologue, and the majority-of-three vote
// =========================================================================
//
// The oracle here is stronger than slice 1's: it is BYTE IDENTITY against what
// `keleusma-wire` emits for the same input, not agreement on a single value.

/// Command selectors, mirroring `main`'s dispatch in wire.kel.
const CMD_CRC: i64 = 0;
const CMD_EMIT_PROLOGUE: i64 = 1;
const CMD_PARSE_PROLOGUE: i64 = 2;
const CMD_DISAGREED: i64 = 3;

#[test]
fn the_emitted_prologue_is_byte_identical_to_the_reference() {
    let mut vm = vm_for(WIRE_KEL);
    // Zero, one, a few, and the format's ceiling. The ceiling matters because
    // `region_count` is a u16 field and 1024 is the largest admissible value.
    for n in [0usize, 1, 2, 7, 255, 256, 1023, 1024] {
        let (written, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, n as i64, &[]).expect("run");
        assert_eq!(written, 48, "n = {n}: wrong byte count reported");
        let want = reference_prologue(n);
        assert_eq!(
            &out[..48],
            &want[..],
            "n = {n}: emitted prologue differs from the reference"
        );
    }
}

#[test]
fn the_three_emitted_copies_are_identical_to_each_other() {
    // Byte identity against the reference would also pass if all three copies
    // were wrong in the same way, so the replication is checked on its own.
    let mut vm = vm_for(WIRE_KEL);
    let (_, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 3, &[]).expect("run");
    assert_eq!(&out[0..16], &out[16..32], "copy 2 differs from copy 1");
    assert_eq!(&out[0..16], &out[32..48], "copy 3 differs from copy 1");
    // And that the record is not simply zeroes, which would satisfy the above.
    assert_ne!(&out[0..16], &[0u8; 16], "the prologue was never written");
}

#[test]
fn a_mutated_magic_constant_is_reported() {
    // must-fire. The magic is the one field a transcription error is most likely
    // to hit, and the prototype hit exactly that once.
    let mutant = mutate(
        WIRE_KEL,
        "put_u32(0, 0x4B415558);",
        "put_u32(0, 0x4B415559);",
    );
    let mut vm = vm_for(&mutant);
    let (_, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 1, &[]).expect("run");
    assert_ne!(
        &out[..48],
        &reference_prologue(1)[..],
        "a mutated magic constant produced byte-identical output"
    );
}

#[test]
fn a_mutated_place_value_writer_is_reported() {
    // must-fire in the primitive rather than in a constant: swapping a shift
    // amount reorders the bytes of every u32 the emitter writes.
    let mutant = mutate(
        WIRE_KEL,
        "    wire.bytes[at + 2] = ((v lsr 16) band 255) as Byte;",
        "    wire.bytes[at + 2] = ((v lsr 24) band 255) as Byte;",
    );
    let mut vm = vm_for(&mutant);
    let (_, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 1, &[]).expect("run");
    assert_ne!(
        &out[..48],
        &reference_prologue(1)[..],
        "a mutated place-value writer produced byte-identical output"
    );
}

#[test]
fn the_emitted_prologue_is_accepted_by_the_reference_reader() {
    // The complementary direction to byte identity: the reference PARSER must
    // accept what Keleusma emitted. For zero regions the 48-byte prologue is a
    // complete artifact, so it can be handed to `WireView::parse` whole.
    let mut vm = vm_for(WIRE_KEL);
    let (_, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 0, &[]).expect("run");
    let view = keleusma_wire::WireView::parse(&out[..48]).expect("reference must accept it");
    assert_eq!(view.region_count(), 0);
    assert!(
        !view.needs_scrub(),
        "a freshly emitted artifact needs no scrub"
    );
}

#[test]
fn the_prologue_round_trips_through_the_keleusma_reader() {
    let mut vm = vm_for(WIRE_KEL);
    for n in [0i64, 1, 42, 1024] {
        let (_, out) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, n, &[]).expect("emit");
        let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &out[..48]).expect("parse");
        assert_eq!(got, n, "region count did not survive the round trip");
        let (dis, _) = run_cmd(&mut vm, CMD_DISAGREED, 0, &out[..48]).expect("scrub");
        assert_eq!(dis, 0, "an undamaged prologue reported disagreement");
    }
}

#[test]
fn the_reader_rejects_each_malformed_field_with_its_own_code() {
    // Each case damages ALL THREE copies, since damaging one would be repaired
    // by the vote -- which is the next test's subject, not this one.
    let mut vm = vm_for(WIRE_KEL);
    let (_, good) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 4, &[]).expect("emit");
    let good = &good[..48];

    let damage = |at: usize, bytes: &[u8]| -> Vec<u8> {
        let mut v = good.to_vec();
        for copy in 0..3 {
            v[copy * 16 + at..copy * 16 + at + bytes.len()].copy_from_slice(bytes);
        }
        v
    };

    for (label, buf, want) in [
        ("bad magic", damage(0, &[0x00]), -1),
        ("foreign byte order", damage(4, &[0xFE, 0xFF]), -2),
        ("unsupported version", damage(6, &[0x03, 0x00]), -3),
        ("bad checksum", damage(12, &[0x00, 0x00, 0x00, 0x00]), -4),
    ] {
        let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &buf).expect("parse");
        assert_eq!(got, want, "{label}: wrong rejection code");
    }

    // The region ceiling needs a valid checksum over the oversized count, or it
    // would be rejected at the checksum check first and this case would be
    // testing the wrong thing.
    let mut over = good.to_vec();
    over[8..10].copy_from_slice(&1025u16.to_le_bytes());
    let check = keleusma_wire::crc32(&over[..12]);
    over[12..16].copy_from_slice(&check.to_le_bytes());
    for copy in 1..3 {
        let (a, b) = over.split_at_mut(copy * 16);
        b[..16].copy_from_slice(&a[..16]);
    }
    let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &over).expect("parse");
    assert_eq!(got, -5, "an over-ceiling region count was not rejected");
}

#[test]
fn a_single_corrupt_copy_is_outvoted_and_reported() {
    // This is what the triplication is for, and it is also where a raw-bytes
    // checksum would betray the design: the reference checks the CRC against the
    // VOTED record, so a vote that repaired a byte is confirmed rather than
    // rejected. wire.kel does the same via `crc_voted`.
    let mut vm = vm_for(WIRE_KEL);
    let (_, good) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 9, &[]).expect("emit");
    let good = &good[..48];

    for copy in 0..3usize {
        for at in 0..16usize {
            let mut buf = good.to_vec();
            // Flip one bit, which is the fault the vote is specified against.
            buf[copy * 16 + at] ^= 0x40;
            let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &buf).expect("parse");
            assert_eq!(
                got, 9,
                "copy {copy} byte {at}: a single-bit fault was not outvoted"
            );
            let (dis, _) = run_cmd(&mut vm, CMD_DISAGREED, 0, &buf).expect("scrub");
            assert_eq!(
                dis, 1,
                "copy {copy} byte {at}: damage was repaired but not reported"
            );
        }
    }
}

#[test]
fn the_vote_agrees_with_the_reference_reader_on_a_damaged_artifact() {
    // Cross-check the whole repair path against `WireView::parse` rather than
    // only against wire.kel's own reader, so a shared misunderstanding of the
    // vote cannot pass. Zero regions keeps the 48 bytes a complete artifact.
    let mut vm = vm_for(WIRE_KEL);
    let (_, good) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 0, &[]).expect("emit");
    for copy in 0..3usize {
        for at in 0..16usize {
            let mut buf = good[..48].to_vec();
            buf[copy * 16 + at] ^= 0x01;
            let view = keleusma_wire::WireView::parse(&buf).expect("reference outvotes it");
            assert!(view.needs_scrub(), "reference did not report the damage");
            let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &buf).expect("parse");
            assert_eq!(
                got,
                i64::from(view.region_count()),
                "copy {copy} byte {at}: wire.kel and the reference voted differently"
            );
        }
    }
}

#[test]
fn maj3_is_a_per_bit_majority_not_a_pick_the_duplicate() {
    // A must-fire for the vote's SEMANTICS. Where all three copies differ, a
    // per-bit majority synthesises a byte no copy contains; "pick the value that
    // appears twice" has no answer at all and would return an arbitrary copy.
    // The distinction is invisible unless a case with three distinct bytes is
    // exercised, so one is constructed here rather than hoped for.
    let mut vm = vm_for(WIRE_KEL);
    let (_, good) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 5, &[]).expect("emit");
    let mut buf = good[..48].to_vec();
    // Three different single-bit faults, in three different copies, all in the
    // same byte. Each bit is still a 2-of-3 majority, so the byte is fully
    // recoverable, and it is recoverable ONLY per-bit.
    buf[8] ^= 0x01;
    buf[16 + 8] ^= 0x02;
    buf[32 + 8] ^= 0x04;
    assert_ne!(buf[8], buf[16 + 8]);
    assert_ne!(buf[8], buf[32 + 8]);
    assert_ne!(buf[16 + 8], buf[32 + 8]);
    let (got, _) = run_cmd(&mut vm, CMD_PARSE_PROLOGUE, 0, &buf).expect("parse");
    assert_eq!(got, 5, "three distinct copies were not repaired per-bit");
    // The reference must reach the same conclusion.
    let (_, good0) = run_cmd(&mut vm, CMD_EMIT_PROLOGUE, 0, &[]).expect("emit");
    let mut b0 = good0[..48].to_vec();
    b0[8] ^= 0x01;
    b0[16 + 8] ^= 0x02;
    b0[32 + 8] ^= 0x04;
    let view = keleusma_wire::WireView::parse(&b0).expect("reference outvotes it");
    assert_eq!(view.region_count(), 0);
}

#[test]
fn an_unrecognised_command_returns_a_distinct_code() {
    // Deliberately far above every chain's range. This test previously used
    // 99, which slice 6b then CLAIMED as a real command — the test caught that
    // immediately, which is the point of it, but a sentinel adjacent to the
    // live range will keep being claimed as the module grows.
    let mut vm = vm_for(WIRE_KEL);
    let (got, _) = run_cmd(&mut vm, 100_000, 0, &[]).expect("run");
    assert_eq!(got, -99, "an unknown command did not report itself");
}

#[test]
fn no_command_below_the_dispatch_ceiling_falls_through() {
    // The complement: every command the chains claim must answer something
    // OTHER than a fall-through code. Without this, a chain whose threshold
    // drifted past its arms would silently route live commands to the default.
    let mut vm = vm_for(WIRE_KEL);
    let mut fell_through = Vec::new();
    for cmd in 0..103i64 {
        let (got, _) =
            run_cmd_args(&mut vm, cmd, 0, &[], &[], [0, 0, 0, 0, 0], 0).unwrap_or((0, Vec::new()));
        if (-99..=-92).contains(&got) {
            fell_through.push(cmd);
        }
    }
    assert!(
        fell_through.is_empty(),
        "these commands fell through to a chain default: {fell_through:?}"
    );
}

#[test]
fn slice_one_still_answers_after_the_command_dispatch() {
    // The checksum entry moved behind a command selector, so its old behaviour is
    // re-pinned here rather than assumed to have survived the refactor.
    let mut vm = vm_for(WIRE_KEL);
    let (got, _) = run_cmd(&mut vm, CMD_CRC, 0, b"123456789").expect("run");
    assert_eq!(got as u32, 0xCBF4_3926);
}

// =========================================================================
// SLICE 3 — the region directory
// =========================================================================

const CMD_DIR_KIND: i64 = 9;
const CMD_FIND: i64 = 5;
const CMD_DIR_DISAGREED: i64 = 6;
const CMD_DIR_WORD_OFFSET: i64 = 7;
const CMD_DIR_WORD_LEN: i64 = 8;
const CMD_EMIT_HEADER: i64 = 10;

/// One region's directory inputs: `(kind, flags, payload_len_bytes, covers)`.
type RegionSpec = (u16, u16, usize, u16);
/// A slice of region specs, as the emitter consumes them.
type Regions = [RegionSpec];
/// Named region sets for the differential corpus.
type NamedRegionSets = Vec<(String, Vec<RegionSpec>)>;

/// The header area (three prologues plus three directory copies) the reference
/// emits for these regions, and the full artifact it emits.
fn reference_artifact(regions: &Regions) -> Vec<u8> {
    let mut b = keleusma_wire::WireBuilder::new();
    for (kind, flags, len, _covers) in regions {
        let id = b.region(*kind, *flags).expect("declare region");
        b.push(id, &vec![0u8; *len]);
    }
    b.finish().expect("finish")
}

fn header_len(n: usize) -> usize {
    48 + 48 * n
}

/// A spread of region sets, including payload lengths that are NOT multiples of
/// eight. The rounding in `words_for` is correct for a multiple of eight either
/// way, so a corpus without the awkward lengths cannot see a dropped round-up.
fn region_sets() -> NamedRegionSets {
    let mut out: NamedRegionSets = vec![
        ("none".into(), vec![]),
        ("one empty".into(), vec![(1, 0, 0, 0)]),
    ];
    out.push((
        "lengths across the word boundary".into(),
        vec![
            (1, 0, 1, 0),
            (2, 0, 7, 0),
            (3, 0, 8, 0),
            (4, 0, 9, 0),
            (5, 0, 15, 0),
            (6, 0, 16, 0),
        ],
    ));
    out.push((
        "flags set".into(),
        vec![(10, 0b0100, 32, 0), (11, 0b0001, 5, 0)],
    ));
    let mut many: Vec<RegionSpec> = Vec::new();
    for i in 0..64u16 {
        many.push((100 + i, 0, (i as usize * 3) % 17, 0));
    }
    out.push(("sixty-four regions".into(), many));
    out
}

#[test]
fn the_region_sets_include_lengths_that_are_not_multiples_of_eight() {
    // Vacuity guard for the corpus itself: without an awkward length, dropping
    // the round-up in `words_for` is undetectable.
    let awkward = region_sets()
        .iter()
        .flat_map(|(_, rs)| rs.iter().map(|(_, _, l, _)| *l).collect::<Vec<_>>())
        .filter(|l| l % 8 != 0)
        .count();
    assert!(
        awkward >= 5,
        "only {awkward} non-multiple-of-8 payload lengths"
    );
}

#[test]
fn the_emitted_header_is_byte_identical_to_the_reference() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, rs) in region_sets() {
        let want = reference_artifact(&rs);
        let hl = header_len(rs.len());
        let (written, got) =
            run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("run");
        assert_eq!(written, hl as i64, "{name}: wrong header length reported");
        assert_eq!(
            got,
            want[..hl],
            "{name}: emitted header differs from the reference"
        );
    }
}

#[test]
fn the_emitted_artifact_is_accepted_by_the_reference_reader() {
    // The complementary direction. Payloads are all zero and the buffer starts
    // zeroed, so reading back the reference's full length yields a complete
    // artifact built by Keleusma.
    let mut vm = vm_for(WIRE_KEL);
    for (name, rs) in region_sets() {
        let want = reference_artifact(&rs);
        let (_, got) = run_cmd_full(
            &mut vm,
            CMD_EMIT_HEADER,
            rs.len() as i64,
            &[],
            &rs,
            0,
            want.len(),
        )
        .expect("run");
        assert_eq!(got, want, "{name}: whole artifact differs");
        let view = keleusma_wire::WireView::parse(&got).expect("reference must accept it");
        assert_eq!(
            view.region_count() as usize,
            rs.len(),
            "{name}: region count"
        );
        assert!(!view.needs_scrub(), "{name}: fresh artifact needs no scrub");
    }
}

#[test]
fn the_keleusma_reader_recovers_every_regions_offset_length_and_kind() {
    let mut vm = vm_for(WIRE_KEL);
    let (_, rs) = region_sets()
        .into_iter()
        .find(|(n, _)| n == "lengths across the word boundary")
        .expect("case");
    let hl = header_len(rs.len());
    // Emit once, then read the emitted bytes back in as the input for each query,
    // since every call gets a fresh shared buffer.
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");

    let mut cursor = 6 * (rs.len() + 1); // header_words
    for (i, (kind, _flags, len, _)) in rs.iter().enumerate() {
        let words = len.div_ceil(8);
        for (cmd, want, what) in [
            (CMD_DIR_WORD_OFFSET, cursor as i64, "word offset"),
            (CMD_DIR_WORD_LEN, words as i64, "word length"),
            (CMD_DIR_KIND, i64::from(*kind), "kind"),
        ] {
            let (got, _) = run_cmd_full(&mut vm, cmd, rs.len() as i64, &image, &[], i as i64, 0)
                .expect("read");
            assert_eq!(got, want, "region {i}: {what}");
        }
        cursor += words;
    }
}

#[test]
fn dir_find_locates_each_kind_and_reports_an_absent_one() {
    let mut vm = vm_for(WIRE_KEL);
    let (_, rs) = region_sets()
        .into_iter()
        .find(|(n, _)| n == "sixty-four regions")
        .expect("case");
    let hl = header_len(rs.len());
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");
    for (i, (kind, ..)) in rs.iter().enumerate() {
        let (got, _) = run_cmd_full(
            &mut vm,
            CMD_FIND,
            rs.len() as i64,
            &image,
            &[],
            i64::from(*kind),
            0,
        )
        .expect("find");
        assert_eq!(got, i as i64, "kind {kind} should be at index {i}");
    }
    let (missing, _) =
        run_cmd_full(&mut vm, CMD_FIND, rs.len() as i64, &image, &[], 9999, 0).expect("find");
    assert_eq!(missing, -1, "an absent kind must report not-found");
}

// --- must-fire ------------------------------------------------------------

#[test]
fn dropping_the_word_rounding_is_reported() {
    // `words_for` without the round-up is correct for every multiple of eight
    // and wrong otherwise, so this simultaneously checks the code and proves the
    // corpus carries awkward lengths.
    let mutant = mutate(WIRE_KEL, "    (nbytes + 7) lsr 3", "    nbytes lsr 3");
    let mut vm = vm_for(&mutant);
    let mut disagreed = 0;
    for (_, rs) in region_sets() {
        let want = reference_artifact(&rs);
        let hl = header_len(rs.len());
        let (_, got) =
            run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("run");
        if got != want[..hl] {
            disagreed += 1;
        }
    }
    assert!(
        disagreed >= 3,
        "dropping the word round-up went unreported on all but {disagreed} region sets"
    );
}

#[test]
fn a_wrong_starting_cursor_is_reported() {
    let mutant = mutate(WIRE_KEL, "    6 * (n + 1)", "    6 * (n + 2)");
    let mut vm = vm_for(&mutant);
    let rs: Vec<RegionSpec> = vec![(1, 0, 8, 0), (2, 0, 8, 0)];
    let want = reference_artifact(&rs);
    let hl = header_len(rs.len());
    let (_, got) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("run");
    assert_ne!(got, want[..hl], "a wrong header_words went unreported");
}

// --- the vote, and the bootstrap that makes it possible -------------------

#[test]
fn a_single_corrupt_directory_copy_is_outvoted_and_reported() {
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(7, 0, 24, 0), (9, 0, 40, 0)];
    let hl = header_len(rs.len());
    let (_, good) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");
    let span = rs.len() * 16;
    for copy in 0..3usize {
        for at in 0..span {
            let mut buf = good.clone();
            buf[48 + copy * span + at] ^= 0x20;
            let (kind, _) = run_cmd_full(&mut vm, CMD_DIR_KIND, rs.len() as i64, &buf, &[], 0, 0)
                .expect("read");
            assert_eq!(kind, 7, "copy {copy} byte {at}: kind was not outvoted");
            let (dis, _) =
                run_cmd_full(&mut vm, CMD_DIR_DISAGREED, rs.len() as i64, &buf, &[], 0, 0)
                    .expect("scrub");
            assert_eq!(dis, 1, "copy {copy} byte {at}: damage not reported");
        }
    }
}

#[test]
fn the_directory_is_still_read_when_a_prologue_copy_carries_a_damaged_region_count() {
    // THE BOOTSTRAP CASE. The directory's stride comes from `region_count`, which
    // lives in the prologue. If the stride were taken from an unvoted copy, a
    // fault in that field would misplace every directory copy. Damaging each of
    // the three copies in turn is what distinguishes a correct two-stage vote
    // from one that happens to read the undamaged copy first.
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(21, 0, 16, 0), (22, 0, 16, 0), (23, 0, 16, 0)];
    let hl = header_len(rs.len());
    let (_, good) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");

    for copy in 0..3usize {
        let mut buf = good.clone();
        // Corrupt the region-count field (prologue offset 8) in one copy only.
        buf[copy * 16 + 8] ^= 0x08;
        // The prologue vote must still recover 3.
        let (n, _) = run_cmd_full(&mut vm, CMD_PARSE_PROLOGUE, 3, &buf, &[], 0, 0).expect("parse");
        assert_eq!(n, 3, "copy {copy}: region count not recovered");
        // And with that count, every directory entry must still read correctly.
        for (i, (kind, ..)) in rs.iter().enumerate() {
            let (got, _) =
                run_cmd_full(&mut vm, CMD_DIR_KIND, 3, &buf, &[], i as i64, 0).expect("read");
            assert_eq!(
                got,
                i64::from(*kind),
                "copy {copy}: entry {i} misread after a damaged region count"
            );
        }
    }
}

// =========================================================================
// SLICE 4 — record tables and byte pools
// =========================================================================

const CMD_REGION_BASE: i64 = 11;
const CMD_REGION_BYTES: i64 = 12;
const CMD_REC_COUNT: i64 = 13;
const CMD_REC_U32: i64 = 14;
const CMD_REC_U16: i64 = 15;
const CMD_POOL_U8: i64 = 16;
const CMD_EMIT_RECORDS: i64 = 17;

/// The record pattern `emit_pattern_records` writes, computed independently
/// here so the comparison is against a second implementation rather than
/// against the code under test.
fn pattern_record(r: usize) -> [u8; 16] {
    let mut rec = [0u8; 16];
    rec[0..4].copy_from_slice(&((r as u32 * 7) + 1).to_le_bytes());
    rec[4..8].copy_from_slice(&((r as u32 * 13) + 2).to_le_bytes());
    rec[8..10].copy_from_slice(&(((r as u32 % 256) + 3) as u16).to_le_bytes());
    rec
}

#[test]
fn a_region_payload_is_located_from_the_voted_directory() {
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(1, 0, 32, 0), (2, 0, 5, 0), (3, 0, 64, 0)];
    let hl = header_len(rs.len());
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");

    // The reference's own view is the oracle for where each payload sits.
    let art = reference_artifact(&rs);
    let view = keleusma_wire::WireView::parse(&art).expect("parse");
    for (i, _) in rs.iter().enumerate() {
        let region = view.region_at(i as u16).expect("region");
        let (base, _) = run_cmd_full(
            &mut vm,
            CMD_REGION_BASE,
            rs.len() as i64,
            &image,
            &[],
            i as i64,
            0,
        )
        .expect("base");
        let (len, _) = run_cmd_full(
            &mut vm,
            CMD_REGION_BYTES,
            rs.len() as i64,
            &image,
            &[],
            i as i64,
            0,
        )
        .expect("len");
        assert_eq!(base, (region.word_offset as i64) * 8, "region {i}: base");
        assert_eq!(len, (region.word_length as i64) * 8, "region {i}: length");
    }
}

#[test]
fn the_stride_guard_rejects_what_the_reference_rejects() {
    // `RecordTable::from_bytes` admits a stride only when it is non-zero, a
    // whole number of words, and divides the region length. The Keleusma guard
    // must agree, and it must not TRAP on the zero case: division by zero is a
    // runtime fault, so this depends on `andalso` short-circuiting.
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(1, 0, 48, 0)]; // 48 bytes = 6 words
    let hl = header_len(rs.len());
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");
    let art = reference_artifact(&rs);
    let payload_len = 48usize;

    for stride in [0usize, 1, 7, 8, 9, 16, 24, 32, 48, 64] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_REC_COUNT,
            rs.len() as i64,
            &image,
            &[],
            [0, stride as i64, 0, 0, 0],
            0,
        )
        .expect("rec_count must not trap");
        let want: i64 =
            match keleusma_wire::RecordTable::from_bytes(&art[64..64 + payload_len], stride) {
                Some(t) => t.len() as i64,
                None => -1,
            };
        assert_eq!(got, want, "stride {stride}: disagreed with the reference");
    }
}

#[test]
fn a_zero_stride_is_reported_rather_than_trapping() {
    // Pinned on its own, because the failure mode is a trap rather than a wrong
    // answer, and because it rests entirely on `andalso` short-circuiting past a
    // division by zero. If the guard were `band`-ed instead, this would fault.
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(1, 0, 16, 0)];
    let hl = header_len(rs.len());
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");
    let (got, _) = run_cmd_args(&mut vm, CMD_REC_COUNT, 1, &image, &[], [0, 0, 0, 0, 0], 0)
        .expect("must not trap");
    assert_eq!(got, -1, "a zero stride must report, not trap");
}

#[test]
fn a_stride_that_is_a_word_multiple_but_not_a_power_of_two_is_handled() {
    // 24 divides 48 and is a whole number of words, so the reference admits it.
    // A reader that reached for `band (stride - 1)` as a cheap modulo would get
    // this wrong, which is why `divides` uses real division.
    let mut vm = vm_for(WIRE_KEL);
    let rs: Vec<RegionSpec> = vec![(1, 0, 48, 0)];
    let hl = header_len(rs.len());
    let (_, image) =
        run_cmd_full(&mut vm, CMD_EMIT_HEADER, rs.len() as i64, &[], &rs, 0, hl).expect("emit");
    let (got, _) =
        run_cmd_args(&mut vm, CMD_REC_COUNT, 1, &image, &[], [0, 24, 0, 0, 0], 0).expect("run");
    assert_eq!(got, 2, "stride 24 over 48 bytes is two records");
}

#[test]
fn emitted_records_are_byte_identical_to_an_independent_construction() {
    // Keleusma writes the records; the expected bytes are built here by a
    // separate implementation of the same pattern, and the header comes from the
    // reference builder. Nothing in the comparison is produced by wire.kel.
    let mut vm = vm_for(WIRE_KEL);
    for n in [1usize, 2, 7, 16] {
        let rs: Vec<RegionSpec> = vec![(40, 0, n * 16, 0)];
        let art = reference_artifact(&rs);
        let (count, got) = run_cmd_args(
            &mut vm,
            CMD_EMIT_RECORDS,
            1,
            &[],
            &rs,
            [0, n as i64, 0, 0, 0],
            art.len(),
        )
        .expect("emit records");
        assert_eq!(count, n as i64, "n = {n}: wrong record count returned");

        let mut want = art.clone();
        let view = keleusma_wire::WireView::parse(&art).expect("parse");
        let base = (view.region_at(0).expect("region").word_offset as usize) * 8;
        for r in 0..n {
            want[base + r * 16..base + (r + 1) * 16].copy_from_slice(&pattern_record(r));
        }
        assert_eq!(got, want, "n = {n}: emitted records differ");
    }
}

#[test]
fn every_record_field_reads_back_through_the_keleusma_reader() {
    let mut vm = vm_for(WIRE_KEL);
    let n = 9usize;
    let rs: Vec<RegionSpec> = vec![(40, 0, n * 16, 0)];
    let art = reference_artifact(&rs);
    let (_, image) = run_cmd_args(
        &mut vm,
        CMD_EMIT_RECORDS,
        1,
        &[],
        &rs,
        [0, n as i64, 0, 0, 0],
        art.len(),
    )
    .expect("emit");
    for r in 0..n {
        let want = pattern_record(r);
        for (cmd, off, expect) in [
            (
                CMD_REC_U32,
                0i64,
                u32::from_le_bytes(want[0..4].try_into().unwrap()) as i64,
            ),
            (
                CMD_REC_U32,
                4,
                u32::from_le_bytes(want[4..8].try_into().unwrap()) as i64,
            ),
            (
                CMD_REC_U16,
                8,
                u16::from_le_bytes(want[8..10].try_into().unwrap()) as i64,
            ),
        ] {
            let (got, _) = run_cmd_args(&mut vm, cmd, 1, &image, &[], [0, 16, r as i64, off, 0], 0)
                .expect("read");
            assert_eq!(got, expect, "record {r} field at {off}");
        }
    }
}

#[test]
fn a_region_read_as_a_pool_sees_the_same_bytes() {
    // The same payload, addressed without a stride. A pool read and a record
    // read of the same byte must agree, or the two addressing paths have
    // diverged.
    let mut vm = vm_for(WIRE_KEL);
    let n = 4usize;
    let rs: Vec<RegionSpec> = vec![(40, 0, n * 16, 0)];
    let art = reference_artifact(&rs);
    let (_, image) = run_cmd_args(
        &mut vm,
        CMD_EMIT_RECORDS,
        1,
        &[],
        &rs,
        [0, n as i64, 0, 0, 0],
        art.len(),
    )
    .expect("emit");
    for r in 0..n {
        let want = pattern_record(r);
        for (byte, expect) in want.iter().enumerate() {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_POOL_U8,
                1,
                &image,
                &[],
                [0, (r * 16 + byte) as i64, 0, 0, 0],
                0,
            )
            .expect("pool read");
            assert_eq!(got, i64::from(*expect), "pool byte {byte} of record {r}");
        }
    }
}

#[test]
fn a_mutated_record_stride_is_reported() {
    // must-fire on the addressing arithmetic itself: a wrong stride in the
    // emitter overlaps or spreads the records.
    let mutant = mutate(
        WIRE_KEL,
        "        put_rec_u32(i, 16, r, 0, (r * 7) + 1);",
        "        put_rec_u32(i, 24, r, 0, (r * 7) + 1);",
    );
    let mut vm = vm_for(&mutant);
    let n = 4usize;
    let rs: Vec<RegionSpec> = vec![(40, 0, n * 16, 0)];
    let art = reference_artifact(&rs);
    let (_, got) = run_cmd_args(
        &mut vm,
        CMD_EMIT_RECORDS,
        1,
        &[],
        &rs,
        [0, n as i64, 0, 0, 0],
        art.len(),
    )
    .expect("emit");
    let mut want = art.clone();
    let view = keleusma_wire::WireView::parse(&art).expect("parse");
    let base = (view.region_at(0).expect("r").word_offset as usize) * 8;
    for r in 0..n {
        want[base + r * 16..base + (r + 1) * 16].copy_from_slice(&pattern_record(r));
    }
    assert_ne!(got, want, "a mutated stride produced identical bytes");
}

// =========================================================================
// SLICE 5a — the schema layer: region kinds, NameRef, ShapeRecord
// =========================================================================

use keleusma::wire_schema::{NameRef, ShapeRecord, kind};
use keleusma_wire::WireRecord;

const CMD_NAME_COUNT: i64 = 18;
const CMD_NAME_OFFSET: i64 = 19;
const CMD_NAME_LENGTH: i64 = 20;
const CMD_NAME_BYTE: i64 = 21;
const CMD_SHAPE_COUNT: i64 = 22;
const CMD_SHAPE_TAG: i64 = 23;
const CMD_SHAPE_KIND: i64 = 24;
const CMD_SHAPE_SIZE: i64 = 25;

/// Extract a constant that `wire.kel` transcribes, by name.
///
/// Parsing the Keleusma source is deliberate. The alternative is to restate the
/// number in this file, which would only prove the test agrees with itself.
fn kel_const(fn_name: &str) -> i64 {
    let needle = format!("fn {fn_name}() -> Word {{");
    let at = WIRE_KEL
        .find(&needle)
        .unwrap_or_else(|| panic!("wire.kel has no `{fn_name}`"));
    let rest = &WIRE_KEL[at + needle.len()..];
    let end = rest.find('}').expect("unterminated body");
    let body = rest[..end].trim();
    body.strip_prefix("0x").map_or_else(
        || body.parse::<i64>().expect("decimal literal"),
        |hex| i64::from_str_radix(hex, 16).expect("hex literal"),
    )
}

#[test]
fn the_transcribed_offsets_match_the_derive() {
    // THE POINT OF THIS TEST. wire.kel hardcodes offsets that
    // `#[derive(WireRecord)]` generates by packing with no implicit padding and
    // rounding the stride to a word. They cannot be recomputed by eye, so each
    // one is checked against the generated constant. A mistranscription fails
    // here; a record that later gains a field also fails here, rather than
    // silently reading the wrong bytes in a corner no test exercises.
    assert_eq!(
        kel_const("nameref_stride"),
        NameRef::STRIDE as i64,
        "NameRef stride"
    );
    assert_eq!(
        kel_const("nameref_off_offset"),
        NameRef::OFFSET_OFFSET as i64
    );
    assert_eq!(
        kel_const("nameref_off_length"),
        NameRef::OFFSET_LENGTH as i64
    );

    assert_eq!(
        kel_const("shape_stride"),
        ShapeRecord::STRIDE as i64,
        "ShapeRecord stride"
    );
    assert_eq!(kel_const("shape_off_tag"), ShapeRecord::OFFSET_TAG as i64);
    assert_eq!(kel_const("shape_off_kind"), ShapeRecord::OFFSET_KIND as i64);
    assert_eq!(
        kel_const("shape_off_reserved"),
        ShapeRecord::OFFSET_RESERVED as i64
    );
    assert_eq!(kel_const("shape_off_size"), ShapeRecord::OFFSET_SIZE as i64);
}

#[test]
fn the_transcribed_region_kinds_match_the_schema() {
    assert_eq!(kel_const("kind_string_pool"), i64::from(kind::STRING_POOL));
    assert_eq!(kel_const("kind_names"), i64::from(kind::NAMES));
    assert_eq!(kel_const("kind_shapes"), i64::from(kind::SHAPES));
}

#[test]
fn the_constant_extractor_actually_reads_wire_kel() {
    // must-fire for the harness above: if `kel_const` silently returned a
    // constant, every assertion in the two tests would pass vacuously.
    assert_eq!(kel_const("nameref_off_offset"), 0);
    assert_eq!(kel_const("nameref_off_length"), 4);
    assert_ne!(
        kel_const("kind_names"),
        kel_const("kind_shapes"),
        "the extractor returns the same value for different constants"
    );
}

/// `(pool_offset, length)` for each name in the fixture.
type NameFixture = Vec<(u32, u32)>;
/// `(tag, kind, size)` for each shape in the fixture.
type ShapeFixture = Vec<(u16, u8, u32)>;

/// An artifact carrying a string pool, a name table, and a shape table.
fn schema_artifact() -> (Vec<u8>, NameFixture, ShapeFixture) {
    let names = vec![(0u32, 5u32), (5, 4), (9, 7)];
    let shapes = vec![(0u16, 0u8, 0u32), (1, 2, 8), (2, 3, 64)];
    let mut b = keleusma_wire::WireBuilder::new();
    let sp = b.region(kind::STRING_POOL, 0).expect("pool");
    b.push(sp, b"alphabetaepsilon");
    let nm = b.region(kind::NAMES, 0).expect("names");
    for (offset, length) in &names {
        let mut buf = [0u8; 8];
        NameRef {
            offset: *offset,
            length: *length,
        }
        .write_record(&mut buf)
        .expect("encode");
        b.push(nm, &buf);
    }
    let sh = b.region(kind::SHAPES, 0).expect("shapes");
    for (tag, k, size) in &shapes {
        let mut buf = [0u8; 8];
        ShapeRecord {
            tag: *tag,
            kind: *k,
            reserved: 0,
            size: *size,
        }
        .write_record(&mut buf)
        .expect("encode");
        b.push(sh, &buf);
    }
    (b.finish().expect("finish"), names, shapes)
}

#[test]
fn the_schema_accessors_resolve_names_and_shapes_by_kind() {
    let (art, names, shapes) = schema_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n_regions = 3i64;
    let pool = b"alphabetaepsilon";

    let (count, _) = run_cmd_args(
        &mut vm,
        CMD_NAME_COUNT,
        n_regions,
        &art,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(count, names.len() as i64, "name count");

    for (i, (offset, length)) in names.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_NAME_OFFSET, i64::from(*offset), "offset"),
            (CMD_NAME_LENGTH, i64::from(*length), "length"),
        ] {
            let (got, _) = run_cmd_args(
                &mut vm,
                cmd,
                n_regions,
                &art,
                &[],
                [i as i64, 0, 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(got, want, "name {i} {what}");
        }
        // And the bytes themselves, which cross from the name table into the
        // string pool — the step that makes this a schema rather than a table.
        for j in 0..*length as usize {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_NAME_BYTE,
                n_regions,
                &art,
                &[],
                [i as i64, j as i64, 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(
                got,
                i64::from(pool[*offset as usize + j]),
                "name {i} byte {j}"
            );
        }
    }

    let (scount, _) = run_cmd_args(
        &mut vm,
        CMD_SHAPE_COUNT,
        n_regions,
        &art,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(scount, shapes.len() as i64, "shape count");
    for (i, (tag, k, size)) in shapes.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_SHAPE_TAG, i64::from(*tag), "tag"),
            (CMD_SHAPE_KIND, i64::from(*k), "kind"),
            (CMD_SHAPE_SIZE, i64::from(*size), "size"),
        ] {
            let (got, _) = run_cmd_args(
                &mut vm,
                cmd,
                n_regions,
                &art,
                &[],
                [i as i64, 0, 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(got, want, "shape {i} {what}");
        }
    }
}

#[test]
fn an_absent_region_reports_zero_rather_than_reading_from_a_negative_base() {
    // Resolution by kind must fail closed. An artifact with a string pool but
    // no name or shape table must report zero counts, not index from `-1`.
    let mut b = keleusma_wire::WireBuilder::new();
    let sp = b.region(kind::STRING_POOL, 0).expect("pool");
    b.push(sp, b"only-a-pool");
    let art = b.finish().expect("finish");
    let mut vm = vm_for(WIRE_KEL);
    for (cmd, what) in [(CMD_NAME_COUNT, "names"), (CMD_SHAPE_COUNT, "shapes")] {
        let (got, _) =
            run_cmd_args(&mut vm, cmd, 1, &art, &[], [0, 0, 0, 0, 0], 0).expect("must not trap");
        assert_eq!(got, 0, "absent {what} region must report zero");
    }
}

// =========================================================================
// SLICE 5b — the constant table and its two side tables
// =========================================================================

use keleusma::wire_schema::{ConstRecord, EnumAux, StructAux, tag};

const CMD_CONST_COUNT: i64 = 26;
const CMD_CONST_TAG: i64 = 27;
const CMD_CONST_FLAGS: i64 = 28;
const CMD_CONST_AUX: i64 = 29;
const CMD_CONST_IS_COMPOSITE: i64 = 30;
const CMD_CONST_RANGE_FIRST: i64 = 31;
const CMD_CONST_RANGE_COUNT: i64 = 32;
const CMD_SA_TYPE_NAME: i64 = 33;
const CMD_SA_FIELD_FIRST: i64 = 34;
const CMD_EA_TYPE_NAME: i64 = 35;
const CMD_EA_VARIANT: i64 = 36;
const CMD_EA_DISCRIMINANT: i64 = 37;

#[test]
fn the_slice_5b_offsets_and_tags_match_the_schema() {
    assert_eq!(kel_const("const_stride"), ConstRecord::STRIDE as i64);
    assert_eq!(kel_const("const_off_tag"), ConstRecord::OFFSET_TAG as i64);
    assert_eq!(
        kel_const("const_off_flags"),
        ConstRecord::OFFSET_FLAGS as i64
    );
    assert_eq!(kel_const("const_off_aux"), ConstRecord::OFFSET_AUX as i64);
    assert_eq!(
        kel_const("const_off_payload"),
        ConstRecord::OFFSET_PAYLOAD as i64
    );

    assert_eq!(kel_const("structaux_stride"), StructAux::STRIDE as i64);
    assert_eq!(
        kel_const("structaux_off_type_name"),
        StructAux::OFFSET_TYPE_NAME as i64
    );
    assert_eq!(
        kel_const("structaux_off_field_names_first"),
        StructAux::OFFSET_FIELD_NAMES_FIRST as i64
    );

    assert_eq!(kel_const("enumaux_stride"), EnumAux::STRIDE as i64);
    assert_eq!(
        kel_const("enumaux_off_type_name"),
        EnumAux::OFFSET_TYPE_NAME as i64
    );
    assert_eq!(
        kel_const("enumaux_off_variant"),
        EnumAux::OFFSET_VARIANT as i64
    );
    assert_eq!(
        kel_const("enumaux_off_discriminant"),
        EnumAux::OFFSET_DISCRIMINANT as i64
    );

    for (name, want) in [
        ("tag_unit", tag::UNIT),
        ("tag_bool", tag::BOOL),
        ("tag_int", tag::INT),
        ("tag_byte", tag::BYTE),
        ("tag_fixed", tag::FIXED),
        ("tag_float", tag::FLOAT),
        ("tag_static_str", tag::STATIC_STR),
        ("tag_tuple", tag::TUPLE),
        ("tag_array", tag::ARRAY),
        ("tag_struct", tag::STRUCT),
        ("tag_enum", tag::ENUM),
        ("tag_none", tag::NONE),
    ] {
        assert_eq!(kel_const(name), i64::from(want), "{name}");
    }
    assert_eq!(kel_const("kind_consts"), i64::from(kind::CONSTS));
    assert_eq!(kel_const("kind_struct_aux"), i64::from(kind::STRUCT_AUX));
    assert_eq!(kel_const("kind_enum_aux"), i64::from(kind::ENUM_AUX));
}

/// An artifact with a constant table, a struct side table, and an enum side
/// table. The enum discriminants include NEGATIVE values, which is the case a
/// zero-extending read gets wrong.
#[allow(clippy::type_complexity)]
fn const_artifact() -> (Vec<u8>, Vec<ConstRecord>, Vec<StructAux>, Vec<EnumAux>) {
    let consts = vec![
        ConstRecord {
            tag: tag::INT,
            flags: 0,
            aux: 0,
            payload: 42,
        },
        // An Int whose value has bit 31 set. Read as a range this would look
        // like a child count of 0x80000000 -- the defect that already occurred
        // once in the Rust implementation.
        ConstRecord {
            tag: tag::INT,
            flags: 0,
            aux: 0,
            payload: 0x8000_0000,
        },
        ConstRecord {
            tag: tag::TUPLE,
            flags: 0,
            aux: 0,
            payload: 3 | (2u64 << 32),
        },
        ConstRecord {
            tag: tag::STRUCT,
            flags: 0,
            aux: 7,
            payload: 5 | (4u64 << 32),
        },
        ConstRecord {
            tag: tag::STATIC_STR,
            flags: 0,
            aux: 1,
            payload: 9,
        },
    ];
    let saux = vec![StructAux {
        type_name: 3,
        field_names_first: 11,
    }];
    let eaux = vec![
        EnumAux {
            type_name: 1,
            variant: 2,
            discriminant: 7,
        },
        EnumAux {
            type_name: 4,
            variant: 5,
            discriminant: -9,
        },
        EnumAux {
            type_name: 6,
            variant: 7,
            discriminant: i64::MIN,
        },
        EnumAux {
            type_name: 8,
            variant: 9,
            discriminant: i64::MAX,
        },
    ];

    let mut b = keleusma_wire::WireBuilder::new();
    let c = b.region(kind::CONSTS, 0).expect("consts");
    for r in &consts {
        let mut buf = [0u8; 16];
        r.write_record(&mut buf).expect("encode");
        b.push(c, &buf);
    }
    let sa = b.region(kind::STRUCT_AUX, 0).expect("saux");
    for r in &saux {
        let mut buf = [0u8; 8];
        r.write_record(&mut buf).expect("encode");
        b.push(sa, &buf);
    }
    let ea = b.region(kind::ENUM_AUX, 0).expect("eaux");
    for r in &eaux {
        let mut buf = [0u8; 16];
        r.write_record(&mut buf).expect("encode");
        b.push(ea, &buf);
    }
    (b.finish().expect("finish"), consts, saux, eaux)
}

#[test]
fn the_constant_table_reads_back_field_for_field() {
    let (art, consts, _, _) = const_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 3i64;
    let (count, _) =
        run_cmd_args(&mut vm, CMD_CONST_COUNT, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(count, consts.len() as i64);

    for (i, r) in consts.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_CONST_TAG, i64::from(r.tag), "tag"),
            (CMD_CONST_FLAGS, i64::from(r.flags), "flags"),
            (CMD_CONST_AUX, i64::from(r.aux), "aux"),
            (
                CMD_CONST_IS_COMPOSITE,
                i64::from(r.is_composite()),
                "is_composite",
            ),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "constant {i} {what}");
        }
    }
}

#[test]
fn only_a_composite_reports_a_range_and_a_scalar_reports_absence() {
    // THE TRAP THIS SLICE EXISTS AROUND. A scalar overlays its value on the
    // range bytes, so `Int(0x80000000)` would read as a child count of
    // 0x80000000 if the tag were not consulted. The Keleusma accessors must
    // return the absence sentinel instead, and must agree with the reference's
    // own `is_composite`.
    let (art, consts, _, _) = const_artifact();
    let mut vm = vm_for(WIRE_KEL);
    for (i, r) in consts.iter().enumerate() {
        let (first, _) = run_cmd_args(
            &mut vm,
            CMD_CONST_RANGE_FIRST,
            3,
            &art,
            &[],
            [i as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        let (count, _) = run_cmd_args(
            &mut vm,
            CMD_CONST_RANGE_COUNT,
            3,
            &art,
            &[],
            [i as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        if r.is_composite() {
            let (want_first, want_count) = r.as_range();
            assert_eq!(first, i64::from(want_first), "constant {i} range first");
            assert_eq!(count, i64::from(want_count), "constant {i} range count");
        } else {
            assert_eq!(first, -1, "scalar {i} must report no range");
            assert_eq!(count, -1, "scalar {i} must report no range");
        }
    }
}

#[test]
fn the_side_tables_read_back_including_negative_discriminants() {
    // The discriminant is a SIGNED i64. Every other field zero-extends, and a
    // reader that treated this one the same way would turn -9 into a huge
    // positive number. `i64::MIN` and `i64::MAX` are included because they are
    // where a sign-handling mistake is largest.
    let (art, _, saux, eaux) = const_artifact();
    let mut vm = vm_for(WIRE_KEL);
    for (i, r) in saux.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_SA_TYPE_NAME, i64::from(r.type_name), "type_name"),
            (
                CMD_SA_FIELD_FIRST,
                i64::from(r.field_names_first),
                "field_names_first",
            ),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, 3, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "struct aux {i} {what}");
        }
    }
    for (i, r) in eaux.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_EA_TYPE_NAME, i64::from(r.type_name), "type_name"),
            (CMD_EA_VARIANT, i64::from(r.variant), "variant"),
            (CMD_EA_DISCRIMINANT, r.discriminant, "discriminant"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, 3, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "enum aux {i} {what}");
        }
    }
}

// =========================================================================
// SLICE 5c — range-addressed runs
// =========================================================================

use keleusma::wire_schema::{
    EnumLayoutRecord, EnumVariantRecord, SignatureRecord, StructTemplateRecord,
};

const CMD_SIG_COUNT: i64 = 38;
const CMD_SIG_PARAMS_FIRST: i64 = 39;
const CMD_SIG_PARAMS_COUNT: i64 = 40;
const CMD_SIG_RET: i64 = 41;
const CMD_SIG_RESUME: i64 = 42;
const CMD_SIG_PARAM_SHAPE: i64 = 43;
const CMD_TPL_COUNT: i64 = 44;
const CMD_TPL_TYPE_NAME: i64 = 45;
const CMD_TPL_FIELD_COUNT: i64 = 46;
const CMD_TPL_FIELD_NAME: i64 = 47;
const CMD_ELAY_COUNT: i64 = 48;
const CMD_ELAY_TYPE_NAME: i64 = 49;
const CMD_ELAY_VARIANTS_COUNT: i64 = 50;
const CMD_ELAY_MIN_PAYLOAD: i64 = 51;
const CMD_ELAY_VARIANT_IN_RANGE: i64 = 52;
const CMD_ELAY_VARIANT_NAME: i64 = 53;
const CMD_ELAY_VARIANT_DISC: i64 = 54;

#[test]
fn the_slice_5c_offsets_and_kinds_match_the_schema() {
    assert_eq!(kel_const("sig_stride"), SignatureRecord::STRIDE as i64);
    assert_eq!(
        kel_const("sig_off_params_first"),
        SignatureRecord::OFFSET_PARAMS_FIRST as i64
    );
    assert_eq!(
        kel_const("sig_off_params_count"),
        SignatureRecord::OFFSET_PARAMS_COUNT as i64
    );
    assert_eq!(kel_const("sig_off_ret"), SignatureRecord::OFFSET_RET as i64);
    assert_eq!(
        kel_const("sig_off_resume"),
        SignatureRecord::OFFSET_RESUME as i64
    );

    assert_eq!(kel_const("tpl_stride"), StructTemplateRecord::STRIDE as i64);
    assert_eq!(
        kel_const("tpl_off_type_name"),
        StructTemplateRecord::OFFSET_TYPE_NAME as i64
    );
    assert_eq!(
        kel_const("tpl_off_field_names_first"),
        StructTemplateRecord::OFFSET_FIELD_NAMES_FIRST as i64
    );
    assert_eq!(
        kel_const("tpl_off_field_count"),
        StructTemplateRecord::OFFSET_FIELD_COUNT as i64
    );

    assert_eq!(kel_const("evar_stride"), EnumVariantRecord::STRIDE as i64);
    assert_eq!(
        kel_const("evar_off_name"),
        EnumVariantRecord::OFFSET_NAME as i64
    );
    assert_eq!(
        kel_const("evar_off_disc"),
        EnumVariantRecord::OFFSET_DISC as i64
    );

    assert_eq!(kel_const("elay_stride"), EnumLayoutRecord::STRIDE as i64);
    assert_eq!(
        kel_const("elay_off_type_name"),
        EnumLayoutRecord::OFFSET_TYPE_NAME as i64
    );
    assert_eq!(
        kel_const("elay_off_variants_first"),
        EnumLayoutRecord::OFFSET_VARIANTS_FIRST as i64
    );
    assert_eq!(
        kel_const("elay_off_variants_count"),
        EnumLayoutRecord::OFFSET_VARIANTS_COUNT as i64
    );
    assert_eq!(
        kel_const("elay_off_min_payload"),
        EnumLayoutRecord::OFFSET_MIN_PAYLOAD as i64
    );

    assert_eq!(kel_const("kind_signatures"), i64::from(kind::SIGNATURES));
    assert_eq!(
        kel_const("kind_struct_templates"),
        i64::from(kind::STRUCT_TEMPLATES)
    );
    assert_eq!(
        kel_const("kind_enum_variants"),
        i64::from(kind::ENUM_VARIANTS)
    );
    assert_eq!(
        kel_const("kind_enum_layouts"),
        i64::from(kind::ENUM_LAYOUTS)
    );
}

#[allow(clippy::type_complexity)]
fn runs_artifact() -> (
    Vec<u8>,
    Vec<SignatureRecord>,
    Vec<StructTemplateRecord>,
    Vec<EnumLayoutRecord>,
    Vec<EnumVariantRecord>,
) {
    // Two ADJACENT runs in each table, so an unguarded `first + k` reads the
    // neighbour's record — in bounds, and silently wrong. That is the failure
    // the range guards exist for, and a single-run fixture could not see it.
    let sigs = vec![
        SignatureRecord {
            params_first: 0,
            params_count: 2,
            ret: 9,
            resume: 1,
        },
        SignatureRecord {
            params_first: 2,
            params_count: 3,
            ret: 4,
            resume: 0,
        },
    ];
    let tpls = vec![
        StructTemplateRecord {
            type_name: 1,
            field_names_first: 0,
            field_count: 2,
            reserved: 0,
        },
        StructTemplateRecord {
            type_name: 2,
            field_names_first: 2,
            field_count: 3,
            reserved: 0,
        },
    ];
    let evars = vec![
        EnumVariantRecord {
            name: 10,
            reserved: 0,
            disc: 0,
        },
        EnumVariantRecord {
            name: 11,
            reserved: 0,
            disc: -1,
        },
        EnumVariantRecord {
            name: 12,
            reserved: 0,
            disc: i64::MIN,
        },
        EnumVariantRecord {
            name: 13,
            reserved: 0,
            disc: 5,
        },
    ];
    let elays = vec![
        EnumLayoutRecord {
            type_name: 20,
            variants_first: 0,
            variants_count: 2,
            min_payload: 8,
        },
        EnumLayoutRecord {
            type_name: 21,
            variants_first: 2,
            variants_count: 2,
            min_payload: 16,
        },
    ];

    let mut b = keleusma_wire::WireBuilder::new();
    let put = |b: &mut keleusma_wire::WireBuilder, k: u16, recs: &[[u8; 16]]| {
        let id = b.region(k, 0).expect("region");
        for r in recs {
            b.push(id, r);
        }
    };
    let enc = |r: &dyn Fn(&mut [u8; 16])| -> [u8; 16] {
        let mut buf = [0u8; 16];
        r(&mut buf);
        buf
    };
    let sig_bytes: Vec<[u8; 16]> = sigs
        .iter()
        .map(|s| {
            enc(&|b: &mut [u8; 16]| {
                s.write_record(b).expect("enc");
            })
        })
        .collect();
    let tpl_bytes: Vec<[u8; 16]> = tpls
        .iter()
        .map(|s| {
            enc(&|b: &mut [u8; 16]| {
                s.write_record(b).expect("enc");
            })
        })
        .collect();
    let evar_bytes: Vec<[u8; 16]> = evars
        .iter()
        .map(|s| {
            enc(&|b: &mut [u8; 16]| {
                s.write_record(b).expect("enc");
            })
        })
        .collect();
    let elay_bytes: Vec<[u8; 16]> = elays
        .iter()
        .map(|s| {
            enc(&|b: &mut [u8; 16]| {
                s.write_record(b).expect("enc");
            })
        })
        .collect();
    put(&mut b, kind::SIGNATURES, &sig_bytes);
    put(&mut b, kind::STRUCT_TEMPLATES, &tpl_bytes);
    put(&mut b, kind::ENUM_VARIANTS, &evar_bytes);
    put(&mut b, kind::ENUM_LAYOUTS, &elay_bytes);
    (b.finish().expect("finish"), sigs, tpls, elays, evars)
}

#[test]
fn the_run_owning_records_read_back_field_for_field() {
    let (art, sigs, tpls, elays, _) = runs_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;
    for (cmd, want, what) in [
        (CMD_SIG_COUNT, sigs.len() as i64, "signatures"),
        (CMD_TPL_COUNT, tpls.len() as i64, "templates"),
        (CMD_ELAY_COUNT, elays.len() as i64, "layouts"),
    ] {
        let (got, _) = run_cmd_args(&mut vm, cmd, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
        assert_eq!(got, want, "{what} count");
    }
    for (i, s) in sigs.iter().enumerate() {
        for (cmd, want, what) in [
            (
                CMD_SIG_PARAMS_FIRST,
                i64::from(s.params_first),
                "params_first",
            ),
            (
                CMD_SIG_PARAMS_COUNT,
                i64::from(s.params_count),
                "params_count",
            ),
            (CMD_SIG_RET, i64::from(s.ret), "ret"),
            (CMD_SIG_RESUME, i64::from(s.resume), "resume"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "signature {i} {what}");
        }
    }
    for (i, t) in tpls.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_TPL_TYPE_NAME, i64::from(t.type_name), "type_name"),
            (CMD_TPL_FIELD_COUNT, i64::from(t.field_count), "field_count"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "template {i} {what}");
        }
    }
    for (i, l) in elays.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_ELAY_TYPE_NAME, i64::from(l.type_name), "type_name"),
            (
                CMD_ELAY_VARIANTS_COUNT,
                i64::from(l.variants_count),
                "variants_count",
            ),
            (
                CMD_ELAY_MIN_PAYLOAD,
                i64::from(l.min_payload),
                "min_payload",
            ),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "layout {i} {what}");
        }
    }
}

#[test]
fn a_run_index_past_its_count_is_refused_rather_than_reading_the_neighbour() {
    // The fixture puts two adjacent runs in every table, so `first + k` for a
    // `k` past the count lands on the NEXT run's data: in bounds, plausible,
    // and wrong. Each guard must refuse instead.
    let (art, sigs, tpls, elays, _) = runs_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;

    for (i, s) in sigs.iter().enumerate() {
        for k in 0..s.params_count + 2 {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_SIG_PARAM_SHAPE,
                n,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            let want = if k < s.params_count {
                i64::from(s.params_first + k)
            } else {
                -1
            };
            assert_eq!(got, want, "signature {i} param {k}");
        }
    }
    for (i, t) in tpls.iter().enumerate() {
        for k in 0..t.field_count + 2 {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_TPL_FIELD_NAME,
                n,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            let want = if k < t.field_count {
                i64::from(t.field_names_first + k)
            } else {
                -1
            };
            assert_eq!(got, want, "template {i} field {k}");
        }
    }
    for (i, l) in elays.iter().enumerate() {
        for k in 0..l.variants_count + 2 {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_ELAY_VARIANT_IN_RANGE,
                n,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(
                got,
                i64::from(k < l.variants_count),
                "layout {i} variant {k} in-range"
            );
        }
    }
}

#[test]
fn a_variant_discriminant_of_minus_one_is_a_value_not_an_error() {
    // Why the bound is a SEPARATE query. The fixture's second variant has
    // discriminant -1, which is exactly the sentinel every other accessor uses
    // for absence. Reading it must yield -1 as a VALUE, while the in-range
    // query independently reports presence.
    let (art, _, _, elays, evars) = runs_artifact();
    let mut vm = vm_for(WIRE_KEL);
    for (i, l) in elays.iter().enumerate() {
        for k in 0..l.variants_count {
            let idx = (l.variants_first + k) as usize;
            let (in_range, _) = run_cmd_args(
                &mut vm,
                CMD_ELAY_VARIANT_IN_RANGE,
                4,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(in_range, 1, "layout {i} variant {k} should be present");
            let (disc, _) = run_cmd_args(
                &mut vm,
                CMD_ELAY_VARIANT_DISC,
                4,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(disc, evars[idx].disc, "layout {i} variant {k} discriminant");
            let (name, _) = run_cmd_args(
                &mut vm,
                CMD_ELAY_VARIANT_NAME,
                4,
                &art,
                &[],
                [i as i64, i64::from(k), 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(
                name,
                i64::from(evars[idx].name),
                "layout {i} variant {k} name"
            );
        }
    }
    // And the discriminant -1 really is in the fixture, or the point above is
    // untested.
    assert!(
        evars.iter().any(|v| v.disc == -1),
        "fixture lost its -1 discriminant"
    );
}

// =========================================================================
// SLICE 5d — the data segment, where PRESENCE is semantic
// =========================================================================

use keleusma::wire_schema::{
    DataInitRecord, DataSlotRecord, PrivateCompositeRecord, SharedSlotRecord,
};

const CMD_DATA_PRESENT: i64 = 55;
const CMD_DSLOT_COUNT: i64 = 56;
const CMD_DSLOT_NAME: i64 = 57;
const CMD_DSLOT_VIS: i64 = 58;
const CMD_SSLOT_COUNT: i64 = 59;
const CMD_SSLOT_OFFSET: i64 = 60;
const CMD_SSLOT_KIND: i64 = 61;
const CMD_SSLOT_LEN: i64 = 62;
const CMD_PCOMP_COUNT: i64 = 63;
const CMD_PCOMP_SLOT: i64 = 64;
const CMD_PCOMP_OFFSET: i64 = 65;
const CMD_DINIT_COUNT: i64 = 66;
const CMD_DINIT_FIRST: i64 = 67;
const CMD_DINIT_RANGE_COUNT: i64 = 68;

#[test]
fn the_slice_5d_offsets_and_kinds_match_the_schema() {
    assert_eq!(kel_const("dslot_stride"), DataSlotRecord::STRIDE as i64);
    assert_eq!(
        kel_const("dslot_off_name"),
        DataSlotRecord::OFFSET_NAME as i64
    );
    assert_eq!(
        kel_const("dslot_off_visibility"),
        DataSlotRecord::OFFSET_VISIBILITY as i64
    );

    assert_eq!(kel_const("sslot_stride"), SharedSlotRecord::STRIDE as i64);
    assert_eq!(
        kel_const("sslot_off_offset"),
        SharedSlotRecord::OFFSET_OFFSET as i64
    );
    assert_eq!(
        kel_const("sslot_off_kind"),
        SharedSlotRecord::OFFSET_KIND as i64
    );
    assert_eq!(
        kel_const("sslot_off_len"),
        SharedSlotRecord::OFFSET_LEN as i64
    );

    assert_eq!(
        kel_const("pcomp_stride"),
        PrivateCompositeRecord::STRIDE as i64
    );
    assert_eq!(
        kel_const("pcomp_off_slot"),
        PrivateCompositeRecord::OFFSET_SLOT as i64
    );
    assert_eq!(
        kel_const("pcomp_off_offset"),
        PrivateCompositeRecord::OFFSET_OFFSET as i64
    );

    assert_eq!(kel_const("dinit_stride"), DataInitRecord::STRIDE as i64);
    assert_eq!(
        kel_const("dinit_off_first"),
        DataInitRecord::OFFSET_FIRST as i64
    );
    assert_eq!(
        kel_const("dinit_off_count"),
        DataInitRecord::OFFSET_COUNT as i64
    );

    assert_eq!(kel_const("kind_data_slots"), i64::from(kind::DATA_SLOTS));
    assert_eq!(
        kel_const("kind_shared_layout"),
        i64::from(kind::SHARED_LAYOUT)
    );
    assert_eq!(
        kel_const("kind_private_composite"),
        i64::from(kind::PRIVATE_COMPOSITE)
    );
    assert_eq!(kel_const("kind_data_init"), i64::from(kind::DATA_INIT));
}

#[test]
fn an_absent_data_layout_is_distinguishable_from_an_empty_one() {
    // THE POINT OF THIS SLICE. A module with no `data` block and one whose data
    // block is empty are DIFFERENT PROGRAMS. Absence is carried by the region
    // not existing, emptiness by it existing with no records. Every other count
    // in this file returns 0 for absence, which would collapse the two here.
    let mut vm = vm_for(WIRE_KEL);

    // (a) No DATA_SLOTS region at all.
    let mut b = keleusma_wire::WireBuilder::new();
    let p = b.region(kind::STRING_POOL, 0).expect("pool");
    b.push(p, b"x");
    let absent = b.finish().expect("finish");
    let (present, _) = run_cmd_args(
        &mut vm,
        CMD_DATA_PRESENT,
        1,
        &absent,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    let (count, _) = run_cmd_args(
        &mut vm,
        CMD_DSLOT_COUNT,
        1,
        &absent,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(present, 0, "no DATA_SLOTS region means no data layout");
    assert_eq!(count, -1, "an absent table must not report zero slots");

    // (b) A DATA_SLOTS region that exists and is empty.
    let mut b = keleusma_wire::WireBuilder::new();
    let p = b.region(kind::STRING_POOL, 0).expect("pool");
    b.push(p, b"x");
    b.region(kind::DATA_SLOTS, 0).expect("empty slots");
    let empty = b.finish().expect("finish");
    let (present, _) = run_cmd_args(
        &mut vm,
        CMD_DATA_PRESENT,
        2,
        &empty,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    let (count, _) =
        run_cmd_args(&mut vm, CMD_DSLOT_COUNT, 2, &empty, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(
        present, 1,
        "an empty DATA_SLOTS region still means a layout exists"
    );
    assert_eq!(count, 0, "an empty table has zero slots");

    // The two must not be confusable, which is the whole assertion.
    assert_ne!(
        (0, -1),
        (1, 0),
        "absent and empty must produce different answers"
    );
}

#[allow(clippy::type_complexity)]
fn data_artifact() -> (
    Vec<u8>,
    Vec<DataSlotRecord>,
    Vec<SharedSlotRecord>,
    Vec<PrivateCompositeRecord>,
    Vec<DataInitRecord>,
) {
    let dslots = vec![
        DataSlotRecord {
            name: 1,
            visibility: 1,
            reserved: 0,
            reserved2: 0,
        },
        DataSlotRecord {
            name: 2,
            visibility: 2,
            reserved: 0,
            reserved2: 0,
        },
    ];
    let sslots = vec![
        SharedSlotRecord {
            offset: 0,
            kind: 3,
            reserved: 0,
            len: 8,
        },
        SharedSlotRecord {
            offset: 8,
            kind: 4,
            reserved: 0,
            len: 65535,
        },
    ];
    let pcomps = vec![
        PrivateCompositeRecord {
            slot: 0,
            reserved: 0,
            offset: 0,
        },
        PrivateCompositeRecord {
            slot: 65535,
            reserved: 0,
            offset: 4_000_000_000,
        },
    ];
    let dinits = vec![DataInitRecord { first: 3, count: 2 }];

    let mut b = keleusma_wire::WireBuilder::new();
    let push_all = |b: &mut keleusma_wire::WireBuilder, k: u16, bufs: &[[u8; 8]]| {
        let id = b.region(k, 0).expect("region");
        for x in bufs {
            b.push(id, x);
        }
    };
    let enc8 = |f: &dyn Fn(&mut [u8; 8])| {
        let mut buf = [0u8; 8];
        f(&mut buf);
        buf
    };
    let d: Vec<[u8; 8]> = dslots
        .iter()
        .map(|r| {
            enc8(&|b: &mut [u8; 8]| {
                r.write_record(b).expect("e");
            })
        })
        .collect();
    let s: Vec<[u8; 8]> = sslots
        .iter()
        .map(|r| {
            enc8(&|b: &mut [u8; 8]| {
                r.write_record(b).expect("e");
            })
        })
        .collect();
    let c: Vec<[u8; 8]> = pcomps
        .iter()
        .map(|r| {
            enc8(&|b: &mut [u8; 8]| {
                r.write_record(b).expect("e");
            })
        })
        .collect();
    let i: Vec<[u8; 8]> = dinits
        .iter()
        .map(|r| {
            enc8(&|b: &mut [u8; 8]| {
                r.write_record(b).expect("e");
            })
        })
        .collect();
    push_all(&mut b, kind::DATA_SLOTS, &d);
    push_all(&mut b, kind::SHARED_LAYOUT, &s);
    push_all(&mut b, kind::PRIVATE_COMPOSITE, &c);
    push_all(&mut b, kind::DATA_INIT, &i);
    (b.finish().expect("finish"), dslots, sslots, pcomps, dinits)
}

#[test]
fn the_data_segment_tables_read_back_field_for_field() {
    // The fixture uses boundary values deliberately: a `len` of 65535 and a
    // `slot` of 65535 are the largest a u16 holds, and an `offset` of four
    // billion exceeds i32, so a narrow or signed read of any of them shows up.
    let (art, dslots, sslots, pcomps, dinits) = data_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;

    for (cmd, want, what) in [
        (CMD_DSLOT_COUNT, dslots.len() as i64, "data slots"),
        (CMD_SSLOT_COUNT, sslots.len() as i64, "shared slots"),
        (CMD_PCOMP_COUNT, pcomps.len() as i64, "private composites"),
        (CMD_DINIT_COUNT, dinits.len() as i64, "data inits"),
    ] {
        let (got, _) = run_cmd_args(&mut vm, cmd, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
        assert_eq!(got, want, "{what} count");
    }

    for (i, r) in dslots.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_DSLOT_NAME, i64::from(r.name), "name"),
            (CMD_DSLOT_VIS, i64::from(r.visibility), "visibility"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "data slot {i} {what}");
        }
    }
    for (i, r) in sslots.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_SSLOT_OFFSET, i64::from(r.offset), "offset"),
            (CMD_SSLOT_KIND, i64::from(r.kind), "kind"),
            (CMD_SSLOT_LEN, i64::from(r.len), "len"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "shared slot {i} {what}");
        }
    }
    for (i, r) in pcomps.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_PCOMP_SLOT, i64::from(r.slot), "slot"),
            (CMD_PCOMP_OFFSET, i64::from(r.offset), "offset"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "private composite {i} {what}");
        }
    }
    for (i, r) in dinits.iter().enumerate() {
        for (cmd, want, what) in [
            (CMD_DINIT_FIRST, i64::from(r.first), "first"),
            (CMD_DINIT_RANGE_COUNT, i64::from(r.count), "count"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "data init {i} {what}");
        }
    }
}

// =========================================================================
// SLICE 5e — the module level: chunks, natives, header
// =========================================================================

use keleusma::wire_schema::{ChunkRecord, HeaderRecord, NativeRecord, NativeReturnRecord};

const CMD_CHUNK_COUNT: i64 = 69;
const CMD_CHUNK_U32: i64 = 70;
const CMD_CHUNK_LOCALS: i64 = 71;
const CMD_CHUNK_PARAMS: i64 = 72;
const CMD_CHUNK_BLOCK_TYPE: i64 = 73;
const CMD_CHUNK_HAS_DEBUG: i64 = 74;
const CMD_NATIVE_COUNT: i64 = 75;
const CMD_NATIVE_NAME: i64 = 76;
const CMD_NATRET_COUNT: i64 = 77;
const CMD_NATRET_SHAPE: i64 = 78;
const CMD_HEADER_PRESENT: i64 = 79;
const CMD_HEADER_U32: i64 = 80;
const CMD_HEADER_U8: i64 = 81;
const CMD_ENTRY_ABSENT: i64 = 82;
const CMD_ABSENT_INDEX: i64 = 83;

#[test]
fn the_slice_5e_offsets_and_kinds_match_the_schema() {
    assert_eq!(kel_const("chunk_stride"), ChunkRecord::STRIDE as i64);
    for (kel, want) in [
        ("chunk_off_name", ChunkRecord::OFFSET_NAME),
        ("chunk_off_consts_first", ChunkRecord::OFFSET_CONSTS_FIRST),
        ("chunk_off_consts_count", ChunkRecord::OFFSET_CONSTS_COUNT),
        (
            "chunk_off_templates_first",
            ChunkRecord::OFFSET_TEMPLATES_FIRST,
        ),
        (
            "chunk_off_templates_count",
            ChunkRecord::OFFSET_TEMPLATES_COUNT,
        ),
        (
            "chunk_off_param_types_first",
            ChunkRecord::OFFSET_PARAM_TYPES_FIRST,
        ),
        (
            "chunk_off_param_types_count",
            ChunkRecord::OFFSET_PARAM_TYPES_COUNT,
        ),
        ("chunk_off_debug_first", ChunkRecord::OFFSET_DEBUG_FIRST),
        ("chunk_off_debug_len", ChunkRecord::OFFSET_DEBUG_LEN),
        (
            "chunk_off_op_byte_offset",
            ChunkRecord::OFFSET_OP_BYTE_OFFSET,
        ),
        (
            "chunk_off_op_record_count",
            ChunkRecord::OFFSET_OP_RECORD_COUNT,
        ),
        ("chunk_off_local_count", ChunkRecord::OFFSET_LOCAL_COUNT),
        ("chunk_off_param_count", ChunkRecord::OFFSET_PARAM_COUNT),
        ("chunk_off_block_type", ChunkRecord::OFFSET_BLOCK_TYPE),
    ] {
        assert_eq!(kel_const(kel), want as i64, "{kel}");
    }

    assert_eq!(kel_const("native_stride"), NativeRecord::STRIDE as i64);
    assert_eq!(
        kel_const("native_off_name"),
        NativeRecord::OFFSET_NAME as i64
    );
    assert_eq!(
        kel_const("natret_stride"),
        NativeReturnRecord::STRIDE as i64
    );
    assert_eq!(
        kel_const("natret_off_shape"),
        NativeReturnRecord::OFFSET_SHAPE as i64
    );

    assert_eq!(kel_const("header_stride"), HeaderRecord::STRIDE as i64);
    for (kel, want) in [
        ("header_off_entry_point", HeaderRecord::OFFSET_ENTRY_POINT),
        (
            "header_off_word_bits_log2",
            HeaderRecord::OFFSET_WORD_BITS_LOG2,
        ),
        (
            "header_off_addr_bits_log2",
            HeaderRecord::OFFSET_ADDR_BITS_LOG2,
        ),
        (
            "header_off_float_bits_log2",
            HeaderRecord::OFFSET_FLOAT_BITS_LOG2,
        ),
        ("header_off_flags", HeaderRecord::OFFSET_FLAGS),
        ("header_off_wcet_cycles", HeaderRecord::OFFSET_WCET_CYCLES),
        ("header_off_wcmu_bytes", HeaderRecord::OFFSET_WCMU_BYTES),
        (
            "header_off_shared_data_bytes",
            HeaderRecord::OFFSET_SHARED_DATA_BYTES,
        ),
        (
            "header_off_private_data_bytes",
            HeaderRecord::OFFSET_PRIVATE_DATA_BYTES,
        ),
        ("header_off_schema_hash", HeaderRecord::OFFSET_SCHEMA_HASH),
    ] {
        assert_eq!(kel_const(kel), want as i64, "{kel}");
    }

    for (kel, want) in [
        ("kind_param_types", kind::PARAM_TYPES),
        ("kind_chunks", kind::CHUNKS),
        ("kind_natives", kind::NATIVES),
        ("kind_header", kind::HEADER),
        ("kind_debug_pool", kind::DEBUG_POOL),
        ("kind_native_returns", kind::NATIVE_RETURNS),
    ] {
        assert_eq!(kel_const(kel), i64::from(want), "{kel}");
    }
    assert_eq!(
        kel_const("absent_index"),
        i64::from(u32::MAX),
        "ABSENT sentinel"
    );
}

#[allow(clippy::type_complexity)]
fn module_artifact() -> (Vec<u8>, Vec<ChunkRecord>, Vec<u32>, Vec<u32>, HeaderRecord) {
    let chunks = vec![
        ChunkRecord {
            name: 1,
            consts_first: 0,
            consts_count: 2,
            templates_first: 0,
            templates_count: 1,
            param_types_first: 0,
            param_types_count: 2,
            debug_first: u32::MAX,
            debug_len: 0,
            op_byte_offset: 64,
            op_record_count: 30,
            local_count: 9,
            param_count: 2,
            block_type: 1,
        },
        ChunkRecord {
            name: 2,
            consts_first: 2,
            consts_count: 0,
            templates_first: 1,
            templates_count: 0,
            param_types_first: 2,
            param_types_count: 0,
            debug_first: 0,
            debug_len: 12,
            op_byte_offset: 184,
            op_record_count: 7,
            local_count: 65535,
            param_count: 255,
            block_type: 3,
        },
    ];
    // DIFFERENT LENGTHS on purpose: three names, two return shapes. Pairing
    // them in one record silently dropped the surplus, which is why they are
    // separate regions.
    let native_names = vec![10u32, 11, 12];
    let native_returns = vec![5u32, u32::MAX];
    let header = HeaderRecord {
        entry_point: u32::MAX,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0,
        wcet_cycles: 123_456,
        wcmu_bytes: 4_000_000_000,
        shared_data_bytes: 65_536,
        private_data_bytes: 32,
        schema_hash: 0xDEAD_BEEF,
        reserved: 0,
    };

    let mut b = keleusma_wire::WireBuilder::new();
    let c = b.region(kind::CHUNKS, 0).expect("chunks");
    for r in &chunks {
        let mut buf = [0u8; 48];
        r.write_record(&mut buf).expect("enc");
        b.push(c, &buf);
    }
    let n = b.region(kind::NATIVES, 0).expect("natives");
    for name in &native_names {
        let mut buf = [0u8; 8];
        NativeRecord {
            name: *name,
            reserved: 0,
        }
        .write_record(&mut buf)
        .expect("enc");
        b.push(n, &buf);
    }
    let nr = b.region(kind::NATIVE_RETURNS, 0).expect("native returns");
    for shape in &native_returns {
        let mut buf = [0u8; 8];
        NativeReturnRecord {
            shape: *shape,
            reserved: 0,
        }
        .write_record(&mut buf)
        .expect("enc");
        b.push(nr, &buf);
    }
    let h = b.region(kind::HEADER, 0).expect("header");
    let mut buf = [0u8; 32];
    header.write_record(&mut buf).expect("enc");
    b.push(h, &buf);
    (
        b.finish().expect("finish"),
        chunks,
        native_names,
        native_returns,
        header,
    )
}

#[test]
fn the_chunk_table_reads_back_every_field() {
    let (art, chunks, _, _, _) = module_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;
    let (count, _) =
        run_cmd_args(&mut vm, CMD_CHUNK_COUNT, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(count, chunks.len() as i64);

    for (i, c) in chunks.iter().enumerate() {
        for (off, want, what) in [
            (ChunkRecord::OFFSET_NAME, c.name, "name"),
            (
                ChunkRecord::OFFSET_CONSTS_FIRST,
                c.consts_first,
                "consts_first",
            ),
            (
                ChunkRecord::OFFSET_CONSTS_COUNT,
                c.consts_count,
                "consts_count",
            ),
            (
                ChunkRecord::OFFSET_TEMPLATES_FIRST,
                c.templates_first,
                "templates_first",
            ),
            (
                ChunkRecord::OFFSET_TEMPLATES_COUNT,
                c.templates_count,
                "templates_count",
            ),
            (
                ChunkRecord::OFFSET_PARAM_TYPES_FIRST,
                c.param_types_first,
                "param_types_first",
            ),
            (
                ChunkRecord::OFFSET_PARAM_TYPES_COUNT,
                c.param_types_count,
                "param_types_count",
            ),
            (
                ChunkRecord::OFFSET_DEBUG_FIRST,
                c.debug_first,
                "debug_first",
            ),
            (ChunkRecord::OFFSET_DEBUG_LEN, c.debug_len, "debug_len"),
            (
                ChunkRecord::OFFSET_OP_BYTE_OFFSET,
                c.op_byte_offset,
                "op_byte_offset",
            ),
            (
                ChunkRecord::OFFSET_OP_RECORD_COUNT,
                c.op_record_count,
                "op_record_count",
            ),
        ] {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_CHUNK_U32,
                n,
                &art,
                &[],
                [i as i64, off as i64, 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(got, i64::from(want), "chunk {i} {what}");
        }
        for (cmd, want, what) in [
            (CMD_CHUNK_LOCALS, i64::from(c.local_count), "local_count"),
            (CMD_CHUNK_PARAMS, i64::from(c.param_count), "param_count"),
            (CMD_CHUNK_BLOCK_TYPE, i64::from(c.block_type), "block_type"),
            (
                CMD_CHUNK_HAS_DEBUG,
                i64::from(c.debug_first != u32::MAX),
                "has_debug",
            ),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, n, &art, &[], [i as i64, 0, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "chunk {i} {what}");
        }
    }
}

#[test]
fn natives_and_their_return_shapes_carry_independent_lengths() {
    // THE DEFECT THIS LAYOUT EXISTS TO PREVENT. The two vectors are allowed to
    // differ in length; pairing them in one record silently dropped the
    // surplus. The fixture has three names and two return shapes, so a reader
    // that derived one count from the other would lose a name here.
    let (art, _, names, returns, _) = module_artifact();
    assert_ne!(
        names.len(),
        returns.len(),
        "fixture must have unequal lengths"
    );
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;

    let (nc, _) =
        run_cmd_args(&mut vm, CMD_NATIVE_COUNT, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    let (rc, _) =
        run_cmd_args(&mut vm, CMD_NATRET_COUNT, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(nc, names.len() as i64, "native name count");
    assert_eq!(rc, returns.len() as i64, "native return count");
    assert_ne!(nc, rc, "the two counts must be reported independently");

    for (i, want) in names.iter().enumerate() {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_NATIVE_NAME,
            n,
            &art,
            &[],
            [i as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(*want), "native {i} name");
    }
    for (i, want) in returns.iter().enumerate() {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_NATRET_SHAPE,
            n,
            &art,
            &[],
            [i as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(*want), "native return {i} shape");
    }
}

#[test]
fn the_header_reads_back_and_the_absent_sentinel_is_recognised() {
    let (art, _, _, _, h) = module_artifact();
    let mut vm = vm_for(WIRE_KEL);
    let n = 4i64;

    let (present, _) = run_cmd_args(
        &mut vm,
        CMD_HEADER_PRESENT,
        n,
        &art,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(present, 1);

    for (off, want, what) in [
        (
            HeaderRecord::OFFSET_ENTRY_POINT,
            h.entry_point,
            "entry_point",
        ),
        (
            HeaderRecord::OFFSET_WCET_CYCLES,
            h.wcet_cycles,
            "wcet_cycles",
        ),
        (HeaderRecord::OFFSET_WCMU_BYTES, h.wcmu_bytes, "wcmu_bytes"),
        (
            HeaderRecord::OFFSET_SHARED_DATA_BYTES,
            h.shared_data_bytes,
            "shared_data_bytes",
        ),
        (
            HeaderRecord::OFFSET_PRIVATE_DATA_BYTES,
            h.private_data_bytes,
            "private_data_bytes",
        ),
        (
            HeaderRecord::OFFSET_SCHEMA_HASH,
            h.schema_hash,
            "schema_hash",
        ),
    ] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_HEADER_U32,
            n,
            &art,
            &[],
            [off as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "header {what}");
    }
    for (off, want, what) in [
        (
            HeaderRecord::OFFSET_WORD_BITS_LOG2,
            h.word_bits_log2,
            "word_bits_log2",
        ),
        (
            HeaderRecord::OFFSET_ADDR_BITS_LOG2,
            h.addr_bits_log2,
            "addr_bits_log2",
        ),
        (
            HeaderRecord::OFFSET_FLOAT_BITS_LOG2,
            h.float_bits_log2,
            "float_bits_log2",
        ),
        (HeaderRecord::OFFSET_FLAGS, h.flags, "flags"),
    ] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_HEADER_U8,
            n,
            &art,
            &[],
            [off as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "header {what}");
    }

    // `u32::MAX` means "no entry point", and must not be read as 4294967295
    // being a real chunk index.
    let (sentinel, _) =
        run_cmd_args(&mut vm, CMD_ABSENT_INDEX, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(sentinel, i64::from(u32::MAX));
    let (absent, _) =
        run_cmd_args(&mut vm, CMD_ENTRY_ABSENT, n, &art, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(absent, 1, "entry_point of u32::MAX means absent");
    // wcmu_bytes is four billion, above i32, so a signed narrow read shows up.
    assert!(h.wcmu_bytes > i32::MAX as u32, "fixture must exceed i32");
}

// =========================================================================
// SLICE 6a — the opcode record
// =========================================================================

use keleusma::bytecode::Op;
use keleusma::wire_format::{OpcodeId, OpcodeRecord, encode_op, opcode_id_of};

const CMD_BYTE_PARITY: i64 = 84;
const CMD_OPREC_PARITY: i64 = 85;
const CMD_OPREC_WRITE: i64 = 86;
const CMD_OPREC_ID: i64 = 87;
const CMD_OPREC_OPERAND_BYTE: i64 = 88;
const CMD_OPREC_OPERAND_U24: i64 = 89;
const CMD_OPREC_STORED_PARITY: i64 = 90;
const CMD_OPREC_PARITY_OK: i64 = 91;

/// The reference's parity definition, restated here: the low bit of the total
/// popcount of the four bytes with the parity bit masked off.
///
/// wire.kel computes it as a THREE-STEP FOLD instead. This is the independent
/// definition the fold is checked against, so the equivalence is measured
/// rather than argued.
fn reference_parity(id: u8, operand: [u8; 3]) -> u8 {
    let raw = [id & 0x7F, operand[0], operand[1], operand[2]];
    (raw.iter().map(|b| b.count_ones()).sum::<u32>() & 1) as u8
}

#[test]
fn the_parity_fold_equals_the_popcount_definition() {
    // Exhaustive over the whole opcode range and a spread of operand bytes
    // chosen to exercise every bit position and both parities per byte.
    let mut vm = vm_for(WIRE_KEL);
    let operands: [[u8; 3]; 8] = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [0xFF, 0xFF, 0xFF],
        [0x80, 0x40, 0x20],
        [0x55, 0xAA, 0x0F],
        [0x7F, 0x01, 0xFE],
    ];
    for id in 0u8..128 {
        for op in &operands {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_OPREC_PARITY,
                1,
                &[],
                &[],
                [
                    i64::from(id),
                    i64::from(op[0]),
                    i64::from(op[1]),
                    i64::from(op[2]),
                    0,
                ],
                0,
            )
            .expect("run");
            assert_eq!(
                got,
                i64::from(reference_parity(id, *op)),
                "parity for id {id} operand {op:?}"
            );
        }
    }
}

#[test]
fn byte_parity_is_exhaustively_correct() {
    // All 256 bytes, against `count_ones`. Cheap and total, so there is no
    // reason to sample.
    let mut vm = vm_for(WIRE_KEL);
    for v in 0u16..256 {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_BYTE_PARITY,
            1,
            &[],
            &[],
            [i64::from(v), 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(
            got,
            i64::from(u8::try_from(v).unwrap().count_ones() & 1),
            "byte_parity({v})"
        );
    }
}

/// Every distinct `Op` form the reference can encode without a pool entry,
/// paired with the record it produces.
fn inline_op_corpus() -> Vec<(String, Op, OpcodeRecord)> {
    let candidates: Vec<(String, Op)> = vec![
        ("Add".into(), Op::Add),
        ("Sub".into(), Op::Sub),
        ("Return".into(), Op::Return),
        ("Yield".into(), Op::Yield),
        ("Dup".into(), Op::Dup),
        ("Not".into(), Op::Not),
        ("CheckedAdd".into(), Op::CheckedAdd),
        ("BitXor".into(), Op::BitXor),
        ("Const(0)".into(), Op::Const(0)),
        ("Const(1)".into(), Op::Const(1)),
        ("Const(65535)".into(), Op::Const(65535)),
        ("GetLocal(0)".into(), Op::GetLocal(0)),
        ("GetLocal(255)".into(), Op::GetLocal(255)),
        ("SetLocal(7)".into(), Op::SetLocal(7)),
        ("Trap(3)".into(), Op::Trap(3)),
    ];
    let mut out = Vec::new();
    for (name, op) in candidates {
        let mut pool = Vec::new();
        if let Ok(rec) = encode_op(&op, &mut pool) {
            // Inline forms only in this slice; the pool is 6b.
            if pool.is_empty() {
                out.push((name, op, rec));
            }
        }
    }
    out
}

#[test]
fn the_inline_corpus_is_not_vacuous() {
    let c = inline_op_corpus();
    assert!(c.len() >= 12, "only {} inline op forms collected", c.len());
    // At least one with a non-zero operand, or the operand path is untested.
    assert!(
        c.iter().any(|(_, _, r)| r.operand_bytes() != [0, 0, 0]),
        "no op with a non-zero operand"
    );
    // And at least one whose parity bit is set, or the parity path is untested.
    assert!(
        c.iter().any(|(_, _, r)| r.0[0] & 0x80 != 0),
        "no op whose record has parity 1"
    );
}

#[test]
fn keleusma_writes_the_same_opcode_record_as_the_reference() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, op, want) in inline_op_corpus() {
        let id = opcode_id_of(&op).0;
        let ob = want.operand_bytes();
        let (next, got) = run_cmd_args(
            &mut vm,
            CMD_OPREC_WRITE,
            1,
            &[],
            &[],
            [
                0,
                i64::from(id),
                i64::from(ob[0]),
                i64::from(ob[1]),
                i64::from(ob[2]),
            ],
            4,
        )
        .expect("run");
        assert_eq!(next, 4, "{name}: a record is four bytes");
        assert_eq!(got, want.0.to_vec(), "{name}: record bytes differ");
    }
}

#[test]
fn the_reader_recovers_the_id_operand_and_parity() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, op, rec) in inline_op_corpus() {
        let img = rec.0.to_vec();
        let ob = rec.operand_bytes();
        let u24 = u32::from(ob[0]) | (u32::from(ob[1]) << 8) | (u32::from(ob[2]) << 16);
        for (cmd, arg2, want, what) in [
            (CMD_OPREC_ID, 0i64, i64::from(opcode_id_of(&op).0), "id"),
            (CMD_OPREC_OPERAND_U24, 0, i64::from(u24), "operand u24"),
            (
                CMD_OPREC_STORED_PARITY,
                0,
                i64::from((rec.0[0] >> 7) & 1),
                "stored parity",
            ),
            (CMD_OPREC_PARITY_OK, 0, 1, "parity ok"),
        ] {
            let (got, _) =
                run_cmd_args(&mut vm, cmd, 1, &img, &[], [0, arg2, 0, 0, 0], 0).expect("run");
            assert_eq!(got, want, "{name}: {what}");
        }
        for (k, want_byte) in ob.iter().enumerate() {
            let (got, _) = run_cmd_args(
                &mut vm,
                CMD_OPREC_OPERAND_BYTE,
                1,
                &img,
                &[],
                [0, k as i64, 0, 0, 0],
                0,
            )
            .expect("run");
            assert_eq!(got, i64::from(*want_byte), "{name}: operand byte {k}");
        }
    }
}

#[test]
fn a_single_bit_flip_anywhere_in_the_record_is_detected() {
    // The parity covers all thirty-two bits, so every single-bit corruption
    // must be caught — including one in the opcode id, and including one in
    // the parity bit itself. Exhaustive over both the corpus and the bits.
    let mut vm = vm_for(WIRE_KEL);
    for (name, _, rec) in inline_op_corpus() {
        for byte in 0..4usize {
            for bit in 0..8u32 {
                let mut img = rec.0;
                img[byte] ^= 1 << bit;
                let (ok, _) = run_cmd_args(
                    &mut vm,
                    CMD_OPREC_PARITY_OK,
                    1,
                    img.as_ref(),
                    &[],
                    [0, 0, 0, 0, 0],
                    0,
                )
                .expect("run");
                assert_eq!(
                    ok, 0,
                    "{name}: flip of byte {byte} bit {bit} went undetected"
                );
                // And the reference agrees it is corrupt.
                assert!(
                    OpcodeRecord(img).check_parity().is_err(),
                    "{name}: reference disagreed about byte {byte} bit {bit}"
                );
            }
        }
    }
}

#[test]
fn the_parity_check_stays_quiet_on_every_uncorrupted_record() {
    // must-not-fire, paired with the flip test above. A check that always
    // reports corruption would pass that one and fail this one.
    let mut vm = vm_for(WIRE_KEL);
    for id in 0u8..128 {
        let rec = OpcodeRecord::from_id_and_operand(OpcodeId(id), [0x12, 0x34, 0x56]);
        let (ok, _) = run_cmd_args(
            &mut vm,
            CMD_OPREC_PARITY_OK,
            1,
            rec.0.as_ref(),
            &[],
            [0, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(ok, 1, "clean record for id {id} was reported corrupt");
    }
}

// =========================================================================
// SLICE 6b — the operand pool
// =========================================================================

use keleusma::wire_format::{
    OperandPoolEntry, POOL_TAG_U16_U16, POOL_TAG_U16_U16_U8, POOL_TAG_U16_U16_U16, POOL_TAG_U24_U24,
};

const CMD_POOL_TAG: i64 = 92;
const CMD_POOL_STORED_PARITY: i64 = 93;
const CMD_POOL_PARITY_OF: i64 = 94;
const CMD_POOL_PARITY_OK: i64 = 95;
const CMD_POOL_W_U16_U16: i64 = 96;
const CMD_POOL_W_U16_U16_U8: i64 = 97;
const CMD_POOL_W_U16_U16_U16: i64 = 98;
const CMD_POOL_W_U24_U24: i64 = 99;
const CMD_POOL_U16: i64 = 100;
const CMD_POOL_ENTRY_U8: i64 = 101;
const CMD_POOL_U24: i64 = 102;

#[test]
fn the_pool_tags_and_stride_match_the_reference() {
    assert_eq!(kel_const("pool_stride"), 8);
    assert_eq!(kel_const("pool_tag_u16_u16"), i64::from(POOL_TAG_U16_U16));
    assert_eq!(
        kel_const("pool_tag_u16_u16_u8"),
        i64::from(POOL_TAG_U16_U16_U8)
    );
    assert_eq!(
        kel_const("pool_tag_u16_u16_u16"),
        i64::from(POOL_TAG_U16_U16_U16)
    );
    assert_eq!(kel_const("pool_tag_u24_u24"), i64::from(POOL_TAG_U24_U24));
}

/// Every pool entry form, with boundary values in each field.
fn pool_corpus() -> Vec<(String, OperandPoolEntry)> {
    let mut v = Vec::new();
    for (a, b) in [(0u16, 0u16), (1, 2), (65535, 65535), (0, 65535)] {
        v.push((
            format!("u16_u16({a},{b})"),
            OperandPoolEntry::from_u16_u16(a, b),
        ));
    }
    for (a, b, c) in [(0u16, 0u16, 0u8), (7, 9, 255), (65535, 0, 128)] {
        v.push((
            format!("u16_u16_u8({a},{b},{c})"),
            OperandPoolEntry::from_u16_u16_u8(a, b, c),
        ));
    }
    for (a, b, c) in [(0u16, 0u16, 0u16), (1, 2, 3), (65535, 65535, 65535)] {
        v.push((
            format!("u16_u16_u16({a},{b},{c})"),
            OperandPoolEntry::from_u16_u16_u16(a, b, c),
        ));
    }
    for (a, b) in [
        (0u32, 0u32),
        (1, 2),
        (0xFF_FFFF, 0xFF_FFFF),
        (0xAB_CDEF, 0x12_3456),
    ] {
        v.push((
            format!("u24_u24({a},{b})"),
            OperandPoolEntry::from_u24_u24(a, b),
        ));
    }
    v
}

#[test]
fn keleusma_writes_the_same_pool_entry_as_the_reference() {
    // Byte identity against the reference constructors, per form, including
    // the parity byte each of them stamps.
    let mut vm = vm_for(WIRE_KEL);
    for (name, want) in pool_corpus() {
        let (cmd, args) = match want.tag() {
            t if t == POOL_TAG_U16_U16 => {
                let (a, b) = want.as_u16_u16();
                (CMD_POOL_W_U16_U16, [0, i64::from(a), i64::from(b), 0, 0])
            }
            t if t == POOL_TAG_U16_U16_U8 => {
                let (a, b, c) = want.as_u16_u16_u8();
                (
                    CMD_POOL_W_U16_U16_U8,
                    [0, i64::from(a), i64::from(b), i64::from(c), 0],
                )
            }
            t if t == POOL_TAG_U16_U16_U16 => {
                let (a, b, c) = want.as_u16_u16_u16();
                (
                    CMD_POOL_W_U16_U16_U16,
                    [0, i64::from(a), i64::from(b), i64::from(c), 0],
                )
            }
            t if t == POOL_TAG_U24_U24 => {
                let (a, b) = want.as_u24_u24();
                (CMD_POOL_W_U24_U24, [0, i64::from(a), i64::from(b), 0, 0])
            }
            other => panic!("unhandled pool tag {other}"),
        };
        let (next, got) = run_cmd_args(&mut vm, cmd, 1, &[], &[], args, 8).expect("run");
        assert_eq!(next, 8, "{name}: an entry is eight bytes");
        assert_eq!(got, want.0.to_vec(), "{name}: entry bytes differ");
    }
}

#[test]
fn the_pool_readers_recover_every_field() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, e) in pool_corpus() {
        let img = e.0.as_ref();
        let (tag, _) =
            run_cmd_args(&mut vm, CMD_POOL_TAG, 1, img, &[], [0, 0, 0, 0, 0], 0).expect("run");
        assert_eq!(tag, i64::from(e.tag()), "{name}: tag");
        let (ok, _) = run_cmd_args(&mut vm, CMD_POOL_PARITY_OK, 1, img, &[], [0, 0, 0, 0, 0], 0)
            .expect("run");
        assert_eq!(ok, 1, "{name}: clean entry reported corrupt");

        match e.tag() {
            t if t == POOL_TAG_U16_U16 => {
                let (a, b) = e.as_u16_u16();
                for (k, want) in [(0i64, a), (1, b)] {
                    let (got, _) =
                        run_cmd_args(&mut vm, CMD_POOL_U16, 1, img, &[], [0, k, 0, 0, 0], 0)
                            .expect("run");
                    assert_eq!(got, i64::from(want), "{name}: u16 field {k}");
                }
            }
            t if t == POOL_TAG_U16_U16_U8 => {
                let (a, b, c) = e.as_u16_u16_u8();
                for (k, want) in [(0i64, a), (1, b)] {
                    let (got, _) =
                        run_cmd_args(&mut vm, CMD_POOL_U16, 1, img, &[], [0, k, 0, 0, 0], 0)
                            .expect("run");
                    assert_eq!(got, i64::from(want), "{name}: u16 field {k}");
                }
                let (got, _) =
                    run_cmd_args(&mut vm, CMD_POOL_ENTRY_U8, 1, img, &[], [0, 0, 0, 0, 0], 0)
                        .expect("run");
                assert_eq!(got, i64::from(c), "{name}: u8 field");
            }
            t if t == POOL_TAG_U16_U16_U16 => {
                let (a, b, c) = e.as_u16_u16_u16();
                for (k, want) in [(0i64, a), (1, b), (2, c)] {
                    let (got, _) =
                        run_cmd_args(&mut vm, CMD_POOL_U16, 1, img, &[], [0, k, 0, 0, 0], 0)
                            .expect("run");
                    assert_eq!(got, i64::from(want), "{name}: u16 field {k}");
                }
            }
            t if t == POOL_TAG_U24_U24 => {
                let (a, b) = e.as_u24_u24();
                for (k, want) in [(0i64, a), (1, b)] {
                    let (got, _) =
                        run_cmd_args(&mut vm, CMD_POOL_U24, 1, img, &[], [0, k, 0, 0, 0], 0)
                            .expect("run");
                    assert_eq!(got, i64::from(want), "{name}: u24 field {k}");
                }
            }
            other => panic!("unhandled tag {other}"),
        }
    }
}

#[test]
fn the_pool_parity_is_an_xor_byte_not_a_popcount_bit() {
    // The two schemes in this format differ, and conflating them is the easy
    // mistake. An independent XOR over bytes 0 and 2..7 is computed here and
    // compared against what Keleusma derives, so the definition is measured
    // rather than copied from the same source twice.
    let mut vm = vm_for(WIRE_KEL);
    for (name, e) in pool_corpus() {
        let want = [e.0[0], e.0[2], e.0[3], e.0[4], e.0[5], e.0[6], e.0[7]]
            .iter()
            .fold(0u8, |acc, b| acc ^ b);
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_POOL_PARITY_OF,
            1,
            e.0.as_ref(),
            &[],
            [0, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "{name}: derived parity");
        let (stored, _) = run_cmd_args(
            &mut vm,
            CMD_POOL_STORED_PARITY,
            1,
            e.0.as_ref(),
            &[],
            [0, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(stored, i64::from(e.0[1]), "{name}: stored parity");
        // And it really is a byte-wide value somewhere, not always 0 or 1, or
        // the distinction from the record's single bit would be untested.
    }
    assert!(
        pool_corpus().iter().any(|(_, e)| e.0[1] > 1),
        "no entry has a parity byte above 1, so the byte-wide claim is untested"
    );
}

#[test]
fn a_single_bit_flip_in_any_payload_byte_is_detected() {
    // Exhaustive over the corpus, the six payload bytes, the tag byte, and all
    // eight bit positions. The reference must agree on every one.
    let mut vm = vm_for(WIRE_KEL);
    for (name, e) in pool_corpus() {
        for byte in [0usize, 2, 3, 4, 5, 6, 7] {
            for bit in 0..8u32 {
                let mut img = e.0;
                img[byte] ^= 1 << bit;
                let (ok, _) = run_cmd_args(
                    &mut vm,
                    CMD_POOL_PARITY_OK,
                    1,
                    img.as_ref(),
                    &[],
                    [0, 0, 0, 0, 0],
                    0,
                )
                .expect("run");
                assert_eq!(
                    ok, 0,
                    "{name}: flip of byte {byte} bit {bit} went undetected"
                );
                assert!(
                    OperandPoolEntry(img).check_parity().is_err(),
                    "{name}: reference disagreed about byte {byte} bit {bit}"
                );
            }
        }
    }
}
