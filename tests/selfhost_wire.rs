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
/// Record-field inputs for the schema emitters. Appended after the `warg`
/// slots, matching the declaration order in `wire.kel`.
const FIN_SLOT: usize = WARG5_SLOT + 1;
const FIN_CAPACITY: usize = 1024;
/// Byte-pool input. A pool has no stride and no fields, so `fin` is the wrong
/// channel: a word per byte would cost eight times the space.
const BIN_SLOT: usize = FIN_SLOT + FIN_CAPACITY;
const BIN_CAPACITY: usize = 8192;

/// The flattener's scratch, `fq` and `fsz`, sits between `bin` and the
/// interner's channels. Nothing seeds it — it is written by the walk — but it
/// occupies slots, so the constants below must step over it. Spelled as two
/// named lengths rather than one number, because a bare `2048` here is the kind
/// of unexplained offset that survives a later array being resized.
const FQ_CAPACITY: usize = 1024;
const FSZ_CAPACITY: usize = 1024;
/// The interner's (length, mode) input, split off `fin` in slice 13b so the
/// flattener can own `fin` for its six-word-per-node preorder.
const NIN_SLOT: usize = BIN_SLOT + BIN_CAPACITY + FQ_CAPACITY + FSZ_CAPACITY;
const NIN_CAPACITY: usize = 1024;

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

/// As `run_cmd_args`, additionally seeding the record-field input array.
///
/// `wire.fin` is how a schema emitter receives a record's fields: a record has
/// more fields than there are `warg` slots, and one slot per field does not
/// scale past the first record kind.
fn run_cmd_fields(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    nregions: i64,
    regions: &Regions,
    fields: &[i64],
    args: [i64; 5],
    read_len: usize,
) -> Result<(i64, Vec<u8>), VmError> {
    assert!(
        fields.len() <= FIN_CAPACITY,
        "{} field inputs exceeds the {FIN_CAPACITY}-word batch buffer",
        fields.len()
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(0))?;
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(nregions))?;
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(args[0]))?;
    vm.set_shared(&mut shared, WARG2_SLOT, Value::Int(args[1]))?;
    vm.set_shared(&mut shared, WARG3_SLOT, Value::Int(args[2]))?;
    vm.set_shared(&mut shared, WARG4_SLOT, Value::Int(args[3]))?;
    vm.set_shared(&mut shared, WARG5_SLOT, Value::Int(args[4]))?;
    for (i, v) in fields.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + i, Value::Int(*v))?;
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

