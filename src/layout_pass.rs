//! Compile-time layout pass.
//!
//! This module bridges the AST type-expression layer
//! ([`crate::ast::TypeExpr`]) to the [`crate::value_layout`] byte-
//! layout descriptors. It is the foundation that subsequent B28
//! phases (P2 onwards) use to emit `AllocTransient(byte_size)`,
//! `WriteScalarAt(offset, kind)`, and `ReadScalarAt(offset, kind)`
//! opcodes for composite construction and field access.
//!
//! The pass operates on the post-monomorphization program where
//! every type expression is concrete. Generic type parameters
//! must have been substituted with concrete types before the
//! layout pass runs; an unsubstituted [`crate::ast::TypeExpr::Named`]
//! with generic arguments is treated as an unresolved error.
//!
//! Labels ([`crate::ast::TypeExpr::Labelled`] and
//! [`crate::ast::TypeExpr::NegativeLabelled`]) do not affect byte layout;
//! the pass transparently descends through them to the underlying
//! type.
//!
//! P1 deliverable: the pass is callable but not yet integrated
//! into the compile pipeline's emission step. Subsequent phases
//! wire it into `Op::AllocTransient` emission and the field-
//! access opcode emission.

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ast::{EnumDef, PrimType, StructDef, TypeExpr};
use crate::value_layout::{LayoutDescriptor, ScalarKind};

/// Errors that can arise during compile-time layout computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// Named type was not found in the struct or enum table.
    /// The runtime cannot compute a byte layout for a name that
    /// does not resolve to a concrete struct or enum definition.
    UnknownType(String),
    /// Generic type parameter was not substituted. The layout
    /// pass requires post-monomorphization input; any
    /// [`TypeExpr::Named`] with non-empty generic arguments is
    /// treated as unresolved.
    UnresolvedGeneric(String),
    /// Array size declared in the source is invalid (negative).
    /// Keleusma's surface syntax allows literal integer sizes;
    /// negative values are rejected by the parser but defensive
    /// checks are kept here.
    InvalidArraySize(i64),
    /// Type expression has no byte representation. Currently
    /// emitted only by future extension paths; the V0.2.x
    /// `TypeExpr` enum covers exhaustively-representable types.
    UnsupportedType(String),
}

/// Context that supplies struct and enum definitions to the
/// layout pass, plus the target's word and float byte widths.
///
/// The context borrows the struct and enum tables for the
/// duration of the layout computation. No internal caching is
/// performed; callers that compute many layouts may wrap a
/// cache around [`LayoutContext::layout_for`].
pub struct LayoutContext<'a> {
    /// Struct definitions indexed by struct type name. After
    /// monomorphization the names include the mangled form
    /// (e.g., `Cell__Word` for `Cell<Word>`).
    structs: &'a BTreeMap<String, StructDef>,
    /// Enum definitions indexed by enum type name. After
    /// monomorphization the names include the mangled form.
    enums: &'a BTreeMap<String, EnumDef>,
    /// Byte width of the target's word type. Equals `8` for
    /// the bundled `i64` runtime.
    word_bytes: usize,
    /// Byte width of the target's float type. Equals `8` for
    /// the bundled `f64` runtime.
    float_bytes: usize,
    /// Newtype names, used only with `opaque_fallback` to avoid
    /// misclassifying a newtype-typed field as opaque. `None` when no
    /// fallback is configured.
    newtypes: Option<&'a BTreeSet<String>>,
    /// When set, a bare `Named` type that is neither a struct, an enum,
    /// nor a newtype is treated as an opaque host reference
    /// (`ScalarKind::Opaque`) rather than an [`LayoutError::UnknownType`]
    /// error (B28 P3). The compiler enables this because it runs after the
    /// type checker, where any surviving unknown named type used in a
    /// signature is necessarily an opaque host type. Default off, so the
    /// pass keeps strict unknown-type detection when used standalone.
    opaque_fallback: bool,
}

