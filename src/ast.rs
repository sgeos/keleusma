extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::Span;

/// A complete Keleusma program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// `use` import declarations.
    pub uses: Vec<UseDecl>,
    /// Type declarations: structs, enums, newtypes.
    pub types: Vec<TypeDef>,
    /// `data` block declarations.
    pub data_decls: Vec<DataDecl>,
    /// Function definitions (every category, including the entry point).
    pub functions: Vec<FunctionDef>,
    /// Trait declarations.
    pub traits: Vec<TraitDef>,
    /// `impl Trait for Type` blocks.
    pub impls: Vec<ImplBlock>,
    /// Span of the source file.
    pub span: Span,
}

/// A trait declaration.
///
/// Traits declare a set of method signatures that an implementing
/// type provides. The body of a trait method declaration is the
/// signature only; concrete implementations are supplied by `impl`
/// blocks. Bounded type parameters in function signatures restrict
/// the parameter to types that implement the named trait.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    /// Trait name.
    pub name: String,
    /// Type parameters in declaration order.
    pub type_params: Vec<TypeParam>,
    /// Method signatures the trait requires.
    pub methods: Vec<TraitMethodSig>,
    /// Span of the trait declaration.
    pub span: Span,
}

/// A method signature inside a trait declaration.
///
/// The signature mirrors a function declaration without a body. The
/// implicit `self` parameter is the first parameter when present and
/// has type `Self` (the implementing type).
#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethodSig {
    /// Method name.
    pub name: String,
    /// Parameter list in declaration order. The implicit `self`
    /// parameter, when present, is the first entry.
    pub params: Vec<Param>,
    /// Return type expression.
    pub return_type: TypeExpr,
    /// Span of the signature.
    pub span: Span,
}

/// A trait implementation for a concrete type.
///
/// `impl Trait for Type { method definitions }`. The methods supply
/// the concrete bodies for the trait's declared signatures. The
/// implementation registers a method-resolution table entry keyed on
/// the (Trait, Type, method) triple at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    /// Name of the trait being implemented.
    pub trait_name: String,
    /// Type parameters introduced by the impl block itself, allowing
    /// `impl Trait for Box<T>` style declarations. Empty for
    /// concrete-type impls.
    pub type_params: Vec<TypeParam>,
    /// The implementing type expression. For nominal types this is
    /// typically `TypeExpr::Named(Type, args)`.
    pub for_type: TypeExpr,
    /// Method bodies supplied by the impl block.
    pub methods: Vec<FunctionDef>,
    /// Span of the impl block.
    pub span: Span,
}

/// A `data` block declaration for persistent mutable state.
///
/// Data blocks define named contexts whose fields persist across RESET
/// boundaries. The host initializes the data slots before execution.
/// Script code reads and writes fields via `name.field` syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct DataDecl {
    /// Data-block name.
    pub name: String,
    /// Fields declared in the data block.
    pub fields: Vec<DataFieldDecl>,
    /// Visibility of the data block to the host. `Shared` is the
    /// default and matches today's behaviour. `Private` data lives in
    /// the arena's persistent region and is not exposed through the
    /// host API.
    pub visibility: DataVisibility,
    /// Span of the data declaration.
    pub span: Span,
}

/// Visibility of a [`DataDecl`] to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataVisibility {
    /// Shared with the host. Default when no modifier is present.
    /// Host reads and writes through `Vm::set_data` and `Vm::get_data`.
    Shared,
    /// Private to the script. Lives in the arena's persistent region.
    /// No host API. Persists across resets.
    Private,
    /// Compile-time constant. Field reads compile to constant
    /// loads; writes are compile errors. No runtime data-segment
    /// slot is allocated. Each field carries a literal
    /// initializer in the source.
    Const,
}

/// A field in a data block declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct DataFieldDecl {
    /// Field name.
    pub name: String,
    /// Declared field type.
    pub type_expr: TypeExpr,
    /// Compile-time initializer. Required for fields of
    /// `const data` declarations; rejected on `shared` and
    /// `private` data declarations where the host or the script
    /// supplies values at runtime.
    pub initializer: Option<ConstInitializer>,
    /// Span of the field declaration.
    pub span: Span,
}