/// Run `cmd` with the byte-pool input array seeded, and read back `read_len`.
fn run_cmd_pool(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    bin: &[u8],
    args: [i64; 5],
    read_len: usize,
) -> Result<(i64, Vec<u8>), VmError> {
    assert!(
        bin.len() <= BIN_CAPACITY,
        "{} pool bytes exceeds the {BIN_CAPACITY}-byte batch buffer",
        bin.len()
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(0))?;
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(0))?;
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(args[0]))?;
    vm.set_shared(&mut shared, WARG2_SLOT, Value::Int(args[1]))?;
    vm.set_shared(&mut shared, WARG3_SLOT, Value::Int(args[2]))?;
    vm.set_shared(&mut shared, WARG4_SLOT, Value::Int(args[3]))?;
    vm.set_shared(&mut shared, WARG5_SLOT, Value::Int(args[4]))?;
    for (i, b) in bin.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(*b))?;
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
    // Swept to the top of the LAST chain, not the top of the last-but-one. This
    // read `0..103` and so stopped exactly where `dispatch_frame` begins,
    // leaving the whole framing chain — the one nearest the depth ceiling and
    // therefore likeliest to need splitting — entirely unswept.
    // THE BOUND IS READ OUT OF THE SOURCE, NOT RESTATED HERE.
    //
    // It was restated for four consecutive slices and I got it wrong once,
    // leaving a newly added command unswept — the exact off-by-one this test
    // exists to catch, committed inside the test. That was a by-name
    // enumeration wearing a different hat.
    //
    // `highest_command()` is load-bearing rather than documentation: `main`
    // refuses anything above it, so a command added past that number is
    // unreachable and fails its own test at once. The control below proves the
    // refusal is real.
    let highest = kel_const("highest_command");
    for cmd in 0..=highest {
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
    // The bound is only load-bearing if `main` actually refuses past it.
    // Without this, `highest_command` could drift BELOW the real top and the
    // sweep would silently narrow rather than fail.
    //
    // A FRESH VM, and that is not tidiness. The sweep runs every command with
    // zero arguments, and some of them legitimately fault there — command 115
    // resolves a HEADER region that a zero-region artifact does not have, and
    // indexes far outside the buffer. The loop tolerates that with
    // `unwrap_or`, but the fault leaves this VM unusable for any later call, so
    // reusing it here fails with the PREVIOUS command's fault rather than
    // answering the question asked.
    let mut fresh = vm_for(WIRE_KEL);
    let (past, _) =
        run_cmd_args(&mut fresh, highest + 1, 0, &[], &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(
        past, -99,
        "main does not refuse past highest_command, so the bound is not load-bearing"
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
        kel_const("tpl_off_reserved"),
        StructTemplateRecord::OFFSET_RESERVED as i64
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
    // Transcribed for the emitter; no reader consults it.
    assert_eq!(
        kel_const("evar_off_reserved"),
        EnumVariantRecord::OFFSET_RESERVED as i64
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
    // Reserved fields, transcribed for the emitter rather than the reader.
    assert_eq!(
        kel_const("dslot_off_reserved"),
        DataSlotRecord::OFFSET_RESERVED as i64
    );
    assert_eq!(
        kel_const("dslot_off_reserved2"),
        DataSlotRecord::OFFSET_RESERVED2 as i64
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
        kel_const("sslot_off_reserved"),
        SharedSlotRecord::OFFSET_RESERVED as i64
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
        kel_const("pcomp_off_reserved"),
        PrivateCompositeRecord::OFFSET_RESERVED as i64
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
        kel_const("native_off_reserved"),
        NativeRecord::OFFSET_RESERVED as i64
    );
    assert_eq!(
        kel_const("natret_stride"),
        NativeReturnRecord::STRIDE as i64
    );
    assert_eq!(
        kel_const("natret_off_shape"),
        NativeReturnRecord::OFFSET_SHAPE as i64
    );
    assert_eq!(
        kel_const("natret_off_reserved"),
        NativeReturnRecord::OFFSET_RESERVED as i64
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
        ("header_off_reserved", HeaderRecord::OFFSET_RESERVED),
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

// =========================================================================
// SLICE 7 — the framing header and the CRC trailer
// =========================================================================

use keleusma::bytecode::{BYTECODE_MAGIC, BYTECODE_VERSION};

const CMD_FH_U32: i64 = 103;
const CMD_FH_U16: i64 = 104;
const CMD_FH_U8: i64 = 105;
const CMD_FH_MAGIC_OK: i64 = 106;
const CMD_FH_VERSION_OK: i64 = 107;
const CMD_FH_FLAG_SET: i64 = 108;
const CMD_EMIT_FRAMING: i64 = 109;
const CMD_SET_SECTION: i64 = 110;
const CMD_SEAL: i64 = 111;
const CMD_INTACT: i64 = 112;
const CMD_MAGIC_WORD: i64 = 113;
const CMD_CRC_RESIDUE: i64 = 114;

#[test]
fn the_framing_constants_match_the_runtime() {
    assert_eq!(kel_const("header_bytes_len"), 64);
    assert_eq!(kel_const("bytecode_version"), i64::from(BYTECODE_VERSION));
    // The magic as a little-endian u32 must equal the bytes the runtime writes.
    assert_eq!(
        kel_const("magic_word"),
        i64::from(u32::from_le_bytes(BYTECODE_MAGIC)),
        "magic word"
    );
}

#[test]
fn the_crc_residue_is_derived_not_asserted() {
    // `WIRE_FORMAT_CRC32_RESIDUE` is private, so restating it here would only
    // prove the test agrees with itself. Derive it instead: appending a
    // message's own CRC makes the checksum of the extended message a constant,
    // and that constant must be what wire.kel uses.
    let mut derived = None;
    for msg in [&b""[..], b"a", b"hello world", &[0xFFu8; 37][..]] {
        let mut buf = msg.to_vec();
        buf.extend_from_slice(&keleusma_wire::crc32(msg).to_le_bytes());
        let residue = keleusma_wire::crc32(&buf);
        match derived {
            None => derived = Some(residue),
            Some(prev) => assert_eq!(prev, residue, "the residue is not constant across messages"),
        }
    }
    let derived = derived.expect("at least one message");
    assert_eq!(
        kel_const("crc32_residue"),
        i64::from(derived),
        "wire.kel's residue disagrees with the derived one"
    );

    // And Keleusma reports the same constant.
    let mut vm = vm_for(WIRE_KEL);
    let (got, _) =
        run_cmd_args(&mut vm, CMD_CRC_RESIDUE, 1, &[], &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(got, i64::from(derived));
}

#[test]
fn the_emitted_framing_header_matches_the_runtimes_field_layout() {
    // Compare against a header this test lays out independently from the
    // documented offsets, rather than against wire.kel's own constants.
    let mut vm = vm_for(WIRE_KEL);
    let (written, got) = run_cmd_args(
        &mut vm,
        CMD_EMIT_FRAMING,
        1,
        &[],
        &[],
        [4096, 6, 6, 6, 0],
        64,
    )
    .expect("run");
    assert_eq!(written, 64, "the framing header is sixty-four bytes");

    let mut want = vec![0u8; 64];
    want[0..4].copy_from_slice(&BYTECODE_MAGIC);
    want[4..6].copy_from_slice(&BYTECODE_VERSION.to_le_bytes());
    want[6..8].copy_from_slice(&64u16.to_le_bytes());
    want[8..12].copy_from_slice(&4096u32.to_le_bytes());
    want[12] = 6;
    want[13] = 6;
    want[14] = 6;
    want[15] = 0;
    assert_eq!(got, want, "framing header bytes differ");
}

#[test]
fn the_header_reader_recognises_magic_version_and_flags() {
    let mut vm = vm_for(WIRE_KEL);
    let (_, img) = run_cmd_args(
        &mut vm,
        CMD_EMIT_FRAMING,
        1,
        &[],
        &[],
        [4096, 6, 6, 6, 0],
        64,
    )
    .expect("run");

    for (cmd, want, what) in [
        (CMD_FH_MAGIC_OK, 1, "magic"),
        (CMD_FH_VERSION_OK, 1, "version"),
    ] {
        let (got, _) = run_cmd_args(&mut vm, cmd, 1, &img, &[], [0, 0, 0, 0, 0], 0).expect("run");
        assert_eq!(got, want, "{what} should be accepted");
    }

    // must-fire: a wrong magic and a wrong version are both rejected, and the
    // version check is what makes a version-1 artifact reject cleanly rather
    // than be misread under version-2 field positions.
    let mut bad_magic = img.clone();
    bad_magic[0] ^= 0xFF;
    let (got, _) = run_cmd_args(
        &mut vm,
        CMD_FH_MAGIC_OK,
        1,
        &bad_magic,
        &[],
        [0, 0, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(got, 0, "a corrupted magic must be rejected");

    let mut v1 = img.clone();
    v1[4..6].copy_from_slice(&1u16.to_le_bytes());
    let (got, _) =
        run_cmd_args(&mut vm, CMD_FH_VERSION_OK, 1, &v1, &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(
        got, 0,
        "a version-1 artifact must be rejected on the version check"
    );

    // Flags read individually.
    for (bit, mask) in [(0u8, 1i64), (1, 2), (2, 4)] {
        let mut f = img.clone();
        f[15] = 1 << bit;
        let (got, _) =
            run_cmd_args(&mut vm, CMD_FH_FLAG_SET, 1, &f, &[], [mask, 0, 0, 0, 0], 0).expect("run");
        assert_eq!(got, 1, "flag bit {bit} should read as set");
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_FH_FLAG_SET,
            1,
            &img,
            &[],
            [mask, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(
            got, 0,
            "flag bit {bit} should read as clear on a zero-flags header"
        );
    }
}

#[test]
fn section_offsets_and_lengths_round_trip_through_the_header() {
    let mut vm = vm_for(WIRE_KEL);
    // Emit the header, then place three sections, reading the image back in
    // between because each call gets a fresh shared buffer.
    let (_, img) = run_cmd_args(
        &mut vm,
        CMD_EMIT_FRAMING,
        1,
        &[],
        &[],
        [4096, 6, 6, 6, 0],
        64,
    )
    .expect("run");
    let (end, img2) = run_cmd_args(
        &mut vm,
        CMD_SET_SECTION,
        1,
        &img,
        &[],
        [32, 36, 64, 120, 0],
        64,
    )
    .expect("run");
    assert_eq!(end, 184, "the section ends where offset plus length says");

    for (off, want) in [(32usize, 64u32), (36, 120)] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_FH_U32,
            1,
            &img2,
            &[],
            [off as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "section field at {off}");
    }
}

#[test]
fn a_sealed_artifact_checksums_to_the_residue_and_damage_breaks_it() {
    // The trailer is validated by running CRC over the WHOLE artifact and
    // comparing to a fixed residue, rather than by recomputing the body's
    // checksum. Both directions are checked.
    let mut vm = vm_for(WIRE_KEL);
    let body: Vec<u8> = (0..60u8).collect();

    let (total, sealed) = run_cmd_args(
        &mut vm,
        CMD_SEAL,
        1,
        &body,
        &[],
        [body.len() as i64, 0, 0, 0, 0],
        64,
    )
    .expect("seal");
    assert_eq!(total, body.len() as i64 + 4, "sealing appends four bytes");

    // The reference agrees the trailer is right.
    assert_eq!(
        keleusma_wire::crc32(&sealed[..body.len()]).to_le_bytes(),
        sealed[body.len()..body.len() + 4],
        "the appended trailer is not the body's CRC"
    );

    // must-not-fire: intact.
    let (ok, _) =
        run_cmd_args(&mut vm, CMD_INTACT, 1, &sealed, &[], [total, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(ok, 1, "a freshly sealed artifact must verify");

    // must-fire: every single-bit flip in the body AND in the trailer breaks it.
    for byte in 0..(body.len() + 4) {
        for bit in [0u32, 3, 7] {
            let mut bad = sealed.clone();
            bad[byte] ^= 1 << bit;
            let (ok, _) = run_cmd_args(&mut vm, CMD_INTACT, 1, &bad, &[], [total, 0, 0, 0, 0], 0)
                .expect("run");
            assert_eq!(ok, 0, "flip of byte {byte} bit {bit} went undetected");
        }
    }
}

#[test]
fn the_narrow_header_readers_and_the_magic_word_agree_with_the_bytes() {
    // Covers the u16 and u8 header readers and the magic accessor. Added
    // because they were otherwise dead: the framing tests reached every field
    // through `fh_u32`, so the narrow readers had no coverage at all and the
    // compiler said so.
    let mut vm = vm_for(WIRE_KEL);
    let (_, img) = run_cmd_args(
        &mut vm,
        CMD_EMIT_FRAMING,
        1,
        &[],
        &[],
        [4096, 6, 5, 4, 0],
        64,
    )
    .expect("run");

    for (off, want, what) in [
        (4usize, u32::from(BYTECODE_VERSION), "version"),
        (6, 64u32, "header length"),
    ] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_FH_U16,
            1,
            &img,
            &[],
            [off as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "u16 {what}");
    }
    for (off, want, what) in [
        (12usize, 6u8, "word bits log2"),
        (13, 5, "addr bits log2"),
        (14, 4, "float bits log2"),
        (15, 0, "flags"),
    ] {
        let (got, _) = run_cmd_args(
            &mut vm,
            CMD_FH_U8,
            1,
            &img,
            &[],
            [off as i64, 0, 0, 0, 0],
            0,
        )
        .expect("run");
        assert_eq!(got, i64::from(want), "u8 {what}");
    }
    // The three width fields were given DIFFERENT values above, so a reader
    // that confused their offsets shows up rather than passing on equal ones.
    let (magic, _) =
        run_cmd_args(&mut vm, CMD_MAGIC_WORD, 1, &[], &[], [0, 0, 0, 0, 0], 0).expect("run");
    assert_eq!(
        magic,
        i64::from(u32::from_le_bytes(BYTECODE_MAGIC)),
        "magic word"
    );
}

// --- WIRING SLICE 1: the emitter meets real compiler output ------------------
//
// Every emission test above builds its region sets by hand, so each one
// exercises the shapes I thought of. `tests/wire_corpus.rs` made exactly this
// argument about the Rust codec and was vindicated within minutes, surfacing a
// quadratic interner that no hand-built case could reach. This is the same
// argument applied to the Keleusma emitter: the ten stage sources are the
// largest real Keleusma programs that exist, and their auxiliary bodies are
// real compiler output rather than my imagination of what a module looks like.
//
// WHAT THIS COVERS. The container header — three prologue copies and three
// directory copies — emitted by `wire.kel` for a real stage's region set, byte
// for byte against what the Rust encoder produced for the same module. This is
// the first time `wire.kel` is driven by anything the compiler actually emitted.
//
// WHAT IT DOES NOT COVER, stated so it is not mistaken for a superset of the
// hand-built corpus. A region's length survives the container only as a WORD
// count, so every length read back here is a multiple of eight. The awkward
// non-multiple-of-eight lengths, which is where a dropped round-up in
// `words_for` would hide, are reachable ONLY from the hand-built sets. Those
// tests stay load-bearing and must not be deleted in favour of this one.
//
// It also covers only the header. The region PAYLOADS are the wiring increment
// proper; nothing here emits a single schema record from real values.

/// The ten stage sources, matching `tests/wire_corpus.rs`.
const CORPUS_STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
];

/// The auxiliary body a module would be serialised from.
///
/// `op_byte_offset` and `op_record_count` are left zero for the same reason
/// `tests/wire_corpus.rs` leaves them zero: they are assigned by the opcode
/// stream layout, which is not what this exercises. Everything else is the
/// compiler's real output.
fn corpus_aux_of(module: &keleusma::bytecode::Module) -> keleusma::wire_format::WireAuxBody {
    use keleusma::wire_format::{WireAuxBody, WireChunk};
    WireAuxBody {
        chunks: module
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: 0,
                debug_pool_bytes: None,
            })
            .collect(),
        signatures: module.signatures.clone(),
        enum_layouts: module.enum_layouts.clone(),
        native_names: module.native_names.clone(),
        native_return_shapes: module.native_return_shapes.clone(),
        data_layout: module.data_layout.clone(),
        entry_point: module.entry_point,
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
    }
}

/// A real stage's region set, in directory order, with the artifact it came from.
///
/// The length is recovered as `word_length * 8` because that is the only length
/// the container stores. `emit_directory` re-rounds it through `words_for`,
/// which is the identity on a multiple of eight, so this feeds the emitter the
/// same quantity the reference wrote.
fn real_stage_regions(src: &str) -> (Vec<RegionSpec>, Vec<u8>) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let mut specs: Vec<RegionSpec> = Vec::new();
    for i in 0..view.region_count() {
        let r = view.region_at(i).expect("region in range");
        specs.push((r.kind, r.flags, (r.word_length as usize) * 8, r.covers));
    }
    (specs, bytes)
}

#[test]
fn the_emitted_container_header_matches_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, src) in CORPUS_STAGES {
        let (specs, want) = real_stage_regions(src);
        let hl = header_len(specs.len());
        let (written, got) = run_cmd_full(
            &mut vm,
            CMD_EMIT_HEADER,
            specs.len() as i64,
            &[],
            &specs,
            0,
            hl,
        )
        .expect("run");
        assert_eq!(written, hl as i64, "{name}: wrong header length reported");
        assert_eq!(
            got,
            want[..hl],
            "{name}: emitted header differs from the reference"
        );
    }
}

/// The corpus is real, so its shape is asserted rather than assumed.
///
/// Without this, a change that made `real_stage_regions` return an empty set
/// would leave the differential above comparing a 48-byte prologue and passing.
#[test]
fn the_real_region_sets_are_the_shape_the_measurement_recorded() {
    for (name, src) in CORPUS_STAGES {
        let (specs, _) = real_stage_regions(src);
        assert_eq!(
            specs.len(),
            19,
            "{name}: expected 19 regions, the measured count for every stage"
        );
        // DEBUG_POOL is the twentieth kind and no stage emits it, because
        // `CompileOptions::emit_debug` defaults to false. Pinned here so the day
        // a stage does emit it, this says so rather than the count drifting
        // silently.
        assert!(
            !specs
                .iter()
                .any(|(k, _, _, _)| *k == keleusma::wire_schema::kind::DEBUG_POOL),
            "{name}: a stage emitted DEBUG_POOL; the emitter now has a case it never had"
        );
        assert!(
            specs.iter().any(|(_, _, l, _)| *l > 0),
            "{name}: every region is empty, which cannot be real output"
        );
        // WHAT REAL OUTPUT DOES NOT EXERCISE, asserted rather than assumed.
        //
        // `SchemaBuilder` declares every region as `region(kind, 0)` and never
        // builds a parity plane, so real artifacts carry flags 0 and covers 0
        // throughout. The differential above therefore says nothing about a
        // non-zero flags or covers field, and the hand-built sets remain the
        // only coverage of those directory fields.
        //
        // Pinned in this direction deliberately: the (72,64) SECDED plane
        // exists in `keleusma-wire` and is currently UNEXERCISED by the
        // shipping encoder. The day that changes, this fires and says so,
        // rather than the emitter quietly gaining an untested case.
        assert!(
            specs.iter().all(|(_, f, _, c)| *f == 0 && *c == 0),
            "{name}: real output now carries non-zero flags or covers; the emitter \
             differential no longer covers what it did, and an ECC plane may now \
             need emitting"
        );
    }
}

/// A named mutation of a region set, applied at a given index.
type Perturbation = (&'static str, fn(&mut Vec<RegionSpec>, usize));

/// Must-fire control for the differential above.
///
/// "The emitted header equals the reference" is only meaningful if the
/// comparison can report inequality. Each perturbation below is one a real
/// mistranscription could produce, and each must be caught.
#[test]
fn the_real_output_comparison_reports_a_perturbed_region_set() {
    let mut vm = vm_for(WIRE_KEL);
    let (base, want) = real_stage_regions(CORPUS_STAGES[9].1);
    let hl = header_len(base.len());

    // The unperturbed case must be quiet, or the perturbations below prove
    // nothing. This is the must-NOT-fire half, in the same test so neither can
    // be deleted without the other.
    let (_, clean) = run_cmd_full(
        &mut vm,
        CMD_EMIT_HEADER,
        base.len() as i64,
        &[],
        &base,
        0,
        hl,
    )
    .expect("run");
    assert_eq!(clean, want[..hl], "control: the clean case must agree");

    // Perturb the LARGEST region rather than a fixed index. The first draft
    // used index 3, which is empty in this stage, so shrinking it underflowed
    // and the control failed on its own arithmetic rather than on the property.
    // A non-empty target is required for the shrink case to mean anything.
    let big = base
        .iter()
        .enumerate()
        .max_by_key(|(_, (_, _, l, _))| *l)
        .map(|(i, _)| i)
        .expect("the corpus has at least one region");
    assert!(
        base[big].2 >= 8,
        "the perturbation target must hold at least one word"
    );

    let perturbations: &[Perturbation] = &[
        ("one region's kind changed", |s, i| s[i].0 += 1),
        ("one region's length grown by a word", |s, i| s[i].2 += 8),
        ("one region's length shrunk by a word", |s, i| s[i].2 -= 8),
        ("one region's flags set", |s, i| s[i].1 |= 1),
        ("the covers field changed", |s, i| s[i].3 += 1),
        ("two regions transposed", |s, i| s.swap(i, i + 1)),
        ("the last region dropped", |s, _| {
            s.pop();
        }),
    ];

    for (what, perturb) in perturbations {
        let mut specs = base.clone();
        // `swap(i, i + 1)` needs a successor; the largest region is never last
        // here, but the assertion states it rather than relying on it.
        assert!(big + 1 < base.len(), "the target needs a successor to swap");
        perturb(&mut specs, big);
        let n = specs.len();
        let (_, got) =
            run_cmd_full(&mut vm, CMD_EMIT_HEADER, n as i64, &[], &specs, 0, hl).expect("run");
        assert_ne!(
            got,
            want[..hl],
            "control did not fire: {what} produced an identical header"
        );
    }
}

// --- WIRING SLICE 2: the first schema emitter, driven by real values ---------
//
// Slice 1 needed no Keleusma change: the container header was already emittable
// and real data was simply another region set. This is where the emitter side
// genuinely grows. `emit_header_record` writes a real record's real fields at
// the transcribed offsets, which is the thing the self-hosted path needs in
// order to PRODUCE an artifact rather than only read one.
//
// WHY A ONE-REGION ARTIFACT. The obvious test would emit into the real
// artifact's layout and compare in place. It cannot: `wire.bytes` is 65,536
// bytes and `lexer`'s auxiliary body is 16,114,608, so `region_base` for a real
// HEADER region lands far outside the buffer. That is the same constraint the
// sizing measurement recorded, showing up in the first place it could. The
// record is therefore emitted into a one-region artifact and compared against
// the HEADER payload extracted from the real one.

/// Header scalars, chosen DISTINCT and non-zero.
///
/// `corpus_aux_of` leaves six header fields zero, matching `wire_corpus.rs`,
/// because a stage compile does not compute them. Emitting six zeroes would
/// make an offset confusion among them invisible: the differential would pass
/// whether or not each field landed in the right place, which is the same trap
/// the hand-built header test avoids by giving the three width fields different
/// values. Every value below is distinct, non-zero, and distinguishable in
/// either byte order.
const HDR_FLAGS: u8 = 0x5A;
const HDR_WCET: u32 = 0x1112_2324;
const HDR_WCMU: u32 = 0x3134_3538;
const HDR_SHARED: u32 = 0x4142_4344;
const HDR_PRIVATE: u32 = 0x5152_5354;
const HDR_SCHEMA_HASH: u32 = 0x6162_6364;

/// A stage's real HEADER record: the eleven field inputs, and the reference
/// bytes the Rust encoder produced for them.
fn real_header_case(src: &str) -> (Vec<i64>, Vec<u8>) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let mut aux = corpus_aux_of(&module);
    aux.flags = HDR_FLAGS;
    aux.wcet_cycles = HDR_WCET;
    aux.wcmu_bytes = HDR_WCMU;
    aux.shared_data_bytes = HDR_SHARED;
    aux.private_data_bytes = HDR_PRIVATE;
    aux.schema_hash = HDR_SCHEMA_HASH;

    let bytes = keleusma::wire_schema::encode_aux_body(&aux).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let region = view
        .find_region(keleusma::wire_schema::kind::HEADER)
        .expect("every module emits a HEADER region");
    let want = view
        .region_bytes(&region)
        .expect("HEADER payload in range")
        .to_vec();

    // The host driver's job, and deliberately NOT read back out of `want`:
    // feeding the reference's own bytes back in would test only that the
    // emitter can echo them.
    let fields = vec![
        aux.entry_point
            .map_or(i64::from(keleusma::wire_schema::ABSENT), |e| e as i64),
        i64::from(aux.word_bits_log2),
        i64::from(aux.addr_bits_log2),
        i64::from(aux.float_bits_log2),
        i64::from(HDR_FLAGS),
        i64::from(HDR_WCET),
        i64::from(HDR_WCMU),
        i64::from(HDR_SHARED),
        i64::from(HDR_PRIVATE),
        i64::from(HDR_SCHEMA_HASH),
        0,
    ];
    (fields, want)
}

/// Emit a HEADER record into a one-region artifact and return its payload.
fn emit_header_record(vm: &mut Vm<'static, 'static>, fields: &[i64]) -> Vec<u8> {
    let specs: Vec<RegionSpec> = vec![(keleusma::wire_schema::kind::HEADER, 0, HEADER_STRIDE, 0)];
    let read_len = header_len(1) + HEADER_STRIDE;
    let (written, got) = run_cmd_fields(
        vm,
        CMD_EMIT_HEADER_RECORD,
        1,
        &specs,
        fields,
        [0, 0, 0, 0, 0],
        read_len,
    )
    .expect("run");
    assert_eq!(
        written, HEADER_STRIDE as i64,
        "the emitter did not report a whole record"
    );
    got[header_len(1)..].to_vec()
}

const CMD_EMIT_HEADER_RECORD: i64 = 115;
const HEADER_STRIDE: usize = 32;

#[test]
fn the_emitted_header_record_matches_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, src) in CORPUS_STAGES {
        let (fields, want) = real_header_case(src);
        assert_eq!(want.len(), HEADER_STRIDE, "{name}: HEADER is one record");
        let got = emit_header_record(&mut vm, &fields);
        assert_eq!(got, want, "{name}: emitted HEADER record differs");
    }
}

/// Must-fire control: perturbing any single field must change the bytes.
///
/// This is the assertion that makes the differential mean something. Eleven
/// fields at eleven offsets, and a writer that put two of them in the same
/// place, or dropped one, would still agree with the reference on every field
/// it happened to get right. Perturbing each in turn requires every offset to
/// be independently observable.
#[test]
fn every_header_field_is_independently_observable() {
    let mut vm = vm_for(WIRE_KEL);
    let (base, want) = real_header_case(CORPUS_STAGES[9].1);
    assert_eq!(
        emit_header_record(&mut vm, &base),
        want,
        "control: the clean case must agree"
    );
    for i in 0..base.len() {
        let mut fields = base.clone();
        // Flip a low bit rather than adding, so a u8 field cannot overflow into
        // its neighbour and report a difference for the wrong reason.
        fields[i] ^= 1;
        let got = emit_header_record(&mut vm, &fields);
        assert_ne!(
            got, want,
            "control did not fire: field {i} is not observable in the output"
        );
    }
}

/// The reference reader must accept what Keleusma wrote, and read back the
/// values that went in.
///
/// Byte identity alone would be satisfied by two implementations that are
/// wrong in the same way. This is the independent direction.
#[test]
fn the_reference_reader_recovers_the_fields_keleusma_emitted() {
    let mut vm = vm_for(WIRE_KEL);
    let (fields, _) = real_header_case(CORPUS_STAGES[9].1);
    let specs: Vec<RegionSpec> = vec![(keleusma::wire_schema::kind::HEADER, 0, HEADER_STRIDE, 0)];
    let read_len = header_len(1) + HEADER_STRIDE;
    let (_, artifact) = run_cmd_fields(
        &mut vm,
        CMD_EMIT_HEADER_RECORD,
        1,
        &specs,
        &fields,
        [0, 0, 0, 0, 0],
        read_len,
    )
    .expect("run");

    let view = keleusma_wire::WireView::parse(&artifact).expect("reference accepts the artifact");
    let region = view
        .find_region(keleusma::wire_schema::kind::HEADER)
        .expect("HEADER region present");
    let table = view
        .records(&region, HEADER_STRIDE)
        .expect("HEADER reads as a record table");
    let rec: keleusma::wire_schema::HeaderRecord =
        table.get_as(0).expect("one HeaderRecord reads back");

    assert_eq!(i64::from(rec.entry_point), fields[0], "entry_point");
    assert_eq!(i64::from(rec.word_bits_log2), fields[1], "word_bits_log2");
    assert_eq!(i64::from(rec.addr_bits_log2), fields[2], "addr_bits_log2");
    assert_eq!(i64::from(rec.float_bits_log2), fields[3], "float_bits_log2");
    assert_eq!(i64::from(rec.flags), fields[4], "flags");
    assert_eq!(i64::from(rec.wcet_cycles), fields[5], "wcet_cycles");
    assert_eq!(i64::from(rec.wcmu_bytes), fields[6], "wcmu_bytes");
    assert_eq!(
        i64::from(rec.shared_data_bytes),
        fields[7],
        "shared_data_bytes"
    );
    assert_eq!(
        i64::from(rec.private_data_bytes),
        fields[8],
        "private_data_bytes"
    );
    assert_eq!(i64::from(rec.schema_hash), fields[9], "schema_hash");
    assert_eq!(i64::from(rec.reserved), fields[10], "reserved");
}

// --- WIRING SLICE 3: a multi-record region, and the batching mechanism -------
//
// `CHUNKS` is the smallest region that cannot be emitted in one batch, at two,
// so the mechanism is built where a failure is legible rather than inside
// `DATA_SLOTS`, which needs 1547 of them. `ChunkRecord` is also the widest
// record in the format at fourteen fields.
//
// WHERE THE INPUTS COME FROM, AND WHY IT DIFFERS FROM SLICE 2. Slice 2 derived
// its field values from the module, because a header's fields ARE module
// properties. A chunk record's fields are not: `consts_first`,
// `param_types_first`, `op_byte_offset` and the rest are allocation results
// produced by `SchemaBuilder` while it lays the artifact out. Reproducing them
// here would mean reimplementing the encoder, which is the driver's job in a
// later slice, not this one. They are therefore DECODED from the reference and
// re-emitted, which tests the emitter's field placement, widths and batching
// against the reference's — and the must-fire control below is what keeps that
// from being a test of nothing.

const CMD_EMIT_CHUNK_RECORDS: i64 = 116;
const CHUNK_STRIDE: usize = 48;
const CHUNK_FIELDS: usize = 14;
/// Records per batch, set by `wire.fin` rather than by the output buffer.
const CHUNKS_PER_BATCH: usize = FIN_CAPACITY / CHUNK_FIELDS;

/// The fourteen fields of each of a stage's chunk records, and the reference
/// bytes for the whole `CHUNKS` region.
fn real_chunk_case(src: &str) -> (Vec<[i64; CHUNK_FIELDS]>, Vec<u8>) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let region = view
        .find_region(keleusma::wire_schema::kind::CHUNKS)
        .expect("every module emits a CHUNKS region");
    let want = view
        .region_bytes(&region)
        .expect("CHUNKS payload in range")
        .to_vec();
    let table = view
        .records(&region, CHUNK_STRIDE)
        .expect("CHUNKS reads as a record table");

    let mut recs = Vec::with_capacity(table.len());
    for i in 0..table.len() {
        let c: keleusma::wire_schema::ChunkRecord = table.get_as(i).expect("record in range");
        recs.push([
            i64::from(c.name),
            i64::from(c.consts_first),
            i64::from(c.consts_count),
            i64::from(c.templates_first),
            i64::from(c.templates_count),
            i64::from(c.param_types_first),
            i64::from(c.param_types_count),
            i64::from(c.debug_first),
            i64::from(c.debug_len),
            i64::from(c.op_byte_offset),
            i64::from(c.op_record_count),
            i64::from(c.local_count),
            i64::from(c.param_count),
            i64::from(c.block_type),
        ]);
    }
    (recs, want)
}

/// Emit every record through the window, one batch at a time, and concatenate.
///
/// This is the staged shape in miniature: the Keleusma side never holds the
/// whole region, only one batch's worth, and the caller appends. `window` is
/// deliberately non-zero so an emitter that ignored its address would write at
/// zero and be caught rather than pass.
fn emit_chunks_batched(
    vm: &mut Vm<'static, 'static>,
    recs: &[[i64; CHUNK_FIELDS]],
    window: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(recs.len() * CHUNK_STRIDE);
    for batch in recs.chunks(CHUNKS_PER_BATCH) {
        let fields: Vec<i64> = batch.iter().flat_map(|r| r.iter().copied()).collect();
        let produced = batch.len() * CHUNK_STRIDE;
        let (written, buf) = run_cmd_fields(
            vm,
            CMD_EMIT_CHUNK_RECORDS,
            0,
            &[],
            &fields,
            [window as i64, batch.len() as i64, 0, 0, 0],
            window + produced,
        )
        .expect("run");
        assert_eq!(
            written, produced as i64,
            "the emitter did not report the batch's byte count"
        );
        out.extend_from_slice(&buf[window..window + produced]);
    }
    out
}

#[test]
fn the_emitted_chunk_records_match_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    let mut batched_at_least_one_stage = false;
    for (name, src) in CORPUS_STAGES {
        let (recs, want) = real_chunk_case(src);
        assert!(!recs.is_empty(), "{name}: no chunk records");
        assert_eq!(
            want.len(),
            recs.len() * CHUNK_STRIDE,
            "{name}: payload is not a whole number of records"
        );
        if recs.len() > CHUNKS_PER_BATCH {
            batched_at_least_one_stage = true;
        }
        let got = emit_chunks_batched(&mut vm, &recs, 64);
        assert_eq!(got, want, "{name}: emitted CHUNKS region differs");
    }
    // The corpus must actually cross a batch boundary, or this suite would be
    // testing the single-batch path and reporting the batching path as covered.
    assert!(
        batched_at_least_one_stage,
        "no stage exceeded {CHUNKS_PER_BATCH} records, so batching was never exercised"
    );
}

