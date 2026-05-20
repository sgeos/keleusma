//! Interval-arithmetic primitive used by the refinement-elision
//! pass (B13 Tier 3) and reserved for future helper-function WCMU
//! analysis (B12) and CallIndirect flow analysis (B14).
//!
//! The lattice is closed signed intervals on `i64` with explicit
//! `None`-as-infinity bounds. The MVP carries:
//!
//! - Constructors for the common shapes: `full`, `empty`,
//!   `singleton`, `at_least`, `at_most`, `range`.
//! - Predicates: `is_empty`, `contains`, `is_subset_of`.
//! - Lattice operations: `intersect`, `union` (the union returns
//!   the convex hull when the inputs are disjoint, a conservative
//!   over-approximation).
//! - Transfer functions for the arithmetic operators currently
//!   used by the refinement-elision evaluator: `neg`, `add`,
//!   `sub`. Multiplication, division, and modulo are not yet
//!   wired into the lattice because they require sign-aware
//!   reasoning that the MVP intentionally omits; the predicate
//!   decompiler returns `None` when it cannot reduce the body to
//!   a finite intersection of half-bounded comparisons.
//!
//! The lattice is intentionally minimal. Items that would extend
//! it for richer analysis (widening operators, sign-aware
//! multiplication and division, byte and fixed-point support)
//! are recorded under the B13 follow-on entry in BACKLOG.md.

use core::cmp::{max, min};

/// A closed signed interval on `i64`. `lo == None` means
/// `-infinity`; `hi == None` means `+infinity`. An empty
/// interval is represented as `lo == Some(1)` and `hi == Some(0)`
/// (any contradictory pair) and constructed exclusively through
/// [`Interval::empty`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

impl Interval {
    /// The top of the lattice: every `i64`.
    pub const fn full() -> Self {
        Self { lo: None, hi: None }
    }

    /// The bottom of the lattice: no values.
    pub const fn empty() -> Self {
        Self {
            lo: Some(1),
            hi: Some(0),
        }
    }

    /// The single-element interval `[n, n]`.
    pub const fn singleton(n: i64) -> Self {
        Self {
            lo: Some(n),
            hi: Some(n),
        }
    }

    /// `[n, +infinity]`.
    pub const fn at_least(n: i64) -> Self {
        Self {
            lo: Some(n),
            hi: None,
        }
    }

    /// `[-infinity, n]`.
    pub const fn at_most(n: i64) -> Self {
        Self {
            lo: None,
            hi: Some(n),
        }
    }

    /// `[lo, hi]`. Returns an empty interval when `lo > hi`.
    pub fn range(lo: i64, hi: i64) -> Self {
        if lo > hi {
            Self::empty()
        } else {
            Self {
                lo: Some(lo),
                hi: Some(hi),
            }
        }
    }

    /// True when the interval contains no values. The empty
    /// interval has `lo > hi` by construction; other empty
    /// representations are not produced by the constructors.
    pub fn is_empty(&self) -> bool {
        match (self.lo, self.hi) {
            (Some(l), Some(h)) => l > h,
            _ => false,
        }
    }

    /// True when `n` lies within the interval.
    pub fn contains(&self, n: i64) -> bool {
        if self.is_empty() {
            return false;
        }
        match self.lo {
            Some(l) if n < l => return false,
            _ => {}
        }
        match self.hi {
            Some(h) if n > h => return false,
            _ => {}
        }
        true
    }

    /// True when every value in `self` also lies in `other`.
    /// An empty interval is a subset of every interval. No
    /// interval is a subset of the empty interval except itself.
    pub fn is_subset_of(&self, other: &Self) -> bool {
        if self.is_empty() {
            return true;
        }
        if other.is_empty() {
            return false;
        }
        let lo_ok = match (self.lo, other.lo) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(s), Some(o)) => s >= o,
        };
        let hi_ok = match (self.hi, other.hi) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(s), Some(o)) => s <= o,
        };
        lo_ok && hi_ok
    }

    /// Lattice meet (set intersection).
    pub fn intersect(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let lo = match (self.lo, other.lo) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) => Some(max(a, b)),
        };
        let hi = match (self.hi, other.hi) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) => Some(min(a, b)),
        };
        match (lo, hi) {
            (Some(l), Some(h)) if l > h => Self::empty(),
            _ => Self { lo, hi },
        }
    }

    /// Lattice join (convex hull). When the inputs are disjoint
    /// the result includes the gap, which is a conservative
    /// over-approximation suitable for soundness in the elision
    /// pass (we never claim a tighter range than truth).
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let lo = match (self.lo, other.lo) {
            (None, _) | (_, None) => None,
            (Some(a), Some(b)) => Some(min(a, b)),
        };
        let hi = match (self.hi, other.hi) {
            (None, _) | (_, None) => None,
            (Some(a), Some(b)) => Some(max(a, b)),
        };
        Self { lo, hi }
    }

    /// Negation transfer: `-[a, b] = [-b, -a]`. Handles the
    /// `i64::MIN` edge by saturating to `full()` on overflow,
    /// preserving soundness (we never narrow the range).
    pub fn neg(&self) -> Self {
        if self.is_empty() {
            return Self::empty();
        }
        let new_lo = match self.hi {
            Some(h) => h.checked_neg(),
            None => None,
        };
        let new_hi = match self.lo {
            Some(l) => l.checked_neg(),
            None => None,
        };
        // `checked_neg` returns None on i64::MIN. In that case
        // the negation of the original bound exceeds i64::MAX,
        // and the sound abstraction is to widen to full() rather
        // than carrying a partial result.
        if (self.hi.is_some() && new_lo.is_none()) || (self.lo.is_some() && new_hi.is_none()) {
            return Self::full();
        }
        Self {
            lo: new_lo,
            hi: new_hi,
        }
    }

    /// Addition transfer: `[a, b] + [c, d] = [a + c, b + d]`.
    /// Returns `full()` on any bound overflow, preserving
    /// soundness.
    pub fn add(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let lo = combine_add(self.lo, other.lo);
        let hi = combine_add(self.hi, other.hi);
        if (self.lo.is_some() && other.lo.is_some() && lo.is_none())
            || (self.hi.is_some() && other.hi.is_some() && hi.is_none())
        {
            return Self::full();
        }
        Self { lo, hi }
    }

    /// Subtraction transfer: `[a, b] - [c, d] = [a - d, b - c]`.
    /// Returns `full()` on bound overflow.
    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }
}

