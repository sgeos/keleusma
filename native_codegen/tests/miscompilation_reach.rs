//! **HOW MANY OF THE VIRTUAL MACHINE'S "SHOULD NEVER HAPPEN" REFUSALS ACTUALLY HAPPEN?**
//!
//! The virtual machine carries a class of refusals whose comment says the
//! artefact is **corrupt or mis-compiled** rather than a bad program. They raise
//! `InvalidBytecode`, **which is the class `verify()` exists to exclude at load
//! time.** Seven such sites, out of 39 distinct invalid-bytecode messages.
//!
//! A program that compiles, passes `verify()`, receives a resource bound, loads,
//! and *then* dies at one of these sites is a **load-time hole**: the verifier
//! admitted something it is supposed to reject. That is the conservative-
//! verification stance failing in the one direction it claims not to.
//!
//! # Reachable means the WHOLE chain
//!
//! **Not merely "a program emits the opcode".** The `Op::Len` witness emits it
//! and is REFUSED A BOUND — the loop has no statically extractable iteration
//! count — so it never loads and never reaches the machine at all. **That is the
//! stance working, not a hole**, and reporting it as reachability would invert
//! the finding.
//!
//! So: compiles, verifies, is granted a bound, loads, and execution arrives.
//!
//! # What this file establishes, and what it does not
//!
//! **The reachability results are the measured part and they survive a
//! correction to the count.** `Op::IsStruct` is reachable through the whole
//! chain; the two `Op::Len` sites were not reached, and the mechanism is that
//! the property reaching the opcode is the property denying the bound.
//!
//! **The COUNT was wrong and is corrected here.** This file first reported three
//! mis-compilation sites; there are seven. See
//! `the_full_invalid_bytecode_surface_and_the_miscompilation_reading`, which
//! records what the earlier extraction missed and why. **A measured reachability
//! result survives a bad denominator; a denominator asserted from a grep does
//! not survive contact with a synonym.**
//!
//! **The four newly-counted sites now carry a bounded SEARCH**, not the guess
//! that stood there before. See
//! `what_was_tried_against_the_form_mismatch_sites`: five compiling probes, zero
//! mismatches, and the shape most able to decide the form twice is rejected
//! after monomorphization. Still not "unreachable".
//!
//! **"Not reached" is not "unreachable"** — this line's own `Op::Reset` lesson.
//! The search is recorded beside the result so a negative reads as *"I looked at
//! these"* rather than *"I looked"*.
//!
//! # `src/vm.rs` is the `v0.2.3` line's
//!
//! Read here, never written. Any repair implied by a finding is theirs.
use keleusma::bytecode::Module;
use keleusma::vm::{auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// How far a probe got along the chain.
#[derive(Debug, PartialEq)]
enum Reach {
    NoCompile,
    NoVerify,
    NoBound,
    NoLoad,
    Ran(String),
}

fn chain(src: &str) -> Reach {
    let Some(m) = tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
    else {
        return Reach::NoCompile;
    };
    if keleusma::verify::verify(&m).is_err() {
        return Reach::NoVerify;
    }
    let Ok(cap) = auto_arena_capacity_for(&m, &[]) else {
        return Reach::NoBound;
    };
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (4 << 20));
    arena.resize_persistent(need).expect("persistent fits");
    let Ok(mut vm) = keleusma::vm::Vm::new(m, &arena) else {
        return Reach::NoLoad;
    };
    let mut shared: Vec<u8> = Vec::new();
    Reach::Ran(format!("{:?}", vm.call_with_shared(&mut shared, &[])))
}

