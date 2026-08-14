//! Differential tests for `src/selfhost/kel/verify_types.kel`, the self-hosted
//! type-REJECTION stage.
//!
//! # Slice 0: the harness, before any rule
//!
//! The stage accepts everything, and that is the point rather than a
//! placeholder. The corpus must then show **every well-typed case agreeing and
//! every ill-typed case DISAGREEING**. A harness that reports success here is
//! broken, and finding that out costs nothing before a rule exists and a great
//! deal afterwards.
//!
//! # The oracle
//!
//! **Verdict agreement. Accept versus reject.** Not message agreement, which
//! would commit the stage to reproducing English the reference is free to
//! reword. This is what the `verify_*.kel` family already uses.
//!
//! # The direction that is not symmetric
//!
//! `verify_structural` and friends may over-approximate and defer to a runtime
//! guard. A type checker may not: **rejecting a valid program is a language
//! change**, not a conservative choice. The well-typed side of this corpus
//! therefore grows with every slice rather than staying at the five controls the
//! sizing spike used, because the "must accept" obligation is unbounded by any
//! corpus while the "must reject" obligation is enumerable.
//!
//! # Why a corpus of rejections alone would be useless
//!
//! It cannot detect a checker that rejects everything, which would score
//! perfectly. The sizing spike recorded the converse mistake, made while
//! building it: a case labelled ill-typed that was in fact well-typed, reported
//! as "accepted but should not be". It did not mislead **only because explicit
//! well-typed controls existed to check it against**.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, required_persistent_capacity_for};

const TYPES_KEL: &str = include_str!("../src/selfhost/kel/verify_types.kel");

/// The fifteen rejection shapes, measured by execution rather than counted from
/// `TypeError` sites.
///
/// Reading gives 163 `TypeError::new` sites in `src/typecheck.rs`, about twenty
/// of which mention traits or bounds and are outside Order-1 scope. **The number
/// that matters is how many an ill-typed program in the SUBSET can reach**, and
/// that needed execution: eighteen ill-typed programs, seventeen rejected.
const ILL_TYPED: &[(&str, &str)] = &[
    ("add-word-and-bool", "fn main() -> Word { 1 + true }"),
    ("body-versus-return", "fn main() -> Word { true }"),
    (
        "wrong-argument-type",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(true) }",
    ),
    (
        "too-few-arguments",
        "fn f(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { f(1) }",
    ),
    (
        "too-many-arguments",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(1, 2) }",
    ),
    ("undefined-function", "fn main() -> Word { nope(1) }"),
    ("undefined-identifier", "fn main() -> Word { nope }"),
    (
        "if-branches-differ",
        "fn main() -> Word { if true { 1 } else { false } }",
    ),
    (
        "non-bool-condition",
        "fn main() -> Word { if 1 { 1 } else { 2 } }",
    ),
    (
        "unknown-field",
        "struct P { a: Word }\nfn main() -> Word { let p = P { a: 1 }; p.b }",
    ),
    (
        "wrong-field-count",
        "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1 }; p.a }",
    ),
    (
        "index-a-scalar",
        "fn main() -> Word { let x: Word = 1; x[0] }",
    ),
    (
        "field-access-on-a-scalar",
        "fn main() -> Word { let x: Word = 1; x.a }",
    ),
    (
        "byte-against-word-argument",
        "fn f(a: Byte) -> Byte { a }\nfn main() -> Word { f(1) as Word }",
    ),
    (
        "array-elements-differ",
        "fn main() -> Word { let a = [1, true]; 0 }",
    ),
    // THE ODD ONE OUT. A V0.2.0 surface restriction rather than a type error,
    // and the one rejection of the fifteen that carries no `type error:`
    // prefix. A stage locating rejections by that prefix would miss it, and a
    // stage reproducing the reference's routing would be reproducing English.
    (
        "calling-a-local",
        "fn g() -> Word { 1 }\nfn main() -> Word { let f = g; f() }",
    ),
];

