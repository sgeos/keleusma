//! Forward data references remain outside the streaming parser's supported subset.
//!
//! The parser resolves `block.field` using data declarations already encountered.
//! A function reading a later block receives a structured unsupported-syntax
//! diagnostic before reconstruction. The reference accepts both declaration orders.
//!
//! `verify_types.kel` formerly hit this limit at `ty_direct`. Its solver storage
//! now precedes its first use, so the stage joins the byte-identity corpus without
//! requiring a two-pass parser. The mutation below restores the old ordering and
//! checks both the unchanged reference artifact and the pipeline refusal.
//!
//! The four-line witness remains useful because compiling the reordered stage
//! does not establish support for arbitrary forward data references.

#![cfg(all(feature = "self-host", feature = "compile"))]

/// A function reading a `data` block declared after it. Four lines, no control flow, no nesting.
const FORWARD: &str = "private data t { hit: Word }\n\
                       fn g() -> Word { t.hit = u.z; t.hit }\n\
                       private data u { z: Word }\n\
                       fn main() -> Word { g() }";

/// The same program with the two declarations swapped. **The only difference is order.**
const BACKWARD: &str = "private data t { hit: Word }\n\
                        private data u { z: Word }\n\
                        fn g() -> Word { t.hit = u.z; t.hit }\n\
                        fn main() -> Word { g() }";

const VERIFY_TYPES: &str = include_str!("../src/selfhost/kel/verify_types.kel");

fn assert_forward_refusal(src: &str) {
    use keleusma::selfhost::{
        SelfHostError, self_hosted_compile, try_call_type_inputs_from_pipeline,
    };
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let error =
        try_call_type_inputs_from_pipeline(src).expect_err("forward reference must be unavailable");
    assert!(
        !error.is_source_rejection(),
        "valid source must not become an invalid-source verdict"
    );
    assert!(
        matches!(error, ParseRecordError::Diagnostic { code, detail }
        if code == PARSE_DIAG_TAG_BASE - 21 && detail >= 0),
        "{error}"
    );
    assert!(matches!(
        self_hosted_compile(src, &keleusma::target::Target::host()),
        Err(SelfHostError::Unsupported { .. })
    ));
}

/// **THE REFERENCE ACCEPTS BOTH ORDERS.** Without this the pair below would be measuring the
/// language rather than the stage, which five probes on this line have done.
#[test]
fn the_reference_accepts_a_forward_data_reference() {
    for (label, src) in [("forward", FORWARD), ("backward", BACKWARD)] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex"))
            .unwrap_or_else(|e| panic!("{label}: the reference must parse it: {e:?}"));
        keleusma::compiler::compile(&ast)
            .unwrap_or_else(|e| panic!("{label}: the reference must compile it: {e:?}"));
    }
}

/// **DECLARATION ORDER IS THE WHOLE DIFFERENCE.**
///
/// The control is what makes this a finding rather than an anecdote: the identical program with
/// its two `data` declarations swapped compiles through the self-hosted pipeline without
/// complaint.
#[test]
fn a_data_block_referenced_before_its_declaration_is_refused() {
    keleusma::selfhost::self_hosted_compile(BACKWARD, &keleusma::target::Target::host())
        .expect("backward-ordered control must compile through the shipping entry");
    assert_forward_refusal(FORWARD);
}

/// Restoring the former declaration order preserves the reference artifact but
/// reintroduces the pipeline refusal. This checks the actual stage, including its
/// shared and private layouts, rather than inferring equivalence from a snippet.
#[test]
fn moving_solver_storage_after_first_use_reproduces_stage_refusal() {
    let source = code_only(VERIFY_TYPES);
    let start = source.find("private data tyb {").expect("solver storage");
    let end = start + source[start..].find("\n}").expect("solver storage end") + 2;
    let block = &source[start..end];
    let mut forward = source.clone();
    forward.replace_range(start..end, "");
    let after_use = forward.find("fn tyb_rounds(").expect("solver round bound");
    forward.insert_str(after_use, &format!("{block}\n\n"));

    let reference_bytes = |src: &str| {
        let ast =
            keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse");
        keleusma::compiler::compile(&ast)
            .expect("reference compilation")
            .to_bytes()
            .expect("reference artifact")
    };
    assert!(
        reference_bytes(VERIFY_TYPES) == reference_bytes(&forward),
        "moving solver storage changes the reference artifact"
    );
    assert_forward_refusal(&forward);
}