impl<'a> LayoutContext<'a> {
    /// Construct a layout context.
    ///
    /// `word_bytes` and `float_bytes` should match the target
    /// descriptor's declared widths (see [`crate::target::Target`]).
    pub fn new(
        structs: &'a BTreeMap<String, StructDef>,
        enums: &'a BTreeMap<String, EnumDef>,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        Self {
            structs,
            enums,
            word_bytes,
            float_bytes,
            newtypes: None,
            opaque_fallback: false,
        }
    }

    /// Enable opaque fallback (B28 P3): a bare `Named` type that is not a
    /// struct, enum, or one of `newtypes` resolves to
    /// [`ScalarKind::Opaque`] rather than erroring. Intended for callers
    /// that run after type checking, where a surviving unknown named type
    /// is an opaque host reference.
    pub fn with_opaque_fallback(mut self, newtypes: &'a BTreeSet<String>) -> Self {
        self.newtypes = Some(newtypes);
        self.opaque_fallback = true;
        self
    }

    /// Compute the byte layout for a type expression.
    ///
    /// Returns a [`LayoutDescriptor`] that subsequent compile
    /// passes can use to compute total byte sizes and field
    /// offsets. Errors propagate through [`LayoutError`].
    pub fn layout_for(&self, ty: &TypeExpr) -> Result<LayoutDescriptor, LayoutError> {
        match ty {
            TypeExpr::Unit(_) => Ok(LayoutDescriptor::Scalar(ScalarKind::Unit)),
            TypeExpr::Prim(prim, _) => Ok(LayoutDescriptor::Scalar(scalar_kind_for_prim(*prim))),
            TypeExpr::Multiword(n, _, _) => {
                // Post-erasure tripwire: a symbolic const dimension must
                // never reach the layout pass; monomorphization resolves
                // it to a literal first (B40).
                let n = n.as_lit().ok_or_else(|| {
                    LayoutError::UnresolvedGeneric(alloc::format!("Multiword word count `{}`", n))
                })?;
                Ok(LayoutDescriptor::Array {
                    element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
                    count: n as usize,
                })
            }
            TypeExpr::Tuple(elems, _) => {
                let mut layouts = Vec::with_capacity(elems.len());
                for elem in elems {
                    layouts.push(self.layout_for(elem)?);
                }
                Ok(LayoutDescriptor::Tuple(layouts))
            }
            TypeExpr::Array(elem, count, _) => {
                let count = count.as_lit().ok_or_else(|| {
                    LayoutError::UnresolvedGeneric(alloc::format!("array size `{}`", count))
                })?;
                if count < 0 {
                    return Err(LayoutError::InvalidArraySize(count));
                }
                let elem_layout = self.layout_for(elem)?;
                Ok(LayoutDescriptor::Array {
                    element: Box::new(elem_layout),
                    count: count as usize,
                })
            }
            TypeExpr::Option(inner, _) => {
                let inner_layout = self.layout_for(inner)?;
                Ok(LayoutDescriptor::Enum {
                    type_name: "Option".to_string(),
                    variants: alloc::vec![
                        ("None".to_string(), alloc::vec![]),
                        ("Some".to_string(), alloc::vec![inner_layout]),
                    ],
                })
            }
            TypeExpr::Labelled(inner, _, _) => self.layout_for(inner),
            TypeExpr::NegativeLabelled(inner, _, _) => self.layout_for(inner),
            TypeExpr::Named(name, args, _) => {
                if !args.is_empty() {
                    return Err(LayoutError::UnresolvedGeneric(name.clone()));
                }
                if let Some(struct_def) = self.structs.get(name) {
                    let mut fields = Vec::with_capacity(struct_def.fields.len());
                    for field in &struct_def.fields {
                        let field_layout = self.layout_for(&field.type_expr)?;
                        fields.push((field.name.clone(), field_layout));
                    }
                    return Ok(LayoutDescriptor::Struct {
                        type_name: name.clone(),
                        fields,
                    });
                }
                if let Some(enum_def) = self.enums.get(name) {
                    let mut variants = Vec::with_capacity(enum_def.variants.len());
                    for variant in &enum_def.variants {
                        let mut payloads = Vec::with_capacity(variant.fields.len());
                        for payload_ty in &variant.fields {
                            payloads.push(self.layout_for(payload_ty)?);
                        }
                        variants.push((variant.name.clone(), payloads));
                    }
                    return Ok(LayoutDescriptor::Enum {
                        type_name: name.clone(),
                        variants,
                    });
                }
                // Post-type-check, a bare `Named` type that is neither a
                // struct, an enum, a newtype, nor a built-in is an opaque
                // host reference (B28 P3). It carries no generic arguments
                // here (those errored above), so it is a fixed-size opaque
                // handle. `Option` is excluded: it is a built-in generic
                // enum that is not in the `enums` map and appears bare (no
                // type argument) when the enum-variant lowering recovers an
                // `Option::*` expression's type, so it must keep erroring
                // here and stay boxed rather than be mistaken for opaque.
                // Standalone use keeps strict unknown detection.
                if self.opaque_fallback
                    && name != "Option"
                    && !self.newtypes.is_some_and(|nt| nt.contains(name))
                {
                    return Ok(LayoutDescriptor::Scalar(ScalarKind::Opaque));
                }
                Err(LayoutError::UnknownType(name.clone()))
            }
        }
    }

    /// Convenience: compute the total byte size of a type
    /// expression's layout.
    ///
    /// Equivalent to
    /// `self.layout_for(ty)?.size_in_bytes(self.word_bytes, self.float_bytes)`.
    pub fn size_in_bytes(&self, ty: &TypeExpr) -> Result<usize, LayoutError> {
        let layout = self.layout_for(ty)?;
        Ok(layout.size_in_bytes(self.word_bytes, self.float_bytes))
    }

    /// The word byte width this context was constructed with.
    pub fn word_bytes(&self) -> usize {
        self.word_bytes
    }

    /// The float byte width this context was constructed with.
    pub fn float_bytes(&self) -> usize {
        self.float_bytes
    }
}

