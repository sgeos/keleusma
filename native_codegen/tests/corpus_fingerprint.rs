//! The shared corpus announces its own change.
//!
//! # The exposure this closes
//!
//! **Thirty-six test files on this line read `src/selfhost/kel/` and
//! `examples/scripts/`**, and those directories are owned by the `v0.2.3` line.
//! Every corpus-derived figure here — coverage, refusal counts, the
//! interprocedural residual, the yield-escape cost — rests on inputs another
//! line commits to.
//!
//! The rule, from the `v0.2.3` line after a pin of theirs failed on this branch:
//! **before pinning a value, ask what the widest input to it is and whether that
//! input is pinned too.** An invariant protects a REGION; it was never going to
//! protect an expectation whose widest input lay outside one.
//!
//! It has never bitten here, because every absorption asks "corpus inputs
//! touched?" before predicting anything — twenty-six times, by hand. **A habit is
//! not a check.** It holds exactly as long as whoever performs it remembers why,
//! and this line has demonstrably forgotten recorded rules before, including one
//! this repository had already written down.
//!
//! # What this does NOT cover, said plainly
//!
//! Only the two directories named below. A figure derived from anything else —
//! the instruction set, the reference compiler's behaviour, the arena — is
//! outside this guard, and describing it as protecting those would be the same
//! overclaim this file exists to prevent.

use std::collections::BTreeMap;

/// The corpus roots this line's figures are derived from, relative to
/// `native_codegen/`.
const CORPUS_DIRS: [&str; 3] = [
    "../examples/scripts",
    "../examples/scripts/rogue",
    "../src/selfhost/kel",
];

/// FNV-1a, 64-bit.
///
/// **Chosen because its output is fixed by its own definition.**
/// `DefaultHasher` is explicitly not stable across toolchain versions, so a pin
/// built on it would move when Rust updates — a tripwire that cries wolf, and a
/// tripwire nobody trusts is worse than none.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Every corpus file, by name, with a digest of its contents.
fn manifest() -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    // **RECURSIVE, BECAUSE THE CORPUS LOADERS ARE.** They walk a stack and push
    // any directory they meet, so `examples/scripts/piano_roll/` is in their
    // population. A first version of this scan read only the three named
    // directories and found 57 files where the loaders see far more — a guard
    // that would have under-covered exactly the inputs it exists to watch.
    let mut stack: Vec<std::path::PathBuf> =
        CORPUS_DIRS.iter().map(std::path::PathBuf::from).collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(bytes) = std::fs::read(&p)
        {
            // Keyed by path from the corpus root, not by bare file name: two
            // directories could hold the same name and a bare key would let one
            // silently replace the other.
            let key = p.to_string_lossy().replace("../", "");
            out.insert(key, fnv1a(&bytes));
        }
    }
    out
}

/// How two manifests differ, in the three ways that matter.
///
/// **Identity as well as content.** A file added and another removed could leave
/// an aggregate digest unchanged in principle; comparing by name makes an
/// identity change visible on its own.
fn diff(
    pinned: &BTreeMap<String, u64>,
    current: &BTreeMap<String, u64>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let added = current
        .keys()
        .filter(|k| !pinned.contains_key(*k))
        .cloned()
        .collect();
    let removed = pinned
        .keys()
        .filter(|k| !current.contains_key(*k))
        .cloned()
        .collect();
    let modified = current
        .iter()
        .filter(|(k, v)| pinned.get(*k).is_some_and(|p| p != *v))
        .map(|(k, _)| k.clone())
        .collect();
    (added, removed, modified)
}

/// **THE COMPARATOR IS SHOWN TO REPORT EACH OF ITS THREE ANSWERS**, without
/// touching the corpus.
///
/// A guard whose only evidence is that it passed on unchanged input is
/// indistinguishable from one that cannot fail. This perturbs a manifest
/// directly, so the demonstration does not depend on editing a file another line
/// owns.
#[test]
fn the_comparator_reports_addition_removal_and_modification() {
    let base: BTreeMap<String, u64> =
        [("a.kel".to_string(), 1u64), ("b.kel".to_string(), 2)].into();

    let unchanged = diff(&base, &base);
    assert_eq!(
        unchanged,
        (vec![], vec![], vec![]),
        "identical manifests must differ in no way; without this the positives \
         below would be satisfied by a comparator that always reports something"
    );

    let mut added = base.clone();
    added.insert("c.kel".into(), 3);
    assert_eq!(diff(&base, &added).0, vec!["c.kel".to_string()]);

    let mut removed = base.clone();
    removed.remove("b.kel");
    assert_eq!(diff(&base, &removed).1, vec!["b.kel".to_string()]);

    let mut modified = base.clone();
    modified.insert("a.kel".into(), 99);
    assert_eq!(diff(&base, &modified).2, vec!["a.kel".to_string()]);
}