fn built(src: &str) -> Option<Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// **THE FULL INVALID-BYTECODE SURFACE, MEASURED — AND A COUNT OF MINE THAT WAS
/// WRONG BY MORE THAN HALF.**
///
/// # The correction, stated before the result
///
/// This file previously reported **three** mis-compilation sites, extracted by
/// filtering messages on `is a compile-time constant`. **There are seven.** The
/// four missed ones say the same thing in different words:
///
/// ```text
/// GetField      operand form does not match struct body
/// GetIndex      operand form does not match array body
/// GetTupleField operand form does not match tuple body
/// GetEnumField  operand form does not match enum body
/// ```
///
/// Their comment reads *"a form mismatch is a corrupted or **mis-compiled**
/// artefact rather than a script error"* — the same class, plainly stated.
///
/// **I missed them by one word stem.** My search for the concept used
/// `mis-compilation`; those sites say `mis-compiled`. And the commit that
/// introduced the three-site count is the same commit that recorded the rule *"a
/// machine-checked marker written in a form prose can take will eventually match
/// prose"*. I fixed the SYNTACTIC half — requiring the marker at line start so a
/// comment could not impersonate a message — and never considered that a marker
/// can also be **too tight**. Fifth instance of that family, and the first where
/// the rule and its violation are in one file.
///
/// # So this now separates what is MEASURED from what is EDITORIAL
///
/// **Measured**: every distinct message raised as `InvalidBytecode`, extracted
/// from source. This needs no classification and cannot be wrong by a synonym.
///
/// **Editorial**: which of them are "the compiler should not have emitted this"
/// rather than "this artefact is corrupt". That is a READING. The stem is shown
/// and the full list is printed so the reading can be audited instead of
/// trusted — replacing one hand-chosen phrase with a better one would repeat the
/// method that just failed.
#[test]
fn the_full_invalid_bytecode_surface_and_the_miscompilation_reading() {
    let src = std::fs::read_to_string("../src/vm.rs").expect("read vm.rs");

    // MEASURED. Every raise site's message, however phrased.
    let mut messages: Vec<String> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        if !line.contains("VmError::InvalidBytecode(") {
            continue;
        }
        // The message may sit on this line or the next two.
        let window = src.lines().skip(i).take(3).collect::<Vec<_>>().join(" ");
        if let Some(start) = window.find('"') {
            if let Some(len) = window[start + 1..].find('"') {
                messages.push(window[start + 1..start + 1 + len].to_string());
            }
        }
    }
    messages.sort();
    messages.dedup();

    // EDITORIAL. The stem catches both spellings; it is still a reading.
    let flagged: Vec<&str> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("mis-compil"))
        .map(|(_, l)| l.trim())
        .collect();

    println!("\n================ INVALID-BYTECODE SURFACE");
    println!("  distinct messages (MEASURED): {}", messages.len());
    for m in &messages {
        println!("     {m}");
    }
    println!(
        "\n  lines mentioning mis-compilation (EDITORIAL READING): {}",
        flagged.len()
    );
    println!(
        "\n  THE SECOND FIGURE IS A READING, NOT A MEASUREMENT. It depends on a\n  \
         word stem appearing in a comment. A previous version of this test used\n  \
         `mis-compilation` and MISSED FOUR SITES that say `mis-compiled` --\n  \
         reporting three where there are seven. The full message list above is\n  \
         printed so the classification can be audited rather than trusted."
    );
    println!("================\n");

    assert!(
        !messages.is_empty(),
        "no InvalidBytecode message was extracted, so the walk is broken and \
         every figure here is an artefact of it"
    );
    assert!(
        messages.len() >= 30,
        "only {} distinct messages extracted; the surface was 38 when this was \
         written, so the extraction has probably regressed",
        messages.len()
    );
    assert!(
        flagged.len() >= 7,
        "only {} mis-compilation mentions found; SEVEN were present when this \
         was corrected, and an earlier version of this very test reported THREE \
         by searching one spelling. If the count fell, check for a THIRD wording \
         before believing it",
        flagged.len()
    );
}

