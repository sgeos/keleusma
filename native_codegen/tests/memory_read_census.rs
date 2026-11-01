//! The copy at the yield of operator question 24, 2026-10-31, reads one word,
//! its cursor, at a constant offset in the entry's region reservation. The
//! entry writes it at `Op::Stream`, before any yield can run, and again at each
//! `Op::Reset`, so every read follows a write of this instance.
//!
//! Order 6 text equality reads four words of two string literal records. Each
//! record is a constant global `{ i64 len, bytes }` that `static_string_global`
//! initialises, and its operand kind is `Text` only when a string constant
//! produced it. The two length reads read the records' first words. The two
//! byte reads lie at offset eight plus an index the loop bounds by the length,
//! so they stay inside a record whose lengths were just found equal.
//!
//! The Order 4 instruction meter, off by default, reads one word per lowered
//! instruction from the host's meter array before adding one to it. The host
//! defines the array and zeroes it, as `LowerOptions::op_meter` documents.
//! A host that did not zero it would only offset its own counts.
//!
//! Coroutine trap reporting, Order 4, reads three words. Each function's trap
//! block loads its own trap-code cell, which its prologue initialises to the
//! contract code before any branch and which each `Trap` overwrites
//! immediately before branching. The entry and a callee each have one such
//! load. The stable handles load the frame's trap record after every step.
//! Start and the entry prologue both clear it, and only a trap writes it, so
//! it is zero unless the step trapped. A caller reads a callee's trap word
//! through the ownership load helper, after clearing it before the call.
//!
//! The ten-field delegate context is initialized before use. Fields five and
//! six point to entry kind and size. Seven through nine point to latest-reply
//! ownership, kind and size. Start initializes all latest metadata and every
//! direct or delegated yield updates it before Reset restores local zero.
//! The common ownership load helper serves all four Reset metadata reads.
//! Generalized builder loads replace the separate flat ownership destination.
//!
//! Coroutine private scalar kind reads use a zeroed persistent metadata word.
//! Zero selects a compiler-emitted immutable table of load-time kinds. Writes
//! store actual nonzero kinds. The table and metadata have one word per private
//! slot, and the published private image initializes each scalar payload.
//!
//! Internal call metadata is written by the caller before callee reads, and
//! by the callee before result reads. Each of its three arrays has arg_count+1
//! words. Private body metadata retains actual length and kind. Zero denotes
//! unwritten Unit on the coroutine route. Reading Unit itself does not fault.
//! The private scalar word is initialized by the published private image.
//!
//! The yield-site query reads initialized metadata only while the handle is
//! suspended. The fifth delegate-context pointer names that metadata cell.
//! Coroutine kind and size metadata is seeded by producers and transported
//! by local and stack copies. Private body tags share the initialization word.
//! Enum discriminants are read only after the runtime tag equals Enum.
//!
//! Flat-input ownership flags are initialized before local or operand reads.
//! Call contexts hold one flag per argument plus an initialized return flag.
//! The delegate context now has a fourth pointer for entry ownership. Its new
//! destination reads use fields initialized by start before any suspension.
//!
//! **EVERY PLACE THE EMITTER READS MEMORY IT DID NOT WRITE, AND WHAT GUARANTEES
//! THE CONTENTS.**
//!
//! # The third axis, and the one that was missing
//!
//! `pointer_offset_census.rs` covers how an address is FORMED.
//! `value_movement_census.rs` covers how a value is MOVED to a destination.
//! **Neither asks what is in memory before the emitter reads it**, and two
//! defects in one day sat on exactly that:
//!
//! - a composite data slot read before it was written answered `0`, because the
//!   persistent pool is bytes and bytes have no `Unit`;
//! - a private scalar slot with a declared `= 7` read as `0`, because the
//!   runtime applies `private_init` at load and **there is no native load step**.
//!
//! The second was found by writing this census's question down and asking it of
//! all sixteen read sites, rather than by another accident.
//!
//! | read | what guarantees the contents |
//! |---|---|
//! | stable handle continuation | start writes it before use, resume replaces it, and release clears it; the slot remains caller-owned until release |
//! | operand slot (`peek`, `pop`) | a push earlier in the same call |
//! | local slot (`GetLocal`) | zeroed on first entry, cleared at `Op::Reset`; the runtime's `Unit` is this backend's zero |
//! | operand spill slot | written at the `yield` that suspended, read only by that yield's resume block |
//! | composite field and array element | the constructor that built the body, which is the only producer of a pointer to it |
//! | enum discriminant | the same |
//! | **stream resume-state word** | **the host's zeroed persistent buffer.** Zero means "the loop top", which is the correct meaning for a fresh instance |
//! | **composite initialisation word** | **the host, installing `region::private_init_image`.** Since absorption 124 a composite slot starts with its type's zero, so the image writes its body and sets the word. Zero still means "never written" for a slot initialized to `Unit`, and a read of such a slot FAULTS |
//! | **private slot array** | **the host, installing `region::private_init_image`.** This is the guarantee that did not exist |
//! | retcon reply cell | the host writes scalar bits or a body pointer before each resume, as specified by `coroutine::lower`; exercised by `retcon_bytecode.rs` |
//! | retcon delegate context | five pointer fields initialised by the entry before any call, promoted into the frame by LLVM; the helper reads the host reply cell under the same resume contract |
//! | retcon latest reply | initialised from the start argument and updated after each suspension; LLVM preserves it in the frame until release |
//! | **delegated loop context** | **the inherited delegate context, read field by field when a delegated `loop` builds its own copy at entry and when its `Reset` reads the latest-reply pointers back.** Every field it reads was written by the entry's `delegate_context` before the first call, so each read has an initialised source |
//! | **shared data segment** | **the host, by contract.** Out of this backend's reach and deliberately so |
//!
//! # Two of the four host-boundary rows are satisfied by a ZEROED buffer
//!
//! A host that zeroes the region satisfies the resume-state word and the shared
//! data it seeds itself, and is wrong about scalar initializers. Until
//! absorption 124 it also satisfied the initialisation flags and every
//! composite slot, which is why the scalar row once went unnoticed: **a
//! plausible host was right three times out of four**, which is exactly the
//! coverage pattern that lets a defect live. Since V0.2.X's widening gave every
//! composite private slot its type's zero, a zeroed buffer is wrong about those
//! too, and the image serves both rows.
//!
//! # What this census cannot do
//!
//! It counts read sites and records a guarantee for each. **It does not verify
//! that any guarantee holds** — the differential does that by execution, and the
//! private-slot row had a written guarantee that was false until the image was
//! published. What it refuses is a new read appearing with no answer to the
//! question.