/// Compile-time initializer for a `const data` field. Distinct
/// from [`Literal`] because const initializers may nest tuples,
/// arrays, struct literals, and enum variant constructions
/// whereas pattern literals are always scalar.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstInitializer {
    /// Scalar primitive literal: integer, float, boolean, text,
    /// or unit. Negation is folded into the literal value at
    /// parse time.
    Scalar(Literal),
    /// Tuple literal: `(init, init, ...)`. Element count and
    /// element types are validated against the declared field
    /// type at compile time.
    Tuple(Vec<ConstInitializer>),
    /// Array literal: `[init, init, ...]`. Length and element
    /// type are validated against the declared field type at
    /// compile time.
    Array(Vec<ConstInitializer>),
    /// Struct literal: `Name { field: init, ... }`. Field names
    /// and types are validated against the declared struct type
    /// at compile time.
    Struct {
        /// Name of the struct type being constructed.
        name: String,
        /// Per-field initializer in source order.
        fields: Vec<(String, ConstInitializer)>,
    },
    /// Enum variant construction: `Enum::Variant` for unit
    /// variants, `Enum::Variant(init, init, ...)` for tuple
    /// payloads. The enum name and variant are validated against
    /// the declared enum type at compile time.
    Enum {
        /// Name of the enum type.
        enum_name: String,
        /// Name of the variant being constructed.
        variant: String,
        /// Positional payload initializers for tuple-variant
        /// constructions. Empty for unit variants.
        args: Vec<ConstInitializer>,
    },
}

/// A `use` import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// Path segments leading to the import target, e.g.
    /// `["audio"]` for `use audio::set_frequency`.
    pub path: Vec<String>,
    /// What the `use` declaration imports (a specific name or a
    /// wildcard).
    pub import: ImportItem,
    /// Optional declared signature for the imported native. When the
    /// surface form is `use host::name(T1, T2, ...) -> R`, the parser
    /// records the parameter types and return type here so the type
    /// checker can enforce them at every call site. When the surface
    /// form is the bare `use host::name`, the signature is `None` and
    /// the type checker falls back to the permissive mode that
    /// accepts any argument types and assigns a fresh type variable
    /// to the result.
    pub signature: Option<NativeSignature>,
    /// True when the surface form is `use external module::name`.
    /// The compiler emits `Op::CallExternalNative` for invocations
    /// of an external-marked import and `Op::CallVerifiedNative`
    /// otherwise. The runtime cross-checks each declared native
    /// against its host registration at `Vm::new`; an import
    /// registered with the wrong classification is rejected at
    /// load time.
    pub is_external: bool,
    /// Span of the `use` declaration.
    pub span: Span,
}

/// What is imported by a `use` declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportItem {
    /// A specific name: `use audio::set_frequency`.
    Name(String),
    /// A wildcard: `use audio::*`.
    Wildcard,
}

/// Declared signature for an imported native function.
///
/// Carries the parameter and return type expressions in surface
/// (`TypeExpr`) form. The type checker resolves both to internal
/// [`crate::typecheck::Type`] values through the same path as user-
/// defined functions, so the resulting type information is fully
/// integrated with Hindley-Milner inference at call sites.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSignature {
    /// Parameter types in declaration order.
    pub params: Vec<TypeExpr>,
    /// Return type.
    pub return_type: TypeExpr,
    /// Span of the parenthesised signature in the source.
    pub span: Span,
}

/// A type definition (struct or enum).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// Struct declaration.
    Struct(StructDef),
    /// Enum declaration.
    Enum(EnumDef),
    /// Newtype declaration (a distinct nominal wrapper).
    Newtype(NewtypeDef),
}

/// A newtype definition.
///
/// `newtype Name = Underlying;` introduces a distinct nominal type
/// `Name` that wraps the underlying type. The bytecode
/// representation is identical to the underlying type's
/// representation; no `Value::Struct` envelope is added. The type
/// checker rejects mixing newtypes with their underlying type
/// without explicit construction or extraction, which makes
/// newtypes the right tool for unit discipline and semantic
/// distinctions that should not be silently coerced.
///
/// Construction is `Name(expr)` at the expression level; the
/// argument is checked against the underlying type. The compiled
/// form is identity: only the inner expression's bytecode is
/// emitted.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtypeDef {
    /// Newtype name (the new nominal wrapper).
    pub name: String,
    /// The underlying type whose bytecode representation this
    /// newtype shares.
    pub underlying: TypeExpr,
    /// Optional refinement predicate. When `Some(name)`, the
    /// compiler emits a call to the named atomic-total function at
    /// every newtype construction site, followed by a trap if the
    /// function returns false. The predicate function must be
    /// declared in the same program and must have signature
    /// `fn(Underlying) -> Bool`. The type checker enforces the
    /// signature; the verifier confirms that the predicate is
    /// total. The runtime cost of the check is paid at every
    /// construction; constant folding may elide the call when the
    /// argument is known at compile time, though that optimisation
    /// is not yet implemented.
    pub refinement: Option<String>,
    /// Optional declared maximum saturation value. The
    /// `saturate_max` keyword inside a checked-overflow construct
    /// resolves to this value when the construct's expected
    /// output type is this newtype. When absent, the keyword
    /// resolves to the underlying type's `MAX` (currently
    /// `i64::MAX` for Word).
    pub saturate_max: Option<i64>,
    /// Optional declared minimum saturation value. Same
    /// semantics as `saturate_max` for the minimum direction.
    pub saturate_min: Option<i64>,
    /// Span of the newtype declaration.
    pub span: Span,
}