/// The controls. **Without these the corpus cannot detect a checker that rejects
/// everything**, which would score perfectly against the table above.
///
/// They are deliberately varied rather than minimal: each reaches a construct
/// some ill-typed case is a near-miss of, so a rule written too broadly fails
/// here rather than passing quietly.
const WELL_TYPED: &[(&str, &str)] = &[
    ("scalar-arith", "fn main() -> Word { 1 + 2 * 3 }"),
    (
        "bool-condition",
        "fn main() -> Word { if true { 1 } else { 2 } }",
    ),
    (
        "matching-argument",
        "fn f(a: Word) -> Word { a }\nfn main() -> Word { f(1) }",
    ),
    (
        "struct-field-read",
        "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1, b: 2 }; p.a }",
    ),
    (
        "byte-to-byte",
        "fn f(a: Byte) -> Byte { a }\nfn main() -> Word { f(1 as Byte) as Word }",
    ),
    (
        "array-of-one-type",
        "fn main() -> Word { let a = [1, 2, 3]; a[0] }",
    ),
    (
        "shared-data-word-field",
        "shared data s { n: Word }\nfn main() -> Word { s.n }",
    ),
];

/// The reference verdict: does `compile` accept this source?
///
/// **This is the oracle, and it is deliberately the WHOLE pipeline** rather than
/// `typecheck::check` alone. "Calling a local" is rejected by the compiler as a
/// surface restriction rather than by the type-check pass, so an oracle narrowed
/// to the type checker would report that case as accepted and the corpus would
/// be wrong about its own contents.
fn reference_accepts(src: &str) -> bool {
    match tokenize(src) {
        Err(_) => false,
        Ok(toks) => match parse(&toks) {
            Err(_) => false,
            Ok(ast) => compile(&ast).is_ok(),
        },
    }
}

/// A literal's type tag, from its SYNTACTIC kind alone.
///
/// **Never from the reference's inference.** Marshalling inferred types would
/// make the stage agree with the reference by construction and prove nothing
/// about the stage. `1` is an integer because it is written as one.
fn literal_tag(l: &keleusma::ast::Literal) -> i64 {
    use keleusma::ast::Literal as L;
    match l {
        L::Int(_) => 1,
        L::Bool(_) => 2,
        L::Byte(_) => 3,
        L::Float(_) => 4,
        L::Fixed { .. } => 5,
        L::Unit => 6,
        _ => 0,
    }
}

/// The tag of an expression, or UNKNOWN.
///
/// Only literals are typed here. Anything else is 0, and an unknown operand is
/// **not** a rejection: this stage may not reject a valid program, so silence is
/// the only sound answer for something it cannot type.
fn expr_tag(e: &keleusma::ast::Expr) -> i64 {
    match e {
        keleusma::ast::Expr::Literal { value, .. } => literal_tag(value),
        _ => 0,
    }
}

/// Every place two values must agree in type, as (left tag, right tag).
///
/// Two sources at this slice: a binary operation's operands, and an array
/// literal's elements against its first.
fn operand_pairs(ast: &keleusma::ast::Program) -> Vec<(i64, i64)> {
    use keleusma::ast::Expr;
    use keleusma::visitor::Visitor;

    /// Collected through the crate's own `Visitor` rather than a hand-written
    /// recursion. **A hand-written walk is a by-name enumeration of the
    /// expression forms**, and it goes stale the moment a form is added --
    /// silently, because a missed form yields fewer pairs and therefore fewer
    /// rejections, which reads as the stage being permissive rather than as the
    /// collector being incomplete.
    struct Pairs(Vec<(i64, i64)>);
    impl Visitor for Pairs {
        fn visit_expr(&mut self, expr: &Expr) {
            match expr {
                Expr::BinOp { left, right, .. } => {
                    self.0.push((expr_tag(left), expr_tag(right)));
                }
                Expr::ArrayLiteral { elements, .. } => {
                    if let Some(first) = elements.first() {
                        let ft = expr_tag(first);
                        for e in elements.iter().skip(1) {
                            self.0.push((ft, expr_tag(e)));
                        }
                    }
                }
                _ => {}
            }
            self.walk_expr(expr);
        }
    }

    let mut p = Pairs(Vec::new());
    for f in &ast.functions {
        p.visit_block(&f.body);
    }
    p.0
}

