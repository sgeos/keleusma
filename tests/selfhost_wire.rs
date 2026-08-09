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
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(seed.len() as i64))?;
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(nregions))?;
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(warg))?;
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
    let mut vm = vm_for(WIRE_KEL);
    let (got, _) = run_cmd(&mut vm, 99, 0, &[]).expect("run");
    assert_eq!(got, -99, "an unknown command did not report itself");
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
