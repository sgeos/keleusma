//! The shipping command compiles every stage source to the reference artifact.
//!
//! The Order 1 audit's shipping-command row rested on a historical manual run of
//! `keleusma compile --compiler self-hosted` over every stage, recorded only in an
//! ignored log. The per-stage library tests in module
//! `production_compiler_matches_complete_stage_artifact` exercise the same entry and
//! serializer in process, and the command fixtures here
//! cover small programs. Neither reaches the binary with the real stage corpus, so a
//! defect in argument handling, shebang stripping, output writing or the command's
//! choice of serializer could pass both. This test runs the built binary once per stage.
//!
//! # The comparison
//!
//! Each artifact is compared with the reference compiler's serialization of the SAME
//! source, computed in this process. It is deliberately not compared with
//! `--compiler rust`, which prepends the CLI native preamble and therefore compiles a
//! different program. The historical run first made exactly that mistake.
//!
//! # What is still NOT established
//!
//! Signed, encrypted, stripped, debug and cross-target outputs are not exercised here.
//! Equality with the reference shows the two implementations agree on these inputs,
//! not that either is correct.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Stage sources in the runtime crate. The directory itself is the inventory.
fn stage_sources() -> Vec<(String, PathBuf)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/selfhost/kel");
    let mut stages: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("stage entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| {
            let name = path
                .file_stem()
                .expect("stage name")
                .to_string_lossy()
                .into_owned();
            (name, path)
        })
        .collect();
    stages.sort();
    stages
}

/// The reference compiler's artifact for `source`, in this process.
fn reference_artifact(source: &str) -> Vec<u8> {
    let tokens = keleusma::lexer::tokenize(source).expect("reference lex");
    let program = keleusma::parser::parse(&tokens).expect("reference parse");
    keleusma::compiler::compile(&program)
        .expect("reference compile")
        .to_bytes()
        .expect("reference artifact")
}

/// Run the shipping self-hosted command on `src`, returning the written artifact.
fn command_artifact(name: &str, src: &Path, out_dir: &Path) -> Vec<u8> {
    let out = out_dir.join(format!("{name}.kbc"));
    let result = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .arg("compile")
        .arg(src)
        .arg("-o")
        .arg(&out)
        .args(["--compiler", "self-hosted"])
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .output()
        .expect("spawn keleusma");
    assert!(
        result.status.success(),
        "{name}: self-hosted compile failed with {:?}\nstderr: {}",
        result.status.code(),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(&out).unwrap_or_else(|e| panic!("{name}: read artifact: {e}"))
}

#[test]
fn the_self_hosted_command_compiles_every_stage_to_the_reference_artifact() {
    let stages = stage_sources();
    // A collapsed inventory would pass vacuously. Fourteen is the current corpus; a
    // stage joining or leaving the directory should update this count deliberately.
    assert_eq!(stages.len(), 15, "stage inventory: {stages:?}");

    let out_dir = std::env::temp_dir().join(format!("kel_stage_corpus_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    // Each process bootstraps the stages independently, so bound concurrency rather
    // than run fifteen compilers at once.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 4);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some((name, path)) = stages.get(index) else {
                        break;
                    };
                    let actual = command_artifact(name, path, &out_dir);
                    results
                        .lock()
                        .unwrap()
                        .push((name.clone(), path.clone(), actual));
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    let _ = std::fs::remove_dir_all(&out_dir);

    assert_eq!(results.len(), stages.len(), "not every stage was compiled");
    for (name, path, actual) in &results {
        let source = std::fs::read_to_string(path).expect("stage source");
        let expected = reference_artifact(&source);
        assert!(
            actual == &expected,
            "{name}: command artifact ({} bytes) differs from the reference ({} bytes)",
            actual.len(),
            expected.len()
        );
        keleusma::bytecode::Module::from_bytes(actual)
            .unwrap_or_else(|e| panic!("{name}: artifact does not load: {e:?}"));
    }
}