fn scalar_kind_for_prim(prim: PrimType) -> ScalarKind {
    match prim {
        PrimType::Byte => ScalarKind::Byte,
        PrimType::Word => ScalarKind::Int,
        PrimType::Fixed(_) => ScalarKind::Fixed,
        #[cfg(feature = "floats")]
        PrimType::Float => ScalarKind::Float,
        #[cfg(not(feature = "floats"))]
        PrimType::Float => ScalarKind::Int,
        PrimType::Bool => ScalarKind::Bool,
        PrimType::Text => ScalarKind::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FieldDecl, VariantDecl};
    use crate::token::Span;

    const I64_BYTES: usize = 8;
    const F64_BYTES: usize = 8;

    fn span() -> Span {
        Span::default()
    }

    fn empty_tables() -> (BTreeMap<String, StructDef>, BTreeMap<String, EnumDef>) {
        (BTreeMap::new(), BTreeMap::new())
    }

    #[test]
    fn primitive_word_is_word_bytes() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Word, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES);
    }

    #[test]
    fn primitive_byte_is_one_byte() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Byte, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 1);
    }

    #[test]
    fn primitive_bool_is_one_byte() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Bool, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 1);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn primitive_float_is_float_bytes() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Float, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), F64_BYTES);
    }

    #[test]
    fn primitive_text_is_two_words() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Text, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 2 * I64_BYTES);
    }

    #[test]
    fn primitive_fixed_is_word_bytes() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Prim(PrimType::Fixed(None), span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES);
        let ty_explicit = TypeExpr::Prim(PrimType::Fixed(Some(16)), span());
        assert_eq!(ctx.size_in_bytes(&ty_explicit).unwrap(), I64_BYTES);
    }

    #[test]
    fn unit_is_zero_bytes() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Unit(span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 0);
    }

    #[test]
    fn tuple_of_primitives() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Tuple(
            alloc::vec![
                TypeExpr::Prim(PrimType::Word, span()),
                TypeExpr::Prim(PrimType::Bool, span()),
                TypeExpr::Prim(PrimType::Byte, span()),
            ],
            span(),
        );
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES + 1 + 1);
    }

    #[test]
    fn array_of_words() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::array_lit(Box::new(TypeExpr::Prim(PrimType::Word, span())), 8, span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 8 * I64_BYTES);
    }

    #[test]
    fn array_negative_size_rejected() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::array_lit(Box::new(TypeExpr::Prim(PrimType::Word, span())), -1, span());
        assert!(matches!(
            ctx.size_in_bytes(&ty),
            Err(LayoutError::InvalidArraySize(-1))
        ));
    }

    #[test]
    fn option_of_word() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Option(Box::new(TypeExpr::Prim(PrimType::Word, span())), span());
        // Word-sized discriminant (B28 P2): 8-byte disc + 8-byte payload.
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES + I64_BYTES);
    }

    #[test]
    fn option_of_bool() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Option(Box::new(TypeExpr::Prim(PrimType::Bool, span())), span());
        // Word-sized discriminant (B28 P2): 8-byte disc + 1-byte payload.
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES + 1);
    }

    #[test]
    fn labelled_descends_to_inner() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Labelled(
            Box::new(TypeExpr::Prim(PrimType::Word, span())),
            alloc::vec!["Sensitive".to_string()],
            span(),
        );
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES);
    }

    #[test]
    fn negative_labelled_descends_to_inner() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::NegativeLabelled(
            Box::new(TypeExpr::Prim(PrimType::Word, span())),
            alloc::vec!["Egress".to_string()],
            span(),
        );
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES);
    }

    #[test]
    fn struct_lookup_sums_field_sizes() {
        let mut structs: BTreeMap<String, StructDef> = BTreeMap::new();
        let enums = BTreeMap::new();
        structs.insert(
            "Point".to_string(),
            StructDef {
                name: "Point".to_string(),
                type_params: alloc::vec![],
                const_params: alloc::vec![],
                fields: alloc::vec![
                    FieldDecl {
                        name: "x".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Word, span()),
                        span: span(),
                    },
                    FieldDecl {
                        name: "y".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Word, span()),
                        span: span(),
                    },
                ],
                span: span(),
            },
        );
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named("Point".to_string(), alloc::vec![], span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 2 * I64_BYTES);
    }

    #[test]
    fn enum_lookup_sums_largest_variant() {
        let structs = BTreeMap::new();
        let mut enums: BTreeMap<String, EnumDef> = BTreeMap::new();
        enums.insert(
            "Color".to_string(),
            EnumDef {
                name: "Color".to_string(),
                type_params: alloc::vec![],
                const_params: alloc::vec![],
                variants: alloc::vec![
                    VariantDecl {
                        name: "Red".to_string(),
                        fields: alloc::vec![],
                        explicit_discriminant: None,
                        discriminant_value: 0,
                        span: span(),
                    },
                    VariantDecl {
                        name: "Custom".to_string(),
                        fields: alloc::vec![
                            TypeExpr::Prim(PrimType::Byte, span()),
                            TypeExpr::Prim(PrimType::Byte, span()),
                            TypeExpr::Prim(PrimType::Byte, span()),
                        ],
                        explicit_discriminant: None,
                        discriminant_value: 1,
                        span: span(),
                    },
                ],
                span: span(),
            },
        );
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named("Color".to_string(), alloc::vec![], span());
        // Word-sized discriminant (B28 P2): 8-byte disc + 3-byte payload.
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES + 3);
    }

    #[test]
    fn unknown_named_type_is_an_error() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named("Missing".to_string(), alloc::vec![], span());
        match ctx.size_in_bytes(&ty) {
            Err(LayoutError::UnknownType(name)) => assert_eq!(name, "Missing"),
            other => panic!("expected UnknownType error, got {:?}", other),
        }
    }

    #[test]
    fn unsubstituted_generic_is_an_error() {
        let (structs, enums) = empty_tables();
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named(
            "Vec".to_string(),
            alloc::vec![TypeExpr::Prim(PrimType::Word, span())],
            span(),
        );
        match ctx.size_in_bytes(&ty) {
            Err(LayoutError::UnresolvedGeneric(name)) => assert_eq!(name, "Vec"),
            other => panic!("expected UnresolvedGeneric error, got {:?}", other),
        }
    }

    #[test]
    fn nested_tuple_in_struct() {
        let mut structs: BTreeMap<String, StructDef> = BTreeMap::new();
        let enums = BTreeMap::new();
        structs.insert(
            "Wrapper".to_string(),
            StructDef {
                name: "Wrapper".to_string(),
                type_params: alloc::vec![],
                const_params: alloc::vec![],
                fields: alloc::vec![FieldDecl {
                    name: "coords".to_string(),
                    type_expr: TypeExpr::Tuple(
                        alloc::vec![
                            TypeExpr::Prim(PrimType::Word, span()),
                            TypeExpr::Prim(PrimType::Word, span()),
                        ],
                        span(),
                    ),
                    span: span(),
                },],
                span: span(),
            },
        );
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named("Wrapper".to_string(), alloc::vec![], span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), 2 * I64_BYTES);
    }

    #[test]
    fn struct_with_text_field() {
        let mut structs: BTreeMap<String, StructDef> = BTreeMap::new();
        let enums = BTreeMap::new();
        structs.insert(
            "Greeting".to_string(),
            StructDef {
                name: "Greeting".to_string(),
                type_params: alloc::vec![],
                const_params: alloc::vec![],
                fields: alloc::vec![
                    FieldDecl {
                        name: "id".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Word, span()),
                        span: span(),
                    },
                    FieldDecl {
                        name: "message".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Text, span()),
                        span: span(),
                    },
                ],
                span: span(),
            },
        );
        let ctx = LayoutContext::new(&structs, &enums, I64_BYTES, F64_BYTES);
        let ty = TypeExpr::Named("Greeting".to_string(), alloc::vec![], span());
        assert_eq!(ctx.size_in_bytes(&ty).unwrap(), I64_BYTES + 2 * I64_BYTES);
    }

    #[test]
    fn narrow_target_widths() {
        let mut structs: BTreeMap<String, StructDef> = BTreeMap::new();
        let enums = BTreeMap::new();
        structs.insert(
            "Point".to_string(),
            StructDef {
                name: "Point".to_string(),
                type_params: alloc::vec![],
                const_params: alloc::vec![],
                fields: alloc::vec![
                    FieldDecl {
                        name: "x".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Word, span()),
                        span: span(),
                    },
                    FieldDecl {
                        name: "y".to_string(),
                        type_expr: TypeExpr::Prim(PrimType::Word, span()),
                        span: span(),
                    },
                ],
                span: span(),
            },
        );
        let ctx_2byte_word = LayoutContext::new(&structs, &enums, 2, 4);
        let ty = TypeExpr::Named("Point".to_string(), alloc::vec![], span());
        assert_eq!(ctx_2byte_word.size_in_bytes(&ty).unwrap(), 2 * 2);

        let ctx_1byte_word = LayoutContext::new(&structs, &enums, 1, 4);
        assert_eq!(ctx_1byte_word.size_in_bytes(&ty).unwrap(), 2);
    }
}