/// A struct definition.
///
/// Generic structs declare type parameters in `<T, U>` form between
/// the struct name and the field block. Field type expressions may
/// reference these parameters. Construction at use sites instantiates
/// the parameters with fresh per-construction type variables in the
/// same way generic functions do.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// Struct name.
    pub name: String,
    /// Generic type parameters declared in `<T, U>` form.
    pub type_params: Vec<TypeParam>,
    /// Declared fields in source order.
    pub fields: Vec<FieldDecl>,
    /// Span of the struct declaration.
    pub span: Span,
}

/// A field in a struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// Field name.
    pub name: String,
    /// Declared field type expression.
    pub type_expr: TypeExpr,
    /// Span of the field declaration.
    pub span: Span,
}

/// An enum definition.
///
/// Generic enums declare type parameters in `<T, U>` form between the
/// enum name and the variant block. Variant payload type expressions
/// may reference these parameters. Variant construction at use sites
/// instantiates the parameters with fresh per-construction type
/// variables.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// Enum name.
    pub name: String,
    /// Generic type parameters declared in `<T, U>` form.
    pub type_params: Vec<TypeParam>,
    /// Declared variants in source order.
    pub variants: Vec<VariantDecl>,
    /// Span of the enum declaration.
    pub span: Span,
}

/// A variant in an enum definition.
///
/// The optional `discriminant` carries the explicit integer value
/// from the source if the variant was declared as `Name = N`.
/// Variants without an explicit value receive an implicit
/// discriminant during parsing: zero for the first variant of an
/// enum, one more than the preceding variant otherwise. The
/// distinction between explicit and implicit is preserved so
/// downstream consumers (linters, doc generators, FFI bridges)
/// can tell the source-level intent apart from auto-assignment.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    /// Variant name.
    pub name: String,
    /// Payload type expressions in declaration order. Empty for
    /// unit variants.
    pub fields: Vec<TypeExpr>,
    /// Explicit `= N` clause from the source. `None` means the
    /// parser auto-filled `discriminant_value` from the preceding
    /// variant; `Some(n)` means the source author specified `n`.
    pub explicit_discriminant: Option<i64>,
    /// Resolved discriminant value, always present after parsing.
    /// Equals `explicit_discriminant` when that is `Some`, else
    /// the auto-assigned value.
    pub discriminant_value: i64,
    /// Span of the variant declaration.
    pub span: Span,
}

/// Function category keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionCategory {
    /// Atomic total function (`fn`).
    Fn,
    /// Non-atomic total function (`yield`).
    Yield,
    /// Productive divergent function (`loop`).
    Loop,
}

