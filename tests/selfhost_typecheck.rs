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
//! # WHAT DECIDES, WHAT IS SUPPLIED, AND WHAT NEITHER REACHES
//!
//! **The rejection RULES are complete.** All fifteen enumerated shapes plus the
//! `calling-a-local` surface restriction are rejected, over a twenty-case
//! ill-typed corpus with seven well-typed controls. The roadmap and the resume
//! channels carried "7 tests against ~15 shapes" for a while: **seven was the
//! TEST count and fifteen the SHAPE count, and they are not the same axis.**
//!
//! **The stage now RESOLVES as well as compares.** It receives `(name, tag)`
//! binding rows and operands marked as "this is name N", and joins one through
//! the other. Before that, every rule fired only where the operands were
//! literals, and since every corpus case placed them so, the limit was invisible.
//!
//! **THE LINE BETWEEN THE TWO SIDES, because it is the point.** The host may
//! report a syntactic fact: this parameter is declared `Word`; this `let` is
//! written `= true`; this one is written `= g()`; this operand IS the name `b`.
//! It may not report the conclusion that a given operand therefore has a given
//! type. That join is the stage's, and
//! `the_stage_and_not_the_host_resolves_an_operand` is what makes the claim
//! checkable: withhold the binding rows and the same program is ACCEPTED.
//!
//! **What is still NOT self-hosted is the extraction.** `stage_verdict` is fed by
//! `decl_call_rows`, `expression_nodes_resolvable`, `field_sets`,
//! `occurrence_rows` and `binding_rows` — Rust functions walking the REFERENCE
//! parser's AST. This slice moved the RESOLUTION into the stage; it did not move
//! the EXTRACTION. **No claim here should call the type checker self-hosted
//! without saying which half is meant.**
//!
//! # What these tests do NOT establish
//!
//! - **The corpus is a case list.** Twenty ill-typed programs are twenty
//!   programs. This project has four recorded instances of a suite whose
//!   coverage was a property of its cases rather than of the thing under test,
//!   and one of them was this corpus.
//! - **Tags reach only what the source declares or literally initialises**, plus
//!   one alias hop for a `let` bound to a call. An operand whose type is DERIVED
//!   from a FIELD READ or an INDEX is still unknown and still accepted;
//!   `a_derived_operand_from_a_field_read_is_still_unreached` pins that edge.
//!   **An ARITHMETIC result is no longer among them** — a bounded fixpoint reaches
//!   it, per `a_derived_operand_is_now_reached_and_the_chain_has_no_depth_limit`.
//!   The hop bound is a decision rather than a limit of the approach.
//! - **Unknown never rejects, by design.** A stage that cannot type an operand
//!   accepts it, because rejecting a valid program is a language change rather
//!   than a conservative choice. The well-typed controls are the half that can
//!   fail.
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
    // ADDED WHEN LOCAL RESOLUTION LANDED. Each is an error the stage already had
    // a rule for and could not SEE, because its operands are not literals. They
    // were pinned as measured disagreements until the stage could resolve a name
    // through its binding table; they are ordinary corpus members now.
    (
        "operand-through-a-let",
        "fn main() -> Word { let b = true; 1 + b }",
    ),
    (
        "operand-through-a-call",
        "fn g() -> Word { 1 }\nfn main() -> Word { g() + true }",
    ),
    (
        "both-operands-through-lets",
        "fn main() -> Word { let a = 1; let b = true; a + b }",
    ),
    (
        "let-bound-to-a-call",
        "fn g() -> Word { 1 }\nfn main() -> Word { let a = g(); a + true }",
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
        // **A `Named` TYPE IS NEVER A PRIMITIVE, AND AN EARLIER REVISION HERE GOT
        // THAT BACKWARDS.** It mapped `Named("Bool")` to the boolean tag, reasoning
        // that matching `Prim` alone "silently drops every `Bool` annotation". True,
        // and the wrong conclusion: those annotations are dropped because they are
        // NOT booleans. Measured — `Word`, `Byte` and `Float` are `Prim` and
        // capitalised, `bool` is `Prim` and LOWERCASE, and `Bool` is an ordinary
        // named type the reference refuses to add to a `Word`.
        //
        // The `Word`/`Byte`/`Float` arms of that mapping were dead besides: all
        // three parse as `Prim` and never arrive here.
        //
        // Pinned by `a_named_type_called_bool_is_not_the_boolean_primitive`.
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

/// Declarations and call sites, kept SEPARATE so the stage joins them.
///
/// Returns (declared parameter counts by index, call sites as (index, argument
/// count), argument-type pairs).
///
/// **The host resolves a name to an index and counts arguments. It does not
/// decide whether the arity is right.** That decision moved into the stage, and
/// the difference between a migration and a relocation is exactly which side
/// holds it.
type DeclCallRows = (Vec<i64>, Vec<(i64, i64)>, Vec<(i64, i64)>);

fn decl_call_rows(ast: &keleusma::ast::Program) -> DeclCallRows {
    use keleusma::ast::Expr;
    use keleusma::visitor::Visitor;
    use std::collections::BTreeMap;

    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut params: Vec<i64> = Vec::new();
    let mut ptypes: Vec<Vec<i64>> = Vec::new();
    for f in &ast.functions {
        index.insert(f.name.clone(), params.len());
        params.push(f.params.len() as i64);
        ptypes.push(
            f.params
                .iter()
                .map(|p| p.type_expr.as_ref().map_or(0, type_tag))
                .collect(),
        );
    }

    struct Sites<'a> {
        index: &'a BTreeMap<String, usize>,
        ptypes: &'a [Vec<i64>],
        sites: Vec<(i64, i64)>,
        args: Vec<(i64, i64)>,
    }
    impl Visitor for Sites<'_> {
        fn visit_expr(&mut self, expr: &Expr) {
            if let Expr::Call { name, args, .. } = expr
                && let Some(i) = self.index.get(name)
            {
                self.sites.push((*i as i64, args.len() as i64));
                for (p, a) in self.ptypes[*i].iter().zip(args.iter()) {
                    self.args.push((*p, expr_tag(a)));
                }
            }
            self.walk_expr(expr);
        }
    }

    let mut s = Sites {
        index: &index,
        ptypes: &ptypes,
        sites: Vec::new(),
        args: Vec::new(),
    };
    for f in &ast.functions {
        s.visit_block(&f.body);
    }
    (params, s.sites, s.args)
}

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

/// Declared names and name occurrences, kept SEPARATE so the stage classifies.
///
/// Returns (declared name indices, occurrences as (name index, is-local,
/// is-call), whether the collector had to give up).
///
/// **The host says "occurrence 4 names index 12 and it is a call". It does not
/// say "that is an undefined function".** Deciding which combinations the
/// language refuses is the stage's, and it is the whole content of this
/// migration.
///
/// Names are interned into their own shared index space, for the reason the
/// field sets are: the stage has no string type, and two spellings of one name
/// must be one index or the comparison misses.
type OccurrenceRows = (Vec<i64>, Vec<(i64, i64, i64)>, bool);

fn occurrence_rows(ast: &keleusma::ast::Program) -> OccurrenceRows {
    use keleusma::ast::{Expr, ImportItem, Pattern, Stmt, TypeDef};
    use keleusma::visitor::Visitor;
    use std::collections::{BTreeMap, BTreeSet};

    let mut ids: BTreeMap<String, i64> = BTreeMap::new();
    let intern = |n: &str, ids: &mut BTreeMap<String, i64>| -> i64 {
        let next = ids.len() as i64;
        *ids.entry(n.to_string()).or_insert(next)
    };

    let mut declared: Vec<i64> = Vec::new();
    let mut wildcard = false;
    for f in &ast.functions {
        let i = intern(&f.name, &mut ids);
        declared.push(i);
    }
    for d in &ast.data_decls {
        let i = intern(&d.name, &mut ids);
        declared.push(i);
    }
    for t in &ast.types {
        let n = match t {
            TypeDef::Struct(d) => &d.name,
            TypeDef::Enum(d) => &d.name,
            TypeDef::Newtype(d) => &d.name,
        };
        let i = intern(n, &mut ids);
        declared.push(i);
    }
    for u in &ast.uses {
        match &u.import {
            ImportItem::Name(n) => {
                let i = intern(n, &mut ids);
                declared.push(i);
            }
            ImportItem::Wildcard => wildcard = true,
        }
    }

    struct Occ {
        locals: BTreeSet<String>,
        seen: Vec<(String, i64, i64)>,
    }
    impl Visitor for Occ {
        fn visit_stmt(&mut self, stmt: &Stmt) {
            if let Stmt::Let(l) = stmt
                && let Pattern::Variable(n, _) = &l.pattern
            {
                self.locals.insert(n.clone());
            }
            self.walk_stmt(stmt);
        }
        fn visit_expr(&mut self, expr: &Expr) {
            match expr {
                Expr::Call { name, .. } => {
                    let local = i64::from(self.locals.contains(name));
                    self.seen.push((name.clone(), local, 1));
                }
                Expr::Ident { name, .. } => {
                    let local = i64::from(self.locals.contains(name));
                    self.seen.push((name.clone(), local, 0));
                }
                _ => {}
            }
            self.walk_expr(expr);
        }
    }

    let mut seen: Vec<(String, i64, i64)> = Vec::new();
    for f in &ast.functions {
        let mut locals: BTreeSet<String> = BTreeSet::new();
        for p in &f.params {
            if let Pattern::Variable(n, _) = &p.pattern {
                locals.insert(n.clone());
            }
        }
        // Two passes: bindings first, because a `let` later in the body still
        // makes the name local to this approximation and a one-pass walk would
        // depend on statement order.
        let mut pre = Occ {
            locals,
            seen: Vec::new(),
        };
        pre.visit_block(&f.body);
        let mut pass = Occ {
            locals: pre.locals,
            seen: Vec::new(),
        };
        pass.visit_block(&f.body);
        seen.extend(pass.seen);
    }

    let occurrences = seen
        .into_iter()
        .map(|(n, local, call)| (intern(&n, &mut ids), local, call))
        .collect();

    (declared, occurrences, wildcard)
}

/// Expression nodes as (kind, a, b), the last channel to migrate.
///
/// **The host reports the shape; the stage decides what the shape means.** A
/// binary operation contributes its two operand tags and the kind that says
/// they must agree. A field access on a scalar contributes the operand tag and
/// the kind that says a scalar will not do. Nothing here computes a verdict.
///
/// One table rather than one per kind, because every rule is "these two must
/// agree" or "this must be bool", and a channel per kind would multiply a slot
/// chain that has already produced two off-by-one defects.
/// A binding row: `(name id, value, form)` where form 0 makes the value a tag
/// and form 1 makes it another name id.
type BindingRow = (i64, i64, i64);

