//! The `wire.kel` shared-slot map, checked against the block it describes.
//!
//! # Why a derivation and not a table
//!
//! `keleusma::selfhost::wire_slots` is a set of byte and word offsets into
//! `wire.kel`'s `shared data wire` block. Nothing in Rust connects them to that
//! block: they are arithmetic over field widths a human read off the stage
//! source, and the stage source is edited by a different kind of change than the
//! driver is.
//!
//! **The hazard is an INSERTED field rather than a wrong constant.** The block is
//! addressed by slot, so a field inserted rather than appended moves every field
//! after it, every constant here becomes wrong at once, and the result is a
//! WRONG artifact rather than a refused one — the driver seeds a value into what
//! it believes is `fin` and the stage reads it as `rcovers`.
//!
//! `wire.kel` says `APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT` in its own
//! comments, four separate times. This test is what makes that instruction
//! enforceable rather than advisory.
//!
//! # What "derive the set from the source" means here
//!
//! The offsets below are computed by reading the block's field declarations in
//! order and accumulating their widths, so the test cannot pass by agreeing with
//! a list someone typed twice. A vacuity guard asserts the parse found the fields
//! it needs, because a reader that silently matched nothing would report every
//! offset as zero and agree with nothing.

#![cfg(feature = "self-host")]

const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

/// Every `(name, width)` in `shared data wire`, in declaration order.
///
/// Width is the number of SLOTS the field occupies: one for a scalar, `N` for a
/// `[T; N]` regardless of `T`, because `set_shared` addresses declared slots and
/// a `[Byte; N]` field consumes `N` of them.
fn declared_fields() -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in WIRE.lines() {
        let t = line.trim();
        if !inside {
            if t.starts_with("shared data wire") {
                inside = true;
            }
            continue;
        }
        if t == "}" {
            break;
        }
        if t.starts_with("//") || t.is_empty() {
            continue;
        }
        let Some((name, ty)) = t.trim_end_matches(',').split_once(':') else {
            continue;
        };
        let (name, ty) = (name.trim(), ty.trim());
        let width = if let Some(rest) = ty.strip_prefix('[') {
            rest.rsplit_once(';')
                .and_then(|(_, n)| n.trim_end_matches(']').trim().parse::<usize>().ok())
                .unwrap_or_else(|| panic!("array field `{name}` has no parsable length: {ty}"))
        } else {
            1
        };
        out.push((name.to_string(), width));
    }
    out
}

/// The slot index the named field starts at, by accumulation.
fn offset_of(fields: &[(String, usize)], want: &str) -> usize {
    let mut at = 0usize;
    for (name, width) in fields {
        if name == want {
            return at;
        }
        at += width;
    }
    panic!("`shared data wire` has no field named `{want}`");
}

/// **EVERY CONSTANT IS THE ACCUMULATED WIDTH OF THE FIELDS ABOVE IT.**
///
/// Both halves matter. The equalities catch a constant that was mistyped; the
/// vacuity guard catches a reader that stopped matching, which would make every
/// equality hold against zero.
#[test]
fn the_slot_map_is_the_block_it_describes() {
    use keleusma::selfhost::wire_slots as w;

    let fields = declared_fields();

    // VACUITY. A reader that matched nothing, or that stopped at the first
    // comment, would produce a short list and agree with itself.
    assert!(
        fields.len() >= 20,
        "only {} fields parsed out of `shared data wire`; the reader is broken and every \
         assertion below would compare zero against zero",
        fields.len()
    );
    assert!(
        fields.iter().any(|(n, _)| n == "bin"),
        "the reader never reached `bin`, which is the last field any constant here names"
    );

    assert_eq!(offset_of(&fields, "len"), w::LEN, "`len`");
    assert_eq!(offset_of(&fields, "bytes"), w::BYTES, "`bytes`");
    assert_eq!(offset_of(&fields, "nregions"), w::NREGIONS, "`nregions`");
    assert_eq!(
        offset_of(&fields, "rkind"),
        w::REGION_INPUTS,
        "`rkind`, the first of the four per-region input arrays"
    );
    assert_eq!(offset_of(&fields, "warg"), w::WARG, "`warg`");
    assert_eq!(offset_of(&fields, "fin"), w::FIN, "`fin`");
    assert_eq!(offset_of(&fields, "bin"), w::BIN, "`bin`");
}

/// **THE FOUR PER-REGION ARRAYS ARE CONTIGUOUS AND THE FIVE `warg` SLOTS FOLLOW
/// THEM.**
///
/// `WARG` is derived as `REGION_INPUTS + 1024 * 4`, which is only correct while
/// `rkind`, `rflags`, `rlen` and `rcovers` are adjacent and equally sized, and
/// `FIN` as `WARG + 5`, which is only correct while there are exactly five
/// argument slots. Neither assumption is visible at the constant, so a field
/// added between them would satisfy the test above — every offset would still be
/// an accumulation — while making the arithmetic in `wire_slots` a coincidence.
#[test]
fn the_derivation_the_constants_assume_still_holds() {
    let fields = declared_fields();
    let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();

    let start = names
        .iter()
        .position(|n| *n == "rkind")
        .expect("`rkind` is declared");
    assert_eq!(
        &names[start..start + 4],
        &["rkind", "rflags", "rlen", "rcovers"],
        "the four per-region arrays are no longer adjacent, so `WARG`'s `1024 * 4` is a \
         coincidence rather than a derivation"
    );
    for (name, width) in &fields[start..start + 4] {
        assert_eq!(*width, 1024, "per-region array `{name}` is not 1024 wide");
    }

    let wstart = names
        .iter()
        .position(|n| *n == "warg")
        .expect("`warg` is declared");
    let args: Vec<&str> = names[wstart..]
        .iter()
        .take_while(|n| n.starts_with("warg"))
        .copied()
        .collect();
    assert_eq!(
        args,
        vec!["warg", "warg2", "warg3", "warg4", "warg5"],
        "the general-argument run is no longer five slots, so `FIN = WARG + 5` is wrong"
    );
}
