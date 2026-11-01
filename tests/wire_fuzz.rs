//! Randomised input testing for the wire-format decoders.
//!
//! # What this adds over the existing tests
//!
//! The suite already exercises *every* single-byte corruption and *every*
//! truncation of a valid artifact. Both are exhaustive, and both are
//! **structured**: the input remains a nearly-valid artifact. This covers the
//! cases those cannot reach —
//!
//! - **multi-byte** corruption, where several fields disagree at once;
//! - **wholly random** bytes, which usually fail at the prologue but
//!   occasionally will not;
//! - **plausible-header garbage**, a valid prologue over a random body, which is
//!   the shape a real attacker or a truncated-then-appended file produces.
//!
//! # Determinism
//!
//! The generator is a fixed-seed xorshift, so a failure is reproducible from the
//! iteration number alone. A randomised test that cannot be replayed reports a
//! defect nobody can then find.
//!
//! # The contract under test
//!
//! Decoding is **total**: any byte sequence yields a value or an error, never a
//! panic, an out-of-bounds read, or a hang. Nothing here asserts that a mutated
//! artifact decodes to anything in particular — a corrupted artifact is allowed
//! to decode to a different valid value.

#![cfg(feature = "compile")]

use keleusma::bytecode::{BlockType, ConstValue, TypeTag, WireShape};
use keleusma::wire_format::{WireAuxBody, WireChunk};
use keleusma::wire_schema::{
    AuxView, ConstTable, DataLayoutTable, LayoutTable, ModuleTable, ParamTypeTable, SignatureTable,
    decode_aux_body, encode_aux_body,
};

/// Byte length of the artifact's header area: the triplicated prologue plus the
/// triplicated region directory.
///
/// Preserving only the 48-byte prologue is **not enough to get past parsing** --
/// the directory is voted too, so randomising it corrupts all three copies and
/// the region bounds fail. Discovered by a vacuity check: a generator that kept
/// 48 bytes produced 0 parsing inputs out of 2000, meaning the readers under
/// test never ran.
fn header_len(bytes: &[u8]) -> usize {
    let region_count = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    48 + region_count * 16 * 3
}

/// Fixed-seed xorshift64*. Small, deterministic, and dependency-free.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Zero is a fixed point of xorshift; force a non-zero state.
        Self(seed | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 33) as u8
    }
}

/// Exercises every reader against one input. The only requirement is that this
/// returns rather than panicking.
fn poke_every_reader(bytes: &[u8]) {
    let _ = decode_aux_body(bytes);
    let _ = ConstTable::parse(bytes);
    let _ = SignatureTable::parse(bytes);
    let _ = LayoutTable::parse(bytes);
    let _ = DataLayoutTable::parse(bytes);
    let _ = ParamTypeTable::parse(bytes);
    let _ = ModuleTable::parse(bytes);

    if let Ok(v) = AuxView::parse(bytes) {
        for c in 0..v.chunk_count().saturating_add(2) {
            let _ = v.local_count(c);
            let _ = v.const_count(c);
            let _ = v.template_count(c);
            for i in 0..3 {
                let _ = v.const_record(c, i);
                let _ = v.chunk_const_str_bytes(c, i);
                let _ = v.template_type_name(c, i);
                let _ = v.template_field_name(c, i, 0);
            }
        }
        for i in 0..v.enum_layout_count().saturating_add(2) {
            let _ = v.enum_type_name(i);
            let _ = v.enum_variant(i, 0);
        }
        let _ = v.word_bits_log2();
        let _ = v.schema_hash();
        if let Some(dl) = v.data_layout() {
            for i in 0..dl.slot_count().saturating_add(2) {
                let _ = dl.slot(i);
                let _ = dl.slot_name(i);
            }
        }
    }
}

fn seed_artifact() -> Vec<u8> {
    let aux = WireAuxBody {
        chunks: vec![
            WireChunk {
                name: "main".into(),
                constants: vec![
                    ConstValue::Int(-3),
                    ConstValue::StaticStr("str".into()),
                    ConstValue::Array(vec![ConstValue::Byte(1), ConstValue::Byte(2)]),
                    ConstValue::Struct {
                        type_name: "P".into(),
                        fields: vec![("x".into(), ConstValue::Unit)],
                    },
                    ConstValue::Enum {
                        type_name: "E".into(),
                        variant: "V".into(),
                        discriminant: Some(-1),
                        fields: vec![ConstValue::Int(9)],
                    },
                ],
                struct_templates: vec![],
                local_count: 3,
                param_count: 1,
                block_type: BlockType::Func,
                param_types: vec![TypeTag::Word],
                op_byte_offset: 16,
                op_record_count: 4,
                debug_pool_bytes: Some(b"dbg".to_vec()),
            },
            WireChunk {
                name: "tick".into(),
                constants: vec![],
                struct_templates: vec![],
                local_count: 0,
                param_count: 0,
                block_type: BlockType::Stream,
                param_types: vec![],
                op_byte_offset: 0,
                op_record_count: 0,
                debug_pool_bytes: None,
            },
        ],
        native_names: vec!["host::n".into()],
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        wcet_cycles: 7,
        wcmu_bytes: 8,
        flags: 1,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 99,
        enum_layouts: vec![],
        signatures: vec![],
        native_return_shapes: vec![WireShape::Top],
    };
    encode_aux_body(&aux).expect("seed artifact must encode")
}

/// How many mutated inputs actually parse, rather than being rejected at the
/// prologue. A randomised suite where nothing parses exercises the framing check
/// and nothing else, so this is asserted rather than assumed.
fn count_parsing<F: FnMut(&mut Rng) -> Vec<u8>>(iters: usize, seed: u64, mut make: F) -> usize {
    let mut rng = Rng::new(seed);
    let mut parsed = 0;
    for _ in 0..iters {
        let buf = make(&mut rng);
        if AuxView::parse(&buf).is_ok() {
            parsed += 1;
        }
    }
    parsed
}