/// The name table and the binding rows [`binding_rows`] returns.
type BindingRows = (std::collections::BTreeMap<String, i64>, Vec<BindingRow>);

/// An expression node with each operand tagged by form: `(kind, a, af, b, bf)`.
type ResolvableNode = (i64, i64, i64, i64, i64);

/// One operand, reported as `(value, form)` and nothing more.
///
/// Form 0 is a TAG, form 1 is a NAME id. **Deliberately shallow.** A literal
/// reports its own kind, which is what it says on the page; a name or a call
/// reports WHICH name, not what type that name has. The second question is the
/// stage's, and answering it here is the change that would make the tests pass
/// while making the checker less self-hosted.
fn operand_form(
    e: &keleusma::ast::Expr,
    names: &std::collections::BTreeMap<String, i64>,
) -> (i64, i64) {
    use keleusma::ast::Expr;
    match e {
        Expr::Literal { value, .. } => (literal_tag(value), 0),
        Expr::Ident { name, .. } => names.get(name).map_or((0, 0), |id| (*id, 1)),
        Expr::Call { name, .. } => names.get(name).map_or((0, 0), |id| (*id, 1)),
        _ => (0, 0),
    }
}

/// The name table and the binding rows, both read straight off the source.
///
/// Returns `(name id by spelling, rows of (name id, tag))`. Three sources, each
/// a syntactic fact:
///
/// 1. a declared parameter type,
/// 2. a declared return type, keyed by the function name so a call resolves,
/// 3. a `let` whose initialiser is a literal.
///
/// **Nothing here is inferred.** Each row restates something the program writes
/// down. What none of them says is which operand has which type: a row records
/// that the NAME `b` was written `= true`, not that the left side of some
/// addition is a bool. The stage performs that join, which is why
/// `the_stage_and_not_the_host_resolves_an_operand` can tell the two apart.
///
/// **Known narrowing, stated rather than discovered later.** One flat namespace
/// covers locals and functions, so a local shadowing a function name would give
/// one row for two meanings. The subset's stage sources do not do this and the
/// corpus does not exercise it; a shadowing case would need the table split.
fn binding_rows(ast: &keleusma::ast::Program) -> BindingRows {
    use keleusma::ast::{Expr, Pattern, Stmt};
    use keleusma::visitor::Visitor;
    use std::collections::BTreeMap;

    let mut names: BTreeMap<String, i64> = BTreeMap::new();
    // `(name id, value, form)`; form 0 is a tag and form 1 a name id.
    let mut rows: Vec<BindingRow> = Vec::new();
    let id_of = |names: &mut BTreeMap<String, i64>, n: &str| -> i64 {
        let next = names.len() as i64 + 1;
        *names.entry(n.to_string()).or_insert(next)
    };

    for f in &ast.functions {
        // A declared return type, keyed by the function's own name.
        let fid = id_of(&mut names, &f.name);
        let t = type_tag(&f.return_type);
        if t != 0 {
            rows.push((fid, t, 0));
        }
        // Declared parameter types.
        for prm in &f.params {
            // Through `type_tag` rather than `prim_tag`, so a `Bool` annotation --
            // which the parser yields as `Named("Bool")` -- is not silently dropped.
            if let (Pattern::Variable(n, _), Some(ty)) = (&prm.pattern, &prm.type_expr) {
                let t = type_tag(ty);
                if t != 0 {
                    let id = id_of(&mut names, n);
                    rows.push((id, t, 0));
                }
            }
        }
    }

    // `let` bindings whose initialiser is a literal.
    struct Lets {
        found: Vec<(String, i64)>,
        aliases: Vec<(String, String)>,
        derived: Vec<String>,
    }
    impl Visitor for Lets {
        fn visit_stmt(&mut self, stmt: &Stmt) {
            if let Stmt::Let(l) = stmt
                && let Pattern::Variable(n, _) = &l.pattern
            {
                match &l.value {
                    Expr::Literal { value, .. } => {
                        let t = literal_tag(value);
                        if t != 0 {
                            self.found.push((n.clone(), t));
                        }
                    }
                    // `let a = g()` says `a` takes whatever `g` returns. That is
                    // a syntactic fact; joining it to `g`'s declared return type
                    // is the stage's alias hop, not this function's.
                    Expr::Call { name, .. } => {
                        self.aliases.push((n.clone(), name.clone()));
                    }
                    // `let a = 1 + 2` says `a` takes whatever that OPERATOR
                    // EXPRESSION yields. Syntactic, like the two above; the join
                    // -- resolving both operands and requiring them to agree --
                    // is the stage's bounded fixpoint, not this function's.
                    Expr::BinOp { .. } => {
                        self.derived.push(n.clone());
                    }
                    _ => {}
                }
            }
            self.walk_stmt(stmt);
        }
    }
    let mut lets = Lets {
        found: Vec::new(),
        aliases: Vec::new(),
        derived: Vec::new(),
    };
    for f in &ast.functions {
        lets.visit_block(&f.body);
    }
    for (n, t) in lets.found {
        let id = id_of(&mut names, &n);
        rows.push((id, t, 0));
    }
    for (n, target) in lets.aliases {
        let id = id_of(&mut names, &n);
        let tid = id_of(&mut names, &target);
        rows.push((id, tid, 1));
    }
    // A `let` whose initialiser is an OPERATOR EXPRESSION gets a name id here but
    // NO ROW: the row needs the initialiser's index in the expression table, which
    // only the node walk knows. Registering the name is what lets a later operand
    // spelling it take form 1 instead of collapsing to an untyped 0.
    for n in lets.derived {
        id_of(&mut names, &n);
    }

    (names, rows)
}

/// The expression table with only tags, which is what every caller predating
/// local resolution wants.
///
/// A thin wrapper over [`expression_nodes_resolvable`] with an EMPTY name table,
/// so there is ONE walk rather than two that could drift. With no names to
/// resolve, every operand that is not a literal takes form 0 and value 0, which
/// is exactly the behaviour before resolution existed.
fn expression_nodes(ast: &keleusma::ast::Program) -> Vec<(i64, i64, i64)> {
    expression_nodes_resolvable(ast, &std::collections::BTreeMap::new())
        .into_iter()
        .map(|(k, a, _af, b, _bf)| (k, a, b))
        .collect()
}

/// The expression table with each operand tagged by FORM: `(kind, a, af, b, bf)`
/// where a form of 0 means the value is a tag and 1 means it is a name the stage
/// resolves through its binding table.
///
/// **This function does not decide any operand's type.** It reports that an
/// operand IS a literal of kind `t`, or that it IS the name `n`. Which type `n`
/// then has, and whether that disagrees with the other operand, is the stage's
/// join. That line is the point of the whole slice.
fn expression_nodes_and_derived(
    ast: &keleusma::ast::Program,
    names: &std::collections::BTreeMap<String, i64>,
) -> (Vec<ResolvableNode>, Vec<(String, i64)>) {
    use keleusma::ast::{Expr, Pattern, Stmt, TypeDef, TypeExpr};
    use keleusma::visitor::Visitor;
    use std::collections::{BTreeMap, BTreeSet};

    const BINOP: i64 = 1;
    const ARRAY_ELEM: i64 = 2;
    const CONDITION: i64 = 3;
    const BRANCH_PAIR: i64 = 4;
    const FIELD_ON_VALUE: i64 = 5;
    const INDEX_ON_VALUE: i64 = 6;
    const STRUCT_LIT: i64 = 7;
    const TAIL_VS_RETURN: i64 = 8;

    let mut struct_fields: BTreeMap<String, i64> = BTreeMap::new();
    for t in &ast.types {
        if let TypeDef::Struct(d) = t {
            struct_fields.insert(d.name.clone(), d.fields.len() as i64);
        }
    }

    struct Nodes<'a> {
        structs: &'a BTreeMap<String, i64>,
        names: &'a BTreeMap<String, i64>,
        scalars: BTreeSet<String>,
        out: Vec<ResolvableNode>,
        // Each `let` whose initialiser is an operator expression, with the index
        // that expression takes in `out`. Collected HERE rather than by a second
        // walk, because two walks over the same tree are exactly how an index and
        // the thing it indexes come to disagree.
        derived: Vec<(String, i64)>,
    }
    impl Visitor for Nodes<'_> {
        fn visit_stmt(&mut self, stmt: &Stmt) {
            if let Stmt::Let(l) = stmt
                && let Pattern::Variable(n, _) = &l.pattern
                && let Some(TypeExpr::Prim(_, _)) = &l.type_expr
            {
                self.scalars.insert(n.clone());
            }
            // The index the initialiser's own node WILL take. Read before
            // `walk_stmt` descends, and correct because `visit_expr` pushes an
            // operator node BEFORE walking its operands, so the outermost one
            // lands at exactly this position.
            if let Stmt::Let(l) = stmt
                && let Pattern::Variable(n, _) = &l.pattern
                && matches!(&l.value, Expr::BinOp { .. })
            {
                self.derived.push((n.clone(), self.out.len() as i64));
            }
            self.walk_stmt(stmt);
        }
        fn visit_expr(&mut self, expr: &Expr) {
            match expr {
                Expr::BinOp { left, right, .. } => {
                    let (a, af) = operand_form(left, self.names);
                    let (b, bf) = operand_form(right, self.names);
                    self.out.push((BINOP, a, af, b, bf));
                }
                Expr::ArrayLiteral { elements, .. } => {
                    if let Some(first) = elements.first() {
                        let (ft, ff) = operand_form(first, self.names);
                        for e in elements.iter().skip(1) {
                            let (t, f) = operand_form(e, self.names);
                            self.out.push((ARRAY_ELEM, ft, ff, t, f));
                        }
                    }
                }
                Expr::If {
                    condition,
                    then_block,
                    else_block,
                    ..
                } => {
                    let (c, cf) = operand_form(condition, self.names);
                    self.out.push((CONDITION, c, cf, 0, 0));
                    if let Some(e) = else_block {
                        let (t, tf) = then_block
                            .tail_expr
                            .as_ref()
                            .map_or((0, 0), |x| operand_form(x, self.names));
                        let (g, gf) = e
                            .tail_expr
                            .as_ref()
                            .map_or((0, 0), |x| operand_form(x, self.names));
                        self.out.push((BRANCH_PAIR, t, tf, g, gf));
                    }
                }
                Expr::FieldAccess { object, .. } => {
                    if let Expr::Ident { name, .. } = object.as_ref()
                        && self.scalars.contains(name)
                    {
                        self.out.push((FIELD_ON_VALUE, 1, 0, 0, 0));
                    }
                }
                Expr::ArrayIndex { object, .. } => {
                    if let Expr::Ident { name, .. } = object.as_ref()
                        && self.scalars.contains(name)
                    {
                        self.out.push((INDEX_ON_VALUE, 1, 0, 0, 0));
                    }
                }
                Expr::StructInit { name, fields, .. } => {
                    if let Some(n) = self.structs.get(name) {
                        self.out.push((STRUCT_LIT, *n, 0, fields.len() as i64, 0));
                    }
                }
                _ => {}
            }
            self.walk_expr(expr);
        }
    }

    let mut out = Vec::new();
    let mut derived: Vec<(String, i64)> = Vec::new();
    for f in &ast.functions {
        let mut n = Nodes {
            structs: &struct_fields,
            names,
            scalars: BTreeSet::new(),
            out: Vec::new(),
            derived: Vec::new(),
        };
        n.visit_block(&f.body);
        // EACH FUNCTION'S WALK NUMBERS FROM ZERO, so a derived index is
        // function-local and must be offset by everything already accumulated.
        // Missing this offset would point every derived binding after the first
        // function at the wrong node -- and at a node that exists, so it would
        // resolve to a plausible wrong tag rather than fail.
        let base = out.len() as i64;
        derived.extend(n.derived.into_iter().map(|(name, i)| (name, base + i)));
        out.extend(n.out);
        if let Some(tail) = f.body.tail_expr.as_ref() {
            let (t, tf) = operand_form(tail, names);
            out.push((TAIL_VS_RETURN, t, tf, type_tag(&f.return_type), 0));
        }
    }
    (out, derived)
}