/// A declared type's tag, from its SYNTACTIC spelling.
///
/// Deliberately parallel to `literal_tag`, and the two must agree on their
/// numbering or a correct argument would read as a mismatch. `Word` and an
/// integer literal are both 1 for that reason.
fn type_tag(t: &keleusma::ast::TypeExpr) -> i64 {
    use keleusma::ast::{PrimType, TypeExpr as T};
    match t {
        T::Prim(PrimType::Word, _) => 1,
        T::Prim(PrimType::Bool, _) => 2,
        T::Prim(PrimType::Byte, _) => 3,
        T::Prim(PrimType::Float, _) => 4,
        _ => 0,
    }
}

/// A program's call sites: arity rows and argument-type rows.
///
/// Named rather than returned as a tuple of two identical vector types, which
/// clippy rejects and which a reader would have to count parentheses to
/// disambiguate. Two `Vec<(i64, i64)>` in a row is exactly the shape where
/// swapping the halves compiles and is wrong.
type CallRows = (Vec<(i64, i64)>, Vec<(i64, i64)>);

/// Call sites paired against their declarations: (declared arity, actual
/// arity), plus an argument-type pair per positional argument.
///
/// **A call to a name with no declaration contributes nothing.** Undefined
/// functions are a later slice, and inventing a declared arity of zero for one
/// would reject it here for a reason this slice cannot defend.
fn call_rows(ast: &keleusma::ast::Program) -> CallRows {
    use keleusma::ast::Expr;
    use keleusma::visitor::Visitor;
    use std::collections::BTreeMap;

    let mut decls: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for f in &ast.functions {
        decls.insert(
            f.name.clone(),
            f.params
                .iter()
                .map(|p| p.type_expr.as_ref().map_or(0, type_tag))
                .collect(),
        );
    }

    struct Calls<'a> {
        decls: &'a BTreeMap<String, Vec<i64>>,
        arity: Vec<(i64, i64)>,
        args: Vec<(i64, i64)>,
    }
    impl Visitor for Calls<'_> {
        fn visit_expr(&mut self, expr: &Expr) {
            if let Expr::Call { name, args, .. } = expr
                && let Some(params) = self.decls.get(name)
            {
                self.arity.push((params.len() as i64, args.len() as i64));
                for (p, a) in params.iter().zip(args.iter()) {
                    self.args.push((*p, expr_tag(a)));
                }
            }
            self.walk_expr(expr);
        }
    }

    let mut c = Calls {
        decls: &decls,
        arity: Vec::new(),
        args: Vec::new(),
    };
    for f in &ast.functions {
        c.visit_block(&f.body);
    }
    (c.arity, c.args)
}

/// The stage's verdict for a program, with its operand pairs marshalled in.
fn stage_accepts_program(pairs: &[(i64, i64)]) -> bool {
    stage_verdict(pairs, &[])
}

