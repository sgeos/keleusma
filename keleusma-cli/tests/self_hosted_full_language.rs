//! The shipping command compiles the full-language census to the reference artifact.
//!
//! Order 6 gate 5 reads that the shipping command line uses the widened toolchain. The
//! census in `tests/selfhost_full_language.rs` pins which AST forms the library entry
//! compiles byte-identically, and every pin is measured in process. Nothing ran the built
//! binary over those forms, so a defect in argument handling, output writing or the
//! command's choice of serializer could pass the census. This test runs
//! `keleusma compile --compiler self-hosted` once per census row the census records as
//! `Identical`, and compares the written artifact with the reference compiler's
//! serialization of the same source, computed in this process.
//!
//! # Where the rows come from
//!
//! The rows are read out of the census table itself, so this test cannot drift from the
//! pins it exercises. A table that reads as empty, or as fewer rows than the census holds,
//! fails here rather than passing vacuously.
//!
//! # What is still NOT established
//!
//! Signed, encrypted, stripped, debug and cross-target outputs are not exercised here.
//! Equality with the reference shows the two implementations agree on these inputs, not
//! that either is correct.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The census table's source, which this test reads its rows from.
fn census_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/selfhost_full_language.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Undo the two escapes the census sources use, a newline and a quote.
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            other => panic!("unexpected escape {other:?} in a census source"),
        }
    }
    out
}

/// Every `(label, source, Class)` row of `AST_CENSUS` whose class is `Identical`.
///
/// A row is three string-or-identifier fields in parentheses, with the source a single
/// string literal. The reader walks the literal text rather than parsing Rust, and fails on
/// any row it cannot read, so a reformatted table is reported rather than skipped.
fn identical_rows(census: &str) -> (Vec<(String, String)>, usize) {
    let start = census
        .find("const AST_CENSUS")
        .expect("the census table is present");
    let end = census[start..]
        .find("\n];")
        .map(|n| start + n)
        .expect("the census table is closed");
    let table = &census[start..end];
    // Each quoted literal in order: label, source, label, source, ...
    let mut literals = Vec::new();
    let mut rest = table;
    while let Some(open) = rest.find('"') {
        let body = &rest[open + 1..];
        let mut close = None;
        let mut escaped = false;
        for (i, c) in body.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                close = Some(i);
                break;
            }
        }
        let close = close.expect("an unterminated census literal");
        let after = &body[close + 1..];
        literals.push((body[..close].to_string(), after));
        rest = after;
    }
    assert!(
        literals.len() % 2 == 0,
        "census literals do not pair into rows"
    );
    let mut rows = Vec::new();
    let total = literals.len() / 2;
    for pair in literals.chunks(2) {
        let (label, _) = &pair[0];
        let (source, after) = &pair[1];
        let class: String = after
            .trim_start_matches([',', ' ', '\n'])
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        assert!(
            !class.is_empty(),
            "{label}: the census row's class is unreadable"
        );
        if class == "Identical" {
            rows.push((label.clone(), unescape(source)));
        }
    }
    (rows, total)
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
fn command_artifact(index: usize, src: &str, out_dir: &Path) -> Result<Vec<u8>, String> {
    let input: PathBuf = out_dir.join(format!("row{index}.kel"));
    let out = out_dir.join(format!("row{index}.kbc"));
    std::fs::write(&input, src).map_err(|e| format!("write source: {e}"))?;
    let result = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .arg("compile")
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .args(["--compiler", "self-hosted"])
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .output()
        .map_err(|e| format!("spawn keleusma: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "self-hosted compile failed with {:?}: {}",
            result.status.code(),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    std::fs::read(&out).map_err(|e| format!("read artifact: {e}"))
}

#[test]
fn the_self_hosted_command_compiles_every_identical_census_row_to_the_reference_artifact() {
    let (rows, total) = identical_rows(&census_source());
    // Non-vacuity. The census held 74 rows, 66 of them Identical, when this was written. A
    // reader that found far fewer has broken, and a census that shrank should say so.
    assert!(total >= 74, "read only {total} census rows");
    assert!(
        rows.len() >= 66,
        "read only {} Identical census rows",
        rows.len()
    );

    let out_dir = std::env::temp_dir().join(format!("kel_full_language_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    // Each process bootstraps the stages independently, so bound concurrency.
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
                    let Some((label, source)) = rows.get(index) else {
                        break;
                    };
                    let actual = command_artifact(index, source, &out_dir);
                    results
                        .lock()
                        .unwrap()
                        .push((label.clone(), source.clone(), actual));
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    let _ = std::fs::remove_dir_all(&out_dir);

    assert_eq!(results.len(), rows.len(), "not every row was compiled");
    let mut failures = Vec::new();
    for (label, source, actual) in &results {
        match actual {
            Err(error) => failures.push(format!("{label}: {error}")),
            Ok(bytes) => {
                if *bytes != reference_artifact(source) {
                    failures.push(format!(
                        "{label}: the command artifact differs from the reference"
                    ));
                } else if let Err(e) = keleusma::bytecode::Module::from_bytes(bytes) {
                    failures.push(format!("{label}: the artifact does not load: {e:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