/// A function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    /// Function category (atomic total, non-atomic total, productive
    /// divergent).
    pub category: FunctionCategory,
    /// Function name.
    pub name: String,
    /// Generic type parameters declared in `<T, U>` form between the
    /// function name and the parameter list. Empty vector for
    /// non-generic functions. The order is significant for
    /// monomorphization: each call site instantiates these in order.
    pub type_params: Vec<TypeParam>,
    /// Parameter list in declaration order.
    pub params: Vec<Param>,
    /// Return type expression.
    pub return_type: TypeExpr,
    /// Optional `when` guard expression on a multi-headed function.
    pub guard: Option<Box<Expr>>,
    /// Function body.
    pub body: Block,
    /// True when the source declared the function with the
    /// `ephemeral` modifier. The modifier is permitted only on the
    /// entry point (`fn main`, `yield main`, `loop main`). The
    /// compile pipeline rejects the function if the verifier cannot
    /// prove ephemerality, and the verifier sets
    /// [`crate::bytecode::FLAG_EPHEMERAL`] on the resulting module.
    /// For non-entry functions or when the modifier is absent, the
    /// field is `false`; the verifier may still infer ephemerality
    /// for the module and set the header bit anyway.
    pub ephemeral: bool,
    /// True when the source declared the function with the
    /// `signed` modifier. The modifier is permitted only on the
    /// entry point (`fn main`, `yield main`, `loop main`); the
    /// compiler emits
    /// [`crate::wire_format::FLAG_REQUIRES_SIGNATURE`] in the
    /// module's header so the load-time runtime refuses to admit
    /// the module unless its cryptographic signature verifies
    /// against the host's trust matrix. The signing itself is a
    /// toolchain step independent of the compiler.
    pub signed: bool,
    /// Span of the function declaration.
    pub span: Span,
}

/// A generic type parameter declared in a signature.
///
/// Carries the parameter's name, optional trait bounds, and source
/// location. A bound restricts the parameter to types implementing
/// the named trait. Multiple bounds can be specified using `+`
/// syntax: `<T: Trait1 + Trait2>`. The empty bounds vector represents
/// an unconstrained parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    /// Parameter name.
    pub name: String,
    /// Trait bounds restricting the parameter. Empty for an
    /// unconstrained parameter.
    pub bounds: Vec<String>,
    /// Span of the parameter declaration.
    pub span: Span,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Pattern bound by the parameter (typically a bare identifier
    /// for `fn(x: T)`).
    pub pattern: Pattern,
    /// Optional declared parameter type.
    pub type_expr: Option<TypeExpr>,
    /// Span of the parameter declaration.
    pub span: Span,
}

