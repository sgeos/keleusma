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
//! Only the roots named below. A figure derived from anything else — the
//! instruction set, the reference compiler's behaviour, the arena — is outside
//! this guard, and describing it as protecting those would be the same overclaim
//! this file exists to prevent.
//!
//! **It covers what the loaders READ, not only what compiles.** Several of these
//! files are skipped because they are not standalone programs; a change that made
//! one compile would move every figure, so watching only the compiling subset
//! would miss exactly the change most worth catching.

use std::collections::BTreeMap;

/// The corpus roots this line's figures are derived from, relative to
/// `native_codegen/`.
///
/// # ⚠ DERIVED FROM WHAT THE CONSUMERS READ, AFTER GETTING IT WRONG
///
/// The first version listed three roots. **The censuses that produce the
/// published figures read four** — `spike_corpus_coverage`, `isa_lowering_census`
/// and `bound_transfer` all include `examples/rtos/scripts` and `compiler/kel`,
/// seven further files that were read and unguarded.
///
/// **That was the third occurrence of one defect at three granularities**: the
/// `v0.2.3` line pinned a value whose input was a directory scan; this line then
/// scanned three named directories where the loaders recurse, finding 57 files
/// against 67; and then this guard, built from that lesson, covered three roots
/// where the consumers read four. **Each time the watched population was
/// narrower than the one that mattered, and each time the narrow scan returned a
/// well-formed answer**, which is what made it invisible.
///
/// `examples/scripts/rogue` is deliberately absent: it is reached by recursion
/// from its parent, and listing it as well is what made three other tests count
/// that directory twice.
const CORPUS_DIRS: [&str; 6] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
    "../src/hosted",
    "../src/crypto_frame",
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
// **RE-PINNED 2026-09-11, absorption 57**, and the movement is stated rather
// than absorbed. Two modules changed content: `src/selfhost/kel/wire.kel` and
// `src/selfhost/kel/verify_types.kel`, both corpus subjects of this backend. The
// population is unchanged at 74 entries -- nothing added, nothing removed, which
// is what the diff reported and what an entry count confirms.
//
// **The chunk population moved +11, measured DIRECTLY** by compiling both
// versions of each file: `wire.kel` 486 -> 492, `verify_types.kel` 28 -> 33.
//
// **Every derived figure was re-run and none moved because of it**: the refusal
// set is still 1, the ISA census still 63 of 66 over 74 compiled modules,
// module-level coverage 98.6%, and the interprocedural and yield-escape suites
// unchanged. Current totals: 1084 of 1085 chunks fully lowerable (99.9%) and
// 90800 of 90845 opcode instances.
//
// ⚠ **THE 1070-of-1074 FIGURE ELSEWHERE IN THIS TREE IS A DATED CALIBRATION
// (2026-08-29), NOT A MEASUREMENT OF THE PRE-ABSORPTION TREE.** Adding the
// measured +11 to it does not reproduce the current 1084 lowerable, and the
// residual is NOT ATTRIBUTED HERE: `14_frame_log.kel` becoming lowerable earlier
// the same day accounts for two of its chunks, and the rest was not measured.
// Stating an arithmetic that does not close as though it did is the failure this
// note exists to avoid.
//
// **RE-PINNED 2026-10-24, absorption 88.** Upstream's Order 3 changed four stage
// sources, `analyze.kel`, `codegen.kel`, `verify_structural.kel` and
// `verify_typed.kel`. Nothing was added or removed. The censuses this file names
// were re-run on the absorbed tree and passed with their pinned figures, except
// the bounds-transfer inversion count, whose cause is recorded where it is pinned.
//
// **RE-PINNED 2026-10-25, absorption 92.** Upstream pull request 519 changed
// `analyze.kel` to mirror the verifier's new charging of structural markers in
// its time bound. Nothing was added or removed. Every other test in the same
// default-feature run passed, including the five censuses this file names.
//
// **RE-PINNED 2026-10-26, absorptions 95 and 96.** Upstream's Order 6 slices
// changed `codegen.kel`, `lexer.kel`, `parse.kel`, `reconstruct.kel` and
// `wire.kel` in both. Nothing was added or removed, and the six witness programs
// moved to `examples/scripts/native_witnesses/` with their contents unchanged.
// Every other test in the same default-feature run passed, including the five
// censuses this file names, except in absorption 95 the bounds-transfer
// inversion count, whose cause is recorded where it is pinned.
//
// **RE-PINNED 2026-10-26, absorption 97.** Upstream pull request 544, Order 6
// slices 5 to 5d, changed `codegen.kel`, `lexer.kel`, `parse.kel` and
// `reconstruct.kel`. Nothing was added or removed. Upstream also adopted this
// line's typed `use` declaration in `rogue_dungen.kel`, whose digest is
// unchanged because the two files are now identical.
//
// **RE-PINNED 2026-10-26, absorption 98.** Upstream pull requests 545 and 546,
// Order 6 slices 6 to 8, changed `analyze.kel` and `parse.kel`. The
// `analyze.kel` change makes the stage charge an enum match as the reference
// verifier does, and the reference is unchanged. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 99.** Upstream pull requests 548 and 549
// changed `codegen.kel` and `parse.kel`, to lower a break in a bare `for` loop
// and wildcard payload patterns. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 100.** Upstream pull request 551, variable
// arms on a scalar match, changed `parse.kel`. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 101.** Upstream pull request 553, tuple
// and struct destructuring arms, changed `parse.kel`. Nothing was added or
// removed.
//
// **RE-PINNED 2026-10-26, absorption 103.** Upstream pull request 556, the
// gate 3 and 4 instruments and a validator fall-through fix, changed
// `verify_depth.kel`. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 104.** Upstream pull requests 559 and 560 raised
// the self-hosted stages' capacities and changed eight stage sources,
// `analyze.kel`, `codegen.kel`, `parse.kel`, `reconstruct.kel`,
// `verify_depth.kel`, `verify_structural.kel`, `verify_typed.kel` and
// `verify_yield.kel`. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 105.** Upstream pull requests 562 and 563,
// the parser's local bindings and untyped native results, changed `parse.kel`.
// Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 106.** Upstream pull request 564, which
// compiles `assert` statements out as the reference does, changed
// `codegen.kel`, `parse.kel` and `reconstruct.kel`. Nothing was added or
// removed.
//
// **RE-PINNED 2026-10-26, absorption 107.** Upstream pull requests 566 and 568,
// the pipe placeholder and builtin generic signature layouts, changed
// `parse.kel` and `reconstruct.kel`. Nothing was added or removed.
//
// **RE-PINNED 2026-10-26, absorption 108.** Upstream pull request 570, a match
// over an `Option` parameter, changed `parse.kel`. Nothing was added or
// removed.
//
// **RE-PINNED 2026-10-26, absorption 109.** Upstream pull requests 572 and 573,
// `Option` construction and `Option` scrutinees, changed `parse.kel`. Nothing
// was added or removed.
//
// **RE-PINNED 2026-10-27, absorption 110.** Upstream pull request 576, generic
// functions instantiated in the parser stage, changed `parse.kel`. Nothing was
// added or removed.
//
// **RE-PINNED 2026-10-27, absorption 112.** Upstream pull request 581, which
// erases information-flow labels and newtypes as the reference does, changed
// `codegen.kel`, `lexer.kel`, `parse.kel` and `reconstruct.kel`. Nothing was
// added or removed.
const PINNED: &[(&str, u64)] = &[
    // **Re-pinned at absorption 60.** `src/selfhost/kel/parse.kel` moved; nothing was
    // added or removed. Every census this instrument names to re-derive was run on
    // the absorbed tree and **held**: the corpus still builds 74 modules with 1
    // refused, and the backend still lowers 63 of 66. The pin follows the
    // re-derivation rather than standing in for it.
    // **Re-pinned at absorption 82.** Upstream added `wire_debug.kel` and
    // `wire_schema.kel` and modified twelve stage sources. Every census this
    // instrument names was re-derived on the absorbed tree: the corpus builds 76
    // modules with 1 refused, and the denominator records 61 of 66 lowered with
    // corpus evidence. `Div` and `Mod` still lower but lost their corpus feeder,
    // because Word and Byte division now compile to the checked forms.
    // **Widened on 2026-10-28, question 6.** The operator ruled that
    // `src/hosted/interp.kel` and `src/crypto_frame/frame.kel` join the native
    // corpus, so `src/hosted` and `src/crypto_frame` became roots and nothing else
    // moved. The corpus builds 78 modules. Each pin that moved was re-derived in
    // the same change and names this ruling.
    ("compiler/kel/prelude.kel", 0x07c8e691e07a309e),
    (
        "examples/rtos/scripts/event_listener.kel",
        0xcdf3885366c16d9c,
    ),
    ("examples/rtos/scripts/faulty.kel", 0x7e4a2f5eb8bd4428),
    ("examples/rtos/scripts/heartbeat.kel", 0x0432d77ccc9ead65),
    ("examples/rtos/scripts/led.kel", 0xec5852a463777501),
    ("examples/rtos/scripts/prelude.kel", 0xb11ce5cf74544397),
    ("examples/rtos/scripts/sensor.kel", 0xf4e298fe169921d2),
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
        "examples/scripts/native_witnesses/external_native_witness.kel",
        0x082cb18d1090211b,
    ),
    (
        "examples/scripts/native_witnesses/fixed_arithmetic.kel",
        0x81978de44e2a4fd6,
    ),
    (
        "examples/scripts/native_witnesses/fixed_conversions.kel",
        0x628add26f68053a8,
    ),
    (
        "examples/scripts/native_witnesses/float_witness.kel",
        0x492f9b0110f02245,
    ),
    (
        "examples/scripts/native_witnesses/opcode_witness.kel",
        0xeda34a70e0546930,
    ),
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
    (
        "examples/scripts/native_witnesses/refused_witness.kel",
        0xbe747a6ae1c8ea22,
    ),
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
    ("src/crypto_frame/frame.kel", 0x93ac54a86007619f),
    ("src/hosted/interp.kel", 0x0b6e4007b12016f5),
    ("src/selfhost/kel/analyze.kel", 0xe45a770e222b1931),
    ("src/selfhost/kel/codegen.kel", 0xecd27ec27b225862),
    ("src/selfhost/kel/lexer.kel", 0x897a561eef4f18d1),
    // Absorption 83 changes cast Byte tracking and FixedDiv classification.
    // The non-corpus run re-derived coverage and refusal figures unchanged.
    ("src/selfhost/kel/parse.kel", 0xb9638e3dd3491b1e),
    ("src/selfhost/kel/reconstruct.kel", 0x5a5314dbf0987c1c),
    ("src/selfhost/kel/verify_datalayout.kel", 0xe625b5656d576b86),
    ("src/selfhost/kel/verify_depth.kel", 0xd65a1422ab29b135),
    ("src/selfhost/kel/verify_structural.kel", 0xe47e628d4f3d9641),
    ("src/selfhost/kel/verify_typed.kel", 0xdf6274221b46406b),
    ("src/selfhost/kel/verify_types.kel", 0xf1a3565245726860),
    ("src/selfhost/kel/verify_yield.kel", 0x12e9047be52a0423),
    ("src/selfhost/kel/wire.kel", 0x7e021df3e15f1793),
    ("src/selfhost/kel/wire_debug.kel", 0xa98ddf75d7e9f7e8),
    ("src/selfhost/kel/wire_schema.kel", 0x6f32806acb4b4763),
];

