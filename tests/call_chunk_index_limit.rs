//! **A CALL TO A CHUNK AT INDEX 256 OR ABOVE USED TO BREAK THE RECORD STREAM.**
//!
//! # The mechanism, located exactly, and now repaired
//!
//! A `Call` record's argument word packs two fields into one: `chunk + count * RADIX`. The
//! radix was **256**, so a chunk index of 256 carried into the count field: the callee
//! became chunk **zero** and the argument count became **one too many**, and the call popped
//! an operand that was never pushed.
//!
//! The radix is now [`keleusma::selfhost_host::CALL_CHUNK_RADIX`], which **equals the chunk
//! capacity**. That is the point of the repair rather than an incidental choice: the
//! chunk-cap guard becomes the single authority on the bound, so no range overflows
//! silently. A larger radix would have left an unguarded span and recreated this defect one
//! power of two higher.
//!
//! # Why this was worth isolating rather than inferring
//!
//! The symptom is an empty-stack pop reported against the **caller**, and the caller may sit
//! more than a thousand lines away from anything that changed. Chunk indices are assigned by
//! **sorted name**, so adding one declaration anywhere alphabetically earlier shifts a whole
//! block of indices by one and can push a callee across the boundary. That is how a
//! declaration late in a file changes the compilation of a function near its start.
//!
//! Two causes were published for this failure before this one and **both were wrong** — a
//! capacity bound read off the `1024` in an index message, and a cap of 256 on the
//! *declaration count*. The second is close enough to be instructive: the number 256 was
//! right, the quantity it applied to was not. A synthetic program of 300 chunks compiles
//! fine when its callee sorts low, which is what refuted it.
//!
//! # Proportionality
//!
//! `self_hosted_compile` cross-checks against the reference and refuses on divergence, so a
//! command-line user gets a loud error rather than a wrong artifact. Direct callers of
//! `self_host_compile` get the refusal from the named reconstruct cause. Before that cause
//! was named this presented as `IndexOutOfBounds(-1, 1024)`, which reads as a capacity bound
//! and is not one.

#![cfg(all(feature = "self-host", feature = "compile"))]

/// `n` chunks whose sort order is their declaration order, where the chunk at sorted index
/// `target` takes one argument and is called by a final chunk.
///
/// Zero-padded names keep sorted order equal to declaration order; without that the target's
/// index would not be the number in its name and this would test something else.
fn program(n: usize, target: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        if i == target {
            s.push_str(&format!("fn c{i:04}(a: Word) -> Word {{ a }}\n"));
        } else {
            s.push_str(&format!("fn c{i:04}() -> Word {{ {i} }}\n"));
        }
    }
    s.push_str(&format!("fn zmain() -> Word {{ c{target:04}(7) }}\n"));
    s
}

/// A call across the OLD boundary now compiles, and compiles CORRECTLY.
///
/// **Correctness is established against the reference, not by the absence of a panic.** The
/// defect produced a wrong callee as well as a wrong operand count, so a program that merely
/// fails to crash does not distinguish a repair from a different accident.
///
/// Re-aimed from the pin that recorded the old 256 boundary in the failing direction.
///
/// **ONE COMPILE PER SIDE.** An earlier draft called the self-hosted compiler twice per case
/// -- once to check it did not panic and once to compare bytes -- and the run was killed
/// three times before that was noticed. A whole-program compile here costs a minute or more.
#[test]
fn a_call_across_the_old_boundary_compiles_and_matches_the_reference() {
    // 258 chunks is the least that can place a callee at index 256, and each case compiles
    // a whole program through both compilers.
    for target in [255usize, 256] {
        let src = program(258, target);
        let leaked: &'static str = Box::leak(src.into_boxed_str());

        let reference = keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(leaked).expect("lex"))
                .expect("parse"),
        )
        .expect("reference compile");
        assert_eq!(
            reference
                .chunks
                .iter()
                .position(|c| c.name == format!("c{target:04}")),
            Some(target),
            "the subject is not at the index it claims, so this tests something else"
        );

        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let mine = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(leaked));
        std::panic::set_hook(prev);
        let mine = mine.unwrap_or_else(|e| {
            let m = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            panic!("a call to the chunk at index {target} still fails: {m}")
        });

        assert_eq!(
            keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
            keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
            "a call to the chunk at index {target} compiles but does not match the reference, \
             which is the wrong-callee half of the old defect surviving the repair"
        );
    }
}

/// The reference accepts both programs, so the refusal is the stage's and not the language's.
///
/// Without this, a program the reference also rejects would satisfy the test above and look
/// like a fact about chunk indices.
#[test]
fn the_reference_compiles_both_sides_of_the_boundary() {
    for target in [255usize, 256] {
        let src = program(260, target);
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex"))
            .expect("the reference must parse a probe before it says anything about the stage");
        keleusma::compiler::compile(&ast).expect("the reference must compile it too");
    }
}

