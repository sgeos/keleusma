//! Validate bytecode through the actual shipping command without executing it.
use keleusma::bytecode::{Module, Op};
use std::process::Command;
fn check(module: &Module, backend: &str, capacity: usize, label: &str) -> std::process::Output {
    let path = std::env::temp_dir().join(format!(
        "kel_validate_{}_{}_{}.kbc",
        std::process::id(),
        label,
        backend
    ));
    std::fs::write(&path, module.to_bytes().unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .args([
            "validate",
            path.to_str().unwrap(),
            "--validator",
            backend,
            "--arena-capacity",
            &capacity.to_string(),
        ])
        .env("KEL_SELFHOST_CACHE", "0")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Debug execution of the large parser artifact measured about 125 seconds
    // after stack repair, exceeding the former shared 120-second deadline.
    // This bounds a test process, not the subject program's execution time.
    let timeout_seconds = if cfg!(debug_assertions) { 600 } else { 120 };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "{label} with {backend} at capacity {capacity} exceeded {timeout_seconds} seconds: {output:?}"
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    std::fs::remove_file(path).unwrap();
    output
}
#[test]
fn command_distinguishes_valid_invalid_and_unavailable() {
    let module = keleusma::selfhost::compile_src("fn main() -> Word { 7 }");
    for backend in ["rust", "self-hosted"] {
        let out = check(&module, backend, 65536, "valid");
        assert!(out.status.success(), "{:?}", out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "validation passed"
        );
        let mut bad = module.clone();
        bad.chunks[0].ops[0] = Op::GetLocal(65535);
        assert_eq!(
            check(&bad, backend, 65536, "invalid").status.code(),
            Some(1)
        );
    }
    let mut large = module.clone();
    large.chunks[0].local_count = 257;
    assert!(check(&large, "rust", 65536, "large").status.success());
    let out = check(&large, "self-hosted", 65536, "large");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unavailable"));
}
#[test]
fn command_checks_resource_capacity_without_running_the_stream() {
    let module = keleusma::selfhost::compile_src("loop main(n: Word) -> Word { yield n }");
    for backend in ["rust", "self-hosted"] {
        assert_eq!(check(&module, backend, 0, "small").status.code(), Some(1));
        assert!(check(&module, backend, 65536, "enough").status.success());
    }
}

#[test]
fn reference_disagreement_prevents_successful_admission() {
    use keleusma::bytecode::StructField;
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    use keleusma::value_layout::ScalarKind;

    // The selected typed-stage profile widens at a join. The reference retains
    // this scalar shape, so the shipping consistency gate must catch the gap.
    let mut module = keleusma::selfhost::compile_src("fn main(n: Word) -> Word { n }");
    module.chunks[0].ops = vec![
        Op::GetLocal(0),
        Op::PushImmediate(1),
        Op::If(3),
        Op::EndIf,
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        admit_module_via_kel(&module, 65536).unwrap(),
        VerificationVerdict::Accepted
    );
    let out = check(&module, "self-hosted", 65536, "disagreement");
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("validator disagreement"));
    assert_eq!(
        check(&module, "rust", 65536, "disagreement").status.code(),
        Some(1)
    );
}

/// The stage sources under `src/selfhost/kel`, as file stems.
fn directory_stage_names() -> std::collections::BTreeSet<String> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/selfhost/kel");
    let names: std::collections::BTreeSet<String> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert!(!names.is_empty(), "no stage source found in {directory:?}");
    names
}

/// The stage names the driver embeds, read from the `"<name>.kel" =>` arms of
/// `read_stage` in `src/selfhost/mod.rs`, as `tests/selfhost_stages/registry.rs`
/// reads them for the root crate. The scan fails when either marker is missing or
/// no arm is found, since a scan that stopped matching would report an empty set.
fn driver_stage_names() -> std::collections::BTreeSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/selfhost/mod.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text
        .find("fn read_stage(")
        .expect("the driver still defines read_stage; update this scan if it moved");
    let rest = &text[start..];
    let end = rest
        .find("other => panic!(")
        .expect("read_stage still ends with its fallback arm; update this scan if it changed");
    let names: std::collections::BTreeSet<String> = rest[..end]
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"'))
        .filter_map(|line| line.split_once(".kel\" =>"))
        .map(|(name, _)| name.to_string())
        .collect();
    assert!(!names.is_empty(), "read no stage arm from read_stage");
    names
}

/// Admit one stage source at its stream's memory bound and refuse it one byte
/// below, through both shipping validators.
fn reach_resource_boundary(name: &str, source: &str) {
    use keleusma::bytecode::BlockType;
    let module = keleusma::selfhost::compile_src(source);
    let bounds = keleusma::verify::module_wcmu(&module, &[]).unwrap();
    let capacity = module
        .chunks
        .iter()
        .zip(bounds)
        .filter(|(chunk, _)| chunk.block_type == BlockType::Stream)
        .map(|(_, (stack, heap))| stack as usize + heap as usize)
        .max()
        .expect("stage entry must be a stream");
    assert!(capacity > 0);
    for backend in ["rust", "self-hosted"] {
        let admitted = check(&module, backend, capacity, name);
        assert_eq!(admitted.status.code(), Some(0), "{name}: {admitted:?}");
        let rejected = check(&module, backend, capacity - 1, name);
        assert_eq!(rejected.status.code(), Some(1), "{name}: {rejected:?}");
    }
}

/// One test per stage, so that nextest can spread the stages across its threads and
/// shards. The single test this replaces ran about 20.6 minutes in continuous
/// integration on 2026-10-30 and bounded the Test job. Kaizen item N4, split as K3
/// split the self-host tests. `shipping_validator_covers_every_stage_source` keeps the
/// former count of fifteen and requires the list to equal the stage directory and the
/// driver's embedded table, so a stage cannot leave the list without a failure.
macro_rules! shipping_validator_stage_tests {
    ($($stage:ident),+ $(,)?) => {
        const VALIDATOR_STAGES: &[&str] = &[$(stringify!($stage)),+];

        mod shipping_validator_reaches_every_current_stage {
            $(
                #[test]
                fn $stage() {
                    super::reach_resource_boundary(
                        stringify!($stage),
                        include_str!(concat!(
                            "../../src/selfhost/kel/",
                            stringify!($stage),
                            ".kel"
                        )),
                    );
                }
            )+
        }
    };
}

shipping_validator_stage_tests!(
    analyze,
    codegen,
    lexer,
    parse,
    reconstruct,
    template_scan,
    verify_datalayout,
    verify_depth,
    verify_structural,
    verify_typed,
    verify_types,
    verify_yield,
    wire,
    wire_debug,
    wire_schema,
);

#[test]
fn shipping_validator_covers_every_stage_source() {
    let listed: std::collections::BTreeSet<String> =
        VALIDATOR_STAGES.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        listed.len(),
        VALIDATOR_STAGES.len(),
        "a stage is listed twice"
    );
    assert_eq!(listed.len(), 15, "review the stage census when it changes");
    assert_eq!(listed, directory_stage_names());
    assert_eq!(listed, driver_stage_names());
}
