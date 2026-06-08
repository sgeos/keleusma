#![deny(missing_docs)]
//! Procedural macros for the Keleusma scripting language.
//!
//! This crate is the proc-macro backend for the
//! [`keleusma::KeleusmaType`](https://docs.rs/keleusma/latest/keleusma/trait.KeleusmaType.html)
//! derive, which generates `KeleusmaType` trait implementations that
//! convert between the host type and the runtime `Value` enum at the
//! native function boundary, and the `keleusma::KeleusmaError` derive,
//! which generates `From<E> for keleusma::VmError` for a fieldless
//! enum so a fallible native can report a `Word` error code (B35 P7).
//!
//! # Implementation detail
//!
//! Depend on the `keleusma` crate, not on this one. The derive is
//! re-exported as `keleusma::KeleusmaType`. This crate is published
//! only because Cargo requires proc-macro implementations to live in
//! a separate library; the expansion references types defined in the
//! parent crate and is not standalone-useful.
//!
//! # Supported input shapes
//!
//! - **Named-field structs**: `struct Point { x: f64, y: f64 }`.
//! - **Enums with unit variants**: `enum Color { Red, Green, Blue }`.
//! - **Enums with tuple variants**: `enum Shape { Circle(f64), Rect(f64, f64) }`.
//! - **Enums with struct-style variants**: `enum Event { Click { x: i64, y: i64 } }`.
//!
//! Each field or payload type must itself implement `KeleusmaType`.
//!
//! # Rejected inputs
//!
//! - **Tuple structs** (`struct Wrapper(i64);`) and **unit structs**
//!   (`struct Marker;`) produce a `compile_error!` at expansion time.
//!   Use a named-field struct or the bare tuple type instead.
//! - **Unions** produce a `syn::Error` before expansion. Unions cannot
//!   be safely projected into the runtime `Value` enum because the
//!   active variant is not statically known.
//!
//! See the runtime crate documentation for the full trait contract and
//! the admissibility rules for field types.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Ident, parse_macro_input};

/// Derive [`keleusma::KeleusmaType`] for a struct or enum.
///
/// Generates an `impl ::keleusma::KeleusmaType for T` block whose
/// `from_value` and `into_value` methods route between the host type
/// and the runtime `Value` enum at the native function boundary.
///
/// # Accepted inputs
///
/// - Named-field structs.
/// - Enums whose variants are any combination of unit, tuple-style,
///   and struct-style.
///
/// Each field or payload type must itself implement `KeleusmaType`.
///
/// # Compile errors
///
/// - Unions: rejected before expansion with a `syn::Error` message
///   `"KeleusmaType cannot be derived for unions"`.
/// - Tuple structs and unit structs: rejected during expansion with a
///   `compile_error!` directing the user to a named-field struct or
///   the bare tuple type.
///
/// [`keleusma::KeleusmaType`]: https://docs.rs/keleusma/latest/keleusma/trait.KeleusmaType.html
#[proc_macro_derive(KeleusmaType)]
pub fn derive_keleusma_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let generics = &input.generics;
    let (_, ty_generics, where_clause) = generics.split_for_impl();

    // Synthesise an impl-generics list with the parametric VM's
    // Word and Float type parameters appended so the derived impl
    // applies to any `Vm<W, A, F>` shape.
    let mut generics_with_wf = generics.clone();
    generics_with_wf
        .params
        .push(syn::parse_quote!(__KW: ::keleusma::Word));
    generics_with_wf
        .params
        .push(syn::parse_quote!(__KF: ::keleusma::Float));
    let (impl_generics_wf, _, _) = generics_with_wf.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => derive_struct_body(name, &name_str, data),
        Data::Enum(data) => derive_enum_body(name, &name_str, data),
        Data::Union(_) => {
            return syn::Error::new_spanned(&input, "KeleusmaType cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl #impl_generics_wf ::keleusma::KeleusmaType<__KW, __KF>
            for #name #ty_generics #where_clause
        {
            #body
        }
    };
    expanded.into()
}