/// Every stage source has a named test in the byte-identity corpus.
#[test]
fn the_corpus_covers_every_stage() {
    const ORACLE: &str = include_str!("selfhost_codegen.rs");
    let stages = [
        "analyze",
        "codegen",
        "lexer",
        "parse",
        "reconstruct",
        "verify_datalayout",
        "verify_depth",
        "verify_structural",
        "verify_typed",
        "verify_types",
        "verify_yield",
        "wire",
        "wire_debug",
        "wire_schema",
    ];
    let missing: Vec<&str> = stages
        .iter()
        .copied()
        .filter(|s| !ORACLE.contains(&format!("fn self_host_compiles_{s}_kel_byte_identically")))
        .collect();
    assert!(
        missing.is_empty(),
        "stage sources missing from the byte-identity corpus: {missing:?}"
    );
}

/// Source with `//` line comments removed, so an anchor search matches CODE.
///
/// **Locating on RAW source is the defect this removes.** A comment naming the anchor sends the
/// extraction to the comment; measured elsewhere in this repository, one such line failed four
/// tests in a file with nothing wrong in the code it read.
///
/// Truncating at the first `//` rather than tracking string literals is correct for an anchor
/// search: an early truncation can only make the anchor go missing, which fails loudly through the
/// `expect` below. `tests/call_chunk_index_limit.rs` needs a string-aware strip instead, because its
/// assertion is an ABSENCE one where the same truncation lets a real offender pass silently.
/// **THE ORDERING HAZARD ABOVE, MADE CHECKABLE INSTEAD OF DESCRIBED.**
///
/// The comment beside the ordering assertion states that a comment naming either anchor can move a
/// position and make the test assert the WRONG THING about the stage rather than fail loudly. That
/// was a claim about the strip, and nothing checked it. A guard census on 2026-09-10 found this
/// file among those documenting a hazard while recording no demonstration that its own assertion
/// still discriminates.
///
/// Both directions are pinned. The first input is the one that matters: a historical note naming
/// the LATER declaration placed BEFORE the earlier one, which on raw source inverts the order the
/// assertion reads. The second confirms the strip has not been tightened into finding nothing,
/// which is the way an extraction can be made comment-proof by being made useless.
#[test]
fn a_comment_naming_an_anchor_cannot_invert_the_declaration_order() {
    const DECOY: &str = "// historical: `private data tyb {` used to be declared first\n\
                         fn ty_direct( a ) {\n  tyb.x\n}\n\
                         private data tyb {\n  x: Word\n}\n";
    let stripped = code_only(DECOY);
    let at_fn = stripped
        .find("fn ty_direct(")
        .expect("the strip removed the function declaration itself");
    let at_blk = stripped
        .find("private data tyb {")
        .expect("the strip removed the data block itself");
    assert!(
        at_fn < at_blk,
        "a COMMENT naming the later declaration was located as if it were the declaration, \
         inverting the order; the ordering assertion in this file would then report a structure \
         the stage does not have, and would report it as a pass"
    );

    const PLAIN: &str = "fn ty_direct( a ) {\n  tyb.x\n}\nprivate data tyb {\n  x: Word\n}\n";
    let plain = code_only(PLAIN);
    assert!(
        plain.contains("fn ty_direct(") && plain.contains("private data tyb {"),
        "the strip removed real code, which would make every locate in this file fail to find \
         its anchor and the file comment-proof by finding nothing at all"
    );
}

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