/// Must-fire control: every one of the fourteen fields must be observable.
///
/// Fourteen fields at fourteen offsets, three different widths. A writer that
/// put two in the same place, truncated a u32 to u16, or dropped one, would
/// still agree with the reference on every field it happened to get right.
#[test]
fn every_chunk_field_is_independently_observable() {
    let mut vm = vm_for(WIRE_KEL);
    let (base, want) = real_chunk_case(CORPUS_STAGES[9].1);
    assert_eq!(
        emit_chunks_batched(&mut vm, &base, 64),
        want,
        "control: the clean case must agree"
    );
    for f in 0..CHUNK_FIELDS {
        let mut recs = base.clone();
        // Flip a low bit rather than adding, so a narrow field cannot overflow
        // into its neighbour and report a difference for the wrong reason.
        recs[0][f] ^= 1;
        assert_ne!(
            emit_chunks_batched(&mut vm, &recs, 64),
            want,
            "control did not fire: chunk field {f} is not observable"
        );
    }
}

/// The window address must be honoured, not assumed.
///
/// This is the property slice 2 could not have: its emitter derived its
/// position from `region_base`, so there was no address to get wrong. A staged
/// emitter is handed a window, and the same records at a different window must
/// produce the same bytes.
#[test]
fn the_emitted_records_do_not_depend_on_the_window_address() {
    let mut vm = vm_for(WIRE_KEL);
    let (recs, want) = real_chunk_case(CORPUS_STAGES[9].1);
    for window in [0usize, 8, 64, 4096] {
        let got = emit_chunks_batched(&mut vm, &recs, window);
        assert_eq!(got, want, "window {window}: output moved with the address");
    }
}

/// Splitting a run across batches must not change the bytes.
///
/// The batch size is an implementation detail of the input channel, so a record
/// must encode identically whether it is first in a batch, last, or alone. A
/// boundary-off-by-one in the field indexing shows up here and nowhere else.
#[test]
fn the_batch_boundary_does_not_change_the_output() {
    let mut vm = vm_for(WIRE_KEL);
    let (recs, want) = real_chunk_case(CORPUS_STAGES[1].1);
    assert!(
        recs.len() > CHUNKS_PER_BATCH,
        "this stage must exceed one batch for the test to mean anything"
    );
    // The natural batching is already exercised above. Here every record is
    // emitted alone, which is the maximal number of boundaries.
    let mut one_at_a_time = Vec::new();
    for r in &recs {
        one_at_a_time.extend_from_slice(&emit_chunks_batched(
            &mut vm,
            core::slice::from_ref(r),
            64,
        ));
    }
    assert_eq!(
        one_at_a_time, want,
        "emitting one record per batch differs from the reference"
    );
}

/// A batch larger than the input array is rejected, not silently truncated.
#[test]
fn an_oversized_batch_is_reported_rather_than_truncated() {
    let mut vm = vm_for(WIRE_KEL);
    let over = (FIN_CAPACITY / CHUNK_FIELDS) + 1;
    let (got, _) = run_cmd_fields(
        &mut vm,
        CMD_EMIT_CHUNK_RECORDS,
        0,
        &[],
        &[],
        [64, over as i64, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(
        got, -200,
        "an oversized batch was not reported with its own code"
    );
}

// --- WIRING SLICE 4: a byte pool, where logical length is not stored length --
//
// A pool is the other half of the format: no stride, no fields, no records.
// None of the record machinery applies, and the one thing it has that a record
// table does not is a LOGICAL length distinct from its STORED length. The
// container stores a region's length in whole words, so a pool of 101 bytes
// occupies 104 and the last three are pad.
//
// THE PAD IS THE ONLY PLACE A BUG CAN LIVE, since copying bytes is otherwise a
// no-op transformation. The corpus is unusually good for it — across the ten
// stages `PARAM_TYPES` produces pads of 0, 3, 4, 5 and 7, including the extreme
// of one logical byte in an eight-byte region. Residues 1, 2 and 6 never occur,
// so a hand-built sweep covers all eight rather than leaving three untested.

const CMD_EMIT_POOL_BYTES: i64 = 117;
const CMD_EMIT_POOL_PAD: i64 = 118;

/// A stage's `PARAM_TYPES`: the logical bytes the encoder was given, and the
/// stored region including its pad.
fn real_pool_case(src: &str) -> (Vec<u8>, Vec<u8>) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    // The logical input, reconstructed the way the encoder receives it: every
    // chunk's parameter type tags, concatenated in chunk order.
    let logical: Vec<u8> = module
        .chunks
        .iter()
        // Through the encoder's own tag-to-byte mapping, not a cast: the pool
        // stores one byte per tag and the mapping is the schema's, not ours.
        .flat_map(|c| {
            c.param_types
                .iter()
                .map(|t| keleusma::wire_schema::type_tag_byte(*t))
        })
        .collect();
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let region = view
        .find_region(keleusma::wire_schema::kind::PARAM_TYPES)
        .expect("every module emits a PARAM_TYPES region");
    let stored = view
        .region_bytes(&region)
        .expect("PARAM_TYPES payload in range")
        .to_vec();
    (logical, stored)
}

/// Emit a pool through the window in batches, then its pad, and concatenate.
///
/// This is the staged shape for a pool: the Keleusma side holds one batch, the
/// caller appends, and the pad is written once at the end from the TOTAL length
/// rather than the batch's, which is the part a per-batch implementation would
/// get wrong.
fn emit_pool_batched(
    vm: &mut Vm<'static, 'static>,
    logical: &[u8],
    window: usize,
    batch: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(logical.len());
    for part in logical.chunks(batch) {
        let (written, buf) = run_cmd_pool(
            vm,
            CMD_EMIT_POOL_BYTES,
            part,
            [window as i64, part.len() as i64, 0, 0, 0],
            window + part.len(),
        )
        .expect("run");
        assert_eq!(written, part.len() as i64, "wrong batch byte count");
        out.extend_from_slice(&buf[window..window + part.len()]);
    }
    let (pad, buf) = run_cmd_pool(
        vm,
        CMD_EMIT_POOL_PAD,
        &[],
        [window as i64, logical.len() as i64, 0, 0, 0],
        window + 8,
    )
    .expect("run");
    assert!((0..8).contains(&pad), "pad {pad} is not a byte residue");
    out.extend_from_slice(&buf[window..window + pad as usize]);
    out
}

#[test]
fn the_emitted_pool_matches_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    let mut residues: Vec<usize> = Vec::new();
    for (name, src) in CORPUS_STAGES {
        let (logical, stored) = real_pool_case(src);
        assert!(!logical.is_empty(), "{name}: no parameter type tags");
        residues.push(stored.len() - logical.len());
        let got = emit_pool_batched(&mut vm, &logical, 64, BIN_CAPACITY);
        assert_eq!(got, stored, "{name}: emitted PARAM_TYPES differs");
    }
    // What the corpus actually reached, asserted rather than assumed. If this
    // ever narrows to {0}, the padding path is untested by real data and the
    // hand-built sweep below is carrying the whole property.
    residues.sort_unstable();
    residues.dedup();
    assert!(
        residues.len() >= 4,
        "the corpus reached only pad residues {residues:?}, too few to exercise padding"
    );
}

/// Every pad residue, including the three the corpus never reaches.
///
/// A pool's length modulo eight is the whole of what the pad depends on, so
/// leaving 1, 2 and 6 untested would leave three of eight cases to chance.
#[test]
fn every_pad_residue_is_correct_including_those_the_corpus_never_reaches() {
    let mut vm = vm_for(WIRE_KEL);
    for len in 1usize..=32 {
        // A recognisable, non-zero body so a pad byte cannot be mistaken for a
        // payload byte, and vice versa.
        let logical: Vec<u8> = (0..len).map(|i| (i as u8) | 0x80).collect();
        let want_pad = (8 - (len % 8)) % 8;

        let mut want = logical.clone();
        want.extend(core::iter::repeat_n(0u8, want_pad));
        assert_eq!(want.len() % 8, 0, "the expectation is not word-aligned");

        let got = emit_pool_batched(&mut vm, &logical, 64, BIN_CAPACITY);
        assert_eq!(
            got,
            want,
            "length {len} (residue {}, pad {want_pad}) emitted wrongly",
            len % 8
        );
    }
}

/// The pad must be WRITTEN, not inherited from a zeroed buffer.
///
/// This is the assertion the emitter's comment claims and nothing else checks.
/// A staged emitter reuses one window across batches, so the day the window
/// holds a previous batch's bytes, a pad that relies on initial zeroes emits
/// stale payload instead.
///
/// `wire.bytes` starts zeroed on every call, so the ONLY way to see this is to
/// seed the buffer dirty first. `run_cmd_args` seeds it and `emit_pool_pad`
/// needs no pool input, so the two compose.
#[test]
fn the_pad_is_written_rather_than_inherited_from_a_zeroed_buffer() {
    let mut vm = vm_for(WIRE_KEL);
    let window = 64usize;
    let dirty = vec![0xEEu8; 128];
    for total in 1usize..=8 {
        let want_pad = (8 - (total % 8)) % 8;
        let (pad, buf) = run_cmd_args(
            &mut vm,
            CMD_EMIT_POOL_PAD,
            0,
            &dirty,
            &[],
            [window as i64, total as i64, 0, 0, 0],
            window + 8,
        )
        .expect("run");
        assert_eq!(pad, want_pad as i64, "total {total}: wrong pad length");
        // The control: the seed really did dirty this window, so zeroes here
        // can only have been written.
        assert_eq!(
            buf[window + want_pad],
            0xEE,
            "total {total}: the buffer was not dirty past the pad, so this proves nothing"
        );
        assert!(
            buf[window..window + want_pad].iter().all(|b| *b == 0),
            "total {total}: the pad kept stale bytes instead of writing zeroes"
        );
    }
}

/// The window address must be honoured for pools too.
#[test]
fn the_emitted_pool_does_not_depend_on_the_window_address() {
    let mut vm = vm_for(WIRE_KEL);
    let (logical, stored) = real_pool_case(CORPUS_STAGES[1].1);
    for window in [0usize, 8, 64, 4096] {
        let got = emit_pool_batched(&mut vm, &logical, window, BIN_CAPACITY);
        assert_eq!(
            got, stored,
            "window {window}: output moved with the address"
        );
    }
}

/// Splitting a pool across batches must not change the bytes, and the pad must
/// come from the TOTAL length rather than the last batch's.
///
/// A per-batch pad is the natural wrong implementation: it pads every batch to
/// a word boundary and produces a longer region with zeroes sprinkled through
/// it. Batch sizes that do not divide the length are what expose it.
#[test]
fn the_pool_batch_size_does_not_change_the_output() {
    let mut vm = vm_for(WIRE_KEL);
    let (logical, stored) = real_pool_case(CORPUS_STAGES[1].1);
    assert!(
        logical.len() > 8,
        "the pool must exceed one batch to matter"
    );
    for batch in [1usize, 3, 7, 8, 13, 64, BIN_CAPACITY] {
        let got = emit_pool_batched(&mut vm, &logical, 64, batch);
        assert_eq!(got, stored, "batch size {batch} changed the output");
    }
}

/// Must-fire control: a perturbed input byte must reach the output.
#[test]
fn the_pool_differential_reports_a_perturbed_byte() {
    let mut vm = vm_for(WIRE_KEL);
    let (logical, stored) = real_pool_case(CORPUS_STAGES[1].1);
    assert_eq!(
        emit_pool_batched(&mut vm, &logical, 64, BIN_CAPACITY),
        stored,
        "control: the clean case must agree"
    );
    for i in [0usize, 1, logical.len() / 2, logical.len() - 1] {
        let mut perturbed = logical.clone();
        perturbed[i] ^= 1;
        assert_ne!(
            emit_pool_batched(&mut vm, &perturbed, 64, BIN_CAPACITY),
            stored,
            "control did not fire: input byte {i} is not observable"
        );
    }
}

/// A batch larger than the input array is rejected, not silently truncated.
#[test]
fn an_oversized_pool_batch_is_reported_rather_than_truncated() {
    let mut vm = vm_for(WIRE_KEL);
    let (got, _) = run_cmd_pool(
        &mut vm,
        CMD_EMIT_POOL_BYTES,
        &[],
        [64, BIN_CAPACITY as i64 + 1, 0, 0, 0],
        0,
    )
    .expect("run");
    assert_eq!(
        got, -201,
        "an oversized pool batch was not reported with its own code"
    );
}

