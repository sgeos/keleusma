extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::Span;

/// A complete Keleusma program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub uses: Vec<UseDecl>,
    pub types: Vec<TypeDef>,
    pub data_decls: Vec<DataDecl>,
    pub functions: Vec<FunctionDef>,
    pub traits: Vec<TraitDef>,
    pub impls: Vec<ImplBlock>,
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
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<TraitMethodSig>,
    pub span: Span,
}

/// A method signature inside a trait declaration.
///
/// The signature mirrors a function declaration without a body. The
/// implicit `self` parameter is the first parameter when present and
/// has type `Self` (the implementing type).
#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethodSig {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
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
    pub trait_name: String,
    /// Type parameters introduced by the impl block itself, allowing
    /// `impl Trait for Box<T>` style declarations. Empty for
    /// concrete-type impls.
    pub type_params: Vec<TypeParam>,
    /// The implementing type expression. For nominal types this is
    /// typically `TypeExpr::Named(Type, args)`.
    pub for_type: TypeExpr,
    pub methods: Vec<FunctionDef>,
    pub span: Span,
}

/// A `data` block declaration for persistent mutable state.
///
/// Data blocks define named contexts whose fields persist across RESET
/// boundaries. The host initializes the data slots before execution.
/// Script code reads and writes fields via `name.field` syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct DataDecl {
    pub name: String,
    pub fields: Vec<DataFieldDecl>,
    pub span: Span,
}

/// A field in a data block declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct DataFieldDecl {
    pub name: String,
    pub type_expr: TypeExpr,
    pub span: Span,
}

/// A `use` import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub import: ImportItem,
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

/// A type definition (struct or enum).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    Struct(StructDef),
    Enum(EnumDef),
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
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

/// A field in a struct definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub type_expr: TypeExpr,
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
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

/// A variant in an enum definition.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    pub name: String,
    pub fields: Vec<TypeExpr>,
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
    pub category: FunctionCategory,
    pub name: String,
    /// Generic type parameters declared in `<T, U>` form between the
    /// function name and the parameter list. Empty vector for
    /// non-generic functions. The order is significant for
    /// monomorphization: each call site instantiates these in order.
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub guard: Option<Box<Expr>>,
    pub body: Block,
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
    pub name: String,
    pub bounds: Vec<String>,
    pub span: Span,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub pattern: Pattern,
    pub type_expr: Option<TypeExpr>,
    pub span: Span,
}

/// A block of statements with an optional trailing expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail_expr: Option<Box<Expr>>,
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(LetStmt),
    For(ForStmt),
    Break(Span),
    /// Assignment to a data block field: `data_name.field = expr;`.
    DataFieldAssign {
        data_name: String,
        field: String,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
}

/// A `let` binding.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub pattern: Pattern,
    pub type_expr: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

/// A `for` loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub var: String,
    pub iterable: Iterable,
    pub body: Block,
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
            | Expr::TupleIndex { span, .. }
            | Expr::ArrayIndex { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::EnumVariant { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::TupleLiteral { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Placeholder { span } => *span,
        }
    }
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    /// The unit literal `()`.
    Unit,
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Neg,
    Not,
}

/// A match arm: `pattern => expression`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub expr: Expr,
    pub span: Span,
}

/// A field initializer in a struct expression.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: String,
    pub value: Expr,
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
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Prim(_, span)
            | TypeExpr::Named(_, _, span)
            | TypeExpr::Tuple(_, span)
            | TypeExpr::Array(_, _, span)
            | TypeExpr::Option(_, span)
            | TypeExpr::Unit(span) => *span,
        }
    }
}

/// Primitive type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimType {
    I64,
    F64,
    Bool,
    KString,
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
    pub name: String,
    pub pattern: Option<Pattern>,
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
