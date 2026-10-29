//! Opaque type names of each native's signature, read from the source.
//!
//! A named type the program does not declare as a struct, an enum or a
//! newtype is opaque, which is the type checker's rule. The module records an
//! opaque result only as scalar kind 7 and records no native parameter types,
//! so the names a typed header needs are read from the `use` declarations
//! here. They feed [`HostContracts::opaque_natives`].
//!
//! A bare `use host::name` declares no signature and contributes nothing, so
//! its positions stay `int64_t` in the header.
//!
//! [`HostContracts::opaque_natives`]: crate::coroutine::HostContracts::opaque_natives

use crate::coroutine::NativeOpaque;
use keleusma::ast::{ImportItem, Program, TypeDef, TypeExpr};
use keleusma::bytecode::Module;
use std::collections::{BTreeMap, BTreeSet};

/// The opaque positions of every native `module` calls, keyed by its index in
/// `Module::native_names`. A native with no opaque position has no entry.
///
/// `program` must be the source `module` was compiled from. A declaration
/// whose native the module does not call is ignored.
pub fn opaque_natives(program: &Program, module: &Module) -> BTreeMap<u16, NativeOpaque> {
    let declared: BTreeSet<&str> = program
        .types
        .iter()
        .map(|t| match t {
            TypeDef::Struct(s) => s.name.as_str(),
            TypeDef::Enum(e) => e.name.as_str(),
            TypeDef::Newtype(n) => n.name.as_str(),
        })
        .collect();
    let opaque = |t: &TypeExpr| match t {
        TypeExpr::Named(name, _, _, _) if !declared.contains(name.as_str()) => Some(name.clone()),
        _ => None,
    };
    let mut out = BTreeMap::new();
    for decl in &program.uses {
        let (ImportItem::Name(name), Some(sig)) = (&decl.import, &decl.signature) else {
            continue;
        };
        let full = if decl.path.is_empty() {
            name.clone()
        } else {
            let mut full = decl.path.join("::");
            full.push_str("::");
            full.push_str(name);
            full
        };
        let Some(index) = module.native_names.iter().position(|n| *n == full) else {
            continue;
        };
        let Ok(index) = u16::try_from(index) else {
            continue;
        };
        let entry = NativeOpaque {
            params: sig.params.iter().map(opaque).collect(),
            ret: opaque(&sig.return_type),
        };
        if entry.any() {
            out.insert(index, entry);
        }
    }
    out
}