// --- WIRING SLICE 5: the two accumulator regions -----------------------------
//
// `NAMES` and `STRING_POOL` are the pair the residency measurement singled out.
// `SchemaBuilder` writes them LAST, after every other contributor has interned
// into them, so they are the emission's resident set rather than transient
// buffers — 9,776,392 bytes for `lexer`, 58.3% of the shared ceiling.
//
// They are one of each shape, which is why they are done together: `NAMES` is a
// record table and `STRING_POOL` is the byte pool it indexes. The pool needed no
// new Keleusma code at all; slice 4's emitter already does it, and this is the
// first time that emitter meets something large enough to batch hundreds of
// times.
//
// THESE ARE THE LARGEST TABLES IN THE FORMAT, AND THEY COST REAL GATE TIME.
// `lexer` alone is 395,804 name records, 774 input batches and 807 pool batches.
// Measured 2026-08-09: this one test is **201 s**, taking the suite from about
// 23 s to 152 s, and the gate runs the suite once per feature configuration, so
// it adds roughly nine minutes to a 2h33m gate.
//
// KEPT AT FULL COVERAGE DELIBERATELY, and the cost is stated rather than hidden.
// The time is not waste to be optimised away: it is ~7.4 million `set_shared`
// and `get_shared` calls in a debug build, which is what driving 6.6 MB through
// the public shared-data API costs. Hoisting the buffer would not help, and the
// batching depth is the property under test.
//
// `parse` alone would give 226 and 131 batches for about a third of the time,
// which is also "deep". Whether that trade is worth taking is a GATE-SCOPE
// decision and therefore the operator's, in the same class as trimming the
// feature matrix — not the loop's to take quietly. Recorded in REVERSE_PROMPT.

const CMD_EMIT_NAME_RECORDS: i64 = 119;
const NAMEREF_STRIDE: usize = 8;
const NAMEREF_FIELDS: usize = 2;
const NAMES_PER_BATCH: usize = FIN_CAPACITY / NAMEREF_FIELDS;

/// A stage's `NAMES` records and `STRING_POOL`, with the pool's LOGICAL length.
///
/// The logical length is not stored anywhere: the container keeps whole words,
/// so a pool of 6,609,957 bytes reports 6,609,960. It is recovered as the
/// furthest extent any name reaches, which is exact because names are appended
/// in interning order and the last one ends at the pool's true end.
struct AccumCase {
    names: Vec<(u32, u32)>,
    names_stored: Vec<u8>,
    pool_logical: Vec<u8>,
    pool_stored: Vec<u8>,
}

fn real_accum_case(src: &str) -> AccumCase {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");

    let nregion = view
        .find_region(keleusma::wire_schema::kind::NAMES)
        .expect("NAMES region");
    let names_stored = view.region_bytes(&nregion).expect("NAMES payload").to_vec();
    let table = view
        .records(&nregion, NAMEREF_STRIDE)
        .expect("NAMES reads as a record table");
    let mut names = Vec::with_capacity(table.len());
    for i in 0..table.len() {
        let r: keleusma::wire_schema::NameRef = table.get_as(i).expect("record in range");
        names.push((r.offset, r.length));
    }

    let pregion = view
        .find_region(keleusma::wire_schema::kind::STRING_POOL)
        .expect("STRING_POOL region");
    let pool_stored = view
        .region_bytes(&pregion)
        .expect("STRING_POOL payload")
        .to_vec();
    let logical_len = names
        .iter()
        .map(|(o, l)| (*o as usize) + (*l as usize))
        .max()
        .unwrap_or(0);
    assert!(
        logical_len <= pool_stored.len(),
        "a name reaches past the pool, so the extent calculation is wrong"
    );
    assert!(
        pool_stored.len() - logical_len < 8,
        "the recovered logical length is more than a word short, so it is not the true extent"
    );
    let pool_logical = pool_stored[..logical_len].to_vec();

    AccumCase {
        names,
        names_stored,
        pool_logical,
        pool_stored,
    }
}

fn emit_names_batched(
    vm: &mut Vm<'static, 'static>,
    names: &[(u32, u32)],
    window: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(names.len() * NAMEREF_STRIDE);
    for batch in names.chunks(NAMES_PER_BATCH) {
        let fields: Vec<i64> = batch
            .iter()
            .flat_map(|(o, l)| [i64::from(*o), i64::from(*l)])
            .collect();
        let produced = batch.len() * NAMEREF_STRIDE;
        let (written, buf) = run_cmd_fields(
            vm,
            CMD_EMIT_NAME_RECORDS,
            0,
            &[],
            &fields,
            [window as i64, batch.len() as i64, 0, 0, 0],
            window + produced,
        )
        .expect("run");
        assert_eq!(written, produced as i64, "wrong batch byte count");
        out.extend_from_slice(&buf[window..window + produced]);
    }
    out
}

#[test]
fn the_emitted_accumulator_regions_match_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    let mut most_name_batches = 0usize;
    let mut most_pool_batches = 0usize;
    for (name, src) in CORPUS_STAGES {
        let case = real_accum_case(src);
        most_name_batches = most_name_batches.max(case.names.len().div_ceil(NAMES_PER_BATCH));
        most_pool_batches = most_pool_batches.max(case.pool_logical.len().div_ceil(BIN_CAPACITY));

        let got_names = emit_names_batched(&mut vm, &case.names, 64);
        assert_eq!(got_names, case.names_stored, "{name}: NAMES differs");

        let got_pool = emit_pool_batched(&mut vm, &case.pool_logical, 64, BIN_CAPACITY);
        assert_eq!(got_pool, case.pool_stored, "{name}: STRING_POOL differs");
    }
    // Everything before this slice batched at most twice. Asserted so a corpus
    // change that shrank these tables would report the loss of deep-batch
    // coverage rather than silently keeping the test green.
    assert!(
        most_name_batches > 100,
        "deepest NAMES run was only {most_name_batches} batches"
    );
    assert!(
        most_pool_batches > 100,
        "deepest STRING_POOL run was only {most_pool_batches} batches"
    );
}

/// Must-fire control for both accumulators.
#[test]
fn the_accumulator_differentials_report_a_perturbation() {
    let mut vm = vm_for(WIRE_KEL);
    let case = real_accum_case(CORPUS_STAGES[9].1);
    assert_eq!(
        emit_names_batched(&mut vm, &case.names, 64),
        case.names_stored,
        "control: clean NAMES must agree"
    );
    // Both fields of a NameRef, since they are the same width at adjacent
    // offsets and a swapped pair is the natural mistranscription.
    for field in 0..2 {
        let mut names = case.names.clone();
        if field == 0 {
            names[1].0 ^= 1;
        } else {
            names[1].1 ^= 1;
        }
        assert_ne!(
            emit_names_batched(&mut vm, &names, 64),
            case.names_stored,
            "control did not fire: NameRef field {field} is not observable"
        );
    }
    // A transposed pair of records, which a per-record loop off-by-one produces
    // and which perturbing a single field would not catch.
    let mut swapped = case.names.clone();
    swapped.swap(0, 1);
    assert_ne!(
        emit_names_batched(&mut vm, &swapped, 64),
        case.names_stored,
        "control did not fire: record order is not observable"
    );
}

// --- WIRING SLICE 6: the two per-slot tables ---------------------------------
//
// `DATA_SLOTS` and `SHARED_LAYOUT` complete the four regions that are 99.96% of
// `lexer`'s auxiliary body, alongside `NAMES` and `STRING_POOL` from slice 5.
// All three record tables carry the same count, one entry per data slot,
// because every array element becomes its own slot.
//
// A DELIBERATE, STATED COVERAGE CAP, WHICH IS THE POINT OF THIS COMMENT.
// `lexer` has 395,784 records in each of these tables. Emitting them all would
// cost roughly 130 s per table, on top of slice 5's measured 201 s, and would
// add close to half an hour to a gate across the feature matrix.
//
// It would also buy almost nothing. What is NEW here is FIELD PLACEMENT for two
// more record shapes, which needs a handful of records. DEEP BATCHING is the
// property slice 5 established, at 774 and 807 batches, and re-establishing it
// per record kind is repetition rather than coverage.
//
// So each stage is compared over its first `SLOT_RECORD_CAP` records. The cap is
// named, asserted to actually cross several batch boundaries, and reported here
// rather than left for a reader to infer from a magic number.
const SLOT_RECORD_CAP: usize = 2048;

const CMD_EMIT_DATA_SLOT_RECORDS: i64 = 120;
const CMD_EMIT_SHARED_SLOT_RECORDS: i64 = 121;
const SLOT_STRIDE: usize = 8;
const SLOT_FIELDS: usize = 4;
const SLOTS_PER_BATCH: usize = FIN_CAPACITY / SLOT_FIELDS;

/// Both per-slot tables of a stage, as field rows and as reference bytes.
struct SlotCase {
    data: Vec<[i64; SLOT_FIELDS]>,
    data_want: Vec<u8>,
    shared: Vec<[i64; SLOT_FIELDS]>,
    shared_want: Vec<u8>,
}

fn real_slot_case(src: &str) -> SlotCase {
    use keleusma::wire_schema::{DataSlotRecord, SharedSlotRecord, kind};
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");

    let dregion = view
        .find_region(kind::DATA_SLOTS)
        .expect("DATA_SLOTS region");
    let dbytes = view.region_bytes(&dregion).expect("payload").to_vec();
    let dtable = view.records(&dregion, SLOT_STRIDE).expect("record table");
    let dn = dtable.len().min(SLOT_RECORD_CAP);
    let mut dslots = Vec::with_capacity(dn);
    for i in 0..dn {
        let r: DataSlotRecord = dtable.get_as(i).expect("record in range");
        dslots.push([
            i64::from(r.name),
            i64::from(r.visibility),
            i64::from(r.reserved),
            i64::from(r.reserved2),
        ]);
    }

    let sregion = view
        .find_region(kind::SHARED_LAYOUT)
        .expect("SHARED_LAYOUT region");
    let sbytes = view.region_bytes(&sregion).expect("payload").to_vec();
    let stable = view.records(&sregion, SLOT_STRIDE).expect("record table");
    let sn = stable.len().min(SLOT_RECORD_CAP);
    let mut sslots = Vec::with_capacity(sn);
    for i in 0..sn {
        let r: SharedSlotRecord = stable.get_as(i).expect("record in range");
        sslots.push([
            i64::from(r.offset),
            i64::from(r.kind),
            i64::from(r.reserved),
            i64::from(r.len),
        ]);
    }

    SlotCase {
        data: dslots,
        data_want: dbytes[..dn * SLOT_STRIDE].to_vec(),
        shared: sslots,
        shared_want: sbytes[..sn * SLOT_STRIDE].to_vec(),
    }
}

fn emit_slots_batched(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    recs: &[[i64; SLOT_FIELDS]],
    window: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(recs.len() * SLOT_STRIDE);
    for batch in recs.chunks(SLOTS_PER_BATCH) {
        let fields: Vec<i64> = batch.iter().flat_map(|r| r.iter().copied()).collect();
        let produced = batch.len() * SLOT_STRIDE;
        let (written, buf) = run_cmd_fields(
            vm,
            cmd,
            0,
            &[],
            &fields,
            [window as i64, batch.len() as i64, 0, 0, 0],
            window + produced,
        )
        .expect("run");
        assert_eq!(written, produced as i64, "wrong batch byte count");
        out.extend_from_slice(&buf[window..window + produced]);
    }
    out
}

#[test]
fn the_emitted_per_slot_tables_match_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    let mut deepest = 0usize;
    for (name, src) in CORPUS_STAGES {
        let case = real_slot_case(src);
        assert!(!case.data.is_empty(), "{name}: no data slots");
        deepest = deepest.max(case.data.len().div_ceil(SLOTS_PER_BATCH));

        let dgot = emit_slots_batched(&mut vm, CMD_EMIT_DATA_SLOT_RECORDS, &case.data, 64);
        assert_eq!(dgot, case.data_want, "{name}: DATA_SLOTS differs");

        let sgot = emit_slots_batched(&mut vm, CMD_EMIT_SHARED_SLOT_RECORDS, &case.shared, 64);
        assert_eq!(sgot, case.shared_want, "{name}: SHARED_LAYOUT differs");
    }
    // The cap must still cross several batch boundaries, or it would have
    // quietly reduced this to a single-batch test.
    assert!(
        deepest >= 8,
        "the record cap left only {deepest} batches, too few to exercise batching at all"
    );
}

/// Must-fire control for both per-slot tables, including the reserved fields.
///
/// The reserved fields are the interesting ones: no reader consults them, so
/// nothing else in the suite would notice an emitter that skipped them, and an
/// emitter that skipped them would still pass against a zeroed buffer.
#[test]
fn every_per_slot_field_is_independently_observable_including_reserved() {
    let mut vm = vm_for(WIRE_KEL);
    let case = real_slot_case(CORPUS_STAGES[9].1);
    for (label, cmd, base, want) in [
        (
            "DATA_SLOTS",
            CMD_EMIT_DATA_SLOT_RECORDS,
            &case.data,
            &case.data_want,
        ),
        (
            "SHARED_LAYOUT",
            CMD_EMIT_SHARED_SLOT_RECORDS,
            &case.shared,
            &case.shared_want,
        ),
    ] {
        assert_eq!(
            &emit_slots_batched(&mut vm, cmd, base, 64),
            want,
            "{label}: the clean case must agree"
        );
        for f in 0..SLOT_FIELDS {
            let mut recs = base.clone();
            recs[0][f] ^= 1;
            assert_ne!(
                &emit_slots_batched(&mut vm, cmd, &recs, 64),
                want,
                "{label}: control did not fire, field {f} is not observable"
            );
        }
    }
}

/// An oversized batch is rejected with its own code, per table.
#[test]
fn an_oversized_per_slot_batch_is_reported_rather_than_truncated() {
    let mut vm = vm_for(WIRE_KEL);
    let over = (FIN_CAPACITY / SLOT_FIELDS) + 1;
    for (cmd, code) in [
        (CMD_EMIT_DATA_SLOT_RECORDS, -203),
        (CMD_EMIT_SHARED_SLOT_RECORDS, -204),
    ] {
        let (got, _) =
            run_cmd_fields(&mut vm, cmd, 0, &[], &[], [64, over as i64, 0, 0, 0], 0).expect("run");
        assert_eq!(got, code, "command {cmd}: wrong or missing rejection code");
    }
}

// --- WIRING SLICE 7: the remaining populated record tables -------------------
//
// Six kinds, all mechanical: every offset was already transcribed for the
// readers, and batching, window addressing and the oversize guard are unchanged
// since slice 3. One kind, `ENUM_VARIANTS`, needed a reserved field transcribed
// for the emitter, as the per-slot tables did.
//
// TWO CARRY 64-BIT FIELDS, which is the one genuinely new thing. `ConstRecord`
// has a `payload` and `EnumVariantRecord` a SIGNED `disc`, so `put_u64` writes
// two little-endian limbs. The signed case is not hypothetical — the reader
// suite already pins negative discriminants — and it is correct only because
// `lsr` is logical over the whole word, which is asserted by the corpus rather
// than assumed.

struct RecordKind {
    label: &'static str,
    cmd: i64,
    kind: u16,
    stride: usize,
    fields: usize,
}

const SLICE7_KINDS: &[RecordKind] = &[
    RecordKind {
        label: "SHAPES",
        cmd: 122,
        kind: keleusma::wire_schema::kind::SHAPES,
        stride: 8,
        fields: 4,
    },
    RecordKind {
        label: "SIGNATURES",
        cmd: 123,
        kind: keleusma::wire_schema::kind::SIGNATURES,
        stride: 16,
        fields: 4,
    },
    RecordKind {
        label: "ENUM_VARIANTS",
        cmd: 124,
        kind: keleusma::wire_schema::kind::ENUM_VARIANTS,
        stride: 16,
        fields: 3,
    },
    RecordKind {
        label: "ENUM_LAYOUTS",
        cmd: 125,
        kind: keleusma::wire_schema::kind::ENUM_LAYOUTS,
        stride: 16,
        fields: 4,
    },
    RecordKind {
        label: "DATA_INIT",
        cmd: 126,
        kind: keleusma::wire_schema::kind::DATA_INIT,
        stride: 8,
        fields: 2,
    },
    RecordKind {
        label: "CONSTS",
        cmd: 127,
        kind: keleusma::wire_schema::kind::CONSTS,
        stride: 16,
        fields: 4,
    },
];