/// **WHY TWO CENSUSES OVER DIFFERENT POPULATIONS REPORT THE SAME TOTAL.**
///
/// `spike_corpus_coverage` reads four roots; `interproc_yield_escape` reads the
/// three under `examples/` and `src/`. Both report **1074 chunks**, and an
/// earlier increment recorded that agreement as "a cross-check that had not
/// existed". **That was overstated**: two measurements over different populations
/// agreeing is not corroboration until the difference is shown to contribute
/// nothing.
///
/// # The first explanation offered here was also wrong
///
/// It said the extra files are "not standalone programs, so they do not
/// compile". **Two of the seven do compile**, both named `prelude.kel`. The test
/// asserting otherwise failed, which is the only reason this paragraph is right
/// rather than plausible.
///
/// **The actual reason is that they compile to ZERO CHUNKS.** A prelude declares;
/// it has no function bodies to emit. So they enter the module list and add
/// nothing to any chunk total, and the two censuses agree because the extra roots
/// contribute no chunks — **not** because they contribute no modules, and **not**
/// because the populations coincide.
///
/// **The module counts do differ**, and any figure quoted as "modules" from a
/// four-root census is two higher than one from a three-root census. That is a
/// real difference and it is stated rather than smoothed over.
#[test]
fn the_extra_roots_add_modules_but_no_chunks() {
    use keleusma::compiler::compile;
    use keleusma::lexer::tokenize;
    use keleusma::parser::parse;

    let extra = ["../examples/rtos/scripts", "../compiler/kel"];
    let mut read = 0usize;
    let mut compiled: Vec<(String, usize)> = Vec::new();
    for d in extra {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if !p.extension().is_some_and(|x| x == "kel") {
                continue;
            }
            read += 1;
            let Ok(src) = std::fs::read_to_string(&p) else {
                continue;
            };
            if let Some(m) = tokenize(&src)
                .ok()
                .and_then(|t| parse(&t).ok())
                .and_then(|a| compile(&a).ok())
            {
                compiled.push((
                    format!(
                        "{}/{}",
                        d.trim_start_matches("../"),
                        p.file_name().unwrap().to_string_lossy()
                    ),
                    m.chunks.len(),
                ));
            }
        }
    }

    println!("\n================ WHAT THE EXTRA ROOTS CONTRIBUTE");
    println!("  files read only by the four-root loaders : {read}");
    println!(
        "  of those, compiling standalone           : {}",
        compiled.len()
    );
    for (n, c) in &compiled {
        println!("    {n} -> {c} chunk(s)");
    }
    println!(
        "\n  So a four-root census sees {} MORE MODULES than a three-root one and\n  \
         the SAME number of chunks. That is why both report 1074, and it is a\n  \
         weaker fact than the corroboration an earlier increment claimed.\n================\n",
        compiled.len()
    );

    assert!(
        read > 0,
        "the extra roots hold no files at all, so this explains nothing"
    );
    assert!(
        !compiled.is_empty(),
        "nothing in the extra roots compiles, which would be a different \
         explanation from the one recorded here"
    );
    let total_chunks: usize = compiled.iter().map(|(_, c)| *c).sum();
    assert_eq!(
        total_chunks, 0,
        "the extra roots now contribute {total_chunks} chunk(s), so a four-root \
         and a three-root census can no longer report the same chunk total and \
         every such figure needs re-deriving with its population named"
    );
}
