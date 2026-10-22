//! The documentation gate excludes local generated copies while checking public files.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

fn run(directory: &Path) -> Output {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-md-links.kel");
    Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .arg("run")
        .arg(script)
        .current_dir(directory)
        .output()
        .unwrap()
}

#[test]
fn public_links_are_checked_and_generated_local_copies_are_excluded() {
    let directory = std::env::temp_dir().join(format!("kel_md_links_{}", std::process::id()));
    std::fs::create_dir_all(directory.join("secret")).unwrap();
    std::fs::write(directory.join("target.md"), "# Target\n").unwrap();
    std::fs::write(directory.join("README.md"), "[target](target.md)\n").unwrap();
    std::fs::write(
        directory.join("secret/generated.md"),
        "[relative link from the original location](missing.md)\n",
    )
    .unwrap();
    let valid = run(&directory);
    assert!(valid.status.success(), "{valid:?}");

    std::fs::write(directory.join("README.md"), "[broken](missing.md)\n").unwrap();
    let invalid = run(&directory);
    assert_eq!(invalid.status.code(), Some(1), "{invalid:?}");
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("README.md broken-link missing.md"));

    std::fs::write(directory.join("README.md"), "[target](target.md)\n").unwrap();
    let restored = run(&directory);
    assert!(restored.status.success(), "{restored:?}");
    std::fs::remove_dir_all(directory).unwrap();
}
