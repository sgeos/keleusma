//! Replays the committed fuzz seed corpus through the fuzz harness on the stable toolchain.
//!
//! # What this covers
//!
//! The cargo-fuzz targets under `fuzz/` need a nightly toolchain and run on a schedule,
//! not on every change. This file calls the same harness bodies, included from
//! `fuzz/harness.rs` by path, on every file under `fuzz/corpus/<target>/`, so that a
//! regression on a committed seed fails the ordinary suite. A reproducer the fuzzer finds
//! is added to the corpus and then replays here.
//!
//! # The contract
//!
//! Each harness body must return for every corpus file, without panicking and within
//! [`PER_INPUT_TIMEOUT`]. The census below also requires each target's corpus to reach a
//! minimum depth, because a corpus that the decoder now refuses whole would replay green
//! and test nothing. Committed bytecode goes stale when the wire format changes. When the
//! census fails for that reason, regenerate the corpus with
//! `cargo test --features signatures --test fuzz_corpus_replay -- --ignored regenerate`.
//!
//! # What this does not cover
//!
//! The signed-loader corpus replays only when the `signatures` feature is enabled. The
//! replay exercises the committed seeds and nothing a fuzzing run would add, so it is a
//! regression check and not a substitute for fuzzing.

#![cfg(all(feature = "compile", feature = "verify"))]

// The replay calls every harness body, but the regeneration helper uses only some items.
#[allow(dead_code)]
#[path = "../fuzz/harness.rs"]
mod harness;

use harness::Reach;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// The longest one corpus input may take before the replay reports a hang.
const PER_INPUT_TIMEOUT: Duration = Duration::from_secs(30);

/// The stack given to each replay thread. It matches the eight mebibytes of the main
/// thread a libFuzzer run executes on, so a stack overflow found by one is found by both.
const REPLAY_STACK_BYTES: usize = 8 * 1024 * 1024;

/// The largest total size the committed corpus may reach.
const MAX_CORPUS_BYTES: u64 = 1024 * 1024;

/// One harness body, by target name.
type Body = fn(&[u8]) -> Reach;

/// The targets this build can replay, with the minimum number of corpus files that must
/// reach each named depth.
fn targets() -> Vec<(&'static str, Body, Reach, usize)> {
    vec![
        ("load_bytes", harness::load_bytes, Reach::Loaded, 6),
        #[cfg(feature = "signatures")]
        ("load_signed", harness::load_signed, Reach::Loaded, 6),
        ("verify_module", harness::verify_module, Reach::Verified, 6),
        ("module_bounds", harness::module_bounds, Reach::Verified, 6),
        ("front_end", harness::front_end, Reach::Verified, 6),
    ]
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus")
}

/// The corpus files of one target, sorted for a deterministic replay order.
fn corpus_files(target: &str) -> Vec<PathBuf> {
    let dir = corpus_root().join(target);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("corpus directory {} is unreadable: {e}", dir.display()))
        .map(|entry| entry.expect("a readable corpus entry").path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    files
}