/// Decode one region's records into field rows, per kind.
///
/// Explicit per kind rather than generic over a layout table: the widths and
/// signedness differ, and a generic decoder would need the very offset
/// knowledge this suite exists to check independently.
fn decode_slice7(kind: u16, table: &keleusma_wire::RecordTable<'_>) -> Vec<Vec<i64>> {
    use keleusma::wire_schema as w;
    let mut out = Vec::with_capacity(table.len());
    for i in 0..table.len() {
        let row: Vec<i64> = match kind {
            k if k == w::kind::SHAPES => {
                let r: w::ShapeRecord = table.get_as(i).expect("record");
                vec![
                    i64::from(r.tag),
                    i64::from(r.kind),
                    i64::from(r.reserved),
                    i64::from(r.size),
                ]
            }
            k if k == w::kind::SIGNATURES => {
                let r: w::SignatureRecord = table.get_as(i).expect("record");
                vec![
                    i64::from(r.params_first),
                    i64::from(r.params_count),
                    i64::from(r.ret),
                    i64::from(r.resume),
                ]
            }
            k if k == w::kind::ENUM_VARIANTS => {
                let r: w::EnumVariantRecord = table.get_as(i).expect("record");
                vec![i64::from(r.name), i64::from(r.reserved), r.disc]
            }
            k if k == w::kind::ENUM_LAYOUTS => {
                let r: w::EnumLayoutRecord = table.get_as(i).expect("record");
                vec![
                    i64::from(r.type_name),
                    i64::from(r.variants_first),
                    i64::from(r.variants_count),
                    i64::from(r.min_payload),
                ]
            }
            k if k == w::kind::DATA_INIT => {
                let r: w::DataInitRecord = table.get_as(i).expect("record");
                vec![i64::from(r.first), i64::from(r.count)]
            }
            k if k == w::kind::CONSTS => {
                let r: w::ConstRecord = table.get_as(i).expect("record");
                vec![
                    i64::from(r.tag),
                    i64::from(r.flags),
                    i64::from(r.aux),
                    r.payload as i64,
                ]
            }
            other => panic!("no decoder for kind {other:#06x}"),
        };
        out.push(row);
    }
    out
}

fn emit_rows_batched(
    vm: &mut Vm<'static, 'static>,
    rk: &RecordKind,
    rows: &[Vec<i64>],
    window: usize,
) -> Vec<u8> {
    let per_batch = FIN_CAPACITY / rk.fields;
    let mut out = Vec::with_capacity(rows.len() * rk.stride);
    for batch in rows.chunks(per_batch) {
        let fields: Vec<i64> = batch.iter().flatten().copied().collect();
        let produced = batch.len() * rk.stride;
        let (written, buf) = run_cmd_fields(
            vm,
            rk.cmd,
            0,
            &[],
            &fields,
            [window as i64, batch.len() as i64, 0, 0, 0],
            window + produced,
        )
        .expect("run");
        assert_eq!(
            written, produced as i64,
            "{}: wrong batch byte count",
            rk.label
        );
        out.extend_from_slice(&buf[window..window + produced]);
    }
    out
}

/// A stage's rows and reference bytes for one record kind.
fn slice7_case(src: &str, rk: &RecordKind) -> (Vec<Vec<i64>>, Vec<u8>) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let region = view.find_region(rk.kind).expect("region present");
    let want = view.region_bytes(&region).expect("payload").to_vec();
    let table = view.records(&region, rk.stride).expect("record table");
    (decode_slice7(rk.kind, &table), want)
}

#[test]
fn the_remaining_populated_tables_match_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    // Which kinds the corpus actually populates, asserted rather than assumed:
    // three of these six are empty in every stage, and a differential over an
    // empty table passes without emitting a single record.
    let mut populated: Vec<&str> = Vec::new();
    for (name, src) in CORPUS_STAGES {
        for rk in SLICE7_KINDS {
            let (rows, want) = slice7_case(src, rk);
            if !rows.is_empty() && !populated.contains(&rk.label) {
                populated.push(rk.label);
            }
            let got = emit_rows_batched(&mut vm, rk, &rows, 64);
            assert_eq!(got, want, "{name}: {} differs", rk.label);
        }
    }
    populated.sort_unstable();
    assert_eq!(
        populated,
        vec![
            "CONSTS",
            "DATA_INIT",
            "ENUM_LAYOUTS",
            "ENUM_VARIANTS",
            "SHAPES",
            "SIGNATURES"
        ],
        "the set of corpus-populated kinds changed; coverage moved"
    );
}

/// Must-fire control: every field of every kind must be observable.
#[test]
fn every_field_of_the_remaining_tables_is_independently_observable() {
    let mut vm = vm_for(WIRE_KEL);
    for rk in SLICE7_KINDS {
        // Pick a stage that actually populates this kind, or the control would
        // be perturbing an empty table and passing for the wrong reason.
        let Some((_, rows, want)) = CORPUS_STAGES.iter().find_map(|(n, src)| {
            let (rows, want) = slice7_case(src, rk);
            (!rows.is_empty()).then_some((n, rows, want))
        }) else {
            panic!("{}: no stage populates this kind", rk.label);
        };
        assert_eq!(
            emit_rows_batched(&mut vm, rk, &rows, 64),
            want,
            "{}: the clean case must agree",
            rk.label
        );
        for f in 0..rk.fields {
            let mut perturbed = rows.clone();
            perturbed[0][f] ^= 1;
            assert_ne!(
                emit_rows_batched(&mut vm, rk, &perturbed, 64),
                want,
                "{}: control did not fire, field {f} is not observable",
                rk.label
            );
        }
    }
}

/// A negative enum discriminant survives the two-limb write.
///
/// `put_u64` splits a `Word` into low and high halves with `lsr`, which is
/// logical over the whole word. A signed shift there would sign-extend the high
/// limb and corrupt every negative discriminant, and the corpus may not contain
/// one, so this is constructed rather than hoped for.
#[test]
fn a_negative_discriminant_round_trips_through_the_two_limb_write() {
    let mut vm = vm_for(WIRE_KEL);
    let rk = &SLICE7_KINDS[2];
    assert_eq!(rk.label, "ENUM_VARIANTS");
    for disc in [
        -1i64,
        -2,
        -128,
        -129,
        -2147483648,
        -2147483649,
        i64::MIN,
        0,
        1,
        i64::MAX,
    ] {
        let rows = vec![vec![7i64, 0, disc]];
        let got = emit_rows_batched(&mut vm, rk, &rows, 64);
        let mut want = vec![0u8; 16];
        want[0..4].copy_from_slice(&7u32.to_le_bytes());
        want[8..16].copy_from_slice(&disc.to_le_bytes());
        assert_eq!(got, want, "discriminant {disc} emitted wrongly");
    }
}

// --- WIRING SLICE 8: the kinds the corpus never populates --------------------
//
// Six record shapes are emitted as EMPTY regions by every one of the ten stage
// sources, so no differential against real output can reach them. For a READER
// an empty region and a populated one are different cases and both were already
// covered. For an EMITTER they are the same problem: no record of that kind is
// ever written, so a mistranscribed offset would go unseen indefinitely.
//
// THE ORACLE IS INDEPENDENT CONSTRUCTION, NOT THE CORPUS. `#[derive(WireRecord)]`
// generates `write_record`, which is the authority on the packed layout, and the
// expected bytes come from it. That is stronger than comparing against my own
// idea of the layout and is the same oracle the hand-built record tests use.
//
// These do NOT block the driver: a zero-record region is declared in the
// directory with length zero and needs no emitter. They are what a general
// compiler needs the day a program declares a struct template or a native, which
// the toolchain's own sources happen never to do.

/// Distinct, non-zero, and different in every field position.
///
/// A field that is zero, or equal to its neighbour, cannot expose a swap or a
/// dropped write. The generator is deterministic so a failure is reproducible.
fn distinct(seed: u32, field: usize) -> u32 {
    // Spread across all four bytes so a truncation to u16 or u8 also shows.
    0x0100_0001u32
        .wrapping_mul(seed + 1)
        .wrapping_add((field as u32 + 1) * 0x0011_0101)
        | 0x0080_0080
}

