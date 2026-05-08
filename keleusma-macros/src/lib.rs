//! Procedural macros for the Keleusma scripting language.
//!
//! This crate provides the `KeleusmaType` derive macro, which generates
//! `KeleusmaType` trait implementations for host-defined structs and enums
//! whose field and payload types are admissible interop types. The generated
//! code converts between the host type and the runtime `Value` enum at the
//! native function boundary.
//!
//! See the runtime crate documentation for details on the trait and the
//! admissibility rules.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Ident, parse_macro_input};

/// Derive `KeleusmaType` for a struct or enum whose fields and payloads are
/// admissible interop types.
#[proc_macro_derive(KeleusmaType)]
pub fn derive_keleusma_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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
        impl #impl_generics ::keleusma::KeleusmaType for #name #ty_generics #where_clause {
            #body
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
                fn from_value(v: &::keleusma::Value) -> ::core::result::Result<Self, ::keleusma::VmError> {
                    match v {
                        ::keleusma::Value::Struct { type_name, fields } if type_name == #name_str => {
                            #(
                                let #field_names = {
                                    let pair = fields.iter().find(|(n, _)| n == #field_name_strs)
                                        .ok_or_else(|| ::keleusma::VmError::TypeError(
                                            ::alloc::format!("missing field `{}` on `{}`", #field_name_strs, #name_str)
                                        ))?;
                                    <#field_types as ::keleusma::KeleusmaType>::from_value(&pair.1)?
                                };
                            )*
                            ::core::result::Result::Ok(Self { #(#field_names),* })
                        }
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("expected struct `{}`, got {}", #name_str, other.type_name())
                        )),
                    }
                }

                fn into_value(self) -> ::keleusma::Value {
                    let fields: ::alloc::vec::Vec<(::alloc::string::String, ::keleusma::Value)> = ::alloc::vec![
                        #(
                            (
                                ::alloc::string::String::from(#field_name_strs),
                                <#field_types as ::keleusma::KeleusmaType>::into_value(self.#field_names),
                            ),
                        )*
                    ];
                    ::keleusma::Value::Struct {
                        type_name: ::alloc::string::String::from(#name_str),
                        fields,
                    }
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

fn derive_enum_body(_name: &Ident, name_str: &str, data: &DataEnum) -> TokenStream2 {
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
                                let #bindings = <#types as ::keleusma::KeleusmaType>::from_value(&fields[#positions])?;
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
                                let #names = <#types as ::keleusma::KeleusmaType>::from_value(&fields[#positions])?;
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
        .map(|v| {
            let v_ident = &v.ident;
            let v_str = v_ident.to_string();
            match &v.fields {
                Fields::Unit => quote! {
                    Self::#v_ident => ::keleusma::Value::Enum {
                        type_name: ::alloc::string::String::from(#name_str),
                        variant: ::alloc::string::String::from(#v_str),
                        fields: ::alloc::vec::Vec::new(),
                    },
                },
                Fields::Unnamed(unnamed) => {
                    let count = unnamed.unnamed.len();
                    let types: Vec<&syn::Type> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
                    let bindings: Vec<Ident> = (0..count)
                        .map(|i| Ident::new(&format!("__f{}", i), proc_macro2::Span::call_site()))
                        .collect();
                    quote! {
                        Self::#v_ident(#(#bindings),*) => ::keleusma::Value::Enum {
                            type_name: ::alloc::string::String::from(#name_str),
                            variant: ::alloc::string::String::from(#v_str),
                            fields: ::alloc::vec![
                                #(
                                    <#types as ::keleusma::KeleusmaType>::into_value(#bindings),
                                )*
                            ],
                        },
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
                        Self::#v_ident { #(#names),* } => ::keleusma::Value::Enum {
                            type_name: ::alloc::string::String::from(#name_str),
                            variant: ::alloc::string::String::from(#v_str),
                            fields: ::alloc::vec![
                                #(
                                    <#types as ::keleusma::KeleusmaType>::into_value(#names),
                                )*
                            ],
                        },
                    }
                }
            }
        })
        .collect();

    quote! {
        fn from_value(v: &::keleusma::Value) -> ::core::result::Result<Self, ::keleusma::VmError> {
            match v {
                ::keleusma::Value::Enum { type_name, variant, fields } if type_name == #name_str => {
                    match variant.as_str() {
                        #(#from_arms)*
                        other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                            ::alloc::format!("unknown variant `{}::{}`", #name_str, other)
                        )),
                    }
                }
                other => ::core::result::Result::Err(::keleusma::VmError::TypeError(
                    ::alloc::format!("expected enum `{}`, got {}", #name_str, other.type_name())
                )),
            }
        }

        fn into_value(self) -> ::keleusma::Value {
            match self {
                #(#into_arms)*
            }
        }
    }
}