/// The expression table alone, for callers with no derived bindings to place.
///
/// A thin wrapper so there is ONE walk: the pair-returning form is the only one
/// that traverses the tree, and this drops the half its callers do not use.
fn expression_nodes_resolvable(
    ast: &keleusma::ast::Program,
    names: &std::collections::BTreeMap<String, i64>,
) -> Vec<ResolvableNode> {
    expression_nodes_and_derived(ast, names).0
}

/// Struct field sets and field accesses, kept SEPARATE so the stage searches.
///
/// Returns (first index per type, count per type, the flattened field-name
/// indices, the accesses as (type index, field-name index)).
///
/// **Names travel as interned indices, not bytes.** The stage has no string
/// type, and the host already has to assign every name an index. The index
/// space is SHARED between the declared sets and the accesses -- one table for
/// every name either side mentions -- because two spellings of one name would
/// otherwise be two indices and the search would miss.
///
/// **The host no longer answers the question.** It says "type 2, field name 7";
/// whether type 2 declares name 7 is the stage's to determine.
type FieldSets = (Vec<i64>, Vec<i64>, Vec<i64>, Vec<(i64, i64)>);

fn field_sets(ast: &keleusma::ast::Program) -> FieldSets {
    use keleusma::ast::{Expr, Pattern, Stmt, TypeDef, TypeExpr};
    use keleusma::visitor::Visitor;
    use std::collections::BTreeMap;

    // One shared index space for every field name either side mentions.
    let mut names: BTreeMap<String, i64> = BTreeMap::new();
    let intern = |n: &str, names: &mut BTreeMap<String, i64>| -> i64 {
        let next = names.len() as i64;
        *names.entry(n.to_string()).or_insert(next)
    };

    let mut type_index: BTreeMap<String, i64> = BTreeMap::new();
    let mut first: Vec<i64> = Vec::new();
    let mut count: Vec<i64> = Vec::new();
    let mut flat: Vec<i64> = Vec::new();
    for t in &ast.types {
        if let TypeDef::Struct(d) = t {
            type_index.insert(d.name.clone(), first.len() as i64);
            first.push(flat.len() as i64);
            count.push(d.fields.len() as i64);
            for f in &d.fields {
                let i = intern(&f.name, &mut names);
                flat.push(i);
            }
        }
    }

    struct Access<'a> {
        type_index: &'a BTreeMap<String, i64>,
        types: BTreeMap<String, String>,
        pending: Vec<(String, String)>,
    }
    impl Visitor for Access<'_> {
        fn visit_stmt(&mut self, stmt: &Stmt) {
            if let Stmt::Let(l) = stmt
                && let Pattern::Variable(n, _) = &l.pattern
            {
                if let Expr::StructInit { name, .. } = &l.value {
                    self.types.insert(n.clone(), name.clone());
                } else if let Some(TypeExpr::Named(t, _, _, _)) = &l.type_expr {
                    self.types.insert(n.clone(), t.clone());
                }
            }
            self.walk_stmt(stmt);
        }
        fn visit_expr(&mut self, expr: &Expr) {
            if let Expr::FieldAccess { object, field, .. } = expr
                && let Expr::Ident { name, .. } = object.as_ref()
                && let Some(ty) = self.types.get(name)
                && self.type_index.contains_key(ty)
            {
                self.pending.push((ty.clone(), field.clone()));
            }
            self.walk_expr(expr);
        }
    }

    let mut pending = Vec::new();
    for f in &ast.functions {
        let mut a = Access {
            type_index: &type_index,
            types: BTreeMap::new(),
            pending: Vec::new(),
        };
        a.visit_block(&f.body);
        pending.extend(a.pending);
    }

    let accesses = pending
        .into_iter()
        .map(|(ty, field)| {
            let t = type_index[&ty];
            let f = intern(&field, &mut names);
            (t, f)
        })
        .collect();

    (first, count, flat, accesses)
}

/// The stage's verdict for a program, with its operand pairs marshalled in.
fn stage_accepts_program(pairs: &[(i64, i64)]) -> bool {
    stage_verdict(&StageInput {
        pairs,
        ..Default::default()
    })
}

/// The stage's verdict with both tables.
/// Everything the stage reads, in one place.
///
/// **The growing argument list was the symptom, and clippy naming it is fair.**
/// Eight parameters is what an input surface assembled channel by channel looks
/// like from the outside. Bundling them does not consolidate the encoding, but
/// it stops the shape of the debt being hidden behind positional arguments,
/// where swapping two `&[(i64, i64)]` compiles and is wrong.
#[derive(Default)]
struct StageInput<'a> {
    pairs: &'a [(i64, i64)],
    arity: &'a [(i64, i64)],
    claims: &'a [(i64, i64)],
    member: &'a [i64],
    dparams: &'a [i64],
    sites: &'a [(i64, i64)],
    sets: Option<&'a FieldSets>,
    occ: Option<&'a OccurrenceRows>,
    nodes: &'a [(i64, i64, i64)],
    /// `(kind, a, a-form, b, b-form)`. When present it REPLACES `nodes`, so the
    /// two are never seeded together and cannot disagree.
    nodes_resolvable: Option<&'a [ResolvableNode]>,
    /// `(name id, tag)` rows the stage resolves a form-1 operand through.
    bindings: &'a [BindingRow],
}

fn stage_verdict(input: &StageInput<'_>) -> bool {
    stage_verdict_counting(input).0
}