/// Derive `From<E> for keleusma::VmError` for a fieldless
/// (discriminant-only) enum, producing a
/// `keleusma::VmError::NativeErrorCode` whose `code` is the
/// variant's discriminant (B35 P7). The reference is a plain code span
/// because this proc-macro crate does not depend on `keleusma`, so an
/// intra-doc link cannot resolve the path.
///
/// This is the host-side companion of the native-error `error(code)`
/// construct. A fallible native registered with `register_fn_fallible`
/// returns `Result<R, keleusma::VmError>`; with this derive a host can
/// write `return Err(MyError::Variant.into())` (or use `?`), and the
/// script-side `native(args) { ok(v) => ..., error(code) => ... }`
/// construct binds the discriminant as `code`. Pairing the host enum's
/// discriminants with a script-side `enum` lets `code as ScriptEnum {
/// ... }` recover a structured error.
///
/// # Accepted inputs
///
/// - Enums all of whose variants are unit (fieldless), with implicit
///   or explicit discriminants.
///
/// # Compile errors
///
/// - Non-enum types, and enums with any payload-bearing variant, are
///   rejected: the `Word` error code is the variant discriminant, and
///   only a fieldless enum casts to its discriminant.
#[proc_macro_derive(KeleusmaError)]
pub fn derive_keleusma_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "KeleusmaError can only be derived for fieldless enums",
            )
            .to_compile_error()
            .into();
        }
    };
    for v in &data.variants {
        if !matches!(v.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                v,
                "KeleusmaError requires a fieldless (discriminant-only) enum; the Word error code is the variant discriminant",
            )
            .to_compile_error()
            .into();
        }
    }

    let expanded = quote! {
        impl ::core::convert::From<#name> for ::keleusma::VmError {
            fn from(e: #name) -> Self {
                // A fieldless enum casts directly to its discriminant.
                let code = e as i64;
                ::keleusma::VmError::NativeErrorCode {
                    code,
                    message: ::alloc::format!("{} error code {}", #name_str, code),
                }
            }
        }
    };
    expanded.into()
}