/// **REACHABLE: `Op::IsStruct` on a flat struct.**
///
/// A GENERIC struct destructured in a parameter — ordinary code — compiles,
/// verifies, takes a bound, loads, and dies with `InvalidBytecode`. **The whole
/// chain, not merely emission.**
///
/// The `v0.2.3` line repaired the un-annotated case in the compiler and recorded
/// the opcode as having no producer. This is the counter-example: their repair
/// covers exactly the case they tested, and the control below returns `Int(3)`.
#[test]
fn the_is_struct_miscompilation_site_is_reachable() {
    const GENERIC: &str = "struct P<T> { a: T, b: T }\n\
                           fn g(P { a, b }: P<Word>) -> Word { a + b }\n\
                           fn main() -> Word { g(P { a: 1, b: 2 }) }";
    let m = built(GENERIC).expect("the generic witness must compile");
    assert!(
        m.chunks.iter().any(|c| c
            .ops
            .iter()
            .any(|o| format!("{o:?}").starts_with("IsStruct"))),
        "the generic construct no longer emits Op::IsStruct. If the fold was \
         extended to cover generics, THAT IS NEWS and the reachability verdict \
         in this file needs re-measuring -- do not delete this."
    );
    match chain(GENERIC) {
        Reach::Ran(outcome) => assert!(
            outcome.contains("InvalidBytecode"),
            "the generic witness RAN without hitting the mis-compilation site: \
             {outcome}. That would mean the load-time hole is closed, which is \
             good news and needs the verdict rewritten rather than deleted"
        ),
        other => panic!(
            "the generic witness no longer reaches the virtual machine at all \
             ({other:?}), so this file can no longer demonstrate the site is \
             reachable. Establish WHY before concluding anything."
        ),
    }
}

/// **The control that makes the verdict above mean something.** The
/// non-generic form is exactly what the `v0.2.3` fold repaired, and it runs.
#[test]
fn the_non_generic_form_is_repaired_and_runs() {
    const PLAIN: &str = "struct P { a: Word, b: Word }\n\
                         fn g(P { a, b }: P) -> Word { a + b }\n\
                         fn main() -> Word { g(P { a: 1, b: 2 }) }";
    let m = built(PLAIN).expect("compiles");
    assert!(
        !m.chunks.iter().any(|c| c
            .ops
            .iter()
            .any(|o| format!("{o:?}").starts_with("IsStruct"))),
        "the ANNOTATED form emits Op::IsStruct, so the generic verdict above is \
         not about generics at all and this file is measuring the wrong thing"
    );
}

/// **NOT REACHED: `Op::Len` on a flat array — and the mechanism is the bound.**
///
/// The only construct known to emit `Op::Len` from the for-in site is an `if`
/// EXPRESSION as the source. **It is refused a resource bound**, so it never
/// loads. The property that reaches the opcode is the property that denies the
/// bound: `Op::Len` fires when the source length is not statically known, and a
/// loop whose trip count is not statically known is what the extractor refuses.
///
/// **This is the stance working, not a hole**, and it is recorded as a mechanism
/// rather than as an outcome.
#[test]
fn the_len_array_site_is_not_reached_because_the_bound_is_refused() {
    const IF_SOURCE: &str = "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
                             for x in if c { a } else { b } { let _d = x; } 0 }\n\
                             fn main() -> Word { f(true) }";
    let m = built(IF_SOURCE).expect("compiles");
    assert!(
        m.chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with("Len"))),
        "the construct no longer emits Op::Len, so this verdict is about nothing"
    );
    assert_eq!(
        chain(IF_SOURCE),
        Reach::NoBound,
        "the Op::Len witness is now GRANTED a bound. If it also reaches the \
         virtual machine, the flat-array mis-compilation site has become \
         reachable and this file's verdict is wrong -- re-measure it."
    );
}

/// **NOT REACHED: `Op::Len` on a flat tuple, and the mechanism is a TYPE ERROR.**
///
/// `Op::Len` is emitted from two sites: the for-in loop bound, and the
/// bounds-check synthesis. **Neither can be fed a tuple.** for-in over a tuple is
/// rejected outright — *"for-in expects an array"* — and the bounds check indexes
/// arrays.
///
/// **The second emission site was found by grepping the emission points**, not by
/// guessing constructs; the `v0.2.3` line's account named only the for-in guard.
#[test]
fn for_in_over_a_tuple_is_a_type_error_so_the_tuple_site_has_no_feeder() {
    const TUPLE_SOURCE: &str = "fn f(c: bool) -> Word { let a = (1, 2); let b = (3, 4); \
                                for x in if c { a } else { b } { let _d = x; } 0 }\n\
                                fn main() -> Word { f(true) }";
    assert_eq!(
        chain(TUPLE_SOURCE),
        Reach::NoCompile,
        "for-in over a tuple now COMPILES. That is the feeder the flat-tuple \
         mis-compilation site never had -- re-measure whether the site is now \
         reachable rather than assuming it is not."
    );
}

