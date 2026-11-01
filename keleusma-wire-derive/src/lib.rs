#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! `#[derive(WireRecord)]` for `keleusma-wire`.
//!
//! Implementation detail of `keleusma-wire`. Depend on that crate with the
//! `derive` feature rather than on this one directly.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type, parse_macro_input, spanned::Spanned};

/// Derives a fixed-size wire record: offset constants, a stride, and total
/// read/write methods.
///
/// Fields are laid out **packed, in declaration order, with no implicit
/// padding** — the container is byte-addressed and adds none — and the record as
/// a whole is padded to a whole 64-bit word so that element *i* of a table sits
/// at a power-of-two stride.
///
/// Permitted field types are the fixed-width little-endian scalars (`u8`, `u16`,
/// `u32`, `u64`, `i8`, `i16`, `i32`, `i64`) and byte arrays (`[u8; N]`). Anything
/// else is rejected with an error naming the field, because a type whose in-memory
/// size differs from its wire width — anything with alignment padding, a pointer,
/// or a platform-dependent size — would silently produce offsets that do not match
/// the bytes.
#[proc_macro_derive(WireRecord)]
pub fn derive_wire_record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// How a field is read and written on the wire.
enum Kind {
    /// Unsigned little-endian scalar of the given byte width.
    Unsigned(usize),
    /// Signed little-endian scalar, read through its unsigned counterpart.
    Signed(usize),
    /// Fixed-length byte array.
    Bytes,
}

fn classify(ty: &Type) -> Option<Kind> {
    if let Type::Array(arr) = ty {
        if let Type::Path(p) = &*arr.elem {
            if p.path.is_ident("u8") {
                return Some(Kind::Bytes);
            }
        }
        return None;
    }
    let Type::Path(p) = ty else { return None };
    let ident = p.path.get_ident()?.to_string();
    match ident.as_str() {
        "u8" => Some(Kind::Unsigned(1)),
        "u16" => Some(Kind::Unsigned(2)),
        "u32" => Some(Kind::Unsigned(4)),
        "u64" => Some(Kind::Unsigned(8)),
        "i8" => Some(Kind::Signed(1)),
        "i16" => Some(Kind::Signed(2)),
        "i32" => Some(Kind::Signed(4)),
        "i64" => Some(Kind::Signed(8)),
        _ => None,
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "WireRecord can only be derived for a struct: a wire record is a fixed \
             sequence of fields, which an enum or union is not",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            input.span(),
            "WireRecord requires named fields, since the offset constants are named after them",
        ));
    };
    if fields.named.is_empty() {
        return Err(syn::Error::new(
            input.span(),
            "WireRecord requires at least one field",
        ));
    }
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "WireRecord does not support generic parameters: a record's layout must be \
             a single fixed set of offsets",
        ));
    }

    let mut offset_consts = Vec::new();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    // Running offset expression, built up so that `[u8; N]` widths resolve during
    // const evaluation rather than needing to be known here.
    let mut offset_expr = quote! { 0usize };

    for field in &fields.named {
        let fname = field.ident.as_ref().expect("named fields checked above");
        let ty = &field.ty;
        let kind = classify(ty).ok_or_else(|| {
            syn::Error::new(
                ty.span(),
                format!(
                    "field `{fname}` has an unsupported type for a wire record. \
                     Permitted: u8/u16/u32/u64, i8/i16/i32/i64, and [u8; N]. \
                     A type whose in-memory size differs from its wire width would \
                     produce offsets that silently disagree with the bytes"
                ),
            )
        })?;

        let const_name = offset_ident(fname);
        offset_consts.push(quote! {
            #[allow(missing_docs)]
            pub const #const_name: usize = #offset_expr;
        });

        let off = quote! { Self::#const_name };
        let width = quote! { ::core::mem::size_of::<#ty>() };

        match kind {
            Kind::Unsigned(w) | Kind::Signed(w) => {
                let reader = format_ident!("u{}_at", w * 8);
                let writer = format_ident!("u{}_bytes", w * 8);
                let unsigned = format_ident!("u{}", w * 8);
                match kind {
                    Kind::Signed(_) => {
                        reads.push(quote! {
                            #fname: ::keleusma_wire::scalar::#reader(bytes, #off)? as #ty,
                        });
                        writes.push(quote! {
                            out[#off .. #off + #width].copy_from_slice(
                                &::keleusma_wire::scalar::#writer(self.#fname as #unsigned)
                            );
                        });
                    }
                    _ => {
                        reads.push(quote! {
                            #fname: ::keleusma_wire::scalar::#reader(bytes, #off)?,
                        });
                        writes.push(quote! {
                            out[#off .. #off + #width].copy_from_slice(
                                &::keleusma_wire::scalar::#writer(self.#fname)
                            );
                        });
                    }
                }
            }
            Kind::Bytes => {
                reads.push(quote! {
                    #fname: {
                        let mut buf = [0u8; #width];
                        buf.copy_from_slice(bytes.get(#off .. #off + #width)?);
                        buf
                    },
                });
                writes.push(quote! {
                    out[#off .. #off + #width].copy_from_slice(&self.#fname);
                });
            }
        }

        offset_expr = quote! { #off + #width };
    }

    // Total packed size, then the record padded up to a whole word.
    let packed = offset_expr;

    Ok(quote! {
        impl #name {
            #(#offset_consts)*

            /// Total size of the fields, before word padding.
            pub const PACKED_BYTES: usize = #packed;
        }

        impl ::keleusma_wire::WireRecord for #name {
            const STRIDE: usize = Self::PACKED_BYTES.next_multiple_of(
                ::keleusma_wire::layout::WORD
            );

            fn read_record(bytes: &[u8]) -> ::core::option::Option<Self> {
                if bytes.len() < <Self as ::keleusma_wire::WireRecord>::STRIDE {
                    return ::core::option::Option::None;
                }
                ::core::option::Option::Some(Self {
                    #(#reads)*
                })
            }

            fn write_record(&self, out: &mut [u8]) -> ::core::option::Option<()> {
                if out.len() < <Self as ::keleusma_wire::WireRecord>::STRIDE {
                    return ::core::option::Option::None;
                }
                #(#writes)*
                ::core::option::Option::Some(())
            }
        }
    })
}

/// `name_off` becomes `OFFSET_NAME_OFF`.
fn offset_ident(field: &Ident) -> Ident {
    format_ident!("OFFSET_{}", field.to_string().to_uppercase())
}
