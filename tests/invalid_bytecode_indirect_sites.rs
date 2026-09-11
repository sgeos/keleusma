//! The `InvalidBytecode` sites a source grep for the variant CANNOT see.
//!
//! # Why this file exists
//!
//! `docs/decisions/INVALID_BYTECODE_CENSUS.md` derives its population with
//! `grep -rn "VmError::InvalidBytecode" src/`, which matches the variant where
//! it is CONSTRUCTED. The census says plainly what that instrument misses:
//!
//! > A site that returns a pre-built error value, propagates one from a helper
//! > with `?`, or maps another error kind into this one would not appear. One
//! > such conversion DOES exist and is included because the grep happened to
//! > see it, which is evidence the class has members this scan cannot
//! > enumerate.
//!
//! **That conversion is `impl From<ScalarError> for VmError`, and the grep
//! counts it as ONE site.** It is one construction and many reaching paths: a
//! malformed artefact arrives at it through every call that can raise a
//! `ScalarError`, and the census's table attributes all of them to a single
//! row.
//!
//! This file enumerates those paths, so "the population is a lower bound" stops
//! being a caveat and becomes a number.
//!
//! # The derivation, stated so it can be re-run
//!
//! `GenericValue::read_scalar_le` and `GenericValue::write_scalar_le` are the
//! only functions returning `Result<_, ScalarError>` that the runtime calls.
//! Every call to either, in a function whose error type is `VmError`, is a path
//! to `InvalidBytecode` that the variant grep does not see. Counted here from
//! source, with comments removed first.
//!
//! # Comments are stripped, because this repository has paid for that
//!
//! The doc comments in both files NAME these functions -- the paragraph above
//! does so twice. Counting raw lines would inflate the population with prose,
//! which is the defect a thirteen-file sweep repaired across this line of work.

/// Source with `//` line comments removed, so a search matches CODE.
///
/// An early truncation can only HIDE an occurrence here, which would make the
/// count too low and fail the pinned assertion loudly. The dangerous direction
/// for this guard is the opposite one, prose inflating the count, and that is
/// what the strip removes.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Calls to the two `ScalarError`-returning functions, in one source.
fn scalar_call_sites(raw: &str) -> usize {
    let src = code_only(raw);
    src.matches("read_scalar_le(").count() + src.matches("write_scalar_le(").count()
}

const VM: &str = include_str!("../src/vm.rs");
const MARSHALL: &str = include_str!("../src/marshall.rs");

/// **THE INDIRECT POPULATION, PINNED.**
///
/// Ten call sites, six in `src/vm.rs` and four in `src/marshall.rs`. Each one
/// converts a `ScalarError` into `VmError::InvalidBytecode`, through `?` in a
/// function whose error type is `VmError` or through an explicit
/// `map_err(VmError::from)`.
///
/// A failure here does not mean a defect. It means the census's population
/// figure is STALE, which is the thing a lower bound cannot tell you on its
/// own.
#[test]
fn the_census_population_is_understated_by_exactly_these_indirect_sites() {
    let vm = scalar_call_sites(VM);
    let marshall = scalar_call_sites(MARSHALL);
    assert_eq!(
        (vm, marshall),
        (6, 4),
        "the number of paths from a `ScalarError` to `InvalidBytecode` has changed \
         ({vm} in src/vm.rs, {marshall} in src/marshall.rs, against 6 and 4 recorded). \
         `docs/decisions/INVALID_BYTECODE_CENSUS.md` reports these as a single group-A row, \
         so its population figure now understates the class by a different amount than it says"
    );
}

/// The strip is DEFENSIVE today, not load-bearing, and that was measured.
///
/// Both files' own documentation names these functions, so the obvious claim is
/// that an unstripped count would include prose. **Measured, it would not**:
/// raw and stripped counts are both 6 and 4, because every prose mention
/// happens to omit the opening parenthesis the pattern requires. Saying the
/// strip protects the count today would have been a plausible sentence with
/// nothing behind it.
///
/// It is kept, and pinned here, because the exposure is one comment away -- a
/// note written `read_scalar_le(bytes, ..)` would inflate the population this
/// file reports. The decoy below carries exactly that shape, so the guard
/// fails if the strip is removed even though the tree does not currently
/// contain the offending form.
#[test]
fn prose_naming_the_functions_does_not_enter_the_count() {
    const DECOY: &str = "// calls read_scalar_le( and write_scalar_le( in prose\n\
                         let a = read_scalar_le(x);\n";
    assert_eq!(
        scalar_call_sites(DECOY),
        1,
        "a comment naming the functions was counted as a call site, so the census population \
         this file pins would grow whenever someone documented the mechanism"
    );
    assert!(
        scalar_call_sites(VM) > 0,
        "no call site was found in src/vm.rs at all, which is how a source counter is made \
         prose-proof by being made useless"
    );
}