#[test]
fn the_randomised_inputs_actually_reach_the_readers() {
    // Vacuity check. A "survives corruption" suite proves nothing if every input
    // dies at the magic number: the readers under test would never run.
    let seed = seed_artifact();

    let keep = header_len(&seed);
    let deep = count_parsing(2000, 0xBADC_0FFE, |rng| {
        let mut m = seed.clone();
        let hits = 1 + rng.below(4);
        for _ in 0..hits {
            let at = keep + rng.below(m.len() - keep);
            m[at] = rng.byte();
        }
        m
    });
    // A valid prologue over a randomised body: most should get past framing and
    // into the directory and payload, which is the whole point of that generator.
    assert!(
        deep > 500,
        "payload-perturbed inputs rarely parsed ({deep}/2000); the readers are barely exercised"
    );

    let appended = count_parsing(200, 0xFEED_BEEF, |rng| {
        let mut m = seed.clone();
        for _ in 0..rng.below(128) {
            m.push(rng.byte());
        }
        m
    });
    assert_eq!(appended, 200, "appending must never invalidate an artifact");

    println!("randomised coverage: valid-prologue parsed {deep}/2000, appended {appended}/200");
}

#[test]
fn multi_byte_corruption_is_survived() {
    // Two to sixteen simultaneous byte changes. The exhaustive single-byte test
    // cannot reach a state where several fields disagree at once -- for example a
    // length and the offset it bounds, both plausible individually.
    let seed = seed_artifact();
    let mut rng = Rng::new(0x5EED_1234);

    for iteration in 0..4000 {
        let mut m = seed.clone();
        let hits = 2 + rng.below(15);
        for _ in 0..hits {
            let at = rng.below(m.len());
            m[at] = rng.byte();
        }
        // Panicking here reports the iteration, so the case is reproducible from
        // the seed above.
        poke_every_reader(&m);
        let _ = iteration;
    }
}

#[test]
fn wholly_random_bytes_are_survived() {
    // Almost all of these fail at the prologue. The point is the tail that does
    // not: a random buffer that happens to satisfy the magic and vote.
    let mut rng = Rng::new(0xD1CE_0F00);
    for _ in 0..4000 {
        let len = rng.below(600);
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push(rng.byte());
        }
        poke_every_reader(&buf);
    }
}

#[test]
fn a_valid_prologue_over_a_random_body_is_survived() {
    // The interesting adversarial shape, and the one random bytes rarely reach:
    // framing that parses, directory and payload that do not. This is also what a
    // truncated-then-appended file looks like.
    let seed = seed_artifact();
    let mut rng = Rng::new(0xBADC_0FFE);

    // Preserve the whole header area -- prologue AND directory, both triplicated
    // and voted -- then perturb the payload LIGHTLY.
    //
    // Two calibrations, both forced by the vacuity check rather than guessed.
    // Keeping only the 48-byte prologue left the voted directory corrupt and
    // 0/2000 inputs parsed. Keeping the header but randomising a quarter of the
    // payload still gave only 4/2000, because the decoder validates ordering,
    // name indices, block tags and ranges, so heavy corruption trips one of them
    // before any reader runs. A handful of changed bytes is what actually gets
    // inside.
    let keep = header_len(&seed);
    for _ in 0..4000 {
        let mut m = seed.clone();
        let hits = 1 + rng.below(4);
        for _ in 0..hits {
            let at = keep + rng.below(m.len() - keep);
            m[at] = rng.byte();
        }
        poke_every_reader(&m);
    }
}

#[test]
fn random_truncation_and_extension_are_survived() {
    // Truncation is covered exhaustively elsewhere; extension is not. Trailing
    // bytes must not be mistaken for a region, and a short buffer must not be
    // read past.
    let seed = seed_artifact();
    let mut rng = Rng::new(0x7A11_0000);

    for _ in 0..2000 {
        let mut m = seed.clone();
        match rng.below(3) {
            0 => m.truncate(rng.below(seed.len() + 1)),
            1 => {
                let extra = rng.below(64);
                for _ in 0..extra {
                    m.push(rng.byte());
                }
            }
            _ => {
                m.truncate(rng.below(seed.len() + 1));
                let extra = rng.below(64);
                for _ in 0..extra {
                    m.push(rng.byte());
                }
            }
        }
        poke_every_reader(&m);
    }
}

#[test]
fn appending_bytes_does_not_change_what_a_valid_artifact_decodes_to() {
    // A stronger claim than "does not panic": the region directory bounds every
    // read, so trailing bytes are inert. If this ever fails, some reader is
    // deriving a length from the buffer size rather than from the directory.
    let seed = seed_artifact();
    let want = decode_aux_body(&seed).expect("seed decodes");
    let mut rng = Rng::new(0xFEED_BEEF);

    for _ in 0..200 {
        let mut m = seed.clone();
        for _ in 0..rng.below(128) {
            m.push(rng.byte());
        }
        let got = decode_aux_body(&m).expect("appending must not invalidate");
        assert_eq!(got.chunks.len(), want.chunks.len());
        assert_eq!(got.schema_hash, want.schema_hash);
        assert_eq!(got.entry_point, want.entry_point);
        for (g, w) in got.chunks.iter().zip(&want.chunks) {
            assert_eq!(g.name, w.name);
            assert_eq!(g.constants.len(), w.constants.len());
            assert_eq!(g.debug_pool_bytes, w.debug_pool_bytes);
        }
    }
}
