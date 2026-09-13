//! The compiler produces the same bytes in separate processes, and does not
//! leak the input path into the artefact.
//!
//! # Why a separate process is the point
//!
//! `tests/selfhost_repeat_compile.rs` in the runtime crate compiles twice inside
//! ONE process and says plainly what it cannot see: "a global initialised once
//! per process would survive this check unchanged." That is the class these tests
//! cover — a once-initialised global, an iteration order seeded per process, an
//! ordering that depends on an address.
//!
//! The roadmap requires the toolchain to be byte-reproducible "so the fixed-point
//! and differential-oracle checks are meaningful", and the oracle itself cannot
//! establish it: the oracle compares the two backends against each other, so a
//! non-determinism they shared would pass unnoticed.
//!
//! # What is still NOT established
//!
//! Reproducibility across MACHINES, across toolchain versions, or across builds
//! with different compilation flags. These run one binary twice on one machine.
//! A source of variation that is constant for a given build — an embedded build
//! identifier, say — would pass here and still break a reproducible-build claim.

use std::path::PathBuf;
use std::process::Command;

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kel_repro_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    fn sub(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::create_dir_all(&p).expect("mkdir");
        p
    }
    fn write(&self, name: &str, body: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, body).expect("write source");
        p
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A program with per-function counters, locals and a loop, because the one known
/// defect of this class was a counter that was not reset between functions.
const SRC: &str = "private data d { q: Word }\n\
                   fn a() -> Word { for i in 0..3 limit 3 { d.q = d.q + i; } d.q }\n\
                   fn b(x: Word) -> Word { let y = x * 3; y + 1 }\n\
                   fn main() -> Word { a() + b(4) }\n";

/// Run `compile` in a fresh process and return the artefact's bytes.
fn compile(src: &std::path::Path, out: &std::path::Path, backend: &str, debug: bool) -> Vec<u8> {
    let mut args: Vec<String> = vec![
        "compile".into(),
        src.to_str().expect("utf8").into(),
        "-o".into(),
        out.to_str().expect("utf8").into(),
        "--compiler".into(),
        backend.into(),
    ];
    if debug {
        args.push("--debug".into());
    }
    let res = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .args(&args)
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .output()
        .expect("spawn keleusma");
    assert!(
        res.status.success(),
        "compiling with --compiler {backend} failed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    std::fs::read(out).expect("read the artefact the command reported writing")
}

/// **THE SAME SOURCE, COMPILED IN SEPARATE PROCESSES, GIVES THE SAME BYTES.**
///
/// Three runs rather than two: a value that alternates would pass a two-run check
/// half the time.
#[test]
fn the_same_source_compiles_identically_in_separate_processes() {
    let dir = TmpDir::new("procs");
    let src = dir.write("repro.kel", SRC);

    for backend in ["self-hosted", "rust"] {
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for n in 0..3 {
            let out = dir.0.join(format!("{backend}_{n}.kbc"));
            seen.push(compile(&src, &out, backend, false));
        }

        // NON-VACUITY: an empty or trivial artefact would make the equality hold for
        // reasons unrelated to determinism.
        assert!(
            seen[0].len() > 100,
            "the {backend} backend wrote {} bytes, which is too small to be the module \
             this source describes; the comparison below would be vacuous",
            seen[0].len()
        );

        assert!(
            seen.windows(2).all(|w| w[0] == w[1]),
            "the {backend} backend produced different bytes across separate processes. \
             Something varies per process — a once-initialised global, an iteration order \
             seeded at startup, or an ordering that depends on an address. The same-process \
             check in the runtime crate cannot see this class"
        );
    }
}

/// **THE INPUT PATH DOES NOT REACH THE ARTEFACT**, including under `--debug`,
/// which carries source spans.
///
/// A path embedded in the output is the classic reproducible-build leak: the same
/// source compiled from two checkouts would differ for a reason that has nothing
/// to do with the program.
#[test]
fn the_input_path_does_not_leak_into_the_artefact() {
    let dir = TmpDir::new("paths");
    let here = dir.write("repro.kel", SRC);
    let elsewhere = dir.sub("nested").join("differently_named.kel");
    std::fs::write(&elsewhere, SRC).expect("write second source");

    for debug in [false, true] {
        for backend in ["self-hosted", "rust"] {
            let a = compile(&here, &dir.0.join("a.kbc"), backend, debug);
            let b = compile(&elsewhere, &dir.0.join("b.kbc"), backend, debug);
            assert_eq!(
                a, b,
                "the {backend} backend embedded something path-dependent (debug: {debug}). \
                 The two sources are byte-identical and differ only in directory and file \
                 name, so the same program compiled from two checkouts would produce \
                 different artefacts"
            );
        }
    }
}