/// Runs `body` on `data` in its own thread, returning the reach, or a description of the
/// panic or the timeout.
fn replay_one(body: Body, data: Vec<u8>) -> Result<Reach, String> {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::Builder::new()
        .stack_size(REPLAY_STACK_BYTES)
        .spawn(move || {
            let reach = body(&data);
            // The receiver may have timed out and gone. That case is reported below.
            let _ = tx.send(reach);
        })
        .map_err(|e| format!("could not spawn the replay thread: {e}"))?;
    match rx.recv_timeout(PER_INPUT_TIMEOUT) {
        Ok(reach) => {
            handle
                .join()
                .map_err(|_| String::from("panicked after reporting"))?;
            Ok(reach)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // The thread is left running. The test fails, so the process ends soon.
            Err(format!("did not finish within {PER_INPUT_TIMEOUT:?}"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(String::from("panicked")),
    }
}

#[test]
fn every_corpus_file_replays_without_panicking_or_hanging() {
    let mut failures = Vec::new();
    let mut census: BTreeMap<&str, BTreeMap<Reach, usize>> = BTreeMap::new();
    for (target, body, floor_reach, floor_count) in targets() {
        let files = corpus_files(target);
        assert!(!files.is_empty(), "the `{target}` corpus is empty");
        let counts = census.entry(target).or_default();
        for path in &files {
            let data = std::fs::read(path).expect("a readable corpus file");
            match replay_one(body, data) {
                Ok(reach) => *counts.entry(reach).or_default() += 1,
                Err(why) => failures.push(format!("{target}: {} {why}", path.display())),
            }
        }
        let reached: usize = counts
            .iter()
            .filter(|(reach, _)| **reach >= floor_reach)
            .map(|(_, n)| n)
            .sum();
        if reached < floor_count {
            failures.push(format!(
                "{target}: only {reached} of {} corpus files reached {floor_reach:?}, below the \
                 floor of {floor_count}. The committed bytecode may be stale, see this file's \
                 documentation for the regeneration command. Census {counts:?}",
                files.len()
            ));
        }
    }
    // Printed so that a reader can see the reach without making the test fail.
    println!("fuzz corpus replay census: {census:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The loader corpus must execute at least one seed, or the execution path in the harness
/// is never replayed.
#[test]
fn the_loader_corpus_executes_at_least_one_seed() {
    let executed = corpus_files("load_bytes")
        .into_iter()
        .map(|p| std::fs::read(p).expect("a readable corpus file"))
        .filter(|data| replay_one(harness::load_bytes, data.clone()) == Ok(Reach::Executed))
        .count();
    assert!(executed >= 1, "no load_bytes seed reached execution");
}

/// Resealing a valid artifact must leave it unchanged. This pins the harness's checksum to
/// the one the loader checks, so that a resealed mutation is not refused for a reason the
/// harness introduced.
#[test]
fn resealing_a_valid_artifact_is_the_identity() {
    let mut checked = 0usize;
    for path in corpus_files("load_bytes") {
        let data = std::fs::read(&path).expect("a readable corpus file");
        if keleusma::bytecode::Module::from_bytes(&data).is_ok() {
            assert_eq!(harness::resealed(&data), data, "{}", path.display());
            checked += 1;
        }
    }
    assert!(checked >= 3, "only {checked} decodable seeds were checked");
}

/// KNOWN FINDING, recorded and not fixed, found by the `load_bytes` target on 2026-10-30.
///
/// A header whose word, address or float width exponent is 32 or more, under a valid
/// trailer, makes [`keleusma::vm::Vm::load_bytes`] panic with "attempt to shift left with
/// overflow". The framing check correctly refuses the header with a size-mismatch
/// `LoadError`, and the panic is raised afterwards, when the conversion into `VmError`
/// formats that error. The `Display` implementation in `src/bytecode.rs` computes
/// `1u32 << got` for the three size-mismatch variants, which overflows for an exponent of
/// 32 or more. A build with overflow checks enabled, which includes the test and fuzz
/// profiles, panics. A build without them wraps the shift and reports a wrong width
/// instead.
///
/// The test is ignored so that the suite stays green until the library is fixed, which
/// this change deliberately does not do. Run it with `--ignored` to reproduce. It asserts
/// the contract the fix must satisfy, so it passes once the defect is gone, and the
/// `#[ignore]` should then be removed. The input is built from a committed seed rather than
/// committed to the corpus, because a corpus entry that panics would fail the replay above.
#[test]
#[ignore = "known finding, LoadError Display overflows a shift for a width exponent of 32 or more"]
fn a_width_exponent_of_32_or_more_is_refused_without_panicking() {
    let seed = std::fs::read(corpus_root().join("load_bytes/seed-single_function.kbc"))
        .expect("the single-function seed");
    // Bytes 12, 13 and 14 of the framing header hold the word, address and float width
    // exponents.
    for (offset, field) in [(12usize, "word"), (13, "address"), (14, "float")] {
        let mut bytes = seed.clone();
        bytes[offset] = 38;
        let bytes = harness::resealed(&bytes);
        let outcome = std::panic::catch_unwind(|| {
            let arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
            keleusma::vm::Vm::load_bytes(&bytes, &arena).is_ok()
        });
        assert_eq!(
            outcome.ok(),
            Some(false),
            "a {field} width exponent of 38 must be refused without a panic"
        );
    }
}

/// Every fuzz target declared in `fuzz/Cargo.toml` has a corpus directory, and every
/// corpus directory names a declared target, so that a target cannot be added without a
/// corpus the replay reaches.
#[test]
fn every_declared_target_has_a_corpus_and_the_reverse() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/Cargo.toml"))
            .expect("the fuzz manifest");
    let mut declared: Vec<String> = manifest
        .split("[[bin]]")
        .skip(1)
        .filter_map(|section| {
            let line = section
                .lines()
                .find(|l| l.trim_start().starts_with("name"))?;
            Some(line.split('"').nth(1)?.to_string())
        })
        .collect();
    declared.sort();
    let mut dirs: Vec<String> = std::fs::read_dir(corpus_root())
        .expect("the corpus root")
        .map(|e| {
            e.expect("a corpus root entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    dirs.sort();
    assert!(
        declared.len() >= 5,
        "parsed only {declared:?} from fuzz/Cargo.toml"
    );
    assert_eq!(declared, dirs);
}

/// The committed corpus stays small enough to keep in the repository.
#[test]
fn the_corpus_stays_under_its_size_ceiling() {
    let mut total = 0u64;
    for entry in std::fs::read_dir(corpus_root()).expect("the corpus root") {
        let dir = entry.expect("a corpus root entry").path();
        for file in std::fs::read_dir(&dir).expect("a corpus directory") {
            total += file
                .expect("a corpus entry")
                .metadata()
                .expect("metadata")
                .len();
        }
    }
    assert!(total > 0, "the corpus is empty");
    assert!(total <= MAX_CORPUS_BYTES, "the corpus is {total} bytes");
}

/// The single-function program whose seed reaches execution in the loader target.
const SINGLE_FUNCTION_SRC: &str =
    "fn main() -> Word { let a = 7; if a > 3 { a * 2 } else { a + 1 } }";

/// A stream program, so that the corpus carries a `Stream` chunk.
const STREAM_SRC: &str =
    "loop main(seed: Word) -> Word { let a = seed + 1; if a > 3 { yield a } else { yield seed } }";

/// The repository sources the corpus is compiled from, by seed name.
fn seed_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        ("single_function", SINGLE_FUNCTION_SRC),
        ("stream", STREAM_SRC),
        (
            "01_arithmetic",
            include_str!("../examples/scripts/01_arithmetic.kel"),
        ),
        (
            "02_struct_field",
            include_str!("../examples/scripts/02_struct_field.kel"),
        ),
        (
            "03_enum_match",
            include_str!("../examples/scripts/03_enum_match.kel"),
        ),
        (
            "04_for_in",
            include_str!("../examples/scripts/04_for_in.kel"),
        ),
        (
            "05_pipeline",
            include_str!("../examples/scripts/05_pipeline.kel"),
        ),
        (
            "06_multiheaded",
            include_str!("../examples/scripts/06_multiheaded.kel"),
        ),
    ]
}

fn write_seed(target: &str, name: &str, bytes: &[u8]) {
    let dir = corpus_root().join(target);
    std::fs::create_dir_all(&dir).expect("the corpus directory is creatable");
    std::fs::write(dir.join(format!("seed-{name}")), bytes).expect("the seed is writable");
}

/// Regenerates the committed corpus from repository sources. Ignored, because it writes
/// into the source tree. Run it after a wire-format change has made the bytecode seeds
/// stale, and commit the result.
#[test]
#[ignore = "writes the corpus into the source tree, so it runs only when asked"]
fn regenerate() {
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
    for (name, source) in seed_sources() {
        write_seed("front_end", &format!("{name}.kel"), source.as_bytes());
        let module = compile(&parse(&tokenize(source).expect("lex")).expect("parse"))
            .unwrap_or_else(|e| panic!("seed {name} does not compile: {e:?}"));
        let bytes = module.to_bytes().expect("encode");
        for target in ["load_bytes", "verify_module", "module_bounds"] {
            write_seed(target, &format!("{name}.kbc"), &bytes);
        }
        #[cfg(feature = "signatures")]
        {
            let key = ed25519_dalek::SigningKey::from_bytes(&harness::TEST_SIGNING_SEED);
            let signed =
                keleusma::wire_format::module_to_signed_wire_bytes(&module, &key).expect("sign");
            write_seed("load_signed", &format!("{name}.signed.kbc"), &signed);
        }
    }
    // An unsigned artifact, so that the signed loader's unsigned branch is seeded.
    #[cfg(feature = "signatures")]
    {
        let module = compile(&parse(&tokenize(SINGLE_FUNCTION_SRC).expect("lex")).expect("parse"))
            .expect("compile");
        write_seed(
            "load_signed",
            "unsigned.kbc",
            &module.to_bytes().expect("encode"),
        );
    }
    // The committed fixture from tests/fixtures.
    write_seed(
        "load_bytes",
        "fixture_int_to_float.kbc",
        include_bytes!("fixtures/int_to_float.kbc"),
    );
    // A synthesized-module seed for the structured targets. A `Func` header, then the
    // records `Const 0` and `Return`, in the layout `harness::synthesized` reads.
    let const_id = keleusma::wire_format::opcode_id_of(&keleusma::bytecode::Op::Const(0)).0;
    let return_id = keleusma::wire_format::opcode_id_of(&keleusma::bytecode::Op::Return).0;
    let synth = [0u8, 1, 0, const_id, 0, 0, 0, return_id, 0, 0, 0];
    for target in ["verify_module", "module_bounds"] {
        write_seed(target, "synth_const_return", &synth);
    }
}