fn derive_struct_body(_name: &Ident, name_str: &str, data: &DataStruct) -> TokenStream2 {
    match &data.fields {
        Fields::Named(fields_named) => {
            let field_names: Vec<&Ident> = fields_named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            let field_name_strs: Vec<String> = field_names.iter().map(|i| i.to_string()).collect();
            let field_types: Vec<&syn::Type> = fields_named.named.iter().map(|f| &f.ty).collect();

            quote! {
                fn from_value(v: &::keleusma::GenericValue<__KW, __KF>)
                    -> ::core::result::Result<Self, ::keleusma::VmError>
                {
                    match v {
                        ::keleusma::GenericValue::Struct(
                            ::keleusma::bytecode::StructBody::Boxed { type_name, fields }
                        ) if type_name == #name_str => {
                            #(
                                let #field_names = {
                                    let pair = fields.iter().find(|(n, _)| n == #field_name_strs)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("missing field `{}` on `{}`", #field_name_strs, #name_str)
                                        ))?;
                                    <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value(&pair.1)?
                                };
                            )*
                            ::core::result::Result::Ok(Self { #(#field_names),* })
                        }
                        // A flat struct body is pure bytes; the field types
                        // supply their flat sizes, read at the packed offsets
                        // in declaration order, recursing through nested flat
                        // composites (B28 P2).
                        ::keleusma::GenericValue::Struct(
                            ::keleusma::bytecode::StructBody::Flat(__fc)
                        ) => {
                            let __wb = (1usize << <__KW as ::keleusma::Word>::BITS_LOG2) / 8;
                            let __fb = (1usize << <__KF as ::keleusma::Float>::BITS_LOG2) / 8;
                            <Self as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes(__fc.as_bytes(), __wb, __fb)
                        }
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("expected struct `{}`, got {}", #name_str, other.type_name())
                        )),
                    }
                }

                #[allow(unused_assignments)]
                fn flat_byte_size(__wb: usize, __fb: usize) -> ::core::option::Option<usize> {
                    let mut __total = 0usize;
                    #(
                        __total += <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)?;
                    )*
                    ::core::option::Option::Some(__total)
                }

                #[allow(unused_assignments)]
                fn from_flat_bytes(__bytes: &[u8], __wb: usize, __fb: usize)
                    -> ::core::result::Result<Self, ::keleusma::VmError>
                {
                    let mut __offset = 0usize;
                    #(
                        let #field_names = {
                            let __size = <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                .ok_or_else(|| ::keleusma::VmError::TypeError(
                                    ::alloc::format!("field `{}` of `{}` is not flat-eligible", #field_name_strs, #name_str)
                                ))?;
                            let __val = <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes(
                                &__bytes[__offset..__offset + __size], __wb, __fb,
                            )?;
                            __offset += __size;
                            __val
                        };
                    )*
                    ::core::result::Result::Ok(Self { #(#field_names),* })
                }

                fn from_value_ctx(
                    v: &::keleusma::GenericValue<__KW, __KF>,
                    __ctx: &::keleusma::RefContext<'_>,
                ) -> ::core::result::Result<Self, ::keleusma::VmError>
                {
                    match v {
                        ::keleusma::GenericValue::Struct(
                            ::keleusma::bytecode::StructBody::Boxed { type_name, fields }
                        ) if type_name == #name_str => {
                            #(
                                let #field_names = {
                                    let pair = fields.iter().find(|(n, _)| n == #field_name_strs)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("missing field `{}` on `{}`", #field_name_strs, #name_str)
                                        ))?;
                                    <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value_ctx(&pair.1, __ctx)?
                                };
                            )*
                            ::core::result::Result::Ok(Self { #(#field_names),* })
                        }
                        ::keleusma::GenericValue::Struct(
                            ::keleusma::bytecode::StructBody::Flat(__fc)
                        ) => {
                            // Read at the module's packed widths from the
                            // context, which differ from the host `Word`
                            // width on a narrow-word build (B28 P3).
                            <Self as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes_ctx(
                                __fc.as_bytes(), __ctx.word_bytes, __ctx.float_bytes, __ctx,
                            )
                        }
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("expected struct `{}`, got {}", #name_str, other.type_name())
                        )),
                    }
                }

                #[allow(unused_assignments)]
                fn from_flat_bytes_ctx(__bytes: &[u8], __wb: usize, __fb: usize, __ctx: &::keleusma::RefContext<'_>)
                    -> ::core::result::Result<Self, ::keleusma::VmError>
                {
                    let mut __offset = 0usize;
                    #(
                        let #field_names = {
                            let __size = <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                .ok_or_else(|| ::keleusma::VmError::TypeError(
                                    ::alloc::format!("field `{}` of `{}` is not flat-eligible", #field_name_strs, #name_str)
                                ))?;
                            let __val = <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes_ctx(
                                &__bytes[__offset..__offset + __size], __wb, __fb, __ctx,
                            )?;
                            __offset += __size;
                            __val
                        };
                    )*
                    ::core::result::Result::Ok(Self { #(#field_names),* })
                }

                fn into_value(self) -> ::keleusma::GenericValue<__KW, __KF> {
                    let fields: ::alloc::vec::Vec<(
                        ::alloc::string::String,
                        ::keleusma::GenericValue<__KW, __KF>,
                    )> = ::alloc::vec![
                        #(
                            (
                                ::alloc::string::String::from(#field_name_strs),
                                <#field_types as ::keleusma::KeleusmaType<__KW, __KF>>::into_value(self.#field_names),
                            ),
                        )*
                    ];
                    // Route through the shared constructor so a host-built
                    // struct has the same representation as a script-built
                    // one of the same type, which equality relies on (B28).
                    ::keleusma::GenericValue::struct_with_widths(
                        ::alloc::string::String::from(#name_str),
                        fields,
                        (1usize << <__KW as ::keleusma::Word>::BITS_LOG2) / 8,
                        (1usize << <__KF as ::keleusma::Float>::BITS_LOG2) / 8,
                    )
                }
            }
        }
        Fields::Unnamed(_) => quote! {
            compile_error!("KeleusmaType derive does not support tuple structs; use a named-field struct or a tuple type instead");
        },
        Fields::Unit => quote! {
            compile_error!("KeleusmaType derive does not support unit structs; use the unit type `()` instead");
        },
    }
}

/// Extract an explicit enum-variant discriminant literal (`= 5`, `= -1`)
/// for the flat-enum marshalling (B28 P2). `None` for a non-literal or
/// absent discriminant; the caller falls back to the running counter.
fn explicit_disc(expr: &syn::Expr) -> Option<i64> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(i),
            ..
        }) => i.base10_parse::<i64>().ok(),
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Neg(_)) => {
            explicit_disc(&u.expr).map(|v| -v)
        }
        _ => None,
    }
}