/// A block of statements with an optional trailing expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Statements in the block.
    pub stmts: Vec<Stmt>,
    /// Optional trailing expression producing the block's value.
    pub tail_expr: Option<Box<Expr>>,
    /// Span of the block.
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let` binding statement.
    Let(LetStmt),
    /// `for` loop statement.
    For(ForStmt),
    /// `break` statement out of the nearest enclosing loop.
    Break(Span),
    /// Assignment to a data block field: `data_name.field = expr;`.
    DataFieldAssign {
        /// Name of the data block being assigned into.
        data_name: String,
        /// Field name being written.
        field: String,
        /// Right-hand-side expression.
        value: Expr,
        /// Span of the assignment statement.
        span: Span,
    },
    /// Indexed assignment into a data-segment array field:
    /// `data_name.field[i][j]... = expr;`. The indices are stored
    /// in source order (outermost first).
    DataFieldIndexAssign {
        /// Name of the data block being assigned into.
        data_name: String,
        /// Field name being written.
        field: String,
        /// Index expressions in source order (outermost first).
        indices: Vec<Expr>,
        /// Right-hand-side expression.
        value: Expr,
        /// Span of the assignment statement.
        span: Span,
    },
    /// Expression-statement (expression evaluated for side effects;
    /// result discarded).
    Expr(Expr),
}

/// A `let` binding.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    /// Pattern that receives the bound value.
    pub pattern: Pattern,
    /// Optional declared binding type.
    pub type_expr: Option<TypeExpr>,
    /// Right-hand-side expression producing the value.
    pub value: Expr,
    /// Span of the let statement.
    pub span: Span,
}

/// A `for` loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    /// Loop variable name.
    pub var: String,
    /// Iterable expression (range or array-shaped expression).
    pub iterable: Iterable,
    /// Loop body.
    pub body: Block,
    /// Span of the for statement.
    pub span: Span,
}

/// The iterable in a `for` loop.
#[derive(Debug, Clone, PartialEq)]
pub enum Iterable {
    /// A plain expression (e.g., an array).
    Expr(Expr),
    /// A range expression (e.g., `0..8`).
    Range(Box<Expr>, Box<Expr>),
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)] // The Expr variants mirror the grammar's
// expression productions; the variant names and field names are
// the documentation. Per-variant prose would duplicate the
// grammar reference in `docs/spec/GRAMMAR.md`.
pub enum Expr {
    /// Literal value.
    Literal { value: Literal, span: Span },
    /// Variable or qualified name reference.
    Ident { name: String, span: Span },
    /// Binary operation.
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Unary operation (`not`, `-`).
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// Function call.
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Pipeline expression: `left |> func(args)`.
    Pipeline {
        left: Box<Expr>,
        func: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Yield expression.
    Yield { value: Box<Expr>, span: Span },
    /// If/else expression.
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    /// Match expression.
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// Loop expression.
    Loop { body: Block, span: Span },
    /// Field access: `expr.field`.
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Method call: `expr.method(args)`. Resolved at compile time to
    /// the impl-defined function for the receiver's type. Generic
    /// receivers are resolved through monomorphization.
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Tuple index: `expr.0`.
    TupleIndex {
        object: Box<Expr>,
        index: u64,
        span: Span,
    },
    /// Array index: `expr[index]`.
    ArrayIndex {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Struct initialization: `Name { field: value }`.
    StructInit {
        name: String,
        fields: Vec<FieldInit>,
        span: Span,
    },
    /// Enum variant: `Enum::Variant(args)`.
    EnumVariant {
        enum_name: String,
        variant: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Array literal: `[a, b, c]`.
    ArrayLiteral { elements: Vec<Expr>, span: Span },
    /// Tuple literal: `(a, b, c)`.
    TupleLiteral { elements: Vec<Expr>, span: Span },
    /// Type cast: `expr as Type`.
    Cast {
        expr: Box<Expr>,
        target: TypeExpr,
        span: Span,
    },
    /// Pipeline placeholder `_`.
    Placeholder { span: Span },
    /// Closure literal: `|args| body` or `|args| -> ret { body }`.
    /// The return type and body are optional in surface syntax: a
    /// bare expression body is wrapped into a single-tail-expression
    /// block automatically by the parser.
    Closure {
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Block,
        span: Span,
    },
    /// Hoisted closure reference produced by the closure-hoisting
    /// pass. Carries the synthetic function's name and the names of
    /// outer-scope locals that the closure captures. The compiler
    /// emits `GetLocal` for each captured name followed by
    /// `Op::MakeClosure(chunk_idx, n_captures)`. User-written code
    /// never produces this variant directly.
    ///
    /// When `recursive` is `true`, the closure was produced by a
    /// `let f = |...| ... f(...) ...` form whose let-binding name
    /// appears in the body. The hoist pass synthesizes the chunk
    /// with the binding name as an additional implicit parameter
    /// after the captures, and the compiler emits
    /// `Op::MakeRecursiveClosure` instead of `Op::MakeClosure`. At
    /// invocation, the runtime pushes the closure value itself into
    /// the self parameter slot before the explicit arguments.
    ClosureRef {
        name: String,
        captures: Vec<String>,
        recursive: bool,
        span: Span,
    },
    /// Overflow-checked expression. The inner expression is a
    /// single arithmetic operation; the arms dispatch on whether
    /// the operation overflowed, underflowed, or completed
    /// normally. The construct's surface form is
    ///
    /// ```text
    /// expr {
    ///     ok(v)          => arm_body,
    ///     overflow(h, l) => arm_body,
    ///     underflow(h, l) => arm_body,
    /// }
    /// ```
    ///
    /// Each arm carries one outcome kind with patterns and an
    /// optional `when guard` clause. Patterns may be a bare
    /// identifier (binds), the wildcard `_` (ignores), or an
    /// integer literal (matches by equality). Multiple arms per
    /// outcome class are admitted as long as the last covering
    /// arm per class is an unguarded catch-all (bare identifier
    /// or wildcard).
    Checked {
        /// The arithmetic operation guarded by the construct. Only
        /// `+`, `-`, `*`, `/`, `%`, and unary `-` on `Word`
        /// operands are supported.
        op_expr: Box<Expr>,
        /// Arms in declaration order.
        arms: Vec<CheckedArm>,
        span: Span,
    },
    /// `saturate_max` literal. Evaluates to the maximum
    /// representable value of the construct's expected type. The
    /// type checker assigns the context type; the compiler emits
    /// a constant.
    SaturateMax { span: Span },
    /// `saturate_min` literal. Evaluates to the minimum
    /// representable value of the construct's expected type.
    SaturateMin { span: Span },
    /// Classify an expression with additional information-flow
    /// labels. Surface form is `classify expr@Label` or
    /// `classify expr@{L1, L2}`. The result's label set is the
    /// union of the expression's labels and the named labels.
    /// Always admitted because adding labels only tightens flow
    /// restrictions.
    Classify {
        value: Box<Expr>,
        labels: Vec<String>,
        span: Span,
    },
    /// Declassify an expression by removing information-flow
    /// labels. Surface form is `declassify expr@Label` or
    /// `declassify expr@{L1, L2}`. The result's label set is the
    /// expression's labels minus the named labels. Records an
    /// audit point because removing labels loosens flow
    /// restrictions and constitutes information disclosure.
    Declassify {
        value: Box<Expr>,
        labels: Vec<String>,
        span: Span,
    },
}