#[test]
fn the_uncovered_record_kinds_emit_what_the_derive_constructs() {
    use keleusma::wire_schema::{
        EnumAux, NativeRecord, NativeReturnRecord, PrivateCompositeRecord, StructAux,
        StructTemplateRecord,
    };
    let mut vm = vm_for(WIRE_KEL);
    const N: usize = 5;
    let window = 64usize;

    // StructAux: two u32.
    {
        let recs: Vec<StructAux> = (0..N as u32)
            .map(|i| StructAux {
                type_name: distinct(i, 0),
                field_names_first: distinct(i, 1),
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| vec![i64::from(r.type_name), i64::from(r.field_names_first)])
            .collect();
        check_uncovered(
            &mut vm,
            128,
            StructAux::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "STRUCT_AUX",
        );
    }

    // EnumAux: two u32 and a SIGNED 64-bit discriminant.
    {
        let discs = [-1i64, i64::MIN, 0, 1, i64::MAX];
        let recs: Vec<EnumAux> = (0..N)
            .map(|i| EnumAux {
                type_name: distinct(i as u32, 0),
                variant: distinct(i as u32, 1),
                discriminant: discs[i],
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| vec![i64::from(r.type_name), i64::from(r.variant), r.discriminant])
            .collect();
        check_uncovered(
            &mut vm,
            129,
            EnumAux::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "ENUM_AUX",
        );
    }

    // StructTemplateRecord: four u32, including a reserved field.
    {
        let recs: Vec<StructTemplateRecord> = (0..N as u32)
            .map(|i| StructTemplateRecord {
                type_name: distinct(i, 0),
                field_names_first: distinct(i, 1),
                field_count: distinct(i, 2),
                reserved: distinct(i, 3),
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| {
                vec![
                    i64::from(r.type_name),
                    i64::from(r.field_names_first),
                    i64::from(r.field_count),
                    i64::from(r.reserved),
                ]
            })
            .collect();
        check_uncovered(
            &mut vm,
            130,
            StructTemplateRecord::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "STRUCT_TEMPLATES",
        );
    }

    // PrivateCompositeRecord: two u16 then a u32, the only mixed-width case here.
    {
        let recs: Vec<PrivateCompositeRecord> = (0..N as u32)
            .map(|i| PrivateCompositeRecord {
                slot: distinct(i, 0) as u16,
                reserved: distinct(i, 1) as u16,
                offset: distinct(i, 2),
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| {
                vec![
                    i64::from(r.slot),
                    i64::from(r.reserved),
                    i64::from(r.offset),
                ]
            })
            .collect();
        check_uncovered(
            &mut vm,
            131,
            PrivateCompositeRecord::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "PRIVATE_COMPOSITE",
        );
    }

    // NativeRecord and NativeReturnRecord: two u32 each, distinct kinds sharing
    // a shape. Emitting one with the other's command must still be caught, which
    // the differing values below ensure.
    {
        let recs: Vec<NativeRecord> = (0..N as u32)
            .map(|i| NativeRecord {
                name: distinct(i, 0),
                reserved: distinct(i, 1),
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| vec![i64::from(r.name), i64::from(r.reserved)])
            .collect();
        check_uncovered(
            &mut vm,
            132,
            NativeRecord::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "NATIVES",
        );
    }
    {
        let recs: Vec<NativeReturnRecord> = (0..N as u32)
            .map(|i| NativeReturnRecord {
                shape: distinct(i, 2),
                reserved: distinct(i, 3),
            })
            .collect();
        let rows: Vec<Vec<i64>> = recs
            .iter()
            .map(|r| vec![i64::from(r.shape), i64::from(r.reserved)])
            .collect();
        check_uncovered(
            &mut vm,
            133,
            NativeReturnRecord::STRIDE,
            &rows,
            &pack(&recs),
            window,
            "NATIVE_RETURNS",
        );
    }
}

/// Expected bytes, built by the derive rather than by hand.
fn pack<T: keleusma_wire::WireRecord>(recs: &[T]) -> Vec<u8> {
    let mut out = vec![0u8; recs.len() * T::STRIDE];
    for (i, r) in recs.iter().enumerate() {
        r.write_record(&mut out[i * T::STRIDE..(i + 1) * T::STRIDE])
            .expect("write_record");
    }
    out
}

/// Emit `rows` through `cmd` and require both byte identity and that every
/// field is independently observable.
fn check_uncovered(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    stride: usize,
    rows: &[Vec<i64>],
    want: &[u8],
    window: usize,
    label: &str,
) {
    let fields = rows[0].len();
    let emit = |vm: &mut Vm<'static, 'static>, rows: &[Vec<i64>]| -> Vec<u8> {
        let flat: Vec<i64> = rows.iter().flatten().copied().collect();
        let produced = rows.len() * stride;
        let (written, buf) = run_cmd_fields(
            vm,
            cmd,
            0,
            &[],
            &flat,
            [window as i64, rows.len() as i64, 0, 0, 0],
            window + produced,
        )
        .expect("run");
        assert_eq!(written, produced as i64, "{label}: wrong byte count");
        buf[window..window + produced].to_vec()
    };

    assert_eq!(emit(vm, rows), want, "{label}: differs from the derive");

    // Must-fire, per field. Without this the agreement above could come from two
    // implementations wrong in the same way, or from a field never written.
    for f in 0..fields {
        let mut perturbed = rows.to_vec();
        perturbed[0][f] ^= 1;
        assert_ne!(
            emit(vm, &perturbed),
            want,
            "{label}: control did not fire, field {f} is not observable"
        );
    }
}

/// Every uncovered kind rejects an oversized batch with its own code.
#[test]
fn the_uncovered_kinds_reject_an_oversized_batch() {
    let mut vm = vm_for(WIRE_KEL);
    for (cmd, fields, code) in [
        (128i64, 2usize, -211i64),
        (129, 3, -212),
        (130, 4, -213),
        (131, 3, -214),
        (132, 2, -215),
        (133, 2, -216),
    ] {
        let over = (FIN_CAPACITY / fields) + 1;
        let (got, _) =
            run_cmd_fields(&mut vm, cmd, 0, &[], &[], [64, over as i64, 0, 0, 0], 0).expect("run");
        assert_eq!(got, code, "command {cmd}: wrong or missing rejection code");
    }
}

// --- WIRING SLICE 9: DEBUG_POOL, the twentieth region kind -------------------
//
// `DEBUG_POOL` was the last kind with no emitter coverage, and the plan document
// recorded it as needing "a hand-built case or a compile with `emit_debug` on".
// The second turns out to be reachable directly: `compile_with_options` is
// public and `CompileOptions { emit_debug: true }` produces real strippable
// debug metadata, so this is driven by real compiler output like every other
// populated kind rather than by a fixture I invented.
//
// NO NEW KELEUSMA CODE. `DEBUG_POOL` is a byte pool, so slice 4's
// `emit_pool_bytes` and `emit_pool_pad` already emit it; what was missing was a
// case, not an emitter. That is worth stating because it is the second time a
// "missing coverage" item turned out to need only a driver.
//
// This is the twentieth region kind, and with it **every region kind the format
// defines is emitted from real compiler output.**

/// A stage's auxiliary body WITH strippable debug metadata.
///
/// `corpus_aux_of` sets `debug_pool_bytes: None`, matching `wire_corpus.rs` and
/// the default compile. This takes the path the reference takes at
/// `wire_format.rs:1616` instead.
fn corpus_aux_with_debug(
    module: &keleusma::bytecode::Module,
) -> keleusma::wire_format::WireAuxBody {
    let mut aux = corpus_aux_of(module);
    for (wc, c) in aux.chunks.iter_mut().zip(&module.chunks) {
        wc.debug_pool_bytes = c
            .debug_pool
            .as_ref()
            .map(keleusma::debug_meta::DebugPool::encode);
    }
    aux
}

/// A stage's `DEBUG_POOL`: the logical bytes and the stored region with its pad.
fn real_debug_pool_case(src: &str) -> (Vec<u8>, Vec<u8>, usize) {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let (module, _) = keleusma::compiler::compile_with_options(
        &program,
        &keleusma::target::Target::default(),
        &keleusma::compiler::CompileOptions { emit_debug: true },
    )
    .expect("compile with debug");

    // The logical input, concatenated in chunk order exactly as the encoder
    // receives it from `add_chunk`.
    let logical: Vec<u8> = module
        .chunks
        .iter()
        .filter_map(|c| {
            c.debug_pool
                .as_ref()
                .map(keleusma::debug_meta::DebugPool::encode)
        })
        .flatten()
        .collect();

    let bytes =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_with_debug(&module)).expect("encode");
    let view = keleusma_wire::WireView::parse(&bytes).expect("reference artifact parses");
    let region = view
        .find_region(keleusma::wire_schema::kind::DEBUG_POOL)
        .expect("a debug compile must emit DEBUG_POOL");
    let stored = view.region_bytes(&region).expect("payload").to_vec();
    (logical, stored, view.region_count() as usize)
}

#[test]
fn the_emitted_debug_pool_matches_the_reference_on_real_compiler_output() {
    let mut vm = vm_for(WIRE_KEL);
    let mut pads = Vec::new();
    for (name, src) in CORPUS_STAGES {
        let (logical, stored, regions) = real_debug_pool_case(src);
        assert!(
            !logical.is_empty(),
            "{name}: a debug compile produced no metadata"
        );
        // The twentieth kind. Every other compile in this suite emits 19.
        assert_eq!(
            regions, 20,
            "{name}: a debug compile must emit all twenty region kinds"
        );
        pads.push(stored.len() - logical.len());
        let got = emit_pool_batched(&mut vm, &logical, 64, BIN_CAPACITY);
        assert_eq!(got, stored, "{name}: emitted DEBUG_POOL differs");
    }
    // The pad path is shared with PARAM_TYPES, but a corpus that happened to be
    // all word-aligned here would exercise none of it, so what was reached is
    // asserted rather than assumed.
    pads.sort_unstable();
    pads.dedup();
    assert!(
        pads.iter().any(|p| *p != 0) || pads.len() > 1,
        "every DEBUG_POOL was already word-aligned; the pad path went unexercised: {pads:?}"
    );
}

/// Must-fire control, and a guard that the debug compile really differs.
#[test]
fn the_debug_pool_differential_reports_a_perturbed_byte() {
    let mut vm = vm_for(WIRE_KEL);
    let (logical, stored, _) = real_debug_pool_case(CORPUS_STAGES[9].1);
    assert_eq!(
        emit_pool_batched(&mut vm, &logical, 64, BIN_CAPACITY),
        stored,
        "control: the clean case must agree"
    );
    for i in [0usize, logical.len() / 2, logical.len() - 1] {
        let mut perturbed = logical.clone();
        perturbed[i] ^= 1;
        assert_ne!(
            emit_pool_batched(&mut vm, &perturbed, 64, BIN_CAPACITY),
            stored,
            "control did not fire: debug byte {i} is not observable"
        );
    }
}

/// The default compile must still emit NO debug pool.
///
/// This is the must-not-fire half of the pair, and it also pins the reason the
/// gap existed: `emit_debug` defaults false, so the ten-stage corpus everything
/// else in this suite uses emits nineteen regions, not twenty.
#[test]
fn the_default_compile_still_emits_no_debug_pool() {
    let (specs, _) = real_stage_regions(CORPUS_STAGES[9].1);
    assert_eq!(
        specs.len(),
        19,
        "a default compile must emit nineteen regions"
    );
    assert!(
        !specs
            .iter()
            .any(|(k, _, _, _)| *k == keleusma::wire_schema::kind::DEBUG_POOL),
        "a default compile emitted DEBUG_POOL; the twentieth kind is no longer debug-only"
    );
}

// --- SLICE 10: the driver's region table -------------------------------------
//
// The first piece of the DRIVER rather than of the emitters. Every slice so far
// took its region lengths from the host; this computes them, which moves the
// stride of all seventeen record kinds onto the Keleusma side.
//
// The oracle is a real module's own header area: the reference's first
// `48 + 48n` bytes encode every region's offset and length, so if Keleusma
// derives the same lengths from record COUNTS alone, the two agree byte for
// byte. A wrong stride for any kind shifts every later offset and is caught.

const CMD_BUILD_REGION_TABLE: i64 = 134;

/// Region kinds paired with what the driver would know: a record COUNT for a
/// record table, a BYTE LENGTH for a pool.
fn region_counts_for(bytes: &[u8]) -> Vec<RegionSpec> {
    use keleusma::wire_schema::kind;
    let view = keleusma_wire::WireView::parse(bytes).expect("artifact parses");
    let mut out: Vec<RegionSpec> = Vec::new();
    for i in 0..view.region_count() {
        let r = view.region_at(i).expect("region");
        let len = (r.word_length as usize) * 8;
        let is_pool = matches!(
            r.kind,
            kind::STRING_POOL | kind::PARAM_TYPES | kind::DEBUG_POOL
        );
        // A pool passes bytes; a record table passes its record count, which is
        // what a driver holds before it knows any byte length.
        let stride = match r.kind {
            kind::NAMES
            | kind::STRUCT_AUX
            | kind::SHAPES
            | kind::DATA_SLOTS
            | kind::SHARED_LAYOUT
            | kind::PRIVATE_COMPOSITE
            | kind::DATA_INIT
            | kind::NATIVES
            | kind::NATIVE_RETURNS => 8,
            kind::CHUNKS => 48,
            kind::HEADER => 32,
            _ => 16,
        };
        let passed = if is_pool { len } else { len / stride };
        out.push((r.kind, r.flags, passed, r.covers));
    }
    out
}

#[test]
fn the_driver_derives_every_region_length_from_record_counts() {
    let mut vm = vm_for(WIRE_KEL);
    for (name, src) in CORPUS_STAGES {
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module))
            .expect("encode aux body");
        let specs = region_counts_for(&want);
        let hl = header_len(specs.len());
        let (written, got) = run_cmd_full(
            &mut vm,
            CMD_BUILD_REGION_TABLE,
            specs.len() as i64,
            &[],
            &specs,
            0,
            hl,
        )
        .expect("run");
        assert_eq!(written, hl as i64, "{name}: wrong header length reported");
        assert_eq!(
            got,
            want[..hl],
            "{name}: a region length derived from counts differs"
        );
    }
}

/// Must-fire: a wrong stride for any kind must move the header.
///
/// Byte identity above would also hold if the emitter ignored the counts and
/// used the lengths directly, so perturbing a COUNT — not a length — is what
/// makes this test about the stride table.
#[test]
fn a_perturbed_record_count_moves_the_derived_header() {
    let mut vm = vm_for(WIRE_KEL);
    let module = compile(&parse(&tokenize(CORPUS_STAGES[9].1).expect("lex")).expect("parse"))
        .expect("compile");
    let want =
        keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode aux body");
    let base = region_counts_for(&want);
    let hl = header_len(base.len());

    let emit = |vm: &mut Vm<'static, 'static>, specs: &Regions| {
        run_cmd_full(
            vm,
            CMD_BUILD_REGION_TABLE,
            specs.len() as i64,
            &[],
            specs,
            0,
            hl,
        )
        .expect("run")
        .1
    };
    assert_eq!(
        emit(&mut vm, &base),
        want[..hl],
        "control: clean must agree"
    );

    // Every non-empty region in turn: one more record must change the header.
    let mut fired = 0;
    for i in 0..base.len() {
        if base[i].2 == 0 {
            continue;
        }
        let mut specs = base.clone();
        specs[i].2 += 1;
        assert_ne!(
            emit(&mut vm, &specs),
            want[..hl],
            "control did not fire: region {i} kind {:#06x} count is not observable",
            base[i].0
        );
        fired += 1;
    }
    assert!(
        fired >= 5,
        "only {fired} regions were non-empty; too few to test"
    );
}

/// An unknown region kind is rejected, not silently sized zero.
#[test]
fn an_unknown_region_kind_is_reported_rather_than_sized_zero() {
    let mut vm = vm_for(WIRE_KEL);
    let specs: Vec<RegionSpec> = vec![(0x00FF, 0, 1, 0)];
    let (got, _) =
        run_cmd_full(&mut vm, CMD_BUILD_REGION_TABLE, 1, &[], &specs, 0, 0).expect("run");
    assert_eq!(
        got, -220,
        "an unknown region kind was not reported with its own code"
    );
}

// --- SLICE 11: a COMPLETE artifact, built by Keleusma -------------------------
//
// The driver's first end-to-end result. Every earlier slice emitted one region
// in isolation; this builds a whole auxiliary body — directory and all fifteen
// regions — and compares it byte for byte against `encode_aux_body`.
//
// WHY IT IS STAGED. Shared data is re-seeded on every VM call, so nothing
// survives between them. The artifact is therefore carried forward AS BYTES:
// each call re-seeds what exists so far, fills in one more region at the place
// the directory says it goes, and hands the result back. That is the same
// staged shape the residency measurement forced for `lexer`, exercised here at
// a size where the whole artifact fits in the buffer — 912 bytes, 1.4% of it.

/// Everything one driver call needs. A struct rather than nine arguments,
/// because clippy is right about that and it reads better besides.
struct Call<'a> {
    cmd: i64,
    nregions: i64,
    seed: &'a [u8],
    regions: &'a Regions,
    fields: &'a [i64],
    /// The interner's (length, mode) pairs, seeded into `nin`. Separate from
    /// `fields` because slice 13b gives the interner its own channel.
    names: &'a [i64],
    pool: &'a [u8],
    args: [i64; 5],
    read_len: usize,
}

fn run_call(vm: &mut Vm<'static, 'static>, c: &Call<'_>) -> Result<(i64, Vec<u8>), VmError> {
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(c.seed.len() as i64))?;
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(c.nregions))?;
    for (i, a) in c.args.iter().enumerate() {
        vm.set_shared(&mut shared, WARG_SLOT + i, Value::Int(*a))?;
    }
    for (i, b) in c.seed.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(*b))?;
    }
    for (i, (kind, flags, len, covers)) in c.regions.iter().enumerate() {
        vm.set_shared(&mut shared, RKIND_SLOT + i, Value::Int(i64::from(*kind)))?;
        vm.set_shared(&mut shared, RFLAGS_SLOT + i, Value::Int(i64::from(*flags)))?;
        vm.set_shared(&mut shared, RLEN_SLOT + i, Value::Int(*len as i64))?;
        vm.set_shared(
            &mut shared,
            RCOVERS_SLOT + i,
            Value::Int(i64::from(*covers)),
        )?;
    }
    for (i, v) in c.fields.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + i, Value::Int(*v))?;
    }
    assert!(
        c.names.len() <= NIN_CAPACITY,
        "interner input is {} words, capacity is {NIN_CAPACITY}",
        c.names.len()
    );
    for (i, v) in c.names.iter().enumerate() {
        vm.set_shared(&mut shared, NIN_SLOT + i, Value::Int(*v))?;
    }
    for (i, b) in c.pool.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(*b))?;
    }
    let ret = match vm.call_with_shared(&mut shared, &[Value::Int(c.cmd)])? {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("unexpected VM state: {other:?}"),
    };
    let n = c.read_len.min(CAPACITY);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        match vm.get_shared(&shared, 1 + i)? {
            Value::Byte(b) => out.push(b),
            other => panic!("slot {i} is not a Byte: {other:?}"),
        }
    }
    Ok((ret, out))
}

/// Decode one region of a reference artifact into the field rows the matching
/// Keleusma emitter consumes, in declaration order.
fn rows_for_kind(view: &keleusma_wire::WireView<'_>, kind: u16) -> Vec<Vec<i64>> {
    use keleusma::wire_schema as w;
    let Some(region) = view.find_region(kind) else {
        return Vec::new();
    };
    let stride = match kind {
        w::kind::NAMES | w::kind::SHAPES => 8,
        w::kind::CHUNKS => 48,
        w::kind::HEADER => 32,
        _ => 16,
    };
    let Ok(t) = view.records(&region, stride) else {
        return Vec::new();
    };
    // The kinds slice 7 already decodes, handled whole rather than per index.
    // The first version looped over them too and then discarded the result,
    // which clippy caught as dead code — correctly, since the loop body could
    // never contribute.
    if matches!(
        kind,
        w::kind::SHAPES
            | w::kind::SIGNATURES
            | w::kind::CONSTS
            | w::kind::ENUM_VARIANTS
            | w::kind::ENUM_LAYOUTS
            | w::kind::DATA_INIT
    ) {
        return decode_slice7(kind, &t);
    }
    let mut out = Vec::with_capacity(t.len());
    for i in 0..t.len() {
        out.push(match kind {
            k if k == w::kind::NAMES => {
                let r: w::NameRef = t.get_as(i).expect("rec");
                vec![i64::from(r.offset), i64::from(r.length)]
            }
            k if k == w::kind::CHUNKS => {
                let c: w::ChunkRecord = t.get_as(i).expect("rec");
                vec![
                    i64::from(c.name),
                    i64::from(c.consts_first),
                    i64::from(c.consts_count),
                    i64::from(c.templates_first),
                    i64::from(c.templates_count),
                    i64::from(c.param_types_first),
                    i64::from(c.param_types_count),
                    i64::from(c.debug_first),
                    i64::from(c.debug_len),
                    i64::from(c.op_byte_offset),
                    i64::from(c.op_record_count),
                    i64::from(c.local_count),
                    i64::from(c.param_count),
                    i64::from(c.block_type),
                ]
            }
            k if k == w::kind::HEADER => {
                let h: w::HeaderRecord = t.get_as(i).expect("rec");
                vec![
                    i64::from(h.entry_point),
                    i64::from(h.word_bits_log2),
                    i64::from(h.addr_bits_log2),
                    i64::from(h.float_bits_log2),
                    i64::from(h.flags),
                    i64::from(h.wcet_cycles),
                    i64::from(h.wcmu_bytes),
                    i64::from(h.shared_data_bytes),
                    i64::from(h.private_data_bytes),
                    i64::from(h.schema_hash),
                    i64::from(h.reserved),
                ]
            }
            other => panic!("rows_for_kind has no decoder for {other:#06x}"),
        });
    }
    out
}

const CMD_EMIT_IN_REGION: i64 = 135;

#[test]
fn keleusma_builds_a_complete_minimal_artifact_byte_for_byte() {
    use keleusma::wire_schema::kind;
    let mut vm = vm_for(WIRE_KEL);

    let module =
        compile(&parse(&tokenize("fn main() -> Word { 42 }").expect("lex")).expect("parse"))
            .expect("compile");
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
    let view = keleusma_wire::WireView::parse(&want).expect("reference parses");

    let specs = region_counts_for(&want);
    let total = want.len();
    assert!(
        total < CAPACITY,
        "this slice needs the whole artifact to fit; it is {total} bytes"
    );

    // Step 1: the directory, with lengths derived from counts by Keleusma.
    let (_, mut art) = run_call(
        &mut vm,
        &Call {
            cmd: CMD_BUILD_REGION_TABLE,
            nregions: specs.len() as i64,
            seed: &[],
            regions: &specs,
            fields: &[],
            names: &[],
            pool: &[],
            args: [0, 0, 0, 0, 0],
            read_len: total,
        },
    )
    .expect("run");
    assert_eq!(
        art[..header_len(specs.len())],
        want[..header_len(specs.len())],
        "the directory differs before any payload was written"
    );

    // Steps 2..N: one region per call, carrying the artifact forward as bytes.
    let mut filled = 0;
    for (k, _, _, _) in &specs {
        let region = view.find_region(*k).expect("region");
        let stored = view.region_bytes(&region).expect("payload");
        if stored.is_empty() {
            continue;
        }
        let is_pool = matches!(*k, kind::STRING_POOL | kind::PARAM_TYPES | kind::DEBUG_POOL);
        let rows = if is_pool {
            Vec::new()
        } else {
            rows_for_kind(&view, *k)
        };
        let flat: Vec<i64> = rows.iter().flatten().copied().collect();
        let n = if is_pool { stored.len() } else { rows.len() };

        let (ret, next) = run_call(
            &mut vm,
            &Call {
                cmd: CMD_EMIT_IN_REGION,
                nregions: specs.len() as i64,
                seed: &art,
                regions: &specs,
                fields: &flat,
                names: &[],
                pool: stored,
                args: [i64::from(*k), n as i64, 0, 0, 0],
                read_len: total,
            },
        )
        .expect("run");
        assert!(ret >= 0, "region {k:#06x} was refused with code {ret}");
        art = next;
        filled += 1;
    }

    assert!(filled >= 7, "only {filled} regions carried a payload");
    assert_eq!(
        art, want,
        "the artifact Keleusma built differs from the reference"
    );
}

/// Must-fire: the whole-artifact comparison must be able to report a difference.
#[test]
fn the_complete_artifact_comparison_reports_a_perturbation() {
    let mut vm = vm_for(WIRE_KEL);
    let module =
        compile(&parse(&tokenize("fn main() -> Word { 42 }").expect("lex")).expect("parse"))
            .expect("compile");
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
    let mut specs = region_counts_for(&want);

    // One extra record in the first non-empty region shifts every later offset.
    let idx = specs
        .iter()
        .position(|(_, _, n, _)| *n > 0)
        .expect("a region");
    specs[idx].2 += 1;
    let (_, got) = run_call(
        &mut vm,
        &Call {
            cmd: CMD_BUILD_REGION_TABLE,
            nregions: specs.len() as i64,
            seed: &[],
            regions: &specs,
            fields: &[],
            names: &[],
            pool: &[],
            args: [0, 0, 0, 0, 0],
            read_len: want.len(),
        },
    )
    .expect("run");
    assert_ne!(
        got[..header_len(specs.len())],
        want[..header_len(specs.len())],
        "control did not fire: a perturbed count left the directory unchanged"
    );
}