fn derive_enum_body(_name: &Ident, name_str: &str, data: &DataEnum) -> TokenStream2 {
    // Per-variant discriminants, mirroring the language's assignment
    // (explicit value, else the previous discriminant plus one starting at
    // zero). These are what a flat enum body stores and what the access
    // ops compare, so host-marshalled enums agree with script-built ones
    // when the Rust enum mirrors the Keleusma enum's discriminants (B28 P2).
    let discs: Vec<i64> = {
        let mut out = Vec::with_capacity(data.variants.len());
        let mut next = 0i64;
        for v in &data.variants {
            let d = v
                .discriminant
                .as_ref()
                .and_then(|(_, expr)| explicit_disc(expr))
                .unwrap_or(next);
            out.push(d);
            next = d + 1;
        }
        out
    };

    // Per-variant flat payload size as an `Option<usize>` expression: the
    // sum of the variant's field flat sizes, or `None` if any field is not
    // flat-eligible (B28 P2 nested inlining). Used to compute the enum's
    // largest-variant payload (`payload_max`), which pads every value to
    // one fixed body size so a nested enum field's slot is fixed and the
    // host representation matches the script's.
    let variant_size_exprs: Vec<TokenStream2> = data
        .variants
        .iter()
        .map(|v| {
            let types: Vec<&syn::Type> = match &v.fields {
                Fields::Unit => Vec::new(),
                Fields::Unnamed(u) => u.unnamed.iter().map(|f| &f.ty).collect(),
                Fields::Named(n) => n.named.iter().map(|f| &f.ty).collect(),
            };
            quote! {
                {
                    let mut __acc = ::core::option::Option::Some(0usize);
                    #(
                        __acc = __acc.and_then(|__a|
                            <#types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                .map(|__x| __a + __x));
                    )*
                    __acc
                }
            }
        })
        .collect();

    // Compute `__min_payload` (the largest flat variant payload, or `0`
    // when the enum is not uniformly flat) from `__wb`/`__fb` in scope.
    let min_payload_calc = quote! {
        let __sizes: ::alloc::vec::Vec<::core::option::Option<usize>> =
            ::alloc::vec![ #(#variant_size_exprs),* ];
        let mut __m = 0usize;
        let mut __uniform = true;
        for __s in &__sizes {
            match __s {
                ::core::option::Option::Some(__x) => {
                    if *__x > __m {
                        __m = *__x;
                    }
                }
                ::core::option::Option::None => {
                    __uniform = false;
                }
            }
        }
        let __min_payload: usize = if __uniform { __m } else { 0 };
    };

    let from_arms: Vec<TokenStream2> = data
        .variants
        .iter()
        .map(|v| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            match &v.fields {
                Fields::Unit => quote! {
                    #v_str => {
                        if !fields.is_empty() {
                            return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                ::alloc::format!("variant `{}::{}` expects 0 fields, got {}", #name_str, #v_str, fields.len())
                            ));
                        }
                        ::core::result::Result::Ok(Self::#v_ident)
                    }
                },
                Fields::Unnamed(unnamed) => {
                    let count = unnamed.unnamed.len();
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..count)
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    let positions: Vec<usize> = (0..count).collect();
                    quote! {
                        #v_str => {
                            if fields.len() != #count {
                                return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                    ::alloc::format!("variant `{}::{}` expects {} fields, got {}", #name_str, #v_str, #count, fields.len())
                                ));
                            }
                            #(
                                let #bindings = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value(&fields[#positions])?;
                            )*
                            ::core::result::Result::Ok(Self::#v_ident(#(#bindings),*))
                        }
                    }
                }
                Fields::Named(named) => {
                    let count = named.named.len();
                    let names: Vec<&Ident> = named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                    let types: Vec<&syn::Type> = named.named.iter().map(|f| &f.ty).collect();
                    let positions: Vec<usize> = (0..count).collect();
                    quote! {
                        #v_str => {
                            if fields.len() != #count {
                                return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                    ::alloc::format!("variant `{}::{}` expects {} fields, got {}", #name_str, #v_str, #count, fields.len())
                                ));
                            }
                            #(
                                let #names = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value(&fields[#positions])?;
                            )*
                            ::core::result::Result::Ok(Self::#v_ident { #(#names),* })
                        }
                    }
                }
            }
        })
        .collect();

    let into_arms: Vec<TokenStream2> = data
        .variants
        .iter()
        .enumerate()
        .map(|(__vi, v)| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            let disc = discs[__vi];
            // Route through the shared `enum_value` constructor so a
            // host-built enum has the flat-or-boxed representation a
            // script-built one of the same type does (B28 P2).
            match &v.fields {
                Fields::Unit => quote! {
                    Self::#v_ident => ::keleusma::GenericValue::enum_with_widths(
                        ::alloc::string::String::from(#name_str),
                        ::alloc::string::String::from(#v_str),
                        #disc,
                        ::alloc::vec::Vec::new(),
                        __min_payload, __wb, __fb,
                    ),
                },
                Fields::Unnamed(unnamed) => {
                    let count = unnamed.unnamed.len();
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..count)
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    quote! {
                        Self::#v_ident(#(#bindings),*) => ::keleusma::GenericValue::enum_with_widths(
                            ::alloc::string::String::from(#name_str),
                            ::alloc::string::String::from(#v_str),
                            #disc,
                            ::alloc::vec![
                                #(
                                    <#types as ::keleusma::KeleusmaType<__KW, __KF>>::into_value(#bindings),
                                )*
                            ],
                            __min_payload, __wb, __fb,
                        ),
                    }
                }
                Fields::Named(named) => {
                    let names: Vec<&Ident> = named
                        .named
                        .iter()
                        .map(|f| f.ident.as_ref().unwrap())
                        .collect();
                    let types: Vec<&syn::Type> = named.named.iter().map(|f| &f.ty).collect();
                    quote! {
                        Self::#v_ident { #(#names),* } => ::keleusma::GenericValue::enum_with_widths(
                            ::alloc::string::String::from(#name_str),
                            ::alloc::string::String::from(#v_str),
                            #disc,
                            ::alloc::vec![
                                #(
                                    <#types as ::keleusma::KeleusmaType<__KW, __KF>>::into_value(#names),
                                )*
                            ],
                            __min_payload, __wb, __fb,
                        ),
                    }
                }
            }
        })
        .collect();

    // Flat-body read arms keyed on the stored discriminant word (B28 P2).
    // Each reads the variant's payload fields at their packed offsets,
    // past the leading discriminant word, using the field types' kinds.
    let flat_from_arms: Vec<TokenStream2> = data
        .variants
        .iter()
        .enumerate()
        .map(|(__vi, v)| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            let disc = discs[__vi];
            match &v.fields {
                Fields::Unit => quote! {
                    #disc => ::core::result::Result::Ok(Self::#v_ident),
                },
                Fields::Unnamed(unnamed) => {
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..unnamed.unnamed.len())
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    quote! {
                        #disc => {
                            let mut __off = __wb;
                            #(
                                let #bindings = {
                                    let __size = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("flat field of `{}::{}` is not flat-eligible", #name_str, #v_str)
                                        ))?;
                                    let #bindings = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes(
                                        &__bytes[__off..__off + __size], __wb, __fb,
                                    )?;
                                    __off += __size;
                                    #bindings
                                };
                            )*
                            ::core::result::Result::Ok(Self::#v_ident(#(#bindings),*))
                        }
                    }
                }
                Fields::Named(named) => {
                    let names: Vec<&Ident> =
                        named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                    let types: Vec<&syn::Type> = named.named.iter().map(|f| &f.ty).collect();
                    quote! {
                        #disc => {
                            let mut __off = __wb;
                            #(
                                let #names = {
                                    let __size = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("flat field of `{}::{}` is not flat-eligible", #name_str, #v_str)
                                        ))?;
                                    let __val = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes(
                                        &__bytes[__off..__off + __size], __wb, __fb,
                                    )?;
                                    __off += __size;
                                    __val
                                };
                            )*
                            ::core::result::Result::Ok(Self::#v_ident { #(#names),* })
                        }
                    }
                }
            }
        })
        .collect();

    // Context-threading variants of the boxed and flat decode arms (B28
    // P3): identical to `from_arms`/`flat_from_arms` but resolving each
    // field through `from_value_ctx`/`from_flat_bytes_ctx` so a `Text` or
    // opaque payload field is decoded against the VM's arena and registry.
    let from_arms_ctx: Vec<TokenStream2> = data
        .variants
        .iter()
        .map(|v| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            match &v.fields {
                Fields::Unit => quote! {
                    #v_str => {
                        if !fields.is_empty() {
                            return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                ::alloc::format!("variant `{}::{}` expects 0 fields, got {}", #name_str, #v_str, fields.len())
                            ));
                        }
                        ::core::result::Result::Ok(Self::#v_ident)
                    }
                },
                Fields::Unnamed(unnamed) => {
                    let count = unnamed.unnamed.len();
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..count)
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    let positions: Vec<usize> = (0..count).collect();
                    quote! {
                        #v_str => {
                            if fields.len() != #count {
                                return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                    ::alloc::format!("variant `{}::{}` expects {} fields, got {}", #name_str, #v_str, #count, fields.len())
                                ));
                            }
                            #(
                                let #bindings = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value_ctx(&fields[#positions], __ctx)?;
                            )*
                            ::core::result::Result::Ok(Self::#v_ident(#(#bindings),*))
                        }
                    }
                }
                Fields::Named(named) => {
                    let count = named.named.len();
                    let names: Vec<&Ident> = named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                    let types: Vec<&syn::Type> = named.named.iter().map(|f| &f.ty).collect();
                    let positions: Vec<usize> = (0..count).collect();
                    quote! {
                        #v_str => {
                            if fields.len() != #count {
                                return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                                    ::alloc::format!("variant `{}::{}` expects {} fields, got {}", #name_str, #v_str, #count, fields.len())
                                ));
                            }
                            #(
                                let #names = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_value_ctx(&fields[#positions], __ctx)?;
                            )*
                            ::core::result::Result::Ok(Self::#v_ident { #(#names),* })
                        }
                    }
                }
            }
        })
        .collect();

    let flat_from_arms_ctx: Vec<TokenStream2> = data
        .variants
        .iter()
        .enumerate()
        .map(|(__vi, v)| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            let disc = discs[__vi];
            match &v.fields {
                Fields::Unit => quote! {
                    #disc => ::core::result::Result::Ok(Self::#v_ident),
                },
                Fields::Unnamed(unnamed) => {
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..unnamed.unnamed.len())
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    quote! {
                        #disc => {
                            let mut __off = __wb;
                            #(
                                let #bindings = {
                                    let __size = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("flat field of `{}::{}` is not flat-eligible", #name_str, #v_str)
                                        ))?;
                                    let #bindings = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes_ctx(
                                        &__bytes[__off..__off + __size], __wb, __fb, __ctx,
                                    )?;
                                    __off += __size;
                                    #bindings
                                };
                            )*
                            ::core::result::Result::Ok(Self::#v_ident(#(#bindings),*))
                        }
                    }
                }
                Fields::Named(named) => {
                    let names: Vec<&Ident> =
                        named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                    let types: Vec<&syn::Type> = named.named.iter().map(|f| &f.ty).collect();
                    quote! {
                        #disc => {
                            let mut __off = __wb;
                            #(
                                let #names = {
                                    let __size = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::flat_byte_size(__wb, __fb)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("flat field of `{}::{}` is not flat-eligible", #name_str, #v_str)
                                        ))?;
                                    let __val = <#types as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes_ctx(
                                        &__bytes[__off..__off + __size], __wb, __fb, __ctx,
                                    )?;
                                    __off += __size;
                                    __val
                                };
                            )*
                            ::core::result::Result::Ok(Self::#v_ident { #(#names),* })
                        }
                    }
                }
            }
        })
        .collect();

    quote! {
        fn from_value(v: &::keleusma::GenericValue<__KW, __KF>)
            -> ::core::result::Result<Self, ::keleusma::VmError>
        {
            match v {
                ::keleusma::GenericValue::Enum(::keleusma::bytecode::EnumBody::Boxed { type_name, variant, fields }) if type_name == #name_str => {
                    match variant.as_str() {
                        #(#from_arms)*
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("unknown variant `{}::{}`", #name_str, other)
                        )),
                    }
                }
                // A flat enum body carries no type name; the read is shared
                // with `from_flat_bytes`, which the leading discriminant word
                // drives (B28 P2).
                ::keleusma::GenericValue::Enum(::keleusma::bytecode::EnumBody::Flat(__fc)) => {
                    let __wb = (1usize << <__KW as ::keleusma::Word>::BITS_LOG2) / 8;
                    let __fb = (1usize << <__KF as ::keleusma::Float>::BITS_LOG2) / 8;
                    <Self as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes(__fc.as_bytes(), __wb, __fb)
                }
                other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                    ::alloc::format!("expected enum `{}`, got {}", #name_str, other.type_name())
                )),
            }
        }

        fn flat_byte_size(__wb: usize, __fb: usize) -> ::core::option::Option<usize> {
            // A uniformly-flat enum body is `word + payload_max`; a
            // non-uniform enum is not flat-eligible (B28 P2).
            #min_payload_calc
            if __uniform {
                ::core::option::Option::Some(__wb + __min_payload)
            } else {
                ::core::option::Option::None
            }
        }

        fn from_flat_bytes(__bytes: &[u8], __wb: usize, __fb: usize)
            -> ::core::result::Result<Self, ::keleusma::VmError>
        {
            // The leading discriminant word selects the variant, and each
            // payload field is read at its packed offset, recursing through
            // nested flat composites (B28 P2).
            let __disc = match ::keleusma::GenericValue::<__KW, __KF>::read_scalar_le(
                __bytes, 0, ::keleusma::value_layout::ScalarKind::Int, __wb, __fb,
            ) {
                ::keleusma::GenericValue::Int(w) => <__KW as ::keleusma::Word>::to_i64(w),
                _ => {
                    return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                        ::alloc::format!("flat enum `{}` discriminant is not an Int", #name_str)
                    ));
                }
            };
            match __disc {
                #(#flat_from_arms)*
                other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                    ::alloc::format!("unknown discriminant {} for enum `{}`", other, #name_str)
                )),
            }
        }

        fn from_value_ctx(
            v: &::keleusma::GenericValue<__KW, __KF>,
            __ctx: &::keleusma::RefContext<'_>,
        ) -> ::core::result::Result<Self, ::keleusma::VmError>
        {
            match v {
                ::keleusma::GenericValue::Enum(::keleusma::bytecode::EnumBody::Boxed { type_name, variant, fields }) if type_name == #name_str => {
                    match variant.as_str() {
                        #(#from_arms_ctx)*
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("unknown variant `{}::{}`", #name_str, other)
                        )),
                    }
                }
                ::keleusma::GenericValue::Enum(::keleusma::bytecode::EnumBody::Flat(__fc)) => {
                    <Self as ::keleusma::KeleusmaType<__KW, __KF>>::from_flat_bytes_ctx(
                        __fc.as_bytes(), __ctx.word_bytes, __ctx.float_bytes, __ctx,
                    )
                }
                other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                    ::alloc::format!("expected enum `{}`, got {}", #name_str, other.type_name())
                )),
            }
        }

        fn from_flat_bytes_ctx(__bytes: &[u8], __wb: usize, __fb: usize, __ctx: &::keleusma::RefContext<'_>)
            -> ::core::result::Result<Self, ::keleusma::VmError>
        {
            let __disc = match ::keleusma::GenericValue::<__KW, __KF>::read_scalar_le(
                __bytes, 0, ::keleusma::value_layout::ScalarKind::Int, __wb, __fb,
            ) {
                ::keleusma::GenericValue::Int(w) => <__KW as ::keleusma::Word>::to_i64(w),
                _ => {
                    return ::core::result::Result::Err(::keleusma::VmError::TypeError(
                        ::alloc::format!("flat enum `{}` discriminant is not an Int", #name_str)
                    ));
                }
            };
            match __disc {
                #(#flat_from_arms_ctx)*
                other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                    ::alloc::format!("unknown discriminant {} for enum `{}`", other, #name_str)
                )),
            }
        }

        fn into_value(self) -> ::keleusma::GenericValue<__KW, __KF> {
            let __wb = (1usize << <__KW as ::keleusma::Word>::BITS_LOG2) / 8;
            let __fb = (1usize << <__KF as ::keleusma::Float>::BITS_LOG2) / 8;
            #min_payload_calc
            match self {
                #(#into_arms)*
            }
        }
    }
}