/// The stage's verdict with both tables.
fn stage_verdict(pairs: &[(i64, i64)], arity: &[(i64, i64)]) -> bool {
    let module = compile(&parse(&tokenize(TYPES_KEL).expect("lex")).expect("parse"))
        .expect("verify_types.kel compiles");
    let need = required_persistent_capacity_for(&module);
    let arena = Box::leak(Box::new(Arena::with_capacity(
        DEFAULT_ARENA_CAPACITY + need,
    )));
    arena
        .resize_persistent(need)
        .expect("arena persistent region");
    let mut vm = Vm::new(module, arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    // Slot layout: cmd, verdict, n, then lhs[256], then rhs[256].
    const N_SLOT: usize = 2;
    const LHS_SLOT: usize = 3;
    const RHS_SLOT: usize = LHS_SLOT + 256;
    assert!(
        pairs.len() <= 256,
        "the operand table holds 256 rows and this program needs {}",
        pairs.len()
    );
    vm.set_shared(
        &mut shared,
        N_SLOT,
        keleusma::bytecode::Value::Int(pairs.len() as i64),
    )
    .expect("n");
    for (i, (l, r)) in pairs.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            LHS_SLOT + i,
            keleusma::bytecode::Value::Int(*l),
        )
        .expect("lhs");
        vm.set_shared(
            &mut shared,
            RHS_SLOT + i,
            keleusma::bytecode::Value::Int(*r),
        )
        .expect("rhs");
    }
    const CN_SLOT: usize = RHS_SLOT + 256;
    const CDECL_SLOT: usize = CN_SLOT + 1;
    const CACT_SLOT: usize = CDECL_SLOT + 128;
    assert!(
        arity.len() <= 128,
        "the call table holds 128 rows and this program needs {}",
        arity.len()
    );
    vm.set_shared(
        &mut shared,
        CN_SLOT,
        keleusma::bytecode::Value::Int(arity.len() as i64),
    )
    .expect("cn");
    for (i, (d, a)) in arity.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            CDECL_SLOT + i,
            keleusma::bytecode::Value::Int(*d),
        )
        .expect("cdecl");
        vm.set_shared(
            &mut shared,
            CACT_SLOT + i,
            keleusma::bytecode::Value::Int(*a),
        )
        .expect("cact");
    }
    let out = vm
        .call_with_shared(&mut shared, &[keleusma::bytecode::Value::Int(0)])
        .expect("run");
    match out {
        keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(v)) => v == 0,
        other => panic!("verify_types.kel returned {other:?}, not a finished Int verdict"),
    }
}

/// THE CORPUS IS CHECKED AGAINST THE REFERENCE BEFORE ANYTHING ELSE IS.
///
/// A case labelled ill-typed that the reference accepts is a badly-constructed
/// test, not a compiler defect, and the sizing spike made exactly that mistake
/// once. Catching it here means a later slice's failure is about the stage.
#[test]
fn the_corpus_labels_agree_with_the_reference() {
    for (label, src) in ILL_TYPED {
        assert!(
            !reference_accepts(src),
            "{label}: labelled ill-typed and the reference ACCEPTS it, so the corpus is wrong \
             about its own contents"
        );
    }
    for (label, src) in WELL_TYPED {
        assert!(
            reference_accepts(src),
            "{label}: labelled well-typed and the reference REJECTS it, so the corpus is wrong \
             about its own contents"
        );
    }

    // MUST-FIRE on the corpus being non-empty in both directions. A table
    // emptied by a later edit would leave every loop above vacuous.
    assert!(ILL_TYPED.len() >= 15, "the rejection corpus shrank");
    assert!(!WELL_TYPED.is_empty(), "the control corpus is empty");
}

/// SLICE 0: the harness discriminates, and the stage does not yet.
///
/// The stage accepts everything, so agreement must be **exactly** the well-typed
/// set. If this test ever reports full agreement, the harness is comparing
/// something other than what it claims.
#[test]
fn the_accepting_stage_agrees_on_every_control_and_on_no_rejection() {
    let stage = stage_accepts_program(&[]);
    assert!(
        stage,
        "the slice-0 stage rejected, and it is incapable of rejecting, so the harness is reading \
         something other than the stage's verdict"
    );

    let mut agreed = 0;
    let mut disagreed = 0;
    for (_, src) in WELL_TYPED {
        if reference_accepts(src) == stage {
            agreed += 1;
        }
    }
    for (_, src) in ILL_TYPED {
        if reference_accepts(src) != stage {
            disagreed += 1;
        }
    }

    assert_eq!(
        agreed,
        WELL_TYPED.len(),
        "an accepting stage must agree with the reference on every well-typed case"
    );
    assert_eq!(
        disagreed,
        ILL_TYPED.len(),
        "an accepting stage must DISAGREE on every ill-typed case. Full agreement here would mean \
         the harness is not comparing verdicts at all, which is the failure this slice exists to \
         rule out before any rule is written."
    );
}