mod common;

/// The forms by which the emitter reads memory.
const READ_FORMS: &[&str] = &["build_load(", " = load "];

/// Read sites in the emitter, at the stamp.
// 16 -> 18 with the retcon reply cell and latest-reply reads described above.
// 18 -> 22, including the coroutine context and parsed intrinsic helper.
// 22 -> 23 for the stable handle continuation, initialized by start.
// 33 -> 36 for the two trap-code loads and the handle's trap-record load.
// 36 -> 37 for the instruction meter's read of its own word.
// 37 -> 41 for the four text equality reads described above.
// 41 -> 42 for the copy at the yield's cursor, described above.
// 42 -> 44 in absorption 129 for the delegated loop context's two reads.
const RECORDED_READ_SITES: usize = 44;
// 16 at first derivation, 2026-09-11. Four are on the host-provided boundary —
// the resume-state word, the composite initialisation word, the private slot
// array and the shared segment — and the rest read memory this lowering wrote
// earlier in the same call.

fn read_sites() -> Vec<(&'static str, usize, String)> {
    let mut sites = Vec::new();
    for file in [
        "src/lib.rs",
        "src/coroutine.rs",
        "src/coroutine/host.rs",
        "src/coroutine/ownership.rs",
        "src/coroutine/kinds.rs",
        "src/coroutine/packing.rs",
        "src/coroutine/arithmetic.rs",
        "src/coroutine/private.rs",
    ] {
        let src = std::fs::read_to_string(file).expect("the emitter is readable");
        for (i, line) in src.lines().enumerate() {
            if READ_FORMS.iter().any(|form| line.contains(form)) {
                sites.push((file, i + 1, line.trim().to_string()));
            }
        }
    }
    sites
}

