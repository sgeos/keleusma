//! Three module counts describe what a reader takes to be one corpus.
//!
//! `bound_transfer.rs` prints **"modules examined: 74"** and **"modules
//! compared: 71"** in two censuses in the same file, while every other census
//! reports **69 compiling modules** and the fingerprint pins **74 files**.
//!
//! **This is the third instance of one shape**: composite sites quoted as 239
//! and 256, modules as 91 and 67. Both turned out to be a stale figure and a
//! duplicated directory, and **neither was visible until two numbers were placed
//! side by side.**
//!
//! Each count is computed here from the same roots so the differences are
//! attributable rather than guessed.

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use std::collections::BTreeSet;

const CORPUS_DIRS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

fn kel_files() -> Vec<std::path::PathBuf> {
    let mut stack: Vec<std::path::PathBuf> =
        CORPUS_DIRS.iter().map(std::path::PathBuf::from).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn compiles(p: &std::path::Path) -> Option<Module> {
    let src = std::fs::read_to_string(p).ok()?;
    tokenize(&src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// Every candidate population, computed from one root set.
#[test]
fn what_each_module_count_actually_counts() {
    let files = kel_files();
    // **KEYED BY PATH, NOT BY FILE NAME.** The first version of this probe used
    // the bare name and reported 73 files against the fingerprint's 74, because
    // **two files are named `prelude.kel`** — one under the RTOS scripts and one
    // under the compiler. A name key silently merges them, which is the same
    // shape as every population defect this line has found: a well-formed number
    // about a slightly different set.
    let names = |v: &[std::path::PathBuf]| -> BTreeSet<String> {
        v.iter()
            .map(|p| p.to_string_lossy().replace("../", ""))
            .collect()
    };

    let all: BTreeSet<String> = names(&files);
    let compiling: BTreeSet<String> = files
        .iter()
        .filter(|p| compiles(p).is_some())
        .map(|p| p.to_string_lossy().replace("../", ""))
        .collect();
    let non_prelude_files: BTreeSet<String> = all
        .iter()
        .filter(|n| !n.ends_with("prelude.kel"))
        .cloned()
        .collect();
    let compiling_non_prelude: BTreeSet<String> = compiling
        .iter()
        .filter(|n| !n.ends_with("prelude.kel"))
        .cloned()
        .collect();

    println!("\n================ CANDIDATE POPULATIONS, FOUR-ROOT CORPUS");
    println!("  .kel files found                    : {}", all.len());
    println!(
        "  ...that compile                     : {}",
        compiling.len()
    );
    println!(
        "  files, excluding prelude.kel        : {}",
        non_prelude_files.len()
    );
    println!(
        "  compiling, excluding prelude.kel    : {}",
        compiling_non_prelude.len()
    );
    // **THE POPULATION `bound_transfer.rs` USES.** It prepends the RTOS prelude
    // to each non-prelude RTOS script before compiling, so the five that fail
    // standalone succeed there. That is why its census says 74 where every other
    // says 69: **a strictly larger corpus, not a different count of the same
    // one.**
    let with_rtos_prelude: BTreeSet<String> = files
        .iter()
        .filter(|p| {
            // **NO EARLY `return` HERE, DELIBERATELY.** `skippable_tests.rs` scans
            // for a `return` reached before any assertion, and it cannot tell a
            // closure's value from a test's early exit — it flagged this very
            // test when the read used `else { return false; }`. The pin was
            // right to fire and wrong about the cause, so the repair is to write
            // the closure without a `return` rather than to record a test as
            // skippable when it cannot skip.
            let Some(src) = std::fs::read_to_string(p).ok() else {
                unreachable!("the file list came from a directory walk")
            };
            let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
            let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
            let full = if is_rtos && !is_prelude {
                match std::fs::read_to_string("../examples/rtos/scripts/prelude.kel") {
                    Ok(pre) => format!("{pre}\n{src}"),
                    Err(_) => src,
                }
            } else {
                src
            };
            tokenize(&full)
                .ok()
                .and_then(|t| parse(&t).ok())
                .and_then(|a| compile(&a).ok())
                .is_some()
        })
        .map(|p| p.to_string_lossy().replace("../", ""))
        .collect();
    println!(
        "  compiling WITH the RTOS prelude     : {}",
        with_rtos_prelude.len()
    );
    println!("  ------------------------------------------------");
    let not_compiling: Vec<&String> = all.difference(&compiling).collect();
    println!(
        "  read but NOT compiling ({}): {not_compiling:?}",
        not_compiling.len()
    );
    println!(
        "\n  A FIGURE WITHOUT ITS POPULATION IS HOW TWO NUMBERS GET COMPARED THAT\n  \
         MEASURE DIFFERENT THINGS. Each census should say which of these it means.\n================\n"
    );

    // **NON-VACUITY.** Populations that all coincide could not explain a
    // discrepancy, and a scan finding nothing would satisfy any claim.
    assert!(
        all.len() > 50,
        "only {} files found; the scan is not reading the corpus",
        all.len()
    );
    assert_eq!(
        with_rtos_prelude.len(),
        all.len(),
        "every corpus file should compile once the RTOS scripts are given their \
         prelude; that is what makes `bound_transfer.rs`'s 74 equal the file count"
    );
    assert!(
        compiling.len() < all.len(),
        "every file compiles, so 'files' and 'compiling' cannot explain any \
         difference between two counts"
    );
}