/// The corpus is what every corpus-derived figure on this line was measured
/// against.
///
/// **The expectation is a constant in this file, not a recomputation of the same
/// scan.** A fingerprint test that derived its expectation from the directory it
/// is checking would pass unconditionally — the defeater sitting beside the
/// guard, which is the failure mode the `v0.2.3` line named.
#[test]
fn the_corpus_is_what_the_pinned_figures_were_measured_against() {
    let current = manifest();
    assert!(
        current.len() > 50,
        "only {} corpus files found; a fingerprint over a corpus that failed to \
         load would report no change for the wrong reason",
        current.len()
    );

    let pinned: BTreeMap<String, u64> =
        PINNED.iter().map(|(n, h)| ((*n).to_string(), *h)).collect();
    let (added, removed, modified) = diff(&pinned, &current);

    if !(added.is_empty() && removed.is_empty() && modified.is_empty()) {
        println!("\n================ THE SHARED CORPUS MOVED");
        println!("  added    : {added:?}");
        println!("  removed  : {removed:?}");
        println!("  modified : {modified:?}");
        println!(
            "\n  EVERY CORPUS-DERIVED FIGURE ON THIS LINE IS NOW A PREDICTION RATHER\n  \
             THAN A FACT. Re-derive at least: corpus coverage and opcode instances\n  \
             (`spike_corpus_coverage`), the refusal set (`remaining_refusals`), the\n  \
             interprocedural residual (`interproc_yield_escape`), the yield-escape\n  \
             cost (`yield_escape_gate`), and the lowering census\n  \
             (`isa_lowering_census`).\n"
        );
        println!("  The current manifest, ready to replace PINNED:\n");
        for (n, h) in &current {
            println!("    (\"{n}\", 0x{h:016x}),");
        }
        println!("================\n");
    }

    assert_eq!(
        (added.len(), removed.len(), modified.len()),
        (0, 0, 0),
        "the shared corpus changed; see the report above for what moved and what \
         to re-derive"
    );
}