// --- SLICE 12: THE INTERNER, THE DRIVER'S FIRST COMPUTED VALUE ---------------
//
// Every slice up to here handed Keleusma values that had been DECODED out of the
// reference artifact and checked that it re-emitted them. This is the first that
// makes Keleusma compute one. The host supplies the sequence of names the
// encoder interns, with the mode each call site uses; Keleusma produces the
// `STRING_POOL` bytes, the `NAMES` records, and the input-to-index map.
//
// WHY THE CASES ARE CONSTRUCTED RATHER THAN DRAWN FROM THE CORPUS. The corpus
// cannot reach the behaviour under test. Four of the five stages measured carry
// no duplicate names at all, and only `parse` has any — twenty out of 58,053 —
// whose artifact is roughly 16 MB and cannot be driven through a 65,536-byte
// buffer. A deduping-only interner would therefore agree with the corpus on four
// stages out of five and be wrong on the one that matters. These sources are
// about a kilobyte each and reach the duplicate path directly.
//
// The oracle stays REAL: every expectation below comes from `encode_aux_body`
// on a genuinely compiled module, not from a model of the interner.
//
// WHAT IS STILL MODELLED, AND IT IS WORTH BEING PRECISE ABOUT. The interner's
// input is a sequence of (name, mode) PAIRS, and that sequence is a property of
// the encoder's call order, not something recoverable from its output. So
// `interner_input` below is a Rust model of that call order. It is not the
// oracle — the oracle is byte identity of the resulting regions — but if the
// model of the order were wrong, these tests would fail rather than pass
// vacuously. Generating that sequence from the AST is the self-hosted driver's
// job and is NOT done here.

const CMD_INTERN_NAMES: i64 = 136;
const CMD_INTERN_POOL_LEN: i64 = 137;
const CMD_INTERN_EMIT_NAMES: i64 = 138;
const CMD_INTERN_EMIT_POOL: i64 = 139;
const CMD_INTERN_INDEX_OF: i64 = 140;

/// `Names::intern` — reuse an entry with the same bytes.
const MODE_INTERN: i64 = 0;
/// `Names::intern_fresh` — append unconditionally, keeping a run contiguous.
const MODE_FRESH: i64 = 1;

/// Sources chosen so that between them they reach every branch of the interner.
///
/// Each is annotated with what it buys, because "a list of test inputs" with no
/// stated purpose is how a suite acquires cases nobody can later remove.
const INTERNER_CASES: &[(&str, &str)] = &[
    // No duplicates at all: the control. If this failed, nothing below would
    // mean anything.
    ("minimal", "fn main() -> Word { 42 }"),
    ("one-enum", "enum A { X, Y }\nfn main() -> Word { 42 }"),
    // The smallest duplicate: a variant whose name equals its own type name.
    // Both are upper_ident, so the collision is legal.
    (
        "variant-equals-own-type",
        "enum A { A, B }\nfn main() -> Word { 42 }",
    ),
    // Two enums sharing a variant name. `intern_fresh` must append both.
    (
        "two-enums-shared-variant",
        "enum A { X, Y }\nenum B { X, Z }\nfn main() -> Word { 42 }",
    ),
    // Three copies, which distinguishes "dedups once" from "dedups".
    (
        "three-way-duplicate",
        "enum A { X, P }\nenum B { X, Q }\nenum C { X, R }\nfn main() -> Word { 42 }",
    ),
    // THE SHARING DIRECTION. `intern_fresh` records its entry so a later
    // `intern` can reuse it: fresh("B") for the variant runs before intern("B")
    // for the second enum's type name. A port whose fresh mode skips the index
    // emits seven names where the reference emits six.
    (
        "fresh-then-intern-shares",
        "enum A { B, X }\nenum B { Y, Z }\nfn main() -> Word { 42 }",
    ),
];

/// A model of the encoder's interning call order, for sources limited to
/// function definitions and enum declarations.
///
/// Mirrors `SchemaBuilder::add_chunk` (`names.intern`) and `add_enum_layouts`
/// (`names.intern` for the type name, `names.intern_fresh` per variant), in that
/// order. Restricted deliberately: a source with data slots, natives, struct
/// templates or composite constants has more contributors, and silently
/// producing a short sequence for one would make a test pass for the wrong
/// reason. `assert_no_other_contributors` refuses those inputs instead.
fn interner_input(module: &keleusma::bytecode::Module) -> Vec<(String, i64)> {
    let mut seq = Vec::new();
    for c in &module.chunks {
        seq.push((c.name.clone(), MODE_INTERN));
    }
    for l in &module.enum_layouts {
        seq.push((l.type_name.clone(), MODE_INTERN));
        for v in &l.variants {
            seq.push((v.name.clone(), MODE_FRESH));
        }
    }
    seq
}

/// Refuses a module whose names come from a contributor `interner_input` does
/// not model. Without this the model could silently under-generate.
fn assert_no_other_contributors(label: &str, module: &keleusma::bytecode::Module) {
    assert!(
        module.native_names.is_empty(),
        "{label}: natives intern names and the model does not cover them"
    );
    assert!(
        module.data_layout.is_none(),
        "{label}: a data layout interns per-slot names and the model does not cover them"
    );
    for c in &module.chunks {
        assert!(
            c.struct_templates.is_empty(),
            "{label}: struct templates intern names and the model does not cover them"
        );
    }
}

/// The interner input, flattened into the two shared-data channels: `fin` takes
/// (length, mode) pairs and `bin` takes the names concatenated in order.
fn interner_channels(seq: &[(String, i64)]) -> (Vec<i64>, Vec<u8>) {
    let mut fields = Vec::with_capacity(seq.len() * 2);
    let mut pool = Vec::new();
    for (name, mode) in seq {
        fields.push(name.len() as i64);
        fields.push(*mode);
        pool.extend_from_slice(name.as_bytes());
    }
    (fields, pool)
}

/// The reference's `NAMES` entries as byte strings, and the UNPADDED pool
/// length, which is what the interner reports.
fn reference_names(bytes: &[u8]) -> (Vec<Vec<u8>>, usize) {
    use keleusma::wire_schema::kind;
    let view = keleusma_wire::WireView::parse(bytes).expect("artifact parses");
    let pool = view
        .find_region(kind::STRING_POOL)
        .and_then(|r| view.region_bytes(&r).ok())
        .unwrap_or(&[])
        .to_vec();
    let mut names = Vec::new();
    let mut used = 0usize;
    if let Some(r) = view.find_region(kind::NAMES) {
        let t = view.records(&r, 8).expect("names table");
        for i in 0..t.len() {
            let n: keleusma::wire_schema::NameRef = t.get_as(i).expect("rec");
            let (o, l) = (n.offset as usize, n.length as usize);
            names.push(pool[o..o + l].to_vec());
            used = used.max(o + l);
        }
    }
    (names, used)
}

/// One interner call: seeds the (name, mode) channels and nothing else.
fn run_intern(
    vm: &mut Vm<'static, 'static>,
    cmd: i64,
    names: &[i64],
    pool: &[u8],
    args: [i64; 5],
) -> i64 {
    run_call(
        vm,
        &Call {
            cmd,
            nregions: 0,
            seed: &[],
            regions: &[],
            fields: &[],
            names,
            pool,
            args,
            read_len: 0,
        },
    )
    .expect("run")
    .0
}

/// One flattener call. Its input is the PREORDER, which rides `fin`, not the
/// interner's `nin` — a separate helper rather than a flag, because the two
/// commands read different arrays and a boolean at the call site would hide
/// which one.
fn run_flatten(vm: &mut Vm<'static, 'static>, cmd: i64, fields: &[i64], args: [i64; 5]) -> i64 {
    run_call(
        vm,
        &Call {
            cmd,
            nregions: 0,
            seed: &[],
            regions: &[],
            fields,
            names: &[],
            pool: &[],
            args,
            read_len: 0,
        },
    )
    .expect("run")
    .0
}

#[test]
fn keleusma_computes_the_name_table_the_reference_interned() {
    let mut vm = vm_for(WIRE_KEL);
    let mut saw_a_duplicate = false;

    for (label, src) in INTERNER_CASES {
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        assert_no_other_contributors(label, &module);
        let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
        let (ref_names, ref_pool_len) = reference_names(&want);

        let seq = interner_input(&module);
        let (fields, pool) = interner_channels(&seq);
        let n = seq.len() as i64;

        // A duplicate is what separates the two modes. Recording it per case
        // means the suite reports if the property it relies on ever stops
        // holding, rather than quietly testing nothing.
        let mut distinct = ref_names.clone();
        distinct.sort();
        distinct.dedup();
        if distinct.len() < ref_names.len() {
            saw_a_duplicate = true;
        }

        let cnt = run_intern(&mut vm, CMD_INTERN_NAMES, &fields, &pool, [n, 0, 0, 0, 0]);
        assert_eq!(
            cnt,
            ref_names.len() as i64,
            "{label}: Keleusma emitted {cnt} names, the reference {}",
            ref_names.len()
        );

        let plen = run_intern(
            &mut vm,
            CMD_INTERN_POOL_LEN,
            &fields,
            &pool,
            [n, 0, 0, 0, 0],
        );
        assert_eq!(
            plen, ref_pool_len as i64,
            "{label}: Keleusma's pool is {plen} bytes, the reference's {ref_pool_len}"
        );
    }

    assert!(
        saw_a_duplicate,
        "no case produced a duplicate name, so neither mode was distinguished"
    );
}

#[test]
fn the_computed_name_regions_are_byte_identical_in_a_complete_artifact() {
    use keleusma::wire_schema::kind;
    let mut vm = vm_for(WIRE_KEL);

    for (label, src) in INTERNER_CASES {
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        assert_no_other_contributors(label, &module);
        let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
        let view = keleusma_wire::WireView::parse(&want).expect("reference parses");
        let total = want.len();
        assert!(total < CAPACITY, "{label}: artifact is {total} bytes");

        let seq = interner_input(&module);
        let (nfields, npool) = interner_channels(&seq);
        let n = seq.len() as i64;

        // The two regions are sized from KELEUSMA's own figures, not the
        // reference's, so a wrong count moves every later region and the byte
        // comparison at the end fails loudly.
        let cnt = run_intern(&mut vm, CMD_INTERN_NAMES, &nfields, &npool, [n, 0, 0, 0, 0]);
        let plen = run_intern(
            &mut vm,
            CMD_INTERN_POOL_LEN,
            &nfields,
            &npool,
            [n, 0, 0, 0, 0],
        );
        assert!(cnt >= 0 && plen >= 0, "{label}: interner refused");

        let mut specs = region_counts_for(&want);
        for s in &mut specs {
            if s.0 == kind::NAMES {
                s.2 = cnt as usize;
            }
            if s.0 == kind::STRING_POOL {
                s.2 = plen as usize;
            }
        }

        let (_, mut art) = run_call(
            &mut vm,
            &Call {
                cmd: CMD_BUILD_REGION_TABLE,
                nregions: specs.len() as i64,
                seed: &[],
                regions: &specs,
                fields: &[],
                names: &[],
                pool: &[],
                args: [0, 0, 0, 0, 0],
                read_len: total,
            },
        )
        .expect("run");

        for (k, _, _, _) in &specs {
            let region = view.find_region(*k).expect("region");
            let stored = view.region_bytes(&region).expect("payload");
            if stored.is_empty() {
                continue;
            }
            // The two computed regions take the interner's input; every other
            // region is still re-emitted from decoded values, which is the part
            // later slices replace.
            // The interner's (length, mode) pairs ride `names`; every other
            // emitter's field rows ride `fields`. Keeping both in one tuple
            // rather than reusing a single slot, because routing the interner
            // input down the wrong channel is exactly the failure this split
            // produced on its first run — silently empty NAMES and STRING_POOL
            // regions rather than an error.
            let (cmd, fields, names, pl, args) = if *k == kind::NAMES {
                (
                    CMD_INTERN_EMIT_NAMES,
                    Vec::new(),
                    nfields.clone(),
                    npool.clone(),
                    [n, 0, 0, 0, 0],
                )
            } else if *k == kind::STRING_POOL {
                (
                    CMD_INTERN_EMIT_POOL,
                    Vec::new(),
                    nfields.clone(),
                    npool.clone(),
                    [n, 0, 0, 0, 0],
                )
            } else {
                let is_pool = matches!(*k, kind::PARAM_TYPES | kind::DEBUG_POOL);
                let rows = if is_pool {
                    Vec::new()
                } else {
                    rows_for_kind(&view, *k)
                };
                let flat: Vec<i64> = rows.iter().flatten().copied().collect();
                let cnt = if is_pool { stored.len() } else { rows.len() };
                (
                    CMD_EMIT_IN_REGION,
                    flat,
                    Vec::new(),
                    stored.to_vec(),
                    [i64::from(*k), cnt as i64, 0, 0, 0],
                )
            };

            let (ret, next) = run_call(
                &mut vm,
                &Call {
                    cmd,
                    nregions: specs.len() as i64,
                    seed: &art,
                    regions: &specs,
                    fields: &fields,
                    names: &names,
                    pool: &pl,
                    args,
                    read_len: total,
                },
            )
            .expect("run");
            assert!(ret >= 0, "{label}: region {k:#06x} refused with code {ret}");
            art = next;
        }

        assert_eq!(art, want, "{label}: the artifact Keleusma built differs");
    }
}

/// The LAST-MATCH rule, which is invisible in `NAMES` and `STRING_POOL`.
///
/// `intern_fresh` inserts into the reference's bytes-to-index map, overwriting,
/// so a later `intern` of duplicated bytes yields the SECOND index. First-match
/// and last-match produce identical name and pool regions, so this is checked
/// through the input-to-index map — the only place the difference shows — and
/// against the reference's own `ENUM_LAYOUTS.type_name`, not against a belief.
#[test]
fn a_later_intern_resolves_to_the_last_matching_index() {
    use keleusma::wire_schema::kind;
    let src = "enum A { X, P }\nenum B { X, Q }\nenum X { R }\nfn main() -> Word { 42 }";
    let mut vm = vm_for(WIRE_KEL);

    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
    let view = keleusma_wire::WireView::parse(&want).expect("parses");
    let (ref_names, _) = reference_names(&want);

    // The reference's own answer: which index does `enum X`'s type name cite?
    let lt = view
        .records(&view.find_region(kind::ENUM_LAYOUTS).expect("layouts"), 16)
        .expect("table");
    let last: keleusma::wire_schema::EnumLayoutRecord =
        lt.get_as(lt.len() - 1).expect("last layout");
    let cited = last.type_name as usize;

    // The property must be non-vacuous: those bytes must occur more than once,
    // otherwise first-match and last-match agree and this proves nothing.
    let occurrences: Vec<usize> = ref_names
        .iter()
        .enumerate()
        .filter(|(_, s)| *s == &ref_names[cited])
        .map(|(j, _)| j)
        .collect();
    assert!(
        occurrences.len() > 1,
        "vacuous: {:?} occurs once, so the two rules agree",
        String::from_utf8_lossy(&ref_names[cited])
    );
    assert_eq!(
        cited,
        *occurrences.last().expect("occurrence"),
        "the reference did not choose the last match; this test's premise is wrong"
    );
    assert_ne!(
        cited, occurrences[0],
        "vacuous: the last match IS the first match"
    );

    // Keleusma's answer for the same input position.
    let seq = interner_input(&module);
    let (fields, pool) = interner_channels(&seq);
    let n = seq.len() as i64;
    let j = seq
        .iter()
        .rposition(|(name, mode)| *mode == MODE_INTERN && name.as_bytes() == ref_names[cited])
        .expect("the late intern") as i64;

    let got = run_intern(
        &mut vm,
        CMD_INTERN_INDEX_OF,
        &fields,
        &pool,
        [n, j, 0, 0, 0],
    );
    assert_eq!(
        got, cited as i64,
        "Keleusma resolved input {j} to index {got}; the reference cites {cited} \
         (those bytes occur at {occurrences:?}) — a first-match scan would give {}",
        occurrences[0]
    );
}

