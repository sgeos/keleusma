//! **THE DOCUMENTED REGION SUPPLEMENT FORMULA IS THE BACKEND'S.**
//!
//! Operator question 22, answered on 2026-10-31, has V0.2.X fold the native
//! region supplement into `auto_arena_capacity_for`, and V0.2.X asked for the
//! formula as data. `docs/decisions/NATIVE_REGION_SUPPLEMENT_FORMULA.md` states
//! it with a reference implementation in terms of `keleusma` types alone. This
//! file compiles that implementation as the record prints it, and fails if the
//! record's code block and this copy drift apart, or if the formula disagrees
//! with `region::host_arena_supplement_bytes` on any corpus module.
#![allow(dead_code)]
mod common;

mod reference {
    use keleusma::bytecode::{
        BlockType, Chunk, ConstValue, Module, NewCompositeOperand, Op, WireShape,
    };

    fn align(n: u32) -> u32 {
        n.checked_add(7).map_or(u32::MAX, |x| x & !7)
    }

    fn const_len(c: &ConstValue) -> Option<u32> {
        match c {
            ConstValue::Int(_) | ConstValue::Fixed(_) => Some(8),
            ConstValue::Byte(_) | ConstValue::Bool(_) => Some(1),
            ConstValue::Tuple(xs) | ConstValue::Array(xs) => xs
                .iter()
                .try_fold(0u32, |a, x| a.checked_add(const_len(x)?)),
            _ => None,
        }
    }

    fn site_size(chunk: &Chunk, op: &Op) -> Option<u32> {
        match op {
            Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) => {
                Some(u32::from(*byte_size))
            }
            Op::Const(i) => match chunk.constants.get(usize::from(*i))? {
                c @ (ConstValue::Tuple(_) | ConstValue::Array(_)) => const_len(c),
                _ => None,
            },
            _ => None,
        }
    }

    fn sites(chunk: &Chunk) -> u32 {
        chunk
            .ops
            .iter()
            .filter_map(|op| site_size(chunk, op))
            .fold(0u32, |next, size| align(next.saturating_add(size)))
    }

    fn total(module: &Module, chunk: usize, depth: usize) -> u32 {
        if depth > 64 {
            return 0;
        }
        let Some(c) = module.chunks.get(chunk) else {
            return 0;
        };
        let stream = c.block_type == BlockType::Stream;
        let locals = if stream {
            align(u32::from(c.local_count).saturating_mul(8))
        } else {
            0
        };
        let spill = if stream { align(64 * 8) } else { 0 };
        let mut t = align(sites(c).saturating_add(locals).saturating_add(spill));
        for op in &c.ops {
            if let Op::Call(idx, _) = op {
                t = align(t).saturating_add(total(module, usize::from(*idx), depth + 1));
            }
        }
        t
    }

    fn has_hazard(c: &Chunk) -> bool {
        let mut open = Vec::new();
        for (i, op) in c.ops.iter().enumerate() {
            match op {
                Op::Loop(_) => open.push(i),
                Op::EndLoop(_) => {
                    if let Some(s) = open.pop() {
                        let body = &c.ops[s + 1..i];
                        if body.iter().any(|o| matches!(o, Op::Yield))
                            && body.iter().any(|o| {
                                matches!(o, Op::NewComposite(NewCompositeOperand::Flat { .. }))
                            })
                        {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn yield_copy_bytes(module: &Module, entry: usize) -> Option<(u32, u32)> {
        use keleusma::confine::{Confinement, Scope};
        let c = module.chunks.get(entry)?;
        if c.block_type != BlockType::Stream || !has_hazard(c) {
            return None;
        }
        let WireShape::Flat { size, .. } = module.signatures.get(entry)?.ret else {
            return None;
        };
        let n = keleusma::verify::max_resumes_before_reset(module, entry).ok()?;
        if size == 0 || n == 0 {
            return None;
        }
        size.checked_mul(n)?.checked_add(8)?;
        let mut clone = module.clone();
        for op in &mut clone.chunks[entry].ops {
            if matches!(op, Op::Yield) {
                *op = Op::Len;
            }
        }
        let confined = keleusma::confine::module_confinement(&clone)
            .get(entry)
            .is_some_and(|v| {
                v.iter()
                    .filter(|s| matches!(s.scope, Scope::Iteration { .. }))
                    .all(|s| matches!(s.verdict, Confinement::Confined))
            });
        if !confined {
            return None;
        }
        Some((
            align(total(module, entry, 0)),
            align(8u32.saturating_add(size.saturating_mul(n))),
        ))
    }

    pub fn native_region_supplement(module: &Module) -> u32 {
        let Some(entry) = module.entry_point else {
            return 0;
        };
        match yield_copy_bytes(module, entry) {
            Some((offset, bytes)) => offset.saturating_add(bytes),
            None => total(module, entry, 0),
        }
    }
}

#[test]
fn the_documented_formula_reproduces_every_corpus_figure() {
    let mut compared = 0;
    let mut with_copy = 0;
    for (name, m) in common::corpus() {
        let native = keleusma_native::region::host_arena_supplement_bytes(&m);
        assert_eq!(
            reference::native_region_supplement(&m),
            native,
            "{name}: the documented formula disagrees with the backend"
        );
        if keleusma_native::region::yield_copy(&m).is_some() {
            with_copy += 1;
        }
        compared += 1;
    }
    assert!(
        compared >= 70,
        "only {compared} corpus modules were compared"
    );
    assert!(
        with_copy >= 1,
        "no corpus module exercises the copy at the yield term"
    );
}

/// Absorption 127 brought V0.2.X's own copy of the formula,
/// `keleusma::native_region`, which `auto_arena_capacity_for` now adds. The
/// backend stays the authority, so that copy is checked here against it on
/// every corpus module, which is the check V0.2.X's module documentation says
/// this line makes.
#[test]
fn the_shipping_crates_copy_agrees_with_the_backend() {
    let mut compared = 0;
    let mut nonzero = 0;
    for (name, m) in common::corpus() {
        let native = keleusma_native::region::host_arena_supplement_bytes(&m);
        assert_eq!(
            keleusma::native_region::native_region_supplement_bytes(&m),
            native,
            "{name}: keleusma::native_region disagrees with the backend"
        );
        if native > 0 {
            nonzero += 1;
        }
        compared += 1;
    }
    assert!(
        compared >= 70,
        "only {compared} corpus modules were compared"
    );
    assert!(nonzero >= 1, "no corpus module has a supplement to compare");
}

#[test]
fn the_record_prints_this_code() {
    let doc = std::fs::read_to_string("../docs/decisions/NATIVE_REGION_SUPPLEMENT_FORMULA.md")
        .expect("the record is readable");
    let block = doc
        .split("```rust\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .expect("the record holds a rust block");
    let this = std::fs::read_to_string("tests/region_supplement_formula.rs").expect("this file");
    let module = this
        .split("mod reference {\n")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n\n#[test]").next())
        .expect("the reference module");
    let unindented: String = module
        .lines()
        .map(|l| l.strip_prefix("    ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        unindented.trim(),
        block.trim(),
        "the record's code and this file's reference module have drifted"
    );
}