/// [`stage_verdict`] with the number of resumes the fold took.
///
/// The count is what makes the conversion checkable. A stage that did every row
/// in its first step and yielded the verdict immediately would satisfy every
/// verdict assertion in this file while streaming nothing, so
/// `the_fold_advances_one_row_per_resume` reads this rather than the verdict.
fn stage_verdict_counting(input: &StageInput<'_>) -> (bool, usize) {
    let StageInput {
        pairs,
        arity,
        claims,
        member,
        dparams,
        sites,
        sets,
        occ,
        nodes,
        nodes_resolvable,
        bindings,
    } = *input;
    static EMPTY_SETS: FieldSets = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let sets = sets.unwrap_or(&EMPTY_SETS);
    static EMPTY_OCC: OccurrenceRows = (Vec::new(), Vec::new(), false);
    let occ = occ.unwrap_or(&EMPTY_OCC);
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
    // The pre-classified resolution channel is gone from the stage, so its
    // slots are gone from here too. The next block starts where it used to.
    const QN_SLOT: usize = CACT_SLOT + 128;
    const QACT_SLOT: usize = QN_SLOT + 1;
    const QREQ_SLOT: usize = QACT_SLOT + 256;
    assert!(
        claims.len() <= 256,
        "the claim table holds 256 rows and this program needs {}",
        claims.len()
    );
    vm.set_shared(
        &mut shared,
        QN_SLOT,
        keleusma::bytecode::Value::Int(claims.len() as i64),
    )
    .expect("qn");
    for (i, (a, r)) in claims.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            QACT_SLOT + i,
            keleusma::bytecode::Value::Int(*a),
        )
        .expect("qact");
        vm.set_shared(
            &mut shared,
            QREQ_SLOT + i,
            keleusma::bytecode::Value::Int(*r),
        )
        .expect("qreq");
    }
    const MN_SLOT: usize = QREQ_SLOT + 256;
    const MEMBER_SLOT: usize = MN_SLOT + 1;
    assert!(
        member.len() <= 256,
        "the membership table holds 256 rows and this program needs {}",
        member.len()
    );
    vm.set_shared(
        &mut shared,
        MN_SLOT,
        keleusma::bytecode::Value::Int(member.len() as i64),
    )
    .expect("mn");
    for (i, m) in member.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            MEMBER_SLOT + i,
            keleusma::bytecode::Value::Int(*m),
        )
        .expect("member");
    }
    const DN_SLOT: usize = MEMBER_SLOT + 256;
    const DPARAMS_SLOT: usize = DN_SLOT + 1;
    const CSN_SLOT: usize = DPARAMS_SLOT + 128;
    const CSITE_SLOT: usize = CSN_SLOT + 1;
    const CARGS_SLOT: usize = CSITE_SLOT + 128;
    assert!(
        dparams.len() <= 128 && sites.len() <= 128,
        "declaration tables overflow"
    );
    vm.set_shared(
        &mut shared,
        DN_SLOT,
        keleusma::bytecode::Value::Int(dparams.len() as i64),
    )
    .expect("dn");
    for (i, d) in dparams.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            DPARAMS_SLOT + i,
            keleusma::bytecode::Value::Int(*d),
        )
        .expect("dparams");
    }
    vm.set_shared(
        &mut shared,
        CSN_SLOT,
        keleusma::bytecode::Value::Int(sites.len() as i64),
    )
    .expect("csn");
    for (i, (idx, n)) in sites.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            CSITE_SLOT + i,
            keleusma::bytecode::Value::Int(*idx),
        )
        .expect("csite");
        vm.set_shared(
            &mut shared,
            CARGS_SLOT + i,
            keleusma::bytecode::Value::Int(*n),
        )
        .expect("cargs");
    }
    const STN_SLOT: usize = CARGS_SLOT + 128;
    const SFIRST_SLOT: usize = STN_SLOT + 1;
    const SCOUNT_SLOT: usize = SFIRST_SLOT + 64;
    const SFIELD_SLOT: usize = SCOUNT_SLOT + 64;
    const AN_SLOT: usize = SFIELD_SLOT + 256;
    const ATYPE_SLOT: usize = AN_SLOT + 1;
    const ANAME_SLOT: usize = ATYPE_SLOT + 256;
    let (sfirst, scount, sfield, accesses) = sets;
    assert!(
        sfirst.len() <= 64 && sfield.len() <= 256 && accesses.len() <= 256,
        "field-set tables overflow"
    );
    vm.set_shared(
        &mut shared,
        STN_SLOT,
        keleusma::bytecode::Value::Int(sfirst.len() as i64),
    )
    .expect("stn");
    for (i, v) in sfirst.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            SFIRST_SLOT + i,
            keleusma::bytecode::Value::Int(*v),
        )
        .expect("sfirst");
    }
    for (i, v) in scount.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            SCOUNT_SLOT + i,
            keleusma::bytecode::Value::Int(*v),
        )
        .expect("scount");
    }
    for (i, v) in sfield.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            SFIELD_SLOT + i,
            keleusma::bytecode::Value::Int(*v),
        )
        .expect("sfield");
    }
    vm.set_shared(
        &mut shared,
        AN_SLOT,
        keleusma::bytecode::Value::Int(accesses.len() as i64),
    )
    .expect("an");
    for (i, (t, f)) in accesses.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            ATYPE_SLOT + i,
            keleusma::bytecode::Value::Int(*t),
        )
        .expect("atype");
        vm.set_shared(
            &mut shared,
            ANAME_SLOT + i,
            keleusma::bytecode::Value::Int(*f),
        )
        .expect("aname");
    }
    // `hit` is a scratch WORD sitting between the field-set tables and these,
    // so it must be stepped over. Naming it rather than folding a +1 into the
    // next constant: an unexplained +1 in a slot chain is indistinguishable
    // from an off-by-one, and this chain has already produced one.
    const HIT_SLOT: usize = ANAME_SLOT + 256;
    const DNN_SLOT: usize = HIT_SLOT + 1;
    const DNAME_SLOT: usize = DNN_SLOT + 1;
    const ON_SLOT: usize = DNAME_SLOT + 128;
    const ONAME_SLOT: usize = ON_SLOT + 1;
    const OLOCAL_SLOT: usize = ONAME_SLOT + 256;
    const OCALL_SLOT: usize = OLOCAL_SLOT + 256;
    const OSKIP_SLOT: usize = OCALL_SLOT + 256;
    let (declared, occurrences, wildcard) = occ;
    assert!(
        declared.len() <= 128 && occurrences.len() <= 256,
        "occurrence tables overflow"
    );
    vm.set_shared(
        &mut shared,
        DNN_SLOT,
        keleusma::bytecode::Value::Int(declared.len() as i64),
    )
    .expect("dnn");
    for (i, d) in declared.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            DNAME_SLOT + i,
            keleusma::bytecode::Value::Int(*d),
        )
        .expect("dname");
    }
    vm.set_shared(
        &mut shared,
        ON_SLOT,
        keleusma::bytecode::Value::Int(occurrences.len() as i64),
    )
    .expect("on");
    for (i, (n, l, c)) in occurrences.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            ONAME_SLOT + i,
            keleusma::bytecode::Value::Int(*n),
        )
        .expect("oname");
        vm.set_shared(
            &mut shared,
            OLOCAL_SLOT + i,
            keleusma::bytecode::Value::Int(*l),
        )
        .expect("olocal");
        vm.set_shared(
            &mut shared,
            OCALL_SLOT + i,
            keleusma::bytecode::Value::Int(*c),
        )
        .expect("ocall");
    }
    vm.set_shared(
        &mut shared,
        OSKIP_SLOT,
        keleusma::bytecode::Value::Int(i64::from(*wildcard)),
    )
    .expect("oskip");
    const EN_SLOT: usize = OSKIP_SLOT + 1;
    const EKIND_SLOT: usize = EN_SLOT + 1;
    const EA_SLOT: usize = EKIND_SLOT + 256;
    const EB_SLOT: usize = EA_SLOT + 256;
    // Appended after `eb`, matching the stage's declaration order exactly. A
    // slot chain is the defect source this file already carries two notes about,
    // so these are derived from the previous constant rather than written out.
    const BN_SLOT: usize = EB_SLOT + 256;
    const BNAME_SLOT: usize = BN_SLOT + 1;
    const BTAG_SLOT: usize = BNAME_SLOT + 128;
    const BFORM_SLOT: usize = BTAG_SLOT + 128;
    const ALIAS_SLOT: usize = BFORM_SLOT + 128;
    const EAF_SLOT: usize = ALIAS_SLOT + 1;
    const EBF_SLOT: usize = EAF_SLOT + 256;
    // The resolvable table REPLACES the tag-only one when present, so the two are
    // never seeded together. Widening every operand to a form here means the
    // stage sees one shape regardless of which caller built the table.
    let widened: Vec<ResolvableNode> = match nodes_resolvable {
        Some(rows) => rows.to_vec(),
        None => nodes.iter().map(|(k, a, b)| (*k, *a, 0, *b, 0)).collect(),
    };
    assert!(widened.len() <= 256, "the expression table overflows");
    vm.set_shared(
        &mut shared,
        EN_SLOT,
        keleusma::bytecode::Value::Int(widened.len() as i64),
    )
    .expect("en");
    for (i, (k, a, af, b, bf)) in widened.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            EKIND_SLOT + i,
            keleusma::bytecode::Value::Int(*k),
        )
        .expect("ekind");
        vm.set_shared(&mut shared, EA_SLOT + i, keleusma::bytecode::Value::Int(*a))
            .expect("ea");
        vm.set_shared(&mut shared, EB_SLOT + i, keleusma::bytecode::Value::Int(*b))
            .expect("eb");
        vm.set_shared(
            &mut shared,
            EAF_SLOT + i,
            keleusma::bytecode::Value::Int(*af),
        )
        .expect("eaf");
        vm.set_shared(
            &mut shared,
            EBF_SLOT + i,
            keleusma::bytecode::Value::Int(*bf),
        )
        .expect("ebf");
    }
    // The binding table, appended after the expression channel to match the
    // stage's slot order.
    assert!(bindings.len() <= 128, "the binding table overflows");
    vm.set_shared(
        &mut shared,
        BN_SLOT,
        keleusma::bytecode::Value::Int(bindings.len() as i64),
    )
    .expect("bn");
    for (i, (n, t, f)) in bindings.iter().enumerate() {
        vm.set_shared(
            &mut shared,
            BNAME_SLOT + i,
            keleusma::bytecode::Value::Int(*n),
        )
        .expect("bname");
        vm.set_shared(
            &mut shared,
            BTAG_SLOT + i,
            keleusma::bytecode::Value::Int(*t),
        )
        .expect("btag");
        vm.set_shared(
            &mut shared,
            BFORM_SLOT + i,
            keleusma::bytecode::Value::Int(*f),
        )
        .expect("bform");
    }
    // THE STAGE IS A COROUTINE NOW, so drive it to a verdict rather than calling
    // it once. It yields `PENDING` (63) per folded row and the verdict when the
    // fold completes, matching the nine sibling stages.
    //
    // The drive loop is BOUNDED by the stage's own `ty_max_steps`, read from the
    // source rather than restated here, and exhausting the bound is a hard
    // failure. A `loop { resume }` with no cap would hang on a stage that never
    // reported done, which is the one failure mode a total language is supposed
    // to make impossible and a harness should not reintroduce.
    const PENDING: i64 = 63;
    let cap = declared_max_steps();

    // TWO COUNTERS, BECAUSE THEY BOUND DIFFERENT THINGS, and conflating them is
    // what made the first version of `the_fold_advances_one_row_per_resume`
    // report two resumes per row.
    //
    // `yields` counts PENDING yields only, which is what "one row per step"
    // means and what that test measures. A `loop` block also produces a `Reset`
    // between iterations, so a counter incremented on every returned state
    // reports exactly twice the row count -- a property of the drive loop, not
    // of the stage.
    //
    // `states` bounds the whole drive so a stage that never reports done cannot
    // hang the harness. It allows two states per step plus slack, because that
    // is the observed shape rather than a guess.
    let mut yields = 0usize;
    let mut states = 0usize;
    let state_cap = 2 * cap + 4;

    let mut out = vm
        .call_with_shared(&mut shared, &[keleusma::bytecode::Value::Int(0)])
        .expect("run");
    loop {
        match out {
            // The loop block's own RESET between iterations, not an answer. The
            // driver's `next_word` skips it for the same reason.
            keleusma::vm::VmState::Reset => {}
            keleusma::vm::VmState::Yielded(keleusma::bytecode::Value::Int(PENDING)) => {
                yields += 1;
                assert!(
                    yields <= cap,
                    "the stage reported PENDING {yields} times against its own declared \
                     maximum of {cap} steps without producing a verdict"
                );
            }
            keleusma::vm::VmState::Yielded(keleusma::bytecode::Value::Int(v)) => {
                return (v == 0, yields);
            }
            other => panic!("verify_types.kel yielded {other:?}, not an Int"),
        }
        states += 1;
        assert!(
            states <= state_cap,
            "the drive loop saw {states} virtual-machine states against a bound of \
             {state_cap}; the stage is producing something other than one yield and one \
             reset per step"
        );
        out = vm
            .resume_with_shared(&mut shared, keleusma::bytecode::Value::Int(0))
            .expect("resume");
    }
}

/// The stage's own step bound, read out of `verify_types.kel` rather than
/// restated.
///
/// Same discipline `highest_command` uses: a bound written in two places is a
/// bound that drifts, and a harness whose cap is smaller than the stage's real
/// worst case would report a spurious failure on the largest corpus.
fn declared_max_steps() -> usize {
    const SRC: &str = include_str!("../src/selfhost/kel/verify_types.kel");
    let line = SRC
        .lines()
        .find(|l| l.trim_start().starts_with("fn ty_max_steps()"))
        .expect("verify_types.kel declares ty_max_steps");
    let body = line
        .rsplit_once('{')
        .and_then(|(_, t)| t.split_once('}'))
        .expect("ty_max_steps has a literal body")
        .0;
    body.split('+')
        .map(|t| t.trim().parse::<usize>().expect("a sum of literals"))
        .sum()
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
    assert!(ILL_TYPED.len() >= 20, "the rejection corpus shrank");
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
        let stage = stage_verdict(&StageInput {
            pairs: &pairs,
            arity: &arity,
            ..Default::default()
        });
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
            stage_verdict(&StageInput {
                pairs: &pairs,
                arity: &arity,
                ..Default::default()
            }),
            "{label}: slice 2 REJECTED a well-typed program, which narrows the language"
        );
    }
}

