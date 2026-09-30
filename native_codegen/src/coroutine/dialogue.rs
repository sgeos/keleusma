//! Explicit host dialogue contracts, indexed by verified bytecode yield sites.
use super::error;
use crate::LowerError;
use keleusma::bytecode::{BlockType, Module, Op, TypeTag, WireShape};
use std::collections::BTreeMap;

/// A yield instruction in the supplied bytecode module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct YieldSite {
    pub chunk: u32,
    pub instruction: u32,
}
impl YieldSite {
    /// Identifier returned by the emitted host yield-site query.
    pub const fn id(self) -> u64 {
        ((self.chunk as u64) << 32) | self.instruction as u64
    }
}

/// Packed output and reply shapes for one suspension boundary.
/// The host must supply the declared reply representation and readable extent.
/// The completion shape remains the entry function's return signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dialogue {
    pub yielded: WireShape,
    pub reply: WireShape,
}

/// Storage semantics of a native function's declared flat result.
/// The function returns an i64 address of exactly its declared packed body.
/// These are host obligations, just like readable flat argument extents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBodyReturn {
    /// An immutable value. The backend copies the bytes before another native
    /// call or suspension. The host keeps them readable and unchanged until the
    /// next native call or the enclosing start/resume returns, whichever is first.
    /// Returning a pointer into an ended native machine-stack frame is invalid.
    Snapshot,
    /// A view into this instance's shared, private or composite region, with
    /// the declared extent wholly inside that live reservation. Mutations through
    /// other aliases remain visible. The region must outlive every use, including
    /// reading a completed result. Continuation-frame or transient host addresses
    /// do not satisfy this contract. This adds no external memory reservation.
    InstanceBorrow,
}

/// Host representations and lifetimes which bytecode does not fully describe.
#[derive(Clone, Debug, Default)]
pub struct HostContracts {
    /// None selects the uniform dialogue used by `lower`.
    pub dialogues: Option<BTreeMap<YieldSite, Dialogue>>,
    /// Keys index Module::native_names and Module::native_return_shapes.
    /// Each named result must have a bounded flat signature in the module.
    /// A native flat result without an entry remains refused when called.
    pub native_body_returns: BTreeMap<u16, NativeBodyReturn>,
}

pub(crate) struct Plan {
    pub sites: Vec<BTreeMap<usize, (u64, Dialogue)>>,
    pub reachable: Vec<bool>,
    pub native_calls: std::collections::BTreeSet<u16>,
    pub owns_bodies: bool,
    pub metadata_offset: Option<u32>,
    pub native_body_returns: BTreeMap<u16, NativeBodyReturn>,
}

pub(crate) fn tag(shape: WireShape) -> TypeTag {
    match shape {
        WireShape::Scalar { kind: 0 } => TypeTag::Unit,
        WireShape::Scalar { kind: 1 } => TypeTag::Bool,
        WireShape::Scalar { kind: 2 } => TypeTag::Byte,
        WireShape::Scalar { kind: 3 } => TypeTag::Word,
        WireShape::Scalar { kind: 4 } => TypeTag::Fixed,
        WireShape::Scalar { kind: 5 } => TypeTag::Float,
        _ => TypeTag::Composite,
    }
}
fn supported(shape: WireShape) -> bool {
    matches!(
        shape,
        WireShape::Scalar { kind: 0..=5 } | WireShape::Flat { kind: 0..=3, .. }
    )
}
fn reachable_chunks(m: &Module, entry: usize) -> Vec<bool> {
    let mut reachable = vec![false; m.chunks.len()];
    let mut pending = vec![entry];
    while let Some(index) = pending.pop() {
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        for op in &m.chunks[index].ops {
            if let Op::Call(target, _) = op {
                pending.push(*target as usize);
            }
        }
    }
    reachable
}

impl Plan {
    pub fn new(
        m: &Module,
        entry: usize,
        explicit: Option<&BTreeMap<YieldSite, Dialogue>>,
        frame_bytes: u32,
        native_body_returns: &BTreeMap<u16, NativeBodyReturn>,
    ) -> Result<Self, LowerError> {
        for &index in native_body_returns.keys() {
            if usize::from(index) >= m.native_names.len()
                || !matches!(
                    m.native_return_shapes.get(usize::from(index)),
                    Some(WireShape::Flat { kind: 0..=3, .. })
                )
            {
                return Err(error(format!(
                    "native body contract {index} needs a bounded flat return signature"
                )));
            }
        }
        let reachable = reachable_chunks(m, entry);
        let native_calls: std::collections::BTreeSet<u16> = m
            .chunks
            .iter()
            .zip(&reachable)
            .filter(|(_, live)| **live)
            .flat_map(|(chunk, _)| &chunk.ops)
            .filter_map(|op| match op {
                Op::CallVerifiedNative(index, _) | Op::CallExternalNative(index, _) => Some(*index),
                _ => None,
            })
            .collect();
        let signature = &m.signatures[entry];
        let default = Dialogue {
            yielded: signature.ret,
            reply: signature
                .params
                .first()
                .copied()
                .unwrap_or(WireShape::Scalar { kind: 0 }),
        };
        let mut sites = vec![BTreeMap::new(); m.chunks.len()];
        let mut owns_bodies = signature
            .params
            .iter()
            .any(|s| matches!(s, WireShape::Flat { .. }));
        owns_bodies |= native_body_returns.iter().any(|(index, contract)| {
            native_calls.contains(index) && *contract == NativeBodyReturn::Snapshot
        });
        for (ci, chunk) in m.chunks.iter().enumerate() {
            if !reachable[ci] {
                continue;
            }
            for (ip, op) in chunk.ops.iter().enumerate() {
                if !matches!(op, Op::Yield) {
                    continue;
                }
                let site = YieldSite {
                    chunk: u32::try_from(ci)
                        .map_err(|_| error("yield chunk identifier overflow"))?,
                    instruction: u32::try_from(ip)
                        .map_err(|_| error("yield instruction identifier overflow"))?,
                };
                let contract = match explicit {
                    Some(map) => *map
                        .get(&site)
                        .ok_or_else(|| error(format!("missing dialogue for {site:?}")))?,
                    None => default,
                };
                if !supported(contract.yielded) || !supported(contract.reply) {
                    return Err(error("dialogue requires scalar or bounded flat shapes"));
                }
                if m.chunks[entry].block_type == BlockType::Stream
                    && let Some(input) = signature.params.first()
                    && !matches!(
                        m.chunks[entry].param_types.first(),
                        Some(TypeTag::Composite)
                    )
                    && contract.reply != *input
                {
                    return Err(error(
                        "stream dialogue reply must match the first parameter shape",
                    ));
                }
                owns_bodies |= matches!(contract.reply, WireShape::Flat { .. });
                sites[ci].insert(ip, (site.id(), contract));
            }
        }
        if explicit.is_some_and(|map| {
            map.keys().any(|site| {
                !matches!(
                    m.chunks
                        .get(site.chunk as usize)
                        .and_then(|chunk| chunk.ops.get(site.instruction as usize)),
                    Some(Op::Yield)
                )
            })
        }) {
            return Err(error("dialogue contract names a non-yield site"));
        }
        let metadata_offset = if explicit.is_some() {
            Some(
                frame_bytes
                    .checked_sub(8)
                    .ok_or_else(|| error("frame is too small for dialogue metadata"))?
                    & !7,
            )
        } else {
            None
        };
        Ok(Self {
            sites,
            reachable,
            native_calls,
            owns_bodies,
            metadata_offset,
            native_body_returns: native_body_returns.clone(),
        })
    }
}
