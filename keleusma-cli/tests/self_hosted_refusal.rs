//! What a user actually sees when `--compiler self-hosted` refuses a program.
//!
//! The library-level tests assert on the error type's `Display`. **That is one
//! layer short of the product.** The command-line front end formats its own
//! message around that error and decides whether to print the retry hint, so a
//! construct name that survives `Display` could still be lost before the
//! terminal. These tests run the binary and read what it prints.
//!
//! The distinction is not hypothetical for this session's work: an earlier
//! increment reasoned about refusal quality from a test harness that unwraps,
//! rather than from the shipping entry point, and was wrong about what a user
//! meets. Asserting on `Display` instead of on the terminal is the same
//! substitution one layer out.

use std::path::PathBuf;
use std::process::Command;

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kel_shref_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    fn write(&self, name: &str, body: &str) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, body).expect("write source");
        p.to_str().expect("utf8 path").to_string()
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().expect("utf8 path").to_string()
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `keleusma compile <src> -o <out> --compiler self-hosted`, returning
/// `(success, stderr)`. The refusal goes to stderr, which is why stdout is not
/// what these tests read.
fn compile_self_hosted(src: &str, out: &str) -> (bool, String) {
    let res = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .args(["compile", src, "-o", out, "--compiler", "self-hosted"])
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .output()
        .expect("spawn keleusma");
    (
        res.status.success(),
        String::from_utf8_lossy(&res.stderr).into_owned(),
    )
}

/// **THE CONSTRUCT NAME REACHES THE TERMINAL.**
///
/// Each of these programs is refused, and the refusal a user reads names the
/// construct they wrote rather than only the stage's internal diagnostic.
#[test]
fn a_refusal_names_the_construct_on_the_terminal() {
    // (tag, source, the phrase a user must see)
    const CASES: &[(&str, &str, &str)] = &[
        // A variable arm on a scalar match compiles since Order 6. On an enum scrutinee it
        // is still refused, and named.
        (
            "varpat",
            "enum E { A, B }\nfn main(e: E) -> Word { match e { E::A => 1, x => 2 } }\n",
            "a variable pattern `x`",
        ),
        (
            "assert",
            "fn main(a: Word) -> Word { assert a > 0, \"positive\"; a }\n",
            "an `assert` statement",
        ),
        // `floatlit` left this corpus in Order 6 slice 5b, when float literals came to
        // compile byte-identically through the self-hosted command.
    ];

    let dir = TmpDir::new("named");
    for (tag, body, phrase) in CASES {
        let src = dir.write(&format!("{tag}.kel"), body);
        let out = dir.path(&format!("{tag}.kbc"));
        let (ok, err) = compile_self_hosted(&src, &out);

        assert!(
            !ok,
            "{tag}: the command SUCCEEDED. If the self-hosted subset genuinely gained \
             this construct, move it out of this corpus rather than relaxing the check"
        );
        assert!(
            err.contains(phrase),
            "{tag}: the terminal does not name the construct, so a user reads only the \
             stage's internal diagnostic even though the library-level message carries \
             the name. Output was: {err}"
        );
        assert!(
            err.contains("--compiler rust"),
            "{tag}: the retry hint is gone. A self-hosted-subset limitation is exactly \
             the case where the reference backend does help. Output was: {err}"
        );
    }
}

/// **THE FLOAT BOUNDARY HOLDS AT THE PRODUCT, NOT ONLY IN THE LIBRARY.**
///
/// A float LITERAL is refused, asserted above. A float-typed signature with no
/// literal compiles. Without this half, the refusal corpus could pass while the
/// subset had quietly narrowed to reject every float-typed program, and the
/// construct name would then be blaming the wrong thing.
#[test]
fn a_float_typed_signature_without_a_literal_still_compiles() {
    let dir = TmpDir::new("ftype");
    let src = dir.write(
        "ftype.kel",
        "fn f(a: Float) -> Float { a }\nfn main(x: Float) -> Float { f(x) }\n",
    );
    let out = dir.path("ftype.kbc");
    let (ok, err) = compile_self_hosted(&src, &out);

    assert!(
        ok,
        "a float-typed function with no float literal no longer compiles through the \
         self-hosted backend. The construct scan names the LITERAL on the strength of \
         this; if the type is now outside the subset too, say which changed. Output \
         was: {err}"
    );
    assert!(
        std::fs::metadata(&out).is_ok(),
        "the command reported success but wrote no output file, so the success above \
         is not evidence that a module was produced"
    );
}

/// Source rejection must come from the independent pipeline and produce no artifact.
#[test]
fn independent_type_rejection_reaches_the_command() {
    let dir = TmpDir::new("type_rejection");
    for (index, source) in [
        "fn main() -> Word { 1 + true }",
        "fn f(n: Word) -> Word { n } fn main() -> Word { f(true) }",
        "fn main() -> Word { unknown_name }",
    ]
    .iter()
    .enumerate()
    {
        let src = dir.write(&format!("bad{index}.kel"), source);
        let out = dir.path(&format!("bad{index}.kbc"));
        let (ok, error) = compile_self_hosted(&src, &out);
        assert!(!ok, "{source}");
        assert!(error.contains("pipeline type rejection"), "{error}");
        assert!(!error.contains("retry with --compiler rust"), "{error}");
        assert!(!std::path::Path::new(&out).exists());
    }
}

/// The command writes the library artifact and executes its independently expected result.
#[test]
fn command_emits_the_production_artifact_and_runs_it() {
    let dir = TmpDir::new("production_artifact");
    let source = "fn main() -> Word { 1 + 2 * 3 }";
    let src = dir.write("valid.kel", source);
    let out = dir.path("valid.kbc");
    let (ok, error) = compile_self_hosted(&src, &out);
    assert!(ok, "{error}");
    let actual = std::fs::read(&out).expect("emitted artifact");
    let expected =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
            .expect("production library")
            .to_bytes()
            .expect("encode library artifact");
    assert_eq!(actual, expected);
    let result = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .args(["run", &out])
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .output()
        .expect("run artifact");
    assert!(result.status.success(), "{:?}", result);
    assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "7");
}

/// Sources already accepted by the compiler must survive serializer window boundaries.
#[test]
fn command_serializes_large_forests_and_text_through_keleusma() {
    let dir = TmpDir::new("serializer_capacity");
    let mut forest = String::new();
    for i in 0..171 {
        forest.push_str(&format!(
            "fn f{i}(x: Word) -> Word {{ x + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 }}\n"
        ));
    }
    forest.push_str("fn main() -> Word { f0(0) }");
    let text = format!("fn main() -> Text {{ \"{}\" }}", "x".repeat(60000));
    for (tag, source) in [("forest", forest), ("text", text)] {
        let input = dir.write(&format!("{tag}.kel"), &source);
        let output = dir.path(&format!("{tag}.kbc"));
        let (ok, error) = compile_self_hosted(&input, &output);
        assert!(ok, "{tag}: {error}");
        let module =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
                .unwrap();
        let actual = std::fs::read(output).unwrap();
        assert_eq!(
            actual,
            keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap()
        );
        assert_eq!(actual, module.to_bytes().unwrap());
    }
}
