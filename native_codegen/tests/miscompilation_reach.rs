//! **HOW MANY OF THE VIRTUAL MACHINE'S "SHOULD NEVER HAPPEN" REFUSALS ACTUALLY HAPPEN?**
//!
//! The virtual machine carries a small class of refusals whose message says the
//! opcode **should never have been emitted at all** — a mis-compilation rather
//! than a bad program. They raise `InvalidBytecode`, **which is the class
//! `verify()` exists to exclude at load time.**
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
//! One site is reachable and two were not reached. **"Not reached" is not
//! "unreachable"** — that is this line's own `Op::Reset` lesson and the `v0.2.3`
//! line adopted it for `IsStruct`. The search is recorded beside the result so a
//! negative reads as *"I looked at these"* rather than *"I looked"*.
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

/// **The class is COUNTED from the source, never transcribed.**
///
/// A hand-written "there are three" is exactly the figure that goes stale when a
/// fourth is added, which is the failure the instruction-set census exists to
/// prevent. The marker is the phrase these sites use to say the opcode should
/// not have been emitted.
#[test]
fn how_many_miscompilation_refusals_does_the_vm_carry() {
    let src = std::fs::read_to_string("../src/vm.rs").expect("read vm.rs");
    // **THE MARKER MUST BE A FORM PROSE CANNOT ACCIDENTALLY TAKE, and my first
    // version was not.** Filtering on the phrase alone matched a COMMENT line
    // -- "// bytes, but tuple length is a compile-time constant" -- and reported
    // FOUR sites while printing three messages.
    //
    // **That is the third instance of this exact defect on this line**, after a
    // witness-claim extractor that matched the prose header "WHAT IT WITNESSES:"
    // and an indexical ownership phrase that inverted when read from the other
    // document. The `v0.2.3` line reports two more of its own. A marker written
    // in a form prose can take will eventually match prose.
    //
    // The refusal MESSAGE is a quoted string, so the line must start with a
    // quote. A comment cannot.
    let sites: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('"') && l.contains("is a compile-time constant"))
        .collect();

    println!("\n================ MIS-COMPILATION REFUSALS");
    println!(
        "  sites raising InvalidBytecode as a mis-compilation: {}",
        sites.len()
    );
    for s in &sites {
        println!("     {s}");
    }
    println!(
        "\n  These are the class `verify()` EXISTS TO EXCLUDE. A program that\n  \
         verifies, takes a bound, loads and then dies at one of them is a\n  \
         LOAD-TIME HOLE rather than a bad program."
    );
    println!("================\n");

    assert!(
        !sites.is_empty(),
        "no mis-compilation refusal was found in vm.rs, so the extraction is \
         broken and every verdict in this file is an artefact of it"
    );
    // A FLOOR, not a pin: a fourth site is news, not a failure. What must not
    // happen is the extraction silently finding none.
    assert!(
        sites.len() >= 3,
        "only {} mis-compilation sites extracted; three were present when this \
         file was written, so the marker phrase has probably changed",
        sites.len()
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