/// **EVERY SITE IN THE FAMILY AGREES ON THE RADIX, AND THE FAMILY IS DERIVED.**
///
/// A cap is a family. Widening one member and leaving another moves the wall rather than
/// removing it, and worse here: a packer and an unpacker disagreeing would mis-read every
/// call in every program, silently.
///
/// **I DERIVED THE FAMILY BY HAND FIRST AND GOT THREE OF FOUR.** The missed one was a
/// fourth implementation of the packing in `tests/selfhost_parse.rs`, which builds the
/// expected record stream to compare against the stage. Eighth recorded instance of deriving
/// a set from the part of the system in view rather than from the system, so this walks the
/// tree instead of naming files.
///
/// The driver copy matters especially: five defects with one cause came from the shipping
/// driver and that copy diverging, while the boundary exercised only the copy.
#[test]
fn every_site_in_the_call_packing_family_agrees_on_the_radix() {
    let radix = keleusma::selfhost_host::CALL_CHUNK_RADIX;
    assert_eq!(
        radix,
        keleusma::selfhost_host::PARSE_CHUNK_CAP,
        "the radix no longer equals the chunk capacity, so a span of chunk indices overflows \
         the field with no guard covering it -- the exact defect this repaired"
    );

    // Walk the tree rather than listing the sites. `65536` is codegen's BYTECODE operand
    // packing, a different encoding that legitimately uses a different radix.
    let mut offenders: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for dir in ["src", "tests", "src/selfhost/kel"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let Some(ext) = path.extension().and_then(|x| x.to_str()) else {
                continue;
            };
            if ext != "rs" && ext != "kel" {
                continue;
            }
            // SKIP THIS FILE. Its pattern list is literally the thing it searches for, so
            // on first run it flagged itself -- the third time a guard in this repository has
            // done that. Excluding it costs nothing: a test file has no business packing a
            // Call record, and the four real implementations all live elsewhere.
            if path.file_name().and_then(|x| x.to_str()) == Some("call_chunk_index_limit.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            scanned += 1;
            for (i, line) in text.lines().enumerate() {
                if line.contains("65536") {
                    continue;
                }
                let hit = line.contains("count * 256")
                    || line.contains("count*256")
                    || line.contains("a % 256")
                    || line.contains("a / 256")
                    || line.contains("arg.rem_euclid(256)")
                    || line.contains("arg.div_euclid(256)");
                if hit {
                    offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }

    // NON-VACUOUS. Two derivations in this repository passed while finding nothing.
    assert!(
        scanned > 50,
        "the walk read only {scanned} files, so it is not seeing the tree it claims to check"
    );
    assert!(
        offenders.is_empty(),
        "these sites still split a Call record on the old eight-bit radix:\n  {}",
        offenders.join("\n  ")
    );

    // And the two stages agree with the driver on the number itself.
    const PARSE: &str = include_str!("../src/selfhost/kel/parse.kel");
    const RECONSTRUCT: &str = include_str!("../src/selfhost/kel/reconstruct.kel");
    assert!(
        PARSE.contains(&format!("fn call_chunk_radix() -> Word {{ {radix} }}")),
        "`parse.kel` packs at a different radix than the driver believes"
    );
    assert!(
        RECONSTRUCT.contains(&format!("fn rc_call_chunk_radix() -> Word {{ {radix} }}")),
        "`reconstruct.kel` unpacks at a different radix than `parse.kel` packs"
    );
}

/// The widest word the encoding can emit fits the declared minimum word width.
///
/// **Stated rather than assumed.** Both stages declare `require word >= 32`; a radix chosen
/// without this arithmetic could overflow the record word instead of the chunk field, which
/// would look like a different defect entirely.
#[test]
fn the_widened_record_word_fits_the_minimum_word_width() {
    let radix = keleusma::selfhost_host::CALL_CHUNK_RADIX as i64;
    // `reconstruct.kel` bounds a call's argument loop at 64.
    let widest = 7 + ((radix - 1) + 64 * radix) * 64;
    assert!(
        widest < i32::MAX as i64,
        "the widest emitted Call word is {widest}, which does not fit the 32-bit minimum \
         both stages require"
    );
}

/// Chunk indices are assigned by SORTED name, which is why a distant declaration matters.
///
/// This is the half of the explanation that is easiest to disbelieve, so it is measured
/// rather than asserted in prose.
#[test]
fn chunk_indices_follow_sorted_name_not_declaration_order() {
    const SRC: &str = "fn zzz() -> Word { 1 }\nfn aaa() -> Word { 2 }\nfn mmm() -> Word { 3 }\n";
    let names = keleusma::selfhost::chunk_names_from_pipeline(SRC);
    let pos = |n: &str| names.iter().position(|s| s == n).expect("name present");
    assert!(
        pos("aaa") < pos("mmm") && pos("mmm") < pos("zzz"),
        "chunk numbering is no longer by sorted name, so the mechanism recorded in this file \
         needs re-deriving. Order was: {names:?}"
    );
}
