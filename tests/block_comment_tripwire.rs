//! No source that a comment-stripping guard reads may contain a BLOCK comment.
//!
//! # Why a tripwire rather than nine parsers
//!
//! `docs/decisions/COMMENT_MATCHING_GUARD_SWEEP.md` records nine guards that strip `//` line
//! comments so their searches match code rather than prose. Every one of them shares a limitation:
//! **none handles `/* … */`**, which needs cross-line state a per-line walk does not carry.
//!
//! **Measured before deciding what to do about it: the exposure today is ZERO.** No source any of
//! those guards reads contains a real block comment. The only `/*` in the stage sources sits inside
//! a LINE comment describing the `+`, `-` and `*` operators, and every occurrence in `src/*.rs` is
//! inside a doc comment or a test string literal.
//!
//! **Keleusma does support block comments**, though — `src/lexer.rs` skips them and has tests for
//! the multi-line and unterminated cases — so a stage source could gain one, and so could a Rust
//! file. The risk is real and latent rather than absent.
//!
//! Teaching nine helpers cross-line comment state is real complexity bought for no current
//! exposure. **This converts the latent risk into a tripwire**: if a block comment ever appears in
//! one of these files, this fails and names the document that says what to do. That is cheaper than
//! nine parsers and, unlike a note in a document, it cannot be forgotten.
//!
//! # What this does NOT claim
//!
//! It does not claim the strips are correct. It claims the input they are known not to handle is
//! not present. A guard reading a file this list omits is outside the check, which is why the list
//! is derived from the sweep document rather than from memory.

use std::path::{Path, PathBuf};

/// The sources read by the guards the sweep document lists.
const SCANNED: &[&str] = &[
    "src/bytecode.rs",
    "src/selfhost/mod.rs",
    "src/selfhost/kel/parse.kel",
    "src/selfhost/kel/codegen.kel",
    "src/selfhost/kel/verify_types.kel",
    "tests/selfhost_codegen.rs",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Whether a line opens a block comment in CODE.
///
/// **A `/*` inside a line comment or a string literal is not one**, and both occur in this tree:
/// `parse.kel` describes the `+`, `-` and `*` operators in prose, and `src/lexer.rs` carries block
/// comments inside test string literals. A detector that flagged either would fire on the very
/// thing the sweep document says is harmless, which is the too-loose direction this whole class is
/// about.
fn opens_block_comment(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '/' && i + 1 < chars.len() {
            match chars[i + 1] {
                // A line comment runs to end of line; nothing after it is code.
                '/' => return false,
                '*' => return true,
                _ => {}
            }
        }
        i += 1;
    }
    false
}

#[test]
fn no_source_a_stripping_guard_reads_carries_a_block_comment() {
    let root = root();
    let mut scanned = 0usize;
    let mut offenders = Vec::new();

    for rel in SCANNED {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("the sweep names `{rel}`, which could not be read: {e}"));
        scanned += 1;
        for (i, line) in text.lines().enumerate() {
            if opens_block_comment(line) {
                offenders.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }

    // **NON-VACUITY.** A path list that stopped resolving would make this pass while reading
    // nothing, which is the failure the sweep document exists to catalogue.
    assert_eq!(
        scanned,
        SCANNED.len(),
        "only {scanned} of {} listed sources were read",
        SCANNED.len()
    );

    assert!(
        offenders.is_empty(),
        "a BLOCK comment appeared in a source that a comment-stripping guard reads. Those guards \
         strip `//` line comments only, so the text inside this block is now searched as if it \
         were code, and the guard reading it may match prose or miss a real occurrence depending \
         on which direction its assertion runs. See \
         docs/decisions/COMMENT_MATCHING_GUARD_SWEEP.md for which guard reads which file and what \
         each one's failure direction costs:\n  {}",
        offenders.join("\n  ")
    );
}

/// The detector must not fire on the two shapes this tree actually contains.
///
/// Without this, tightening the detector until it stops reporting anything would look like a fix.
#[test]
fn a_block_comment_marker_inside_prose_or_a_string_is_not_a_block_comment() {
    assert!(
        !opens_block_comment("    // An ARITHMETIC operator (+/-/*) on two BYTE operands"),
        "a `/*` inside a LINE comment was read as opening a block comment; `parse.kel` contains \
         exactly this line and the check would fire on a file with nothing wrong in it"
    );
    assert!(
        !opens_block_comment(r#"        let result = kinds("foo /* block comment */ bar");"#),
        "a `/*` inside a STRING LITERAL was read as opening a block comment; `src/lexer.rs` \
         contains exactly this shape in its own tests"
    );
    assert!(
        opens_block_comment("let x = 1; /* a real block comment */"),
        "a real block comment in code was not detected, so this check cannot fire at all"
    );
    assert!(
        opens_block_comment("/* opening a multi-line block"),
        "an unterminated block-comment opener was not detected"
    );
}