/// One arm of an overflow-checked expression. The arm fires when
/// the guarded operation's outcome class matches `kind` *and* the
/// kind's pattern(s) match the runtime value(s) *and* the optional
/// `guard` evaluates to true.
///
/// Each arm carries exactly one outcome class. The previous
/// pipe-combined form (`overflow | underflow => body`) is no
/// longer admitted; rewrite as two arms with the same body.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedArm {
    /// Outcome class plus destructuring patterns for this arm.
    pub kind: CheckedArmKind,
    /// Optional `when` guard expression that further constrains the
    /// arm.
    pub guard: Option<Expr>,
    /// Body evaluated when the arm fires.
    pub body: Expr,
    /// Span of the arm.
    pub span: Span,
}

/// Which outcome an arm covers, together with the pattern(s) that
/// destructure the runtime value(s) pushed by the checked op.
///
/// - `Ok(p)` matches when the guarded operation produced an
///   in-range result. The pattern `p` is matched against the
///   result `Word`.
/// - `Overflow(h, l)` matches a positive-overflow outcome. The
///   patterns `h` and `l` are matched against the high and low
///   halves of the i128 intermediate result respectively. The
///   high half carries the sign-extended carry for additive ops
///   and the high `Word` for multiplicative ops.
/// - `Underflow(h, l)` matches a negative-overflow outcome with
///   the same destructuring.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckedArmKind {
    /// `ok(p)`: in-range result; the single pattern matches the
    /// result `Word`.
    Ok(Pattern),
    /// `overflow(h, l)`: positive overflow. On `Word` operands the
    /// two patterns match the high and low halves of the `i128`
    /// intermediate. On `Byte` operands the single pattern (the
    /// second is `None`) matches the wrapped `Byte` result.
    Overflow(Pattern, Option<Pattern>),
    /// `underflow(h, l)`: negative overflow; same shape as
    /// `Overflow`, two patterns on `Word` and a single wrapped-result
    /// pattern on `Byte`.
    Underflow(Pattern, Option<Pattern>),
    /// `zero_divisor(numerator)`: a division or modulo by zero. The
    /// single pattern matches the numerator. Admissible only on `/`
    /// and `%`. An unhandled zero divisor traps as a division by
    /// zero.
    ZeroDivisor(Pattern),
    /// `nan(result)`: a floating-point operation produced a NaN. The
    /// single pattern matches the NaN result. Admissible only on
    /// Float operands.
    Nan(Pattern),
    /// `invalid_index(index)`: an array index fell outside `[0, len)`.
    /// The single pattern matches the offending index `Word`.
    /// Admissible only when the guarded operation is an array index
    /// (B35 P4). Optional; an unhandled out-of-bounds index traps as
    /// `VmError::IndexOutOfBounds`.
    InvalidIndex(Pattern),
    /// `invalid_newtype(value)`: a refined-newtype construction whose
    /// predicate rejected the underlying value. The single pattern
    /// matches the offending underlying value. Admissible only when
    /// the guarded operation constructs a refined newtype (B35 P5).
    /// Optional; an unhandled failure traps as
    /// `VmError::RefinementFailed`.
    InvalidNewtype(Pattern),
    /// `payload_discriminant(Variant)`: in a discriminant-to-enum
    /// conversion, names a payload-bearing variant whose payload the
    /// arm body supplies, or `payload_discriminant(_)` as a catch-all
    /// over the remaining payload variants. The pattern is an
    /// upper-case variant name (`Pattern::Variable`) or a wildcard.
    /// Admissible only on a `Word as Enum` conversion (B35 P6).
    /// Coverage of every payload-bearing variant is mandatory.
    PayloadDiscriminant(Pattern),
    /// `invalid_discriminant(raw)`: in a discriminant-to-enum
    /// conversion, catches a `Word` that matches no variant. The
    /// single pattern binds the raw `Word`. Admissible only on a
    /// `Word as Enum` conversion (B35 P6). Optional; an unhandled
    /// invalid discriminant traps as `VmError::EnumVariantUnmapped`.
    InvalidDiscriminant(Pattern),
    /// `error(code)`: a fallible native call failed. The single
    /// pattern binds the `Word` error code the native reported.
    /// Admissible only when the guarded operation is a native call
    /// (B35 P7). Optional; an unhandled native error propagates the
    /// host failure as it would without the construct.
    Error(Pattern),
}

