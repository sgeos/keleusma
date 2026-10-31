//! The two authorities on which self-hosted stage sources exist, shared by the test
//! binaries that generate one test per stage.
//!
//! A binary that generates its per-stage tests from an explicit list must show that the
//! list is complete, or a stage could leave the list and its test would vanish without
//! a failure. These functions read the stage set from the stage directory and from the
//! driver's embedded-source table, so a per-stage binary can require its list to equal
//! both.

use std::collections::BTreeSet;
use std::path::Path;

/// The stage names under `src/selfhost/kel`, as file stems of the `.kel` sources.
pub fn directory_stage_names() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let names: BTreeSet<String> = std::fs::read_dir(&dir)
        .expect("the stage directory is readable")
        .map(|entry| entry.expect("a stage directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| {
            path.file_stem()
                .expect("a stage file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(!names.is_empty(), "no stage source found in {dir:?}");
    names
}

/// The stage names the driver embeds, read from the arms of `read_stage` in
/// `src/selfhost/mod.rs`.
///
/// The table is private to the driver, so it is read as source text. The scan takes
/// every `"<name>.kel" =>` arm between the function's signature and its fallback arm,
/// and it fails when either marker is missing or no arm is found, because a scan that
/// stopped matching would otherwise report an empty registry.
pub fn driver_stage_names() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/mod.rs");
    let text = std::fs::read_to_string(&path).expect("the driver source is readable");
    let start = text
        .find("fn read_stage(")
        .expect("the driver still defines read_stage; update this scan if it moved");
    let rest = &text[start..];
    let end = rest
        .find("other => panic!(")
        .expect("read_stage still ends with its fallback arm; update this scan if it changed");
    let names: BTreeSet<String> = rest[..end]
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"'))
        .filter_map(|line| line.split_once(".kel\" =>"))
        .map(|(name, _)| name.to_string())
        .collect();
    assert!(!names.is_empty(), "read no stage arm from read_stage");
    names
}