/// **THE SEARCH, RECORDED BESIDE THE RESULT.**
///
/// A negative without its search reads as *"I looked"* when it means *"I looked
/// at these"*. Seven compiling probes; the eighth was a type error and is
/// reported as one rather than counted as a miss.
#[test]
fn what_was_tried_against_the_len_sites() {
    let probes: &[(&str, &str)] = &[
        (
            "for-in over an if-expression",
            "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; for x in if c { a } else { b } { let _d = x; } 0 }\nfn main() -> Word { f(true) }",
        ),
        (
            "for-in over a match expression",
            "fn f(c: Word) -> Word { let a = [1, 2]; let b = [3, 4]; for x in match c { 0 => a, _ => b } { let _d = x; } 0 }\nfn main() -> Word { f(0) }",
        ),
        (
            "for-in over a call result",
            "fn mk() -> [Word; 2] { [1, 2] }\nfn f() -> Word { for x in mk() { let _d = x; } 0 }\nfn main() -> Word { f() }",
        ),
        (
            "index an if-expression array",
            "fn f(c: bool, i: Word) -> Word { let a = [1, 2]; let b = [3, 4]; (if c { a } else { b })[i] }\nfn main() -> Word { f(true, 0) }",
        ),
        (
            "index a call-result array",
            "fn mk() -> [Word; 2] { [1, 2] }\nfn f(i: Word) -> Word { mk()[i] }\nfn main() -> Word { f(0) }",
        ),
        (
            "index a match-expression array",
            "fn f(c: Word, i: Word) -> Word { let a = [1, 2]; let b = [3, 4]; (match c { 0 => a, _ => b })[i] }\nfn main() -> Word { f(0, 0) }",
        ),
        (
            "plain array for-in (control)",
            "fn f() -> Word { let a = [1, 2]; for x in a { let _d = x; } 0 }\nfn main() -> Word { f() }",
        ),
    ];

    println!("\n================ WHAT WAS TRIED AGAINST THE Op::Len SITES");
    let mut compiled = 0usize;
    let mut emitting = 0usize;
    let mut reached = 0usize;
    for (label, src) in probes {
        let Some(m) = built(src) else {
            println!("  {label:<32} DID NOT COMPILE (proves nothing)");
            continue;
        };
        compiled += 1;
        let emits = m
            .chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with("Len")));
        if emits {
            emitting += 1;
        }
        let r = chain(src);
        if matches!(&r, Reach::Ran(o) if o.contains("InvalidBytecode")) {
            reached += 1;
        }
        println!("  {label:<32} emits={emits:<5} {r:?}");
    }
    println!(
        "\n  compiling probes : {compiled} of {}\n  \
         emitting Op::Len : {emitting}\n  \
         REACHING the site: {reached}",
        probes.len()
    );
    println!(
        "\n  NOT REACHED IS NOT UNREACHABLE. Two emission sites were read out of\n  \
         the compiler rather than guessed at, and every construct tried is listed\n  \
         above so the negative can be audited instead of trusted."
    );
    println!("================\n");

    assert!(
        compiled >= 6,
        "only {compiled} probes compiled, so this search is too thin to record \
         as a bounded negative"
    );
    assert_eq!(
        reached, 0,
        "a probe REACHED the Op::Len mis-compilation site. That is a load-time \
         hole and a finding: report it rather than adjusting this count."
    );
}