/// Name and content digest of every corpus file, as measured when the figures
/// recorded in `docs/process/handoffs/v0.3.0.md` were last re-derived.
const PINNED: &[(&str, u64)] = &[
    ("examples/scripts/01_arithmetic.kel", 0x1492d4e670de0e64),
    ("examples/scripts/02_struct_field.kel", 0x5228c096fbca5168),
    ("examples/scripts/03_enum_match.kel", 0x9e40886c7db29964),
    ("examples/scripts/04_for_in.kel", 0xbaf726a497156a23),
    ("examples/scripts/05_pipeline.kel", 0xf3afbb7b0c0c4810),
    ("examples/scripts/06_multiheaded.kel", 0x8033997a29c0e8bc),
    ("examples/scripts/07_refinement.kel", 0xaced12939813dbc8),
    (
        "examples/scripts/08_method_dispatch.kel",
        0x83925ee2a156babf,
    ),
    ("examples/scripts/09_big_numbers.kel", 0xb5976e67c9ce02d6),
    ("examples/scripts/10_multbyte.kel", 0xe1010bb421444c6a),
    ("examples/scripts/11_signed.kel", 0x896923a0d689dc38),
    ("examples/scripts/12_sensor_window.kel", 0x5b3d471b19de0d57),
    (
        "examples/scripts/13_telemetry_stream.kel",
        0x84877e10d1f23c29,
    ),
    ("examples/scripts/14_frame_log.kel", 0x295b974b908054b5),
    ("examples/scripts/15_pixel_blend.kel", 0xdc03ec3362c45a3f),
    (
        "examples/scripts/external_native_witness.kel",
        0xc2dcdcb0d176b3d6,
    ),
    ("examples/scripts/fixed_arithmetic.kel", 0x81978de44e2a4fd6),
    ("examples/scripts/fixed_conversions.kel", 0x628add26f68053a8),
    ("examples/scripts/float_witness.kel", 0x492f9b0110f02245),
    ("examples/scripts/opcode_witness.kel", 0xeda34a70e0546930),
    (
        "examples/scripts/piano_roll/piano_roll_0.kel",
        0x0165242923cee8dd,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_1.kel",
        0xfaa13b5754d65b01,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_2.kel",
        0x1db70632cc31f5f8,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_3.kel",
        0x8087ff83f19468a0,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_4.kel",
        0x3c220b104471c904,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_5.kel",
        0x085be6924ce91825,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_6.kel",
        0x9d85c304d2742910,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_7.kel",
        0xeb52ebf7ad0bad04,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_8.kel",
        0x6b35710a8acbe3da,
    ),
    (
        "examples/scripts/piano_roll/piano_roll_9.kel",
        0x6f0eda1f8db18364,
    ),
    ("examples/scripts/refused_witness.kel", 0x44716aedbe5f189b),
    (
        "examples/scripts/rogue/rogue_ai_boss.kel",
        0x51c63edcd748eebf,
    ),
    (
        "examples/scripts/rogue/rogue_ai_chaser.kel",
        0xc884728391915239,
    ),
    (
        "examples/scripts/rogue/rogue_ai_fast.kel",
        0x8c4181579abf3f26,
    ),
    (
        "examples/scripts/rogue/rogue_ai_hunter.kel",
        0x84ae6e10da6873b2,
    ),
    (
        "examples/scripts/rogue/rogue_ai_idle.kel",
        0x7f9c7ce702e8f357,
    ),
    (
        "examples/scripts/rogue/rogue_ai_ranged.kel",
        0xe72577b99289112e,
    ),
    (
        "examples/scripts/rogue/rogue_ai_sleeper.kel",
        0x254034c6c7fb6e1d,
    ),
    (
        "examples/scripts/rogue/rogue_ai_smart.kel",
        0xdd5f94a8dd41f9e1,
    ),
    (
        "examples/scripts/rogue/rogue_ai_tracker.kel",
        0x8255776014fab509,
    ),
    (
        "examples/scripts/rogue/rogue_ai_wander.kel",
        0xa23914f4a409f545,
    ),
    (
        "examples/scripts/rogue/rogue_bestiary.kel",
        0xea2d9524fccb95ab,
    ),
    (
        "examples/scripts/rogue/rogue_book_keeping.kel",
        0x73ecb687dc7ce7b8,
    ),
    (
        "examples/scripts/rogue/rogue_combat.kel",
        0xa1e76aed1c3d98e6,
    ),
    (
        "examples/scripts/rogue/rogue_consume.kel",
        0x1d1799c7d5ccb8ca,
    ),
    (
        "examples/scripts/rogue/rogue_descend.kel",
        0x815a5e935be55f15,
    ),
    (
        "examples/scripts/rogue/rogue_dungen.kel",
        0xd8521db01b3089cc,
    ),
    ("examples/scripts/rogue/rogue_game.kel", 0xc98e07ad3f8e6df0),
    ("examples/scripts/rogue/rogue_gear.kel", 0xf512e22dbf290af4),
    (
        "examples/scripts/rogue/rogue_item_potion.kel",
        0x4c0bb8aebf4446af,
    ),
    (
        "examples/scripts/rogue/rogue_item_scroll.kel",
        0x3f1f2ec33e8f4db7,
    ),
    (
        "examples/scripts/rogue/rogue_move_resolve.kel",
        0x6097d56785db626b,
    ),
    (
        "examples/scripts/rogue/rogue_pickup.kel",
        0x3d817b59119bef1a,
    ),
    (
        "examples/scripts/rogue/rogue_player_ai.kel",
        0x9e41a15e05d51258,
    ),
    (
        "examples/scripts/rogue/rogue_scroll_apply.kel",
        0xf93a5bd822bfe99c,
    ),
    ("src/selfhost/kel/analyze.kel", 0xca66a2229816243c),
    ("src/selfhost/kel/codegen.kel", 0x696808a6593fe4f9),
    ("src/selfhost/kel/lexer.kel", 0xec5f9dd44b6ba8f8),
    ("src/selfhost/kel/parse.kel", 0xa5773b6b1ede313d),
    ("src/selfhost/kel/reconstruct.kel", 0x137bbac9b6ad2ede),
    ("src/selfhost/kel/verify_datalayout.kel", 0xf30296eb4c09fc1a),
    ("src/selfhost/kel/verify_depth.kel", 0x6a264c7734ccfc1a),
    ("src/selfhost/kel/verify_structural.kel", 0x9dd4124de42674e9),
    ("src/selfhost/kel/verify_typed.kel", 0x63c5af32b11a4d85),
    ("src/selfhost/kel/verify_types.kel", 0xb069ffd53ceaf767),
    ("src/selfhost/kel/verify_yield.kel", 0x1c55b51d5809ae0f),
    ("src/selfhost/kel/wire.kel", 0x80f6fe6dc89c9112),
];