/// SLICE 1: operand agreement, in verdict agreement with the reference.
///
/// The rule is the smallest real one: two operands that must agree in type and
/// do not. It reaches `1 + true` and `[1, true]`.
///
/// **The other fourteen shapes are still expected to disagree**, and this test
/// asserts that rather than tolerating it. A slice that quietly started
/// rejecting a shape it has no rule for would be rejecting for the wrong
/// reason, and the reason is the only thing distinguishing a checker from a
/// coin.
#[test]
fn slice_one_rejects_operand_disagreement_and_nothing_else() {
    // The shapes slice 1 is expected to catch.
    const IN_SCOPE: &[&str] = &["add-word-and-bool", "array-elements-differ"];

    let mut caught = 0;
    let mut out_of_scope_rejected = Vec::new();

    for (label, src) in ILL_TYPED {
        let ast = match tokenize(src).ok().and_then(|t| parse(&t).ok()) {
            Some(a) => a,
            // A source the parser refuses never reaches a type checker at all,
            // and the reference rejects it for that reason. Nothing for this
            // slice to say.
            None => continue,
        };
        let stage = stage_accepts_program(&operand_pairs(&ast));
        if IN_SCOPE.contains(label) {
            assert!(
                !stage,
                "{label}: slice 1 has a rule for this shape and accepted it"
            );
            caught += 1;
        } else if !stage {
            out_of_scope_rejected.push(*label);
        }
    }

    assert_eq!(
        caught,
        IN_SCOPE.len(),
        "not every in-scope shape was reached; the corpus and IN_SCOPE disagree"
    );
    assert!(
        out_of_scope_rejected.is_empty(),
        "slice 1 rejected shapes it has no rule for: {out_of_scope_rejected:?}. A rejection for \
         the wrong reason is indistinguishable from a correct one on this corpus, and the reason \
         is the only thing separating a checker from a coin."
    );

    // THE DIRECTION THAT IS NOT SYMMETRIC. Every well-typed control must still
    // be accepted: a false rejection is a language change, not a conservative
    // choice, so this side admits no over-approximation at all.
    for (label, src) in WELL_TYPED {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        assert!(
            stage_accepts_program(&operand_pairs(&ast)),
            "{label}: slice 1 REJECTED a well-typed program, which narrows the language"
        );
    }
}

/// SLICE 2: call arity and argument types.
///
/// Arity is **purely syntactic** -- it needs no types at all -- which is why
/// two of the fifteen shapes fall to one comparison, and why they are the same
/// comparison rather than two rules: too few and too many arguments are both
/// "declared does not equal actual".
///
/// The argument-type rows reuse slice 1's operand-pair channel, because
/// "these two must agree" is the same claim whether the two are a binary
/// operator's operands or a parameter and its argument.
#[test]
fn slice_two_rejects_arity_and_argument_type_mismatches() {
    const IN_SCOPE: &[&str] = &[
        "add-word-and-bool",
        "array-elements-differ",
        "too-few-arguments",
        "too-many-arguments",
        "wrong-argument-type",
        "byte-against-word-argument",
    ];

    let mut caught = Vec::new();
    let mut out_of_scope_rejected = Vec::new();

    for (label, src) in ILL_TYPED {
        let ast = match tokenize(src).ok().and_then(|t| parse(&t).ok()) {
            Some(a) => a,
            None => continue,
        };
        let (arity, arg_pairs) = call_rows(&ast);
        let mut pairs = operand_pairs(&ast);
        pairs.extend(arg_pairs);
        let stage = stage_verdict(&pairs, &arity);
        if IN_SCOPE.contains(label) {
            assert!(
                !stage,
                "{label}: slice 2 has a rule for this shape and accepted it"
            );
            caught.push(*label);
        } else if !stage {
            out_of_scope_rejected.push(*label);
        }
    }

    assert_eq!(
        caught.len(),
        IN_SCOPE.len(),
        "reached {caught:?}, expected {IN_SCOPE:?}"
    );
    assert!(
        out_of_scope_rejected.is_empty(),
        "slice 2 rejected shapes it has no rule for: {out_of_scope_rejected:?}"
    );

    // The direction that admits no over-approximation.
    for (label, src) in WELL_TYPED {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let (arity, arg_pairs) = call_rows(&ast);
        let mut pairs = operand_pairs(&ast);
        pairs.extend(arg_pairs);
        assert!(
            stage_verdict(&pairs, &arity),
            "{label}: slice 2 REJECTED a well-typed program, which narrows the language"
        );
    }
}
