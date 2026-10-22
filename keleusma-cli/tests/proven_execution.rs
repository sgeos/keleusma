//! The shipping runner exposes strict admission and actual proof-gated execution.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "keleusma_proven_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir_all(path.join("trust")).unwrap();
        std::fs::create_dir_all(path.join("decrypt")).unwrap();
        Self(path)
    }
    fn source(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_keleusma"));
        cmd.env("KELEUSMA_TRUSTED_KEYS_DIR", self.0.join("trust"))
            .env("KELEUSMA_DECRYPTION_KEYS_DIR", self.0.join("decrypt"))
            .env_remove("KELEUSMA_REQUIRE_SIGNED")
            .env_remove("KELEUSMA_REQUIRE_ENCRYPTED");
        cmd
    }
    fn run(&self, file: &Path, flags: &[&str]) -> Output {
        self.command()
            .arg("run")
            .arg(file)
            .args(flags)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
const FREE: &str = "fn main() -> Word { 7 / 0 { ok(v) => v, zero_divisor(n) => n } }";
const TRAPPING: &str = "fn main() -> Word { 7 / 0 }";
#[test]
fn source_strict_and_proven_modes_accept_total_outcomes_and_refuse_bare_traps() {
    let f = Fixture::new();
    let good = f.source("free.kel", FREE);
    let bad = f.source("trap.kel", TRAPPING);
    for flag in ["--require-trap-free", "--proven-execution"] {
        let accepted = f.run(&good, &[flag]);
        assert!(accepted.status.success(), "{}", text(&accepted));
        assert_eq!(String::from_utf8_lossy(&accepted.stdout).trim(), "7");
        let rejected = f.run(&bad, &[flag]);
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(text(&rejected).contains("MayTrap"), "{}", text(&rejected));
    }
}
#[test]
fn both_compiler_artifacts_run_with_proven_execution() {
    let f = Fixture::new();
    let source = f.source("free.kel", FREE);
    for compiler in ["rust", "self-hosted"] {
        let artifact = f.0.join(format!("{compiler}.kbc"));
        let compiled = f
            .command()
            .arg("compile")
            .arg(&source)
            .arg("--compiler")
            .arg(compiler)
            .arg("-o")
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(compiled.status.success(), "{}", text(&compiled));
        let output = f.run(&artifact, &["--proven-execution"]);
        assert!(output.status.success(), "{}", text(&output));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
    }
}
#[test]
fn memory_reporting_and_script_argument_boundaries_are_explicit() {
    let f = Fixture::new();
    let good = f.source("free.kel", FREE);
    let bad = f.source("trap.kel", TRAPPING);
    let conflict = f.run(&good, &["--print-memory", "--proven-execution"]);
    assert!(!conflict.status.success());
    assert!(text(&conflict).contains("cannot be combined"));
    let report = f.run(&good, &["--print-memory", "--require-trap-free"]);
    assert!(report.status.success(), "{}", text(&report));
    assert!(text(&report).contains("arena:"));
    let refused = f.run(&bad, &["--print-memory", "--require-trap-free"]);
    assert!(!refused.status.success());
    assert!(!text(&refused).contains("arena:"));
    let argument = f.run(&bad, &["--", "--proven-execution"]);
    assert!(!argument.status.success());
    assert!(
        text(&argument).contains("DivisionByZero"),
        "{}",
        text(&argument)
    );
    assert!(!text(&argument).contains("trap policy"));
}

#[test]
fn offline_validation_preserves_trap_and_unavailable_distinctions() {
    let f = Fixture::new();
    for (source, expected, diagnostic) in [(FREE, 0, "trap-free"), (TRAPPING, 1, "MayTrap")] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let module = keleusma::compiler::compile(&ast).unwrap();
        let path = f.0.join("subject.kbc");
        std::fs::write(&path, module.to_bytes().unwrap()).unwrap();
        for backend in ["rust", "self-hosted"] {
            let output = f
                .command()
                .arg("validate")
                .arg(&path)
                .args(["--validator", backend, "--require-trap-free"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(expected), "{}", text(&output));
            assert!(text(&output).contains(diagnostic), "{}", text(&output));
        }
    }
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 7 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    module.chunks[0]
        .ops
        .push(keleusma::bytecode::Op::FixedDiv(0));
    let path = f.0.join("unsupported.kbc");
    std::fs::write(&path, module.to_bytes().unwrap()).unwrap();
    for backend in ["rust", "self-hosted"] {
        let output = f
            .command()
            .arg("validate")
            .arg(&path)
            .args(["--validator", backend, "--require-trap-free"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{}", text(&output));
        assert!(
            text(&output).contains("analysis unavailable"),
            "{}",
            text(&output)
        );
    }
}

#[test]
fn proven_execution_requires_more_than_the_offline_trap_verdict() {
    let f = Fixture::new();
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 7 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    module.chunks[0].ops = vec![keleusma::bytecode::Op::Return];
    let path = f.0.join("empty-return.kbc");
    std::fs::write(&path, module.to_bytes().unwrap()).unwrap();
    let classified = f
        .command()
        .arg("validate")
        .arg(&path)
        .arg("--require-trap-free")
        .output()
        .unwrap();
    assert!(classified.status.success(), "{}", text(&classified));
    assert!(text(&classified).contains("trap-free"));
    let refused = f.run(&path, &["--proven-execution"]);
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());
    assert!(
        text(&refused).contains("operand-pop proof"),
        "{}",
        text(&refused)
    );
}
