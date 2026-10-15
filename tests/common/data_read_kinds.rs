//! Source-derived fallback kinds for the legacy parser and codegen test adapters.
use keleusma::ast::{PrimType, Program, TypeExpr};
use std::collections::BTreeMap;

pub fn data_read_kinds(program: &Program) -> BTreeMap<String, i64> {
    program
        .data_decls
        .iter()
        .flat_map(|data| {
            data.fields.iter().map(move |field| {
                let mut ty = &field.type_expr;
                while let TypeExpr::Array(inner, _, _) = ty {
                    ty = inner;
                }
                let kind = match ty {
                    TypeExpr::Prim(PrimType::Word, _) => 3,
                    TypeExpr::Prim(PrimType::Byte, _) => 2,
                    _ => 0,
                };
                (format!("{}.{}", data.name, field.name), kind)
            })
        })
        .collect()
}
