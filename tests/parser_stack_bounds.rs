//! Debug parsing must honor the depth guard on a normal two-mebibyte thread stack.
#![cfg(feature = "compile")]

use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn on_small_stack<F: FnOnce() + Send + 'static>(run: F) {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn every_current_stage_parses_and_compiles_on_a_two_mebibyte_stack() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "kel"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 14, "stage inventory changed");
    for path in paths {
        on_small_stack(move || {
            eprintln!("stage {}", path.display());
            let source = std::fs::read_to_string(path).unwrap();
            let tokens = tokenize(&source).unwrap();
            let program = parse(&tokens).unwrap();
            let module = compile(&program).unwrap();
            assert!(!module.chunks.is_empty());
        });
    }
}

#[test]
fn nesting_boundary_is_unchanged_and_reports_an_error_without_aborting() {
    for (opening, closing) in [
        ("(", ")"),
        ("if true { ", " } else { 0 }"),
        ("if true { ", " }"),
        ("match 0 { _ => ", " }"),
        ("[", "]"),
        ("f(", ")"),
        ("S { value: ", " }"),
        ("E::V(", ")"),
        ("|| { ", " }"),
    ] {
        for (depth, accepted) in [(23, true), (24, false)] {
            let source = format!(
                "fn main() -> Word {{ {}0{} }}",
                opening.repeat(depth),
                closing.repeat(depth)
            );
            on_small_stack(move || {
                eprintln!("shape {opening:?}, depth {depth}");
                let result = parse(&tokenize(&source).unwrap());
                if accepted {
                    result.unwrap();
                } else {
                    let error = result.unwrap_err();
                    assert!(
                        error.message.contains("parser recursion depth 24 exceeded"),
                        "{error:?}"
                    );
                }
            });
        }
    }
}