#[test]
fn every_memory_read_has_a_stated_guarantee() {
    let sites = read_sites();
    println!("\n================ MEMORY READS IN THE EMITTER");
    for (file, n, l) in &sites {
        println!("  {file}:{n}  {}", &l[..l.len().min(72)]);
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", sites.len());
    println!(
        "\n  Every read's guarantee is recorded in this file's header. FIVE are on\n  \
         the host-provided boundary, and a zeroed buffer satisfies two of\n  \
         the original four. Retcon additionally requires a reply before resume.\n================\n"
    );

    assert!(
        sites.len() > 8,
        "only {} read sites found; the matcher has gone stale and this census is \
         measuring nothing",
        sites.len()
    );
    assert_eq!(
        sites.len(),
        RECORDED_READ_SITES,
        "the number of memory reads in the emitter has changed. Do not patch the \
         number: record in this file's header what guarantees the new read's \
         contents. A read whose guarantee is the host's needs a published way for \
         the host to satisfy it — that is what the private slot array lacked."
    );
}

/// **THE HOST-BOUNDARY ROWS, DRIVEN.**
///
/// The header says two of four are satisfied by a zeroed buffer and two need
/// an installed image. That is a claim about this backend's published surface,
/// so it is checked against that surface rather than left as prose.
#[test]
fn the_host_boundary_rows_have_a_published_way_to_be_satisfied() {
    use keleusma_native::region;

    let m = common::build(
        "struct F { a: Word, b: Word }\n\
         private data log { latest: F, count: Word = 5 }\n\
         loop main(t: Word) -> Word { log.latest = F { a: t, b: t }; \
           let _a = yield log.latest.a + log.count; 0 }\n",
    );

    // The image exists and carries the declared literal, so the one row that a
    // zeroed buffer does NOT satisfy has a published remedy.
    let image = region::private_init_image(&m).expect("placeable");
    assert!(
        image.iter().any(|b| *b != 0),
        "the published image is all zeros for a module declaring `= 5`, so the row \
         that needs it is not actually served"
    );

    // Since absorption 124 the image also covers the composite pool and the
    // initialisation words, because the composite slot starts with its type's
    // zero. The resume-state word between them must stay zero, since zero means
    // the loop top for a fresh instance.
    let dl = m.data_layout.as_ref().expect("the subject declares data");
    let private_slots = dl
        .slots
        .iter()
        .filter(|s| s.visibility == keleusma::bytecode::SlotVisibility::Private)
        .count();
    let resume = keleusma::vm::required_persistent_capacity_for(&m);
    let init = resume + region::stream_state_supplement_bytes(&m) as usize;
    assert_eq!(
        image.len(),
        init + dl.private_composite_layout.len() * 8,
        "the image must reach the end of the initialisation words and no further"
    );
    assert!(
        image[resume..init].iter().all(|b| *b == 0),
        "the image writes the resume-state word, so a fresh instance would not start \
         at the loop top"
    );
    // A write stores the body's kind tag with its length above bit 32. The
    // struct tag is 1024 and `F` is sixteen bytes.
    assert_eq!(
        image[init..init + 8],
        (1024u64 | (16 << 32)).to_le_bytes(),
        "the composite slot starts with its type's zero, so its initialisation word \
         must say written, as a write of a sixteen-byte struct says it"
    );
    let entry = &dl.private_composite_layout[0];
    let body = private_slots * 8 + entry.offset as usize;
    assert!(
        image[body..body + 16].iter().all(|b| *b == 0),
        "the zero of `F` is sixteen zero bytes"
    );
}