/// SLICE 3: name resolution, including the shape that is not a type error.
///
/// Three shapes: an undefined function, an undefined identifier, and **calling
/// a local**. The third is a V0.2.0 surface restriction rather than a type
/// error, and it is the one rejection of the fifteen that carries no
/// `type error:` prefix in the reference. A stage locating rejections by that
/// prefix would miss it; a stage reproducing the reference's routing would be
/// reproducing English. It is its own resolution code for that reason.
#[test]
fn slice_three_rejects_unresolved_names_and_called_locals() {
    const IN_SCOPE: &[&str] = &[
        "add-word-and-bool",
        "array-elements-differ",
        "too-few-arguments",
        "too-many-arguments",
        "wrong-argument-type",
        "byte-against-word-argument",
        "undefined-function",
        "undefined-identifier",
        "calling-a-local",
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
        let stage = stage_verdict(&StageInput {
            pairs: &pairs,
            arity: &arity,
            occ: Some(&occurrence_rows(&ast)),
            ..Default::default()
        });
        if IN_SCOPE.contains(label) {
            assert!(
                !stage,
                "{label}: slice 3 has a rule for this shape and accepted it"
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
        "slice 3 rejected shapes it has no rule for: {out_of_scope_rejected:?}"
    );

    // MUST-FIRE on the odd shape specifically. It is the one a prefix-based
    // stage would miss, so its presence in `caught` is asserted by name rather
    // than left to the count.
    assert!(
        caught.contains(&"calling-a-local"),
        "the surface-restriction shape was not among those caught, and it is the one a stage \
         routing on the `type error:` prefix would miss"
    );

    for (label, src) in WELL_TYPED {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let (arity, arg_pairs) = call_rows(&ast);
        let mut pairs = operand_pairs(&ast);
        pairs.extend(arg_pairs);
        assert!(
            stage_verdict(&StageInput {
                pairs: &pairs,
                arity: &arity,
                occ: Some(&occurrence_rows(&ast)),
                ..Default::default()
            }),
            "{label}: slice 3 REJECTED a well-typed program, which narrows the language"
        );
    }
}

/// SLICES 4 and 5: conditions, branch agreement, and structural claims.
///
/// Five more shapes: a non-bool condition, `if` branches of differing types, a
/// struct literal with the wrong field count, indexing a scalar, and a field
/// access on a scalar.
///
/// **Struct field count goes down the call-arity channel.** A struct literal
/// supplying the wrong number of fields is the identical claim as a call
/// supplying the wrong number of arguments, and giving it its own rule would be
/// two spellings of one idea. The second spelling is where they drift apart.
#[test]
fn slices_four_and_five_reject_conditions_and_structural_mismatches() {
    const IN_SCOPE: &[&str] = &[
        "add-word-and-bool",
        "array-elements-differ",
        "too-few-arguments",
        "too-many-arguments",
        "wrong-argument-type",
        "byte-against-word-argument",
        "undefined-function",
        "undefined-identifier",
        "calling-a-local",
        "non-bool-condition",
        "if-branches-differ",
        "wrong-field-count",
        "index-a-scalar",
        "field-access-on-a-scalar",
        // MOVED IN BY THE CONSOLIDATION, not by a new rule. The unified node
        // table carries the function tail against its declared return type, so
        // this shape is now reached by the same input the others are. Asserting
        // it here is STRICTER than leaving it out: an in-scope shape must be
        // rejected, where an out-of-scope one merely must not be rejected for
        // the wrong reason.
        "body-versus-return",
    ];

    let verdict = |src: &str| -> bool {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let (dparams, sites, arg_pairs) = decl_call_rows(&ast);
        stage_verdict(&StageInput {
            pairs: &arg_pairs,
            nodes: &expression_nodes(&ast),
            occ: Some(&occurrence_rows(&ast)),
            dparams: &dparams,
            sites: &sites,
            ..Default::default()
        })
    };

    let mut caught = Vec::new();
    let mut out_of_scope_rejected = Vec::new();

    for (label, src) in ILL_TYPED {
        if tokenize(src).ok().and_then(|t| parse(&t).ok()).is_none() {
            continue;
        }
        let stage = verdict(src);
        if IN_SCOPE.contains(label) {
            assert!(
                !stage,
                "{label}: there is a rule for this shape and it accepted"
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
        "rejected shapes with no rule: {out_of_scope_rejected:?}"
    );

    for (label, src) in WELL_TYPED {
        assert!(
            verdict(src),
            "{label}: REJECTED a well-typed program, which narrows the language"
        );
    }

    // The two still outstanding, asserted as OUTSTANDING rather than left
    // unmentioned. A shape that quietly started passing would otherwise look
    // like coverage nobody claimed.
    // The one shape still outstanding for THIS test's input, asserted as
    // outstanding rather than left unmentioned: a shape that quietly started
    // passing would otherwise look like coverage nobody claimed. It is covered
    // by the whole-corpus test, which passes the field sets this one does not.
    assert!(
        !IN_SCOPE.contains(&"unknown-field"),
        "unknown-field is now in scope here; move it into IN_SCOPE"
    );
}

/// THE FULL CORPUS: every shape rejected, every control accepted.
///
/// The last two are `unknown-field` and `body-versus-return`. With them the
/// stage reaches all sixteen ill-typed programs in **verdict agreement** with
/// the reference, and accepts all seven well-typed ones.
///
/// **Agreement in both directions is the claim, and only the second direction
/// is unbounded.** The rejection side is enumerable and this corpus enumerates
/// it. The acceptance side is not: any valid program must be accepted, and no
/// corpus can establish that. What the controls buy is a check that no rule
/// added along the way narrowed the language, which is the failure mode a
/// rejection-only corpus cannot see.
#[test]
fn the_stage_agrees_with_the_reference_on_the_whole_corpus() {
    let verdict = |src: &str| -> bool {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        // Argument-type pairs remain a pair channel: they compare a DECLARED
        // parameter type against an argument, which is a join with the
        // declaration table rather than a property of one expression.
        let (_, _, arg_pairs) = decl_call_rows(&ast);
        let pairs = arg_pairs;
        // CALL ARITY NO LONGER RIDES THE PRE-JOINED CHANNEL. `arity` carries
        // struct-literal field counts only; call sites go through the join, so
        // the join is load-bearing rather than redundant with a channel that
        // already knew the answer.
        let (dparams, sites, _) = decl_call_rows(&ast);
        // THE WHOLE CORPUS GOES THROUGH THE RESOLVING PATH, which is what makes
        // the four non-literal cases ordinary members rather than exceptions.
        let (bnames, bindings) = binding_rows(&ast);
        let resolvable = expression_nodes_resolvable(&ast, &bnames);
        stage_verdict(&StageInput {
            pairs: &pairs,
            nodes: &[],
            nodes_resolvable: Some(&resolvable),
            bindings: &bindings,
            dparams: &dparams,
            sites: &sites,
            sets: Some(&field_sets(&ast)),
            occ: Some(&occurrence_rows(&ast)),
            ..Default::default()
        })
    };

    let mut checked = 0;
    for (label, src) in ILL_TYPED {
        if tokenize(src).ok().and_then(|t| parse(&t).ok()).is_none() {
            // A source the parser refuses never reaches a type checker, and the
            // reference rejects it for that reason. Counted, not skipped
            // silently.
            checked += 1;
            continue;
        }
        assert!(
            !verdict(src),
            "{label}: the reference rejects this and the stage accepted it"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        ILL_TYPED.len(),
        "not every ill-typed case was reached"
    );

    for (label, src) in WELL_TYPED {
        assert!(
            verdict(src),
            "{label}: the reference accepts this and the stage REJECTED it, which narrows the \
             language rather than being conservative"
        );
    }

    // MUST-FIRE on the corpus being the thing that was checked. Both sides must
    // be non-empty, or the loops above are vacuous and this test reports
    // agreement it never observed.
    assert!(ILL_TYPED.len() >= 20, "the rejection corpus shrank");
    assert!(WELL_TYPED.len() >= 7, "the control corpus shrank");
}

// `the_rules_reach_only_literal_direct_occurrences` STOOD HERE and is retired.
//
// It pinned three programs the reference rejected and the stage accepted, because
// every rule fired only on literal operands. Local resolution reaches all three,
// so they are no longer disagreements: they are ordinary members of `ILL_TYPED`
// above, and the whole-corpus test drives them through the resolving path.
//
// The limit it recorded has not vanished, it has MOVED, and
// `a_derived_operand_from_a_field_read_is_still_unreached` holds the new edge.

// ---------------------------------------------------------------------------
// SIZING SPIKE: what would it take to reach a non-literal operand?
//
// A MEASUREMENT, NOT AN IMPLEMENTATION. Nothing here is wired into the stage.
// The question is what the pipeline would have to compute, and the answer
// determines whether the next increment is small or is a Hindley-Milner port.
// ---------------------------------------------------------------------------

/// A prototype tagger, host-side and deliberately throwaway.
///
/// It extends `expr_tag` with exactly two rules, both LOCAL:
///
/// 1. a `let` whose initialiser is a literal binds that literal's tag;
/// 2. a call to a declared function takes the declared return type's tag.
///
/// **Neither rule unifies anything.** There is no substitution, no occurs check,
/// no type variable. Both are lookups over information the source states
/// outright, which is why this is the cheap end of the design space.
fn prototype_tag(
    e: &keleusma::ast::Expr,
    lets: &std::collections::BTreeMap<String, i64>,
    rets: &std::collections::BTreeMap<String, i64>,
) -> i64 {
    use keleusma::ast::Expr;
    match e {
        Expr::Literal { value, .. } => literal_tag(value),
        Expr::Ident { name, .. } => lets.get(name).copied().unwrap_or(0),
        Expr::Call { name, .. } => rets.get(name).copied().unwrap_or(0),
        _ => 0,
    }
}

/// How far the two local rules get, measured over cases the stage accepts today.
///
/// **This is the sizing result and it is the whole point of the spike.** If every
/// currently-missed rejection falls to local propagation, the next increment is a
/// tagger over records the pipeline already emits. If some need unification, the
/// next increment is much larger and should be planned as such.
#[test]
fn sizing_how_far_local_propagation_reaches() {
    use keleusma::ast::{Expr, Pattern, Stmt, TypeExpr};
    use keleusma::visitor::Visitor;
    use std::collections::BTreeMap;

    // (label, source, needs) where `needs` names the least information that
    // would decide the operand types.
    const CASES: &[(&str, &str, &str)] = &[
        (
            "let-bound literal",
            "fn main() -> Word { let b = true; 1 + b }",
            "local: let initialiser",
        ),
        (
            "call return",
            "fn g() -> Word { 1 }\nfn main() -> Word { g() + true }",
            "local: declared return type",
        ),
        (
            "two let-bound literals",
            "fn main() -> Word { let a = 1; let b = true; a + b }",
            "local: let initialiser",
        ),
        (
            "let bound to a call",
            "fn g() -> Word { 1 }\nfn main() -> Word { let a = g(); a + true }",
            "local: both rules composed",
        ),
        (
            "parameter operand",
            "fn f(a: Word) -> Word { a + true }\nfn main() -> Word { f(1) }",
            "local: declared parameter type",
        ),
    ];

    let mut reached = 0;
    let mut missed: Vec<&str> = Vec::new();

    for (label, src, _needs) in CASES {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");

        // The reference must reject, or the case measures nothing.
        let mut for_ref = ast.clone();
        assert!(
            keleusma::typecheck::check(&mut for_ref).is_err(),
            "{label}: the reference ACCEPTS this; it is not an ill-typed case"
        );

        // Declared return types, by name.
        let mut rets: BTreeMap<String, i64> = BTreeMap::new();
        for f in &ast.functions {
            if let TypeExpr::Prim(p, _) = &f.return_type {
                rets.insert(f.name.clone(), prim_tag(p));
            }
        }

        // Let-bound literals and declared parameters, by name.
        struct Binds<'a> {
            lets: BTreeMap<String, i64>,
            rets: &'a BTreeMap<String, i64>,
        }
        impl Visitor for Binds<'_> {
            fn visit_stmt(&mut self, stmt: &Stmt) {
                if let Stmt::Let(l) = stmt
                    && let Pattern::Variable(n, _) = &l.pattern
                {
                    let t = prototype_tag(&l.value, &self.lets, self.rets);
                    if t != 0 {
                        self.lets.insert(n.clone(), t);
                    }
                }
                self.walk_stmt(stmt);
            }
        }
        let mut binds = Binds {
            lets: BTreeMap::new(),
            rets: &rets,
        };
        for f in &ast.functions {
            for (pname, pty) in f
                .params
                .iter()
                .filter_map(|p| match (&p.pattern, &p.type_expr) {
                    (Pattern::Variable(n, _), Some(TypeExpr::Prim(x, _))) => {
                        Some((n.clone(), prim_tag(x)))
                    }
                    _ => None,
                })
            {
                binds.lets.insert(pname, pty);
            }
            binds.visit_block(&f.body);
        }

        // Would a binary operation now disagree?
        struct Check<'a> {
            lets: &'a BTreeMap<String, i64>,
            rets: &'a BTreeMap<String, i64>,
            caught: bool,
        }
        impl Visitor for Check<'_> {
            fn visit_expr(&mut self, expr: &Expr) {
                if let Expr::BinOp { left, right, .. } = expr {
                    let l = prototype_tag(left, self.lets, self.rets);
                    let r = prototype_tag(right, self.lets, self.rets);
                    if l != 0 && r != 0 && l != r {
                        self.caught = true;
                    }
                }
                self.walk_expr(expr);
            }
        }
        let mut chk = Check {
            lets: &binds.lets,
            rets: &rets,
            caught: false,
        };
        for f in &ast.functions {
            chk.visit_block(&f.body);
        }

        if chk.caught {
            reached += 1;
        } else {
            missed.push(label);
        }
    }

    println!("local propagation reaches {reached} of {}", CASES.len());
    if !missed.is_empty() {
        println!("not reached: {missed:?}");
    }

    // THE PINNED RESULT. If this number moves, the sizing in
    // TYPECHECK_INFERENCE_SIZING.md is stale and must be re-derived.
    assert_eq!(
        reached,
        CASES.len(),
        "local propagation reached {reached} of {}; the cases it missed are {missed:?}, and \
         each one is evidence that something beyond the two local rules is needed",
        CASES.len()
    );
}

/// The primitive-type tag, matching `literal_tag`'s numbering.
fn prim_tag(p: &keleusma::ast::PrimType) -> i64 {
    use keleusma::ast::PrimType as P;
    match p {
        P::Word => 1,
        P::Bool => 2,
        P::Byte => 3,
        P::Float => 4,
        _ => 0,
    }
}

/// Drive the stage with the binding table and resolvable operands.
fn stage_verdict_resolving(src: &str) -> bool {
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    let (names, mut bindings) = binding_rows(&ast);
    let (nodes, derived) = expression_nodes_and_derived(&ast, &names);
    // FORM 2: the binding takes whatever expression node `idx` yields. The host
    // says only WHICH node the initialiser is -- a syntactic fact, like a literal
    // tag or an alias name. Resolving that node's operands and requiring them to
    // agree is the stage's bounded fixpoint.
    for (n, idx) in derived {
        if let Some(&id) = names.get(&n) {
            bindings.push((id, idx, 2));
        }
    }
    let (dparams, sites, arg_pairs) = decl_call_rows(&ast);
    stage_verdict(&StageInput {
        pairs: &arg_pairs,
        nodes: &[],
        nodes_resolvable: Some(&nodes),
        bindings: &bindings,
        dparams: &dparams,
        sites: &sites,
        sets: Some(&field_sets(&ast)),
        occ: Some(&occurrence_rows(&ast)),
        ..Default::default()
    })
}

/// THE SLICE: an error whose operands are NOT literals is now rejected.
///
/// Each case is one the stage accepted before local resolution existed, pinned
/// then in `the_rules_reach_only_literal_direct_occurrences` as a measured
/// disagreement. They are rejections now.
///
/// # What moved, and what did not
///
/// **The stage gained the join.** It receives `(name, tag)` rows the host read
/// off the source and operands tagged as "this is name N", and it resolves one
/// through the other. The host does not decide any operand's type; see
/// `the_stage_and_not_the_host_resolves_an_operand`, which is what makes that
/// claim checkable rather than asserted.
///
/// **The input structure still comes from the reference parser.** This slice
/// moves the RESOLUTION into the stage; it does not move the EXTRACTION. The
/// checker is not self-hosted and no claim here should say it is.
///
/// # What this does NOT establish
///
/// Four cases are a case list, and the tags reach only what the source declares
/// or literally initialises. An operand whose type comes from a FIELD READ or an
/// INDEX is still UNKNOWN and still accepted, pinned by
/// `a_derived_operand_from_a_field_read_is_still_unreached`. **An ARITHMETIC
/// result is no longer in that set**: a bounded fixpoint reaches it.
#[test]
fn resolution_reaches_an_operand_that_is_not_a_literal() {
    let cases: &[(&str, &str)] = &[
        (
            "operand-through-a-let",
            "fn main() -> Word { let b = true; 1 + b }",
        ),
        (
            "operand-through-a-call",
            "fn g() -> Word { 1 }\nfn main() -> Word { g() + true }",
        ),
        (
            "both-operands-through-lets",
            "fn main() -> Word { let a = 1; let b = true; a + b }",
        ),
        (
            "let-bound-to-a-call",
            "fn g() -> Word { 1 }\nfn main() -> Word { let a = g(); a + true }",
        ),
    ];

    let mut rejected = 0;
    for (label, src) in cases {
        let mut for_ref = parse(&tokenize(src).expect("lex")).expect("parse");
        assert!(
            keleusma::typecheck::check(&mut for_ref).is_err(),
            "{label}: the reference ACCEPTS this, so it measures nothing"
        );
        assert!(
            !stage_verdict_resolving(src),
            "{label}: the reference rejects this and the stage accepted it"
        );
        rejected += 1;
    }
    assert_eq!(rejected, cases.len(), "not every case was reached");
}

/// MUST FIRE: without the binding table the same programs are ACCEPTED.
///
/// **This is what separates the stage doing the join from the host doing it.**
/// The operands are identical in both runs; only the rows the stage resolves
/// through are withheld. If these still rejected, the type would be arriving
/// already decided and the slice above would prove nothing about where the
/// reasoning happens.
#[test]
fn the_stage_and_not_the_host_resolves_an_operand() {
    let src = "fn main() -> Word { let b = true; 1 + b }";
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    let (names, bindings) = binding_rows(&ast);
    let nodes = expression_nodes_resolvable(&ast, &names);
    assert!(
        !bindings.is_empty() && nodes.iter().any(|(_, _, af, _, bf)| *af == 1 || *bf == 1),
        "the subject must carry both a binding row and a name-form operand, or this \
         comparison is between two identical runs"
    );

    let (dparams, sites, arg_pairs) = decl_call_rows(&ast);
    let with = stage_verdict(&StageInput {
        pairs: &arg_pairs,
        nodes: &[],
        nodes_resolvable: Some(&nodes),
        bindings: &bindings,
        dparams: &dparams,
        sites: &sites,
        sets: Some(&field_sets(&ast)),
        occ: Some(&occurrence_rows(&ast)),
        ..Default::default()
    });
    let without = stage_verdict(&StageInput {
        pairs: &arg_pairs,
        nodes: &[],
        nodes_resolvable: Some(&nodes),
        bindings: &[],
        dparams: &dparams,
        sites: &sites,
        sets: Some(&field_sets(&ast)),
        occ: Some(&occurrence_rows(&ast)),
        ..Default::default()
    });

    assert!(!with, "with the binding rows the stage must reject");
    assert!(
        without,
        "WITHOUT the binding rows the stage still rejected, so the operand's type is not \
         being resolved from those rows and the host is deciding it somewhere"
    );
}

/// The limit that remains, pinned as a disagreement so it cannot be forgotten.
///
/// A binding table built from declarations and literal initialisers says nothing
/// about an operand whose type is DERIVED — here, the result of an arithmetic
/// expression. The reference rejects; the stage accepts. Reaching it needs a
/// rule that propagates through operators, which is a fixpoint rather than a
/// lookup and is deliberately out of this slice.
#[test]
fn a_derived_operand_is_now_reached_and_the_chain_has_no_depth_limit() {
    // `a` is bound to an OPERATOR EXPRESSION, so before this it was UNKNOWN and
    // `a + b` was accepted. The stage now proves `a` is Word from its operands and
    // rejects the mixed addition.
    let src = "fn main() -> Word { let a = 1 + 2; let b = true; a + b }";
    let mut for_ref = parse(&tokenize(src).expect("lex")).expect("parse");
    assert!(
        keleusma::typecheck::check(&mut for_ref).is_err(),
        "the reference ACCEPTS this, so it measures nothing"
    );
    assert!(
        !stage_verdict_resolving(src),
        "the stage must reject a derived operand it can prove"
    );

    // A CHAIN OF ANY DEPTH, AND THE ROUND CAP IS NOT WHAT ALLOWS IT.
    //
    // Measured, because the obvious reading is wrong: `tyb_rounds()` is 4, so the
    // natural claim is "chains up to four". Setting it to 1 rejects every depth
    // below just the same. **Scoping forces `let` bindings into dependency order**
    // -- `let v3 = v2 + 1` cannot precede `v2` -- so one pass over the table in
    // walk order proves the whole chain. The cap is insurance for a future channel
    // that supplies rows out of order, not the bound on this construct.
    for depth in 1..=6usize {
        let mut s = String::from("fn main() -> Word { let v0 = 1 + 2;");
        for i in 1..depth {
            s.push_str(&format!(" let v{i} = v{} + 1;", i - 1));
        }
        s.push_str(&format!(" let b = true; v{} + b }}", depth - 1));
        let mut r = parse(&tokenize(&s).expect("lex")).expect("parse");
        assert!(
            keleusma::typecheck::check(&mut r).is_err(),
            "depth {depth}: the reference accepts it, so the case measures nothing"
        );
        assert!(
            !stage_verdict_resolving(&s),
            "depth {depth}: the stage accepted a chain it should have proved"
        );
    }
}

/// **WHAT A DERIVED OPERAND STILL DOES NOT REACH**, so the next increment knows
/// where the edge is.
///
/// `tyb_node_tag` proves a tag only for an operator node whose two operands agree.
/// A field read and an index are other node kinds, so a `let` bound to one is
/// still UNKNOWN and therefore ACCEPTED -- the safe direction, and the same one
/// the single alias hop takes.
///
/// This is a PIN, not an aspiration: if a later increment reaches these, it fails
/// here and the author records what it now reaches.
#[test]
fn a_derived_operand_from_a_field_read_is_still_unreached() {
    let src = "struct P { x: Word }\n\
               fn main(p: P) -> Word { let a = p.x; let b = true; a + b }";
    let mut for_ref = parse(&tokenize(src).expect("lex")).expect("parse");
    assert!(
        keleusma::typecheck::check(&mut for_ref).is_err(),
        "the reference ACCEPTS this, so it measures nothing"
    );
    assert!(
        stage_verdict_resolving(src),
        "the stage now reaches a field-read initialiser. Record what it reaches and \
         move this case into the positive test above."
    );
}

/// **THE CHECK THAT MAKES THE COROUTINE CONVERSION MEAN SOMETHING.**
///
/// `verify_types.kel` was `fn main(cmd)` doing all eight table folds in one call.
/// It is now `loop main(resume)` yielding `PENDING` per folded row, matching the
/// nine sibling stages and the windowed-compiler goal.
///
/// Every other test in this file asserts a VERDICT, and a stage that kept doing
/// the whole fold in its first step and yielded the answer immediately would
/// satisfy all of them while streaming nothing. Changing the entry shape and
/// changing the execution shape are different things, and only the resume count
/// tells them apart.
///
/// Two properties, and the second is the load-bearing one:
///
/// 1. A fold with rows takes strictly more resumes than the empty floor, so rows
///    really are consumed one per step.
/// 2. **More rows take strictly more resumes.** A constant count would mean the
///    stage finishes in a fixed number of steps regardless of input, which is
///    what a one-shot fold behind a coroutine shell looks like from outside.
#[test]
fn the_fold_advances_one_row_per_resume() {
    // The floor: no rows at all. Eight phase advances plus the verdict.
    let (_, empty_steps) = stage_verdict_counting(&StageInput::default());

    // Well-typed rows, so the verdict stays ACCEPT and the count is the only
    // thing that moves. A rejecting corpus would fold identically, but tying the
    // measurement to an accepting one keeps this test about stepping rather than
    // about any rule.
    let few: Vec<(i64, i64)> = (0..4).map(|_| (1, 1)).collect();
    let many: Vec<(i64, i64)> = (0..40).map(|_| (1, 1)).collect();

    let (few_ok, few_steps) = stage_verdict_counting(&StageInput {
        pairs: &few,
        ..Default::default()
    });
    let (many_ok, many_steps) = stage_verdict_counting(&StageInput {
        pairs: &many,
        ..Default::default()
    });

    assert!(
        few_ok && many_ok,
        "the subjects must be well typed, or this measures a rejection path"
    );
    assert!(
        few_steps > empty_steps,
        "four rows took {few_steps} resumes against an empty fold's {empty_steps}, so rows \
         are not being consumed one per step"
    );
    assert_eq!(
        many_steps - few_steps,
        many.len() - few.len(),
        "thirty-six extra rows cost {} extra resumes. One row per resume is the property \
         the conversion claims; anything else means the stage is batching internally.",
        many_steps - few_steps
    );
}

/// **THE DECLARED BINDING ROWS NOW COME FROM THE PIPELINE, AND AGREE WITH THE
/// REFERENCE EXTRACTION.**
///
/// Order 1 asks for the type checker's input to come from `parse.kel` plus
/// `reconstruct.kel` rather than from Rust walking the reference AST. This is the
/// first slice of that: the bindings the source states outright — a function's
/// declared return type and each parameter's declared type.
///
/// # Why the comparison is by NAME STRING and not by id
///
/// The two extractions live in different id spaces. The reference one assigns ids by
/// insertion order as it walks; the pipeline uses the lexer's intern table. Comparing
/// ids would compare the numbering, not the content. **Names are the thing both
/// claim to describe**, so the rows are compared as `(name, tag, form)` with the name
/// spelled out.
///
/// # What this does NOT establish
///
/// Only the DECLARED bindings. A `let` bound to a literal or a call is still absent
/// from the pipeline side: its initialiser's shape lives in the body record stream,
/// and reading it means walking the forest rather than the header. That is the next
/// slice, and `the_pipeline_rows_are_the_declared_subset` below pins the boundary so
/// it cannot be mistaken for completeness.
#[cfg(feature = "self-host")]
#[test]
fn the_declared_binding_rows_agree_between_the_pipeline_and_the_reference() {
    const SOURCES: &[&str] = &[
        "fn g(alpha: Word, beta: bool) -> Word { 1 }\nfn main() -> Word { g(1, true) }",
        "fn f(a: Word) -> bool { true }\nfn main() -> Word { 1 }",
        "fn one() -> Word { 1 }\nfn two(x: Byte) -> Byte { x }\nfn main() -> Word { one() }",
        "fn main(p: Word, q: Word) -> Word { p + q }",
        // LET-BOUND LITERALS, folded in on 2026-08-20 when the pipeline reached
        // them. `the_pipeline_rows_are_the_declared_subset` told this test to do
        // exactly that rather than delete its pin.
        "fn main() -> Word { let a = 7; a }",
        "fn main() -> bool { let b = true; b }",
        "fn main() -> Word { let a = 7; let c = 8; a + c }",
    ];

    for src in SOURCES {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let (ref_names, ref_rows) = binding_rows(&ast);

        // The reference rows carry ids; render them as names for comparison.
        let ref_name_of = |id: i64| -> Option<String> {
            ref_names
                .iter()
                .find(|(_, v)| **v == id)
                .map(|(k, _)| k.clone())
        };
        // BOTH FORMS NOW. A form-0 row carries a tag and no target; a form-1 row
        // carries the TARGET'S NAME, so the two extractions compare by name on
        // both halves and neither side's id space enters the comparison.
        let mut want: Vec<keleusma::selfhost::BindingRow> = ref_rows
            .iter()
            .filter_map(|(n, t, f)| {
                let name = ref_name_of(*n)?;
                if *f == 0 {
                    Some((name, *t, 0, String::new()))
                } else {
                    // For an alias the reference stores the TARGET's id where a
                    // form-0 row stores a tag, so the value renders as a name.
                    Some((name, 0, 1, ref_name_of(*t)?))
                }
            })
            .collect();

        let (_, pipeline_rows) = keleusma::selfhost::binding_rows_from_pipeline(src);
        let mut got = pipeline_rows.clone();

        want.sort();
        got.sort();
        assert!(
            !want.is_empty(),
            "{src:?}: the reference produced no declared rows, so this case measures \
             nothing"
        );
        assert_eq!(
            got, want,
            "{src:?}: the pipeline extraction disagrees with the reference on the \
             binding rows"
        );
    }
}

/// **THE BOUNDARY, RESTATED AGAIN: CALLS ARE REACHED; OPERATOR EXPRESSIONS ARE
/// NOT.**
///
/// This pin has now moved twice, and each move is the increment its predecessor
/// asked for. It first asserted the pipeline carried NO `let` row at all; then
/// that it reached literals but not calls, with the instruction *"give the row
/// shape a target STRING so it can be compared without comparing id spaces, then
/// fold this case into the agreement test."* That is done, and the call case is
/// now compared against the reference in
/// [`the_declared_binding_rows_agree_between_the_pipeline_and_the_reference`].
///
/// **Restated rather than removed**, because what it guards has moved rather than
/// gone. One form remains out of reach.
///
/// # An operator expression is blocked by the CHANNEL
///
/// `let d = 1 + 2` needs the initialiser's NODE INDEX to reach the stage's bounded
/// fixpoint, form 2. That index has to survive into the type channel, and nothing
/// carries it there yet.
///
/// Producing no row means the stage ACCEPTS, which is this project's documented
/// conservative stance rather than an oversight.
#[cfg(feature = "self-host")]
#[test]
fn the_pipeline_reaches_calls_but_not_operator_expressions() {
    // THE REACHED CASES FIRST. Without them a pipeline that returned nothing would
    // satisfy the assertion below while having regressed.
    let (_, reached) =
        keleusma::selfhost::binding_rows_from_pipeline("fn main() -> Word { let a = 7; a }");
    assert!(
        reached
            .iter()
            .any(|(n, t, f, _)| n == "a" && *t == 1 && *f == 0),
        "the pipeline no longer carries a let-bound literal, which is a regression \
         rather than a boundary: {reached:?}"
    );

    const CALL: &str = "fn g() -> Word { 1 }\nfn main() -> Word { let c = g(); c }";
    let (_, call_rows) = keleusma::selfhost::binding_rows_from_pipeline(CALL);
    assert!(
        call_rows
            .iter()
            .any(|(n, _, f, target)| n == "c" && *f == 1 && target == "g"),
        "the pipeline no longer carries a let-bound CALL as an alias naming `g`. That is \
         a regression, not a boundary: {call_rows:?}"
    );

    // AN OPERATOR EXPRESSION. The reference registers the name; the pipeline
    // deliberately produces no row.
    let (_, op_rows) =
        keleusma::selfhost::binding_rows_from_pipeline("fn main() -> Word { let d = 1 + 2; d }");
    assert!(
        !op_rows.iter().any(|(n, _, _, _)| n == "d"),
        "the pipeline now carries a let-bound OPERATOR EXPRESSION. Record which form it \
         uses and fold the case into the agreement test."
    );
}

/// **`bool` IS THE BOOLEAN PRIMITIVE. `Bool` IS AN ORDINARY NAMED TYPE.**
///
/// Measured 2026-08-20 by parsing each spelling and reading the `TypeExpr`
/// constructor: `Word`, `Byte` and `Float` are `Prim` and capitalised; **`bool` is
/// `Prim` and lowercase, the only one**; `Bool` is `Named`. The reference rejects
/// `fn f(b: Bool) -> Word { 1 + b }` with "cannot add Word and Bool" — a named type
/// it cannot add, not a boolean.
///
/// # The defect this exists to prevent returning
///
/// An earlier increment added a `Named` arm mapping `Bool` to the stage's boolean
/// tag, on the reasoning that a match on `Prim` alone "silently drops every `Bool`
/// annotation". The observation was true and the conclusion was backwards: those
/// annotations are dropped because they are NOT booleans.
///
/// **The suite could not catch it, and that is the lesson.** The same wrong change
/// was made on BOTH sides of a differential comparison — the reference-AST
/// extraction and the pipeline extraction, which keys on the type name string. Two
/// wrongs agreeing is a green test. A differential oracle only detects a defect
/// introduced on ONE side, and the common cause was the author.
///
/// # Why this asserts on the EXTRACTION and not on the verdict
///
/// The obvious test — that the stage rejects a `Bool`-typed value used as an `if`
/// condition — does not discriminate. **Measured before the fix, the stage accepted
/// it** because it believed the value was a boolean; after the fix it accepts it
/// again because the tag is unknown and the stage defers on unknown, which is this
/// project's documented conservative stance. Same verdict, opposite reasons.
///
/// The tag itself is the thing that was wrong, so the tag is what is asserted.
#[test]
fn a_named_type_called_bool_is_not_the_boolean_primitive() {
    // THE REFERENCE IS THE AUTHORITY, and it is checked first so that a change in
    // its behaviour surfaces here rather than silently redefining the expectation.
    assert!(
        !reference_accepts("fn f(b: Bool) -> Word { 1 + b }\nfn main() -> Word { 1 }"),
        "the reference now adds `Word` and `Bool`, so `Bool` has become the boolean \
         primitive and this test is obsolete"
    );
    assert!(
        reference_accepts(
            "fn f(b: bool) -> Word { if b { 1 } else { 2 } }\nfn main() -> Word { 1 }"
        ),
        "the reference rejects a genuine `bool` condition, so the control is broken"
    );

    // NO ROW AND NO TAG ARE THE SAME ANSWER. A binding whose type yields no tag is
    // never interned, so the name is absent from the table rather than present with
    // a zero — and that absence IS the correct outcome for a type the stage has no
    // scalar for. Treating a missing name as anything but 0 would make the assertion
    // below unreachable.
    let tag_for = |src: &str| -> i64 {
        let ast = parse(&tokenize(src).expect("lex")).expect("parse");
        let (names, rows) = binding_rows(&ast);
        let Some(&id) = names.get("b") else { return 0 };
        rows.iter()
            .find(|(n, _, form)| *n == id && *form == 0)
            .map(|(_, t, _)| *t)
            .unwrap_or(0)
    };

    const REAL: &str = "fn f(b: bool) -> Word { 1 }\nfn main() -> Word { 1 }";
    const NAMED: &str = "fn f(b: Bool) -> Word { 1 }\nfn main() -> Word { 1 }";

    // The control first. Without it, a `tag_for` that returned 0 for everything
    // would satisfy the real assertion while measuring nothing.
    assert_eq!(
        tag_for(REAL),
        2,
        "a `bool` annotation must carry the boolean tag, or the case below is vacuous"
    );
    assert_eq!(
        tag_for(NAMED),
        0,
        "a `Bool` annotation carries the BOOLEAN tag. `Bool` is an ordinary named \
         type and the reference cannot add it to a `Word`; calling it a boolean tells \
         the type channel something false"
    );

    // AND THE PIPELINE EXTRACTION MUST AGREE WITH THE REFERENCE COMPILER, not with
    // the extraction above. Both were wrong together once; comparing them to each
    // other is what failed to notice.
    #[cfg(feature = "self-host")]
    {
        let pipeline_tag = |src: &str| -> i64 {
            let (_, rows) = keleusma::selfhost::binding_rows_from_pipeline(src);
            rows.iter()
                .find(|(n, _, form, _)| n == "b" && *form == 0)
                .map(|(_, t, _, _)| *t)
                .unwrap_or(0)
        };
        assert_eq!(
            pipeline_tag(REAL),
            2,
            "the pipeline drops a genuine `bool` annotation"
        );
        assert_eq!(
            pipeline_tag(NAMED),
            0,
            "the pipeline calls the named type `Bool` a boolean"
        );
    }
}

/// **THE DECLARATION AND CALL-SITE ROWS AGREE BETWEEN THE PIPELINE AND THE REFERENCE.**
///
/// The second of the five type-channel extractions to gain a pipeline analogue, after
/// `binding_rows`. Two of five are now moved; the remaining three still walk the reference
/// parser's abstract syntax tree.
///
/// # The comparison is by NAME on both sides, and that is load-bearing
///
/// The reference numbers functions in DECLARATION order; the pipeline numbers chunks by
/// SORTED name. Comparing indices would compare two unrelated numberings and pass or fail for
/// reasons unrelated to the rows. The previous slice hit the same trap with a name id, and
/// the escape recorded there was that "carrying a string removes the question rather than
/// answering it".
///
/// # What is NOT moved, said plainly
///
/// `decl_call_rows` also returns a per-argument pair of (declared parameter tag, ACTUAL
/// ARGUMENT tag). The actual-argument tag needs an expression classifier, which is a new
/// piece of work rather than a re-projection of data the driver already holds. It stays in
/// Rust and this test does not pretend otherwise.
#[test]
fn the_declaration_and_call_rows_agree_between_the_pipeline_and_the_reference() {
    const SOURCES: &[&str] = &[
        "fn g(alpha: Word, beta: bool) -> Word { 1 }\nfn main() -> Word { g(1, true) }",
        "fn one() -> Word { 1 }\nfn two(x: Byte) -> Byte { x }\nfn main() -> Word { one() }",
        "fn main(p: Word, q: Word) -> Word { p + q }",
        // A callee declared AFTER its caller, so declaration order and sorted order differ.
        // Without this the two numberings could coincide and the test would prove nothing.
        "fn zzz(a: Word) -> Word { a }\nfn aaa() -> Word { zzz(1) }\nfn main() -> Word { aaa() }",
        // Two calls to one callee, and a call with no arguments.
        "fn g(a: Word) -> Word { a }\nfn h() -> Word { 0 }\nfn main() -> Word { g(1) + g(2) + h() }",
    ];

    let mut total_decls = 0usize;
    let mut total_sites = 0usize;

    for src in SOURCES {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex"))
            .expect("the reference must parse a probe before it says anything about the stage");

        // The reference's rows, re-expressed by NAME so neither numbering is compared.
        let mut want_decls: Vec<(String, i64, Vec<i64>)> = ast
            .functions
            .iter()
            .map(|f| {
                (
                    f.name.clone(),
                    f.params.len() as i64,
                    f.params
                        .iter()
                        .map(|p| p.type_expr.as_ref().map_or(0, type_tag))
                        .collect(),
                )
            })
            .collect();

        let (params, sites, _args) = decl_call_rows(&ast);
        let order: Vec<String> = ast.functions.iter().map(|f| f.name.clone()).collect();
        let mut want_sites: Vec<(String, i64)> = sites
            .iter()
            .map(|(i, argc)| (order[*i as usize].clone(), *argc))
            .collect();
        assert_eq!(
            params.len(),
            want_decls.len(),
            "{src:?}: the reference's own two views of its declarations disagree"
        );

        let (mut got_decls, mut got_sites) = keleusma::selfhost::decl_call_rows_from_pipeline(src);

        want_decls.sort();
        got_decls.sort();
        want_sites.sort();
        got_sites.sort();

        assert_eq!(
            got_decls, want_decls,
            "{src:?}: the pipeline disagrees with the reference on the declaration rows"
        );
        assert_eq!(
            got_sites, want_sites,
            "{src:?}: the pipeline disagrees with the reference on the call sites"
        );

        total_decls += got_decls.len();
        total_sites += got_sites.len();
    }

    // **AND THE CORPUS MUST SEPARATE THE TWO NUMBERINGS.** If declaration order and sorted
    // order coincided for every source, comparing by name would be indistinguishable from
    // comparing by index, and the property this test exists to establish would go untested
    // while the test passed. At least one source must declare a function out of sorted order.
    let separates = SOURCES.iter().any(|src| {
        let ast =
            keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse");
        let declared: Vec<String> = ast.functions.iter().map(|f| f.name.clone()).collect();
        let mut sorted = declared.clone();
        sorted.sort();
        sorted.dedup();
        let mut seen = declared.clone();
        seen.dedup();
        seen != sorted
    });
    assert!(
        separates,
        "no source in this corpus declares a function out of sorted order, so comparing by \
         name is indistinguishable from comparing by index here and the test establishes \
         nothing about the numbering"
    );

    // NON-VACUOUS. An empty-versus-empty match is not agreement, and two derivations in this
    // repository have passed while comparing nothing at all.
    assert!(
        total_decls >= 10,
        "only {total_decls} declaration rows were compared across the corpus, so this \
         measures far less than it appears to"
    );
    assert!(
        total_sites >= 5,
        "only {total_sites} call sites were compared, so the call half of this test is \
         close to vacuous"
    );
}

/// **TWO OF THE FIVE TYPE-CHANNEL EXTRACTIONS ARE MOVED, AND THE FIGURE IS DERIVED.**
///
/// The file header names all five. This counts how many have a pipeline analogue rather than
/// restating a number, because a hand-written count is a second definition that goes stale —
/// which is how a handoff came to assert a closed gap was open.
#[test]
fn the_moved_extraction_count_is_two_of_five() {
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");
    let named = [
        "decl_call_rows",
        "expression_nodes_resolvable",
        "field_sets",
        "occurrence_rows",
        "binding_rows",
    ];
    let moved: Vec<&str> = named
        .iter()
        .copied()
        .filter(|n| DRIVER.contains(&format!("pub fn {n}_from_pipeline")))
        .collect();
    assert_eq!(
        moved.len(),
        2,
        "the count of moved extractions changed. Moved: {moved:?}. Up is the point — update \
         this number and say which one moved. Down means an analogue was removed."
    );
}