/// Add two infinity-aware bounds. Returns `None` when either
/// input is `None` (infinity) or when the addition overflows.
fn combine_add(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => x.checked_add(y),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_contains_everything() {
        let f = Interval::full();
        assert!(f.contains(0));
        assert!(f.contains(i64::MAX));
        assert!(f.contains(i64::MIN));
        assert!(!f.is_empty());
    }

    #[test]
    fn empty_contains_nothing() {
        let e = Interval::empty();
        assert!(!e.contains(0));
        assert!(e.is_empty());
    }

    #[test]
    fn singleton_contains_only_the_value() {
        let s = Interval::singleton(42);
        assert!(s.contains(42));
        assert!(!s.contains(41));
        assert!(!s.contains(43));
    }

    #[test]
    fn range_orders_correctly() {
        let r = Interval::range(0, 100);
        assert!(r.contains(0));
        assert!(r.contains(50));
        assert!(r.contains(100));
        assert!(!r.contains(-1));
        assert!(!r.contains(101));
    }

    #[test]
    fn range_with_inverted_bounds_is_empty() {
        let r = Interval::range(10, 5);
        assert!(r.is_empty());
    }

    #[test]
    fn at_least_is_one_sided() {
        let a = Interval::at_least(0);
        assert!(a.contains(0));
        assert!(a.contains(i64::MAX));
        assert!(!a.contains(-1));
    }

    #[test]
    fn at_most_is_one_sided() {
        let a = Interval::at_most(100);
        assert!(a.contains(100));
        assert!(a.contains(0));
        assert!(a.contains(i64::MIN));
        assert!(!a.contains(101));
    }

    #[test]
    fn singleton_is_subset_of_containing_range() {
        let s = Interval::singleton(42);
        let r = Interval::range(0, 100);
        assert!(s.is_subset_of(&r));
        assert!(!r.is_subset_of(&s));
    }

    #[test]
    fn empty_is_subset_of_everything() {
        let e = Interval::empty();
        assert!(e.is_subset_of(&Interval::full()));
        assert!(e.is_subset_of(&Interval::singleton(0)));
    }

    #[test]
    fn intersect_narrows_to_overlap() {
        let a = Interval::range(0, 100);
        let b = Interval::range(50, 200);
        assert_eq!(a.intersect(&b), Interval::range(50, 100));
    }

    #[test]
    fn intersect_of_disjoint_is_empty() {
        let a = Interval::range(0, 10);
        let b = Interval::range(20, 30);
        assert!(a.intersect(&b).is_empty());
    }

    #[test]
    fn intersect_with_half_bounded_works() {
        let a = Interval::at_least(0);
        let b = Interval::at_most(100);
        assert_eq!(a.intersect(&b), Interval::range(0, 100));
    }

    #[test]
    fn union_of_overlapping_widens() {
        let a = Interval::range(0, 50);
        let b = Interval::range(40, 100);
        assert_eq!(a.union(&b), Interval::range(0, 100));
    }

    #[test]
    fn union_of_disjoint_includes_gap() {
        let a = Interval::range(0, 10);
        let b = Interval::range(20, 30);
        assert_eq!(a.union(&b), Interval::range(0, 30));
    }

    #[test]
    fn neg_flips_bounds() {
        let a = Interval::range(0, 100);
        assert_eq!(a.neg(), Interval::range(-100, 0));
    }

    #[test]
    fn neg_of_min_widens_to_full() {
        // i64::MIN.checked_neg() == None; the sound abstraction
        // widens to full() rather than producing a partial bound.
        let a = Interval::singleton(i64::MIN);
        assert_eq!(a.neg(), Interval::full());
    }

    #[test]
    fn add_combines_bounds() {
        let a = Interval::range(0, 10);
        let b = Interval::range(5, 15);
        assert_eq!(a.add(&b), Interval::range(5, 25));
    }

    #[test]
    fn add_overflow_widens_to_full() {
        let a = Interval::singleton(i64::MAX);
        let b = Interval::singleton(1);
        assert_eq!(a.add(&b), Interval::full());
    }

    #[test]
    fn sub_combines_bounds() {
        let a = Interval::range(10, 20);
        let b = Interval::range(2, 5);
        assert_eq!(a.sub(&b), Interval::range(5, 18));
    }
}