impl Expr {
    /// Return the span of this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. }
            | Expr::Ident { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::Pipeline { span, .. }
            | Expr::Yield { span, .. }
            | Expr::If { span, .. }
            | Expr::Match { span, .. }
            | Expr::Loop { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::TupleIndex { span, .. }
            | Expr::ArrayIndex { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::EnumVariant { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::TupleLiteral { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Placeholder { span }
            | Expr::Closure { span, .. }
            | Expr::ClosureRef { span, .. }
            | Expr::Checked { span, .. }
            | Expr::SaturateMax { span }
            | Expr::SaturateMin { span }
            | Expr::Classify { span, .. }
            | Expr::Declassify { span, .. } => *span,
        }
    }
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal.
    Int(i64),
    /// Floating-point literal.
    Float(f64),
    /// Byte literal from a `Byte` suffix, e.g. `42Byte`. The lexer
    /// range-checks the value to `0..=255`.
    Byte(u8),
    /// Fixed-point literal from a `Fixed<N>` suffix, e.g.
    /// `42Fixed<16>` or `3.14Fixed<16>`. `raw` holds the Q-format
    /// bit pattern; `frac_bits` is `N`, the fraction-bit count.
    Fixed {
        /// Q-format raw bit pattern.
        raw: i64,
        /// Fraction-bit count `N`.
        frac_bits: u8,
    },
    /// Text-string literal.
    String(String),
    /// Boolean literal.
    Bool(bool),
    /// The unit literal `()`.
    Unit,
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)] // Binary operators match the grammar's
// arithmetic, comparison, and logical operator productions; the
// variant names are the documentation.
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

/// Unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation `-`.
    Neg,
    /// Logical negation `not`.
    Not,
}

/// A match arm: `pattern [when guard] => expression`.
///
/// The optional `guard` expression must evaluate to `Bool`; when
/// present, the arm fires only if the pattern matches *and* the
/// guard is true. An arm whose guard is present is never treated
/// as a catch-all by the exhaustiveness check regardless of the
/// pattern shape.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Pattern matched against the scrutinee.
    pub pattern: Pattern,
    /// Optional `when` guard expression.
    pub guard: Option<Expr>,
    /// Body expression evaluated when the arm fires.
    pub expr: Expr,
    /// Span of the arm.
    pub span: Span,
}

/// A field initializer in a struct expression.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    /// Field name being initialized.
    pub name: String,
    /// Initializer expression.
    pub value: Expr,
    /// Span of the field initializer.
    pub span: Span,
}

/// A type expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// Primitive type (`i64`, `f64`, `bool`, `String`).
    Prim(PrimType, Span),
    /// Named type (struct, enum, or opaque) with optional generic
    /// arguments. The `Vec<TypeExpr>` is empty for non-generic
    /// references.
    Named(String, Vec<TypeExpr>, Span),
    /// Tuple type: `(T, U)`.
    Tuple(Vec<TypeExpr>, Span),
    /// Array type: `[T; N]`.
    Array(Box<TypeExpr>, i64, Span),
    /// Option type: `Option<T>`.
    Option(Box<TypeExpr>, Span),
    /// Unit type: `()`.
    Unit(Span),
    /// Type with information-flow labels. Surface form is
    /// `T@Label` or `T@{Label1, Label2}`. Labels are user-defined
    /// names; the empty label set is the pure (open) state and is
    /// represented by the absence of a `Labelled` wrapper. The
    /// labels propagate through arithmetic operations and through
    /// `classify`/`declassify` operators; assignment and parameter
    /// passing check `source.labels ⊆ target.labels`.
    Labelled(Box<TypeExpr>, Vec<String>, Span),
    /// Type with a *negative* information-flow label set. Surface
    /// form is `T@!Label` or `T@{!N1, !N2}`. V0.2.x admits this
    /// wrapper at three boundary-position categories: function
    /// parameter and return types (including `use` natives),
    /// `shared` data field types (the host-script boundary), and
    /// `private` data field types (the yield-resume boundary).
    /// The type checker treats the negatives as a boundary clause
    /// checked at every call site, every resume, every return
    /// statement, every yield expression, and every script-side
    /// data field write: the value flowing through the boundary
    /// must not carry any of the listed labels. Inside the
    /// function body the parameter (or return-bound expression)
    /// is typed as the underlying type without label tracking;
    /// similarly, a data field read produces a value of the inner
    /// type with no labels. The negative constraint does not
    /// propagate as a labelled type through the lattice. Mixed
    /// positive/negative sets are rejected at parse time;
    /// value-side negative labels (the product-lattice extension)
    /// are out of V0.2.x scope and are recorded in
    /// `docs/decisions/BACKLOG.md` (B21).
    NegativeLabelled(Box<TypeExpr>, Vec<String>, Span),
}

