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

pub(crate) struct Plan {
    pub sites: Vec<BTreeMap<usize, (u64, Dialogue)>>,
    pub owns_bodies: bool,
    pub metadata_offset: Option<u32>,
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
impl Plan {
    pub fn new(
        m: &Module,
        entry: usize,
        explicit: Option<&BTreeMap<YieldSite, Dialogue>>,
        frame_bytes: u32,
    ) -> Result<Self, LowerError> {
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
        let mut count = 0;
        for (ci, chunk) in m.chunks.iter().enumerate() {
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
                    && contract.reply != *input
                {
                    return Err(error(
                        "stream dialogue reply must match the first parameter shape",
                    ));
                }
                owns_bodies |= matches!(contract.reply, WireShape::Flat { .. });
                sites[ci].insert(ip, (site.id(), contract));
                count += 1;
            }
        }
        if explicit.is_some_and(|map| map.len() != count) {
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
            owns_bodies,
            metadata_offset,
        })
    }
}