/// Must-fire: a deduping-only interner disagrees with the reference.
///
/// Forcing every mode to `intern` is exactly the port the plan document warns
/// about. On a source with duplicate names it must produce fewer records than
/// the reference — if it did not, the two modes would be indistinguishable and
/// every test above would be measuring nothing.
#[test]
fn a_dedup_only_interner_produces_the_wrong_count() {
    let mut vm = vm_for(WIRE_KEL);
    let src = "enum A { X, Y }\nenum B { X, Z }\nfn main() -> Word { 42 }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
    let (ref_names, _) = reference_names(&want);

    let seq = interner_input(&module);
    let (mut fields, pool) = interner_channels(&seq);
    for i in 0..seq.len() {
        fields[i * 2 + 1] = MODE_INTERN;
    }
    let cnt = run_intern(
        &mut vm,
        CMD_INTERN_NAMES,
        &fields,
        &pool,
        [seq.len() as i64, 0, 0, 0, 0],
    );
    assert!(
        cnt < ref_names.len() as i64,
        "control did not fire: dedup-only emitted {cnt}, the reference {}",
        ref_names.len()
    );
}

/// Must-fire: an append-only interner disagrees too, in the other direction.
///
/// The complementary error — treating every call as `intern_fresh` — must
/// over-produce on a source where a later `intern` should share.
#[test]
fn an_append_only_interner_produces_the_wrong_count() {
    let mut vm = vm_for(WIRE_KEL);
    let src = "enum A { B, X }\nenum B { Y, Z }\nfn main() -> Word { 42 }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
    let (ref_names, _) = reference_names(&want);

    let seq = interner_input(&module);
    let (mut fields, pool) = interner_channels(&seq);
    for i in 0..seq.len() {
        fields[i * 2 + 1] = MODE_FRESH;
    }
    let cnt = run_intern(
        &mut vm,
        CMD_INTERN_NAMES,
        &fields,
        &pool,
        [seq.len() as i64, 0, 0, 0, 0],
    );
    assert!(
        cnt > ref_names.len() as i64,
        "control did not fire: append-only emitted {cnt}, the reference {}",
        ref_names.len()
    );
}

/// The stated caps are enforced with a code rather than by truncating.
#[test]
fn the_interner_reports_an_input_it_cannot_hold() {
    let mut vm = vm_for(WIRE_KEL);

    // Too many names. 257 exceeds the 256 the map's placement allows.
    let over: Vec<(String, i64)> = (0..257).map(|i| (format!("n{i}"), MODE_INTERN)).collect();
    let (fields, pool) = interner_channels(&over);
    assert_eq!(
        run_intern(&mut vm, CMD_INTERN_NAMES, &fields, &pool, [257, 0, 0, 0, 0]),
        -230,
        "an oversized name count was not reported"
    );

    // A name longer than the comparison loop's static bound.
    let mut vm = vm_for(WIRE_KEL);
    let long = vec![("x".repeat(257), MODE_INTERN)];
    let (fields, pool) = interner_channels(&long);
    assert_eq!(
        run_intern(&mut vm, CMD_INTERN_NAMES, &fields, &pool, [1, 0, 0, 0, 0]),
        -231,
        "an oversized name was not reported"
    );

    // Asking for an index past the input.
    let mut vm = vm_for(WIRE_KEL);
    let one = vec![("main".to_string(), MODE_INTERN)];
    let (fields, pool) = interner_channels(&one);
    assert_eq!(
        run_intern(
            &mut vm,
            CMD_INTERN_INDEX_OF,
            &fields,
            &pool,
            [1, 1, 0, 0, 0]
        ),
        -235,
        "an out-of-range map query was not reported"
    );
}

// --- SLICE 13: THE FLATTENER'S BREADTH-FIRST REORDERING ----------------------
//
// The driver's second computed value. `flatten` turns a constant FOREST into
// the flat `CONSTS` table, and the ordering is the whole of the difficulty: the
// roots occupy `0..nroots` in order and children are numbered BREADTH-FIRST
// after them, which is what makes every range point forward.
//
// THE INPUT IS DEPTH-FIRST, ON PURPOSE. Keleusma receives a preorder walk —
// three words per node: tag, payload, child count. Handing it a breadth-first
// input would make the test vacuous, so `orders_differ_somewhere` below asserts
// that the two orders actually disagree on this corpus rather than assuming it.
//
// THE ORACLE IS REAL, and that is a recent correction. This plan previously
// recorded that the flattener would need hand-built constant trees, because all
// 2,192 constant nodes in the ten stage sources are scalars. That measurement is
// about the corpus; `const data`, referenced from a function, emits genuine
// composite constants to depth 2 in about a kilobyte.

const CMD_FLATTEN_EMIT_CONSTS: i64 = 141;

/// Sources whose chunk constant pools contain composites. Every one is a real
/// compiled module, so the oracle is `encode_aux_body`.
const FLATTEN_CASES: &[(&str, &str)] = &[
    // Control: no composite at all. If this failed, nothing below would mean
    // anything.
    ("scalars-only", "fn main() -> Word { 42 }"),
    (
        "tuple-d1",
        "const data k { t: (Word, Word) = (1, 2) }\nfn main() -> Word { k.t.0 }",
    ),
    (
        "array-d1",
        "const data k { xs: [Word; 3] = [1, 2, 3] }\nfn main() -> Word { k.xs[0] }",
    ),
    // DEPTH 2, which is the case that separates breadth-first from depth-first.
    (
        "array-of-tuple-d2",
        "const data k { a: [(Word, Word); 2] = [(1, 2), (3, 4)] }\n\
         fn take(v: [(Word, Word); 2]) -> Word { v[0].0 }\nfn main() -> Word { take(k.a) }",
    ),
    (
        "nested-tuple-d2",
        "const data k { t: (Word, (Word, Word)) = (1, (2, 3)) }\nfn main() -> Word { k.t.0 }",
    ),
    // A COMPOSITE THAT IS NOT THE LAST CHILD. When every composite sits last,
    // the two walks coincide — which is why `nested-tuple-d2` above does NOT
    // distinguish them, and why the vacuity check below caught that four of the
    // first five cases were proving nothing about the reordering.
    (
        "tuple-composite-first",
        "const data k { t: ((Word, Word), Word) = ((1, 2), 3) }\n         fn main() -> Word { k.t.1 }",
    ),
];

/// Every chunk's constants, concatenated in chunk order — which is exactly what
/// `SchemaBuilder::const_roots` accumulates and hands to `flatten`.
fn const_roots_of(module: &keleusma::bytecode::Module) -> Vec<keleusma::bytecode::ConstValue> {
    let mut roots = Vec::new();
    for c in &module.chunks {
        roots.extend(c.constants.iter().cloned());
    }
    roots
}

/// Serialize one node depth-first: tag, payload, child count, then the children.
///
/// Panics on a tag outside this slice's scope rather than emitting a plausible
/// record. `STATIC_STR`, `STRUCT` and `ENUM` intern names as they walk, which
/// couples the flattener to the interner; that is the next slice.
fn push_preorder(c: &keleusma::bytecode::ConstValue, out: &mut Vec<i64>) {
    use keleusma::bytecode::ConstValue as K;
    let (tag, payload, children): (i64, i64, &[K]) = match c {
        K::Unit => (1, 0, &[]),
        K::Bool(b) => (2, i64::from(*b), &[]),
        K::Int(v) => (3, *v, &[]),
        K::Byte(v) => (4, i64::from(*v), &[]),
        K::Fixed(v) => (5, *v, &[]),
        K::None => (12, 0, &[]),
        K::Tuple(v) => (8, 0, v.as_slice()),
        K::Array(v) => (9, 0, v.as_slice()),
        other => panic!("constant is outside slice 13's scope: {other:?}"),
    };
    out.push(tag);
    out.push(payload);
    out.push(children.len() as i64);
    for ch in children {
        push_preorder(ch, out);
    }
}

fn preorder_of(roots: &[keleusma::bytecode::ConstValue]) -> Vec<i64> {
    let mut out = Vec::new();
    for r in roots {
        push_preorder(r, &mut out);
    }
    out
}

/// The (tag, payload) sequence in each order, for the vacuity check.
///
/// TAGS ALONE ARE TOO COARSE, and the first version of this compared only tags.
/// For `((1, 2), 3)` both walks give 8, 8, 3, 3, 3 while visiting the scalars in
/// different orders — so a tag-only check would have called that case
/// indistinguishable when it is exactly the shape the reordering exists for.
type NodeSeq = Vec<(i64, i64)>;

fn node_orders(roots: &[keleusma::bytecode::ConstValue]) -> (NodeSeq, NodeSeq) {
    use keleusma::bytecode::ConstValue as K;
    fn kids(c: &K) -> &[K] {
        match c {
            K::Tuple(v) | K::Array(v) => v.as_slice(),
            _ => &[],
        }
    }
    fn node(c: &K) -> (i64, i64) {
        match c {
            K::Unit => (1, 0),
            K::Bool(b) => (2, i64::from(*b)),
            K::Int(v) => (3, *v),
            K::Byte(v) => (4, i64::from(*v)),
            K::Fixed(v) => (5, *v),
            K::None => (12, 0),
            K::Tuple(_) => (8, 0),
            K::Array(_) => (9, 0),
            other => panic!("out of scope: {other:?}"),
        }
    }
    fn dfs(c: &K, out: &mut Vec<(i64, i64)>) {
        out.push(node(c));
        for k in kids(c) {
            dfs(k, out);
        }
    }
    let mut depth_first = Vec::new();
    for r in roots {
        dfs(r, &mut depth_first);
    }
    let mut breadth_first = Vec::new();
    let mut queue: Vec<&K> = roots.iter().collect();
    let mut head = 0;
    while head < queue.len() {
        let n = queue[head];
        head += 1;
        breadth_first.push(node(n));
        queue.extend(kids(n).iter());
    }
    (breadth_first, depth_first)
}

fn node_count(roots: &[keleusma::bytecode::ConstValue]) -> usize {
    node_orders(roots).0.len()
}

#[test]
fn keleusma_flattens_a_constant_forest_breadth_first() {
    use keleusma::wire_schema::kind;
    let mut vm = vm_for(WIRE_KEL);
    let mut saw_depth_two = false;

    for (label, src) in FLATTEN_CASES {
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let want = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&module)).expect("encode");
        let view = keleusma_wire::WireView::parse(&want).expect("reference parses");
        let total = want.len();
        assert!(total < CAPACITY, "{label}: artifact is {total} bytes");

        let roots = const_roots_of(&module);
        let fields = preorder_of(&roots);
        let nroots = roots.len() as i64;
        let nnodes = node_count(&roots) as i64;

        // The reference's own CONSTS count must agree, or the input model is
        // wrong and everything below would be comparing the wrong thing.
        let ref_consts = view
            .find_region(kind::CONSTS)
            .and_then(|r| view.records(&r, 16).ok())
            .map_or(0, |t| t.len());
        assert_eq!(
            ref_consts, nnodes as usize,
            "{label}: model counts {nnodes} nodes, the reference emitted {ref_consts}"
        );

        let (bf, df) = node_orders(&roots);
        if bf != df {
            saw_depth_two = true;
        }

        let specs = region_counts_for(&want);
        let (_, mut art) = run_call(
            &mut vm,
            &Call {
                cmd: CMD_BUILD_REGION_TABLE,
                nregions: specs.len() as i64,
                seed: &[],
                regions: &specs,
                fields: &[],
                names: &[],
                pool: &[],
                args: [0, 0, 0, 0, 0],
                read_len: total,
            },
        )
        .expect("run");

        for (k, _, _, _) in &specs {
            let region = view.find_region(*k).expect("region");
            let stored = view.region_bytes(&region).expect("payload");
            if stored.is_empty() {
                continue;
            }
            let (cmd, f, pl, args) = if *k == kind::CONSTS {
                (
                    CMD_FLATTEN_EMIT_CONSTS,
                    fields.clone(),
                    Vec::new(),
                    [nroots, nnodes, 0, 0, 0],
                )
            } else {
                let is_pool =
                    matches!(*k, kind::STRING_POOL | kind::PARAM_TYPES | kind::DEBUG_POOL);
                let rows = if is_pool {
                    Vec::new()
                } else {
                    rows_for_kind(&view, *k)
                };
                let flat: Vec<i64> = rows.iter().flatten().copied().collect();
                let n = if is_pool { stored.len() } else { rows.len() };
                (
                    CMD_EMIT_IN_REGION,
                    flat,
                    stored.to_vec(),
                    [i64::from(*k), n as i64, 0, 0, 0],
                )
            };
            let (ret, next) = run_call(
                &mut vm,
                &Call {
                    cmd,
                    nregions: specs.len() as i64,
                    seed: &art,
                    regions: &specs,
                    fields: &f,
                    names: &[],
                    pool: &pl,
                    args,
                    read_len: total,
                },
            )
            .expect("run");
            assert!(ret >= 0, "{label}: region {k:#06x} refused with code {ret}");
            art = next;
        }

        assert_eq!(art, want, "{label}: the artifact Keleusma built differs");
    }

    assert!(
        saw_depth_two,
        "no case distinguished breadth-first from depth-first, so the reordering was not tested"
    );
}

/// Must-fire, and it is about the CORPUS rather than the code: unless some case
/// orders differently under the two walks, a flattener that emitted its input
/// unchanged would pass the test above.
#[test]
fn the_two_walk_orders_genuinely_disagree_on_this_corpus() {
    let mut disagreements = 0;
    for (label, src) in FLATTEN_CASES {
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let roots = const_roots_of(&module);
        let (bf, df) = node_orders(&roots);
        assert_eq!(
            bf.len(),
            df.len(),
            "{label}: the two walks visited different counts"
        );
        if bf != df {
            disagreements += 1;
        }
    }
    assert!(
        disagreements >= 2,
        "only {disagreements} case(s) distinguish the two orders; the reordering is barely tested"
    );
}

/// The stated caps and the scope boundary are reported, not silently accepted.
#[test]
fn the_flattener_reports_input_it_will_not_flatten() {
    // A forest larger than `wire.fin` can describe.
    let mut vm = vm_for(WIRE_KEL);
    let big: Vec<i64> = (0..342).flat_map(|_| [3_i64, 0, 0]).collect();
    assert_eq!(
        run_flatten(&mut vm, CMD_FLATTEN_EMIT_CONSTS, &big, [342, 342, 0, 0, 0]),
        -240,
        "an oversized forest was not reported"
    );

    // More roots than nodes.
    let mut vm = vm_for(WIRE_KEL);
    assert_eq!(
        run_flatten(
            &mut vm,
            CMD_FLATTEN_EMIT_CONSTS,
            &[3, 7, 0],
            [2, 1, 0, 0, 0]
        ),
        -246,
        "nroots > nnodes was not reported"
    );

    // A child count that cannot be a bound.
    let mut vm = vm_for(WIRE_KEL);
    assert_eq!(
        run_flatten(
            &mut vm,
            CMD_FLATTEN_EMIT_CONSTS,
            &[8, 0, 99],
            [1, 1, 0, 0, 0]
        ),
        -241,
        "an impossible child count was not reported"
    );

    // A NODE COUNT THAT DOES NOT MATCH THE FOREST. Found by reading the walk
    // back, not by a failing test: with one childless root and nnodes = 3, the
    // walk ran past the queue and emitted three copies of node 0, silently. The
    // roots' subtree sizes must cover the forest exactly.
    //
    // The forest must be WELL FORMED and merely miscounted, which the first
    // version of this got wrong: passing one node's worth of fields while
    // declaring three left nodes 1 and 2 reading tag 0 from unseeded slots, so
    // the tag guard fired first with -245. Both codes are right; the test was
    // not exercising the one it named. Three valid scalars, one declared root.
    let mut vm = vm_for(WIRE_KEL);
    assert_eq!(
        run_flatten(
            &mut vm,
            CMD_FLATTEN_EMIT_CONSTS,
            &[3, 7, 0, 3, 8, 0, 3, 9, 0],
            [1, 3, 0, 0, 0]
        ),
        -248,
        "a node count larger than the roots' subtrees was not reported"
    );

    // MUST-NOT-FIRE for the same guard: a well-formed forest must get PAST the
    // cover check. It then fails at the region lookup, because this harness
    // seeds no directory — `-247`, not `-248`, is the evidence the cover check
    // stayed quiet.
    let mut vm = vm_for(WIRE_KEL);
    assert_eq!(
        run_flatten(
            &mut vm,
            CMD_FLATTEN_EMIT_CONSTS,
            &[3, 7, 0, 3, 8, 0],
            [2, 2, 0, 0, 0]
        ),
        -247,
        "a well-formed two-root forest was rejected before the region lookup"
    );

    // A tag this slice does not implement. STRUCT carries an `aux` index into a
    // side table, so emitting it here would produce a plausible wrong record.
    let mut vm = vm_for(WIRE_KEL);
    assert_eq!(
        run_flatten(
            &mut vm,
            CMD_FLATTEN_EMIT_CONSTS,
            &[10, 0, 0],
            [1, 1, 0, 0, 0]
        ),
        -245,
        "an out-of-scope tag was not reported"
    );
}