impl TypeExpr {
    /// Return the span of this type expression.
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Prim(_, span)
            | TypeExpr::Named(_, _, span)
            | TypeExpr::Tuple(_, span)
            | TypeExpr::Array(_, _, span)
            | TypeExpr::Option(_, span)
            | TypeExpr::Unit(span)
            | TypeExpr::Labelled(_, _, span) => *span,
            TypeExpr::NegativeLabelled(_, _, span) => *span,
        }
    }
}

/// Primitive type.
///
/// Keleusma's V0.2 canonical numeric type set is `Byte`, `Word`,
/// `Fixed`, `Float`. The surface keyword convention is uppercase
/// initial letter; the legacy `i64`/`f64` lowercase forms are
/// rejected at parse time. `Byte` and `Fixed` are introduced
/// alongside the rename; this enum currently carries `Word` and
/// `Float` only, with `Byte` and `Fixed` to follow in subsequent
/// commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimType {
    /// Eight-bit unsigned integer. Range `[0, 255]`. Arithmetic
    /// uses wrapping `u8` semantics; conversions to and from
    /// `Word` go through the `as` cast.
    Byte,
    /// Target word size. Signed two's complement. On the V0.1.x
    /// runtime this is 64-bit; smaller widths are reserved for
    /// future embedded targets (16/32-bit) and are documented in
    /// the Target descriptor.
    Word,
    /// Signed Q-format fixed-point. `None` means the target-scaled
    /// default (Q31.32 on a 64-bit target, Q15.16 on a 32-bit
    /// target, Q7.8 on a 16-bit target — the half-word convention).
    /// `Some(n)` explicitly pins the fraction-bit count. The
    /// surface syntax is `Fixed` for the default form and
    /// `Fixed<N>` for the explicit form, where `N` is an integer
    /// literal in the range `[0, 62]`.
    Fixed(Option<u8>),
    /// Target floating-point width. IEEE 754 binary64 on the host
    /// runtime; narrower widths are reserved for future embedded
    /// targets.
    Float,
    /// Boolean.
    Bool,
    /// UTF-8 text.
    Text,
}

/// A pattern for matching.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Literal pattern: `42`, `"hello"`, `true`.
    Literal(Literal, Span),
    /// Enum variant pattern: `Enum::Variant(p1, p2)`.
    Enum(String, String, Vec<Pattern>, Span),
    /// Struct destructuring: `Name { field, field2: pat }`.
    Struct(String, Vec<FieldPattern>, Span),
    /// Tuple pattern: `(a, b)`.
    Tuple(Vec<Pattern>, Span),
    /// Wildcard: `_`.
    Wildcard(Span),
    /// Variable binding.
    Variable(String, Span),
}

impl Pattern {
    /// Return the span of this pattern.
    pub fn span(&self) -> Span {
        match self {
            Pattern::Literal(_, span)
            | Pattern::Enum(_, _, _, span)
            | Pattern::Struct(_, _, span)
            | Pattern::Tuple(_, span)
            | Pattern::Wildcard(span)
            | Pattern::Variable(_, span) => *span,
        }
    }
}

/// A field pattern in struct destructuring.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldPattern {
    /// Field name being destructured.
    pub name: String,
    /// Optional sub-pattern; absent means the field binds to a
    /// local variable named the same as the field
    /// (shorthand form).
    pub pattern: Option<Pattern>,
    /// Span of the field pattern.
    pub span: Span,
}

/// Merge two spans into one covering both.
pub fn merge_spans(start: Span, end: Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
        line: start.line,
        column: start.column,
    }
}