/// **THE FOUR FORM-MISMATCH SITES: A BOUNDED SEARCH, REPLACING A GUESS.**
///
/// Correcting the class from three sites to seven added four that carried no
/// verdict, and what stood in for one was a sentence of mine: *"they require a
/// form mismatch between construction and access, which the compiler chooses by
/// static type — that suggests unreachability, and suggestion is not evidence."*
/// This replaces the suggestion with a search that can be audited.
///
/// # How the form is chosen, which is what the verdict rests on
///
/// **Construction** emits `NewComposite(Flat { .. })` when the composite is
/// flat-eligible. **Access** calls `struct_field_access(fc, type_name, field)`,
/// which looks up `type_info.struct_field_order` BY NAME and — critically —
/// **falls back to `StructField::Boxed` when that lookup MISSES.**
///
/// So the mismatch shape is concrete rather than hypothetical: construction bakes
/// `Flat`, access looks up a name that is not registered, falls back to `Boxed`,
/// and the runtime finds a boxed operand against a flat body.
///
/// **That is the `Op::IsStruct` defect one level down.** Its root cause was a
/// one-sided rewrite — a pattern's type rewritten to `P__Word` on specialization
/// while the pattern itself still said `P`. Anything that rewrites one side of a
/// name and not the other reopens this class, and generics are where a type is
/// decided twice.
///
/// # The result, and the mechanism behind it
///
/// **Five compiling probes, zero mismatches.** And the shape most likely to
/// decide the form twice — a GENERIC FUNCTION over a GENERIC STRUCT — is
/// **rejected after monomorphization** and never reaches code generation:
///
/// ```text
/// generic fn, scalar argument          COMPILES
/// generic fn, plain struct argument    COMPILES
/// generic fn over a GENERIC STRUCT     REJECTED "type error after monomorphization"
/// plain fn over a generic instance     COMPILES
/// ```
///
/// **NOT UNREACHABLE.** Five probes against one hypothesis, with the shape that
/// motivated the hypothesis rejected by a check rather than shown safe.
#[test]
fn what_was_tried_against_the_form_mismatch_sites() {
    let probes: &[(&str, &str)] = &[
        (
            "non-generic field access (control)",
            "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1, b: 2 }; p.a }",
        ),
        (
            "generic struct, direct access",
            "struct P<T> { a: T, b: T }\nfn main() -> Word { let p = P { a: 1, b: 2 }; p.a }",
        ),
        (
            "nested generic struct",
            "struct Q<T> { v: T }\nstruct P<T> { a: Q<T>, b: T }\nfn main() -> Word { let p = P { a: Q { v: 1 }, b: 2 }; p.b }",
        ),
        (
            "generic struct in an array",
            "struct P<T> { a: T }\nfn main() -> Word { let xs = [P { a: 1 }, P { a: 2 }]; xs[0].a }",
        ),
        ("tuple field", "fn main() -> Word { let t = (1, 2); t.0 }"),
        (
            "generic fn over a generic struct",
            "struct P<T> { a: T }\nfn get<T>(p: P<T>) -> T { p.a }\nfn main() -> Word { get(P { a: 1 }) }",
        ),
    ];

    println!("\n================ FORM-MISMATCH SEARCH");
    let mut compiled = 0usize;
    let mut mismatched = 0usize;
    for (label, src) in probes {
        let Some(m) = built(src) else {
            println!("  {label:<36} REJECTED (proves nothing about reachability)");
            continue;
        };
        compiled += 1;
        let ops: Vec<String> = m
            .chunks
            .iter()
            .flat_map(|c| c.ops.iter())
            .map(|o| format!("{o:?}"))
            .collect();
        let ctor_flat = ops.iter().any(|d| d.starts_with("NewComposite(Flat"));
        let get_boxed = ops.iter().any(|d| d.starts_with("GetField(Boxed"));
        // The mismatch signature: a flat construction with a boxed access.
        let suspect = ctor_flat && get_boxed;
        if suspect {
            mismatched += 1;
        }
        println!("  {label:<36} ctorFlat={ctor_flat:<5} getBoxed={get_boxed:<5} suspect={suspect}");
    }
    println!("\n  compiling probes : {compiled} of {}", probes.len());
    println!("  form mismatches  : {mismatched}");
    println!(
        "\n  THE ACCESS SIDE FALLS BACK TO `Boxed` WHEN ITS NAME LOOKUP MISSES,\n  \
         which is the concrete shape of this defect class and the same one-sided\n  \
         name rewrite that produced the `Op::IsStruct` hole. The shape most able\n  \
         to decide the form twice -- a GENERIC FUNCTION over a GENERIC STRUCT --\n  \
         is REJECTED after monomorphization and never reaches code generation.\n  \
         \n  \
         NOT UNREACHABLE. Five probes against one hypothesis."
    );
    println!("================\n");

    assert!(
        compiled >= 5,
        "only {compiled} probes compiled, too thin to record as a bounded search"
    );
    assert_eq!(
        mismatched, 0,
        "A PROBE CONSTRUCTS FLAT AND ACCESSES BOXED. That is the body-form \
         mis-compilation reaching a real program -- report it to the line that \
         owns the compiler before adjusting anything here."
    );
}
