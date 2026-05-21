//! Interval-arithmetic primitive used by the refinement-elision
//! pass (B13 Tier 3) and reserved for future helper-function WCMU
//! analysis (B12) and CallIndirect flow analysis (B14).
//!
//! Two layers of abstraction:
//!
//! - [`Interval`](crate::interval::Interval): a single closed signed range on `i64` with
//!   `None`-as-infinity bounds. Constructors `full`, `empty`,
//!   `singleton`, `at_least`, `at_most`, `range`. Predicates
//!   `is_empty`, `contains`, `is_subset_of`. Lattice operations
//!   `intersect`, `union` (convex hull). Transfer functions
//!   `neg`, `add`, `sub`, `mul`, `div`, `rem`. Sign-aware
//!   multiplication and division compute the corner products /
//!   quotients and take the bounding box; widens to `full()` on
//!   overflow or open bounds. Modulo handles the
//!   positive-singleton-divisor case tightly and widens
//!   otherwise.
//!
//! - [`IntervalSet`](crate::interval::IntervalSet): a sorted list of disjoint non-empty
//!   `Interval`s. Constructors `empty`, `full`, `singleton`,
//!   `from_interval`, `from_intervals` (normalising). Predicates
//!   `is_empty`, `contains`, `is_subset_of`. Lattice operations
//!   `intersect`, `union`, `complement` (exact). Transfer
//!   functions distribute over component pairs and renormalise.
//!   Admits non-convex true sets such as `not (x == 5)` and
//!   `x < 0 or x > 100`.
//!
//! Items reserved for a future pass: byte and fixed-point lattices,
//! widening operators for unbounded analyses, and cross-function
//! range summaries. See the B13 follow-on entry in BACKLOG.md.

use alloc::vec::Vec;
use core::cmp::{Ordering, max, min};

/// A closed signed interval on `i64`. `lo == None` means
/// `-infinity`; `hi == None` means `+infinity`. An empty
/// interval is represented as `lo == Some(1)` and `hi == Some(0)`
/// (any contradictory pair) and constructed exclusively through
/// [`Interval::empty`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    /// Closed lower bound, or `None` for negative infinity.
    pub lo: Option<i64>,
    /// Closed upper bound, or `None` for positive infinity.
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

    /// Widening operator (Cousot-Cousot style). Compares `self`
    /// (the previous iteration's value) with `other` (the new
    /// candidate). Bounds that grow are widened to infinity in
    /// the growth direction; bounds that shrink or stay are
    /// preserved. Used by the function-summary fixed-point pass
    /// to force convergence on recursive functions whose body
    /// would otherwise expand the range by a constant each
    /// iteration.
    ///
    /// Soundness: the widened result is always a superset of
    /// `other`. The pass may overshoot precision but never
    /// undershoots safety.
    pub fn widen(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let lo = match (self.lo, other.lo) {
            (None, _) => None,
            (_, None) => None,
            (Some(a), Some(b)) => {
                if b < a {
                    None
                } else {
                    Some(a)
                }
            }
        };
        let hi = match (self.hi, other.hi) {
            (None, _) => None,
            (_, None) => None,
            (Some(a), Some(b)) => {
                if b > a {
                    None
                } else {
                    Some(a)
                }
            }
        };
        Self { lo, hi }
    }

    /// Multiplication transfer. For fully-bounded operand
    /// intervals `[a, b] * [c, d]` the result is the convex hull
    /// of the four corner products. Either operand having an
    /// open bound (`None`) widens to `full()`; the same applies
    /// on any overflow. The convex hull is exact for
    /// multiplication on signed integers because the four
    /// corners reach the extreme values of the true product set.
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let (a, b) = match (self.lo, self.hi) {
            (Some(l), Some(h)) => (l, h),
            _ => return Self::full(),
        };
        let (c, d) = match (other.lo, other.hi) {
            (Some(l), Some(h)) => (l, h),
            _ => return Self::full(),
        };
        let products: [Option<i64>; 4] = [
            a.checked_mul(c),
            a.checked_mul(d),
            b.checked_mul(c),
            b.checked_mul(d),
        ];
        if products.iter().any(|p| p.is_none()) {
            return Self::full();
        }
        let values: [i64; 4] = [
            products[0].unwrap(),
            products[1].unwrap(),
            products[2].unwrap(),
            products[3].unwrap(),
        ];
        let lo = *values.iter().min().unwrap();
        let hi = *values.iter().max().unwrap();
        Self {
            lo: Some(lo),
            hi: Some(hi),
        }
    }

    /// Division transfer. Sign-aware corner analysis when the
    /// divisor's range excludes zero; widens to `full()` when
    /// the divisor's range includes zero (the result could be
    /// any value or trap at runtime). Open operand bounds also
    /// widen to `full()`. The `i64::MIN / -1` corner is handled
    /// soundly by checking `checked_div`.
    pub fn div(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let (a, b) = match (self.lo, self.hi) {
            (Some(l), Some(h)) => (l, h),
            _ => return Self::full(),
        };
        let (c, d) = match (other.lo, other.hi) {
            (Some(l), Some(h)) => (l, h),
            _ => return Self::full(),
        };
        // Divisor range includes zero: result is unbounded.
        if c <= 0 && d >= 0 {
            return Self::full();
        }
        let quotients: [Option<i64>; 4] = [
            a.checked_div(c),
            a.checked_div(d),
            b.checked_div(c),
            b.checked_div(d),
        ];
        if quotients.iter().any(|p| p.is_none()) {
            return Self::full();
        }
        let values: [i64; 4] = [
            quotients[0].unwrap(),
            quotients[1].unwrap(),
            quotients[2].unwrap(),
            quotients[3].unwrap(),
        ];
        let lo = *values.iter().min().unwrap();
        let hi = *values.iter().max().unwrap();
        Self {
            lo: Some(lo),
            hi: Some(hi),
        }
    }

    /// Modulo transfer. The mathematical range of `a % b` for a
    /// non-zero divisor lies in `(-|b|, |b|)`. For the common
    /// case where the divisor is a positive singleton `[d, d]`,
    /// the result is `[0, d-1]` when the dividend is non-
    /// negative and `[-(d-1), d-1]` otherwise. Other shapes
    /// widen to `full()` for soundness. A divisor that includes
    /// zero widens to `full()` because the operation may trap at
    /// runtime.
    pub fn rem(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let (c, d) = match (other.lo, other.hi) {
            (Some(l), Some(h)) => (l, h),
            _ => return Self::full(),
        };
        if c <= 0 && d >= 0 {
            return Self::full();
        }
        // Divisor is a positive singleton: tight bounds.
        if c == d && c > 0 {
            let modulus = c;
            let dividend_nonneg = matches!(self.lo, Some(l) if l >= 0);
            if dividend_nonneg {
                return Self::range(0, modulus - 1);
            }
            return Self::range(-(modulus - 1), modulus - 1);
        }
        // Other shapes: widen.
        Self::full()
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

/// A set of disjoint convex intervals on `i64`. Held as a sorted
/// list of non-empty `Interval`s with no overlap and no touching
/// gaps (adjacent intervals always have at least one integer
/// separating them; merging is handled by the normalising
/// constructor). Provides exact lattice and transfer functions
/// for predicates whose true sets are not convex, such as
/// `x < 0 or x > 100` and `not (x == 5)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalSet {
    intervals: Vec<Interval>,
}

impl IntervalSet {
    /// Empty set: contains no values.
    pub fn empty() -> Self {
        Self {
            intervals: Vec::new(),
        }
    }

    /// Full set: contains every `i64`.
    pub fn full() -> Self {
        Self {
            intervals: alloc::vec![Interval::full()],
        }
    }

    /// Singleton set containing only `n`.
    pub fn singleton(n: i64) -> Self {
        Self {
            intervals: alloc::vec![Interval::singleton(n)],
        }
    }

    /// Lift a single `Interval` to an `IntervalSet`. Empty
    /// intervals lift to the empty set.
    pub fn from_interval(i: Interval) -> Self {
        if i.is_empty() {
            Self::empty()
        } else {
            Self {
                intervals: alloc::vec![i],
            }
        }
    }

    /// Build a set from an arbitrary collection of intervals,
    /// normalising by removing empties, sorting by lower bound,
    /// and merging overlapping or touching ranges. This is the
    /// canonical entry point for transfer functions that produce
    /// multiple component intervals.
    pub fn from_intervals(mut v: Vec<Interval>) -> Self {
        v.retain(|i| !i.is_empty());
        v.sort_by(|a, b| cmp_lo(a.lo, b.lo));
        let mut merged: Vec<Interval> = Vec::new();
        for i in v {
            if let Some(last) = merged.last_mut()
                && let Some(joined) = merge_if_touching(last, &i)
            {
                *last = joined;
                continue;
            }
            merged.push(i);
        }
        Self { intervals: merged }
    }

    /// Iterate the component intervals in order.
    pub fn parts(&self) -> &[Interval] {
        &self.intervals
    }

    /// True when the set contains no values.
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// True when `n` is a member of any component interval.
    pub fn contains(&self, n: i64) -> bool {
        self.intervals.iter().any(|i| i.contains(n))
    }

    /// True when every value in `self` lies in `other`.
    /// Implemented by requiring each component of `self` to be a
    /// subset of some component of `other`. This is exact for
    /// disjoint normalised sets because a single convex interval
    /// can be a subset of a non-convex set only when it lies
    /// entirely within one of that set's components.
    pub fn is_subset_of(&self, other: &Self) -> bool {
        if self.is_empty() {
            return true;
        }
        self.intervals
            .iter()
            .all(|si| other.intervals.iter().any(|oi| si.is_subset_of(oi)))
    }

    /// Set intersection (meet). Exact.
    pub fn intersect(&self, other: &Self) -> Self {
        let mut parts: Vec<Interval> = Vec::new();
        for a in &self.intervals {
            for b in &other.intervals {
                let p = a.intersect(b);
                if !p.is_empty() {
                    parts.push(p);
                }
            }
        }
        Self::from_intervals(parts)
    }

    /// Set union (join). Exact.
    pub fn union(&self, other: &Self) -> Self {
        let mut all: Vec<Interval> =
            Vec::with_capacity(self.intervals.len() + other.intervals.len());
        all.extend(self.intervals.iter().copied());
        all.extend(other.intervals.iter().copied());
        Self::from_intervals(all)
    }

    /// Set complement: every `i64` not in `self`. Walks the
    /// sorted components and emits the gaps between them plus
    /// the regions outside the first and last bounds.
    ///
    /// State machine: `cursor` tracks the lower bound of the
    /// next-emittable gap. `Some(c)` means "the next gap starts
    /// at `c`"; `None` means "we are at `-infinity` and have not
    /// yet seen any component covering it". The `finished` flag
    /// is set when a component reaches `+infinity`; further gap
    /// emission is suppressed and the trailing-region step is
    /// skipped.
    pub fn complement(&self) -> Self {
        if self.intervals.is_empty() {
            return Self::full();
        }
        let mut parts: Vec<Interval> = Vec::new();
        let mut cursor: Option<i64> = None;
        let mut at_neg_infinity: bool = true;
        let mut finished: bool = false;
        for i in &self.intervals {
            if finished {
                break;
            }
            // Emit the gap from cursor up to (component_lo - 1).
            match (at_neg_infinity, i.lo) {
                (true, None) => {
                    // The first component covers -infinity; no
                    // gap to emit at the start.
                }
                (true, Some(l)) => {
                    if let Some(h) = l.checked_sub(1) {
                        parts.push(Interval::at_most(h));
                    }
                }
                (false, None) => {
                    // Cannot happen for a normalised set: a
                    // component with lo=None would have been the
                    // first component and merged with predecessors.
                }
                (false, Some(l)) => {
                    if let (Some(c), Some(h)) = (cursor, l.checked_sub(1))
                        && c <= h
                    {
                        parts.push(Interval::range(c, h));
                    }
                }
            }
            at_neg_infinity = false;
            // Advance cursor past this component.
            match i.hi {
                None => {
                    finished = true;
                }
                Some(h) => match h.checked_add(1) {
                    Some(next) => cursor = Some(next),
                    None => {
                        finished = true;
                    }
                },
            }
        }
        if !finished && let Some(c) = cursor {
            parts.push(Interval::at_least(c));
        }
        Self::from_intervals(parts)
    }

    /// Negation transfer: applied per component, then
    /// renormalised.
    pub fn neg(&self) -> Self {
        Self::from_intervals(self.intervals.iter().map(|i| i.neg()).collect())
    }

    /// Addition transfer: pairwise apply, then union.
    pub fn add(&self, other: &Self) -> Self {
        pairwise(self, other, Interval::add)
    }

    /// Subtraction transfer: pairwise apply, then union.
    pub fn sub(&self, other: &Self) -> Self {
        pairwise(self, other, Interval::sub)
    }

    /// Multiplication transfer: pairwise apply, then union.
    pub fn mul(&self, other: &Self) -> Self {
        pairwise(self, other, Interval::mul)
    }

    /// Division transfer: pairwise apply, then union.
    pub fn div(&self, other: &Self) -> Self {
        pairwise(self, other, Interval::div)
    }

    /// Modulo transfer: pairwise apply, then union.
    pub fn rem(&self, other: &Self) -> Self {
        pairwise(self, other, Interval::rem)
    }

    /// Widening at the set level. Reduces both sets to their
    /// convex hulls and widens those. Coarse compared to a
    /// piecewise widening but sound; the customer (function-
    /// summary convergence) only needs a final upper bound.
    pub fn widen(&self, other: &Self) -> Self {
        let self_hull = hull(self);
        let other_hull = hull(other);
        Self::from_interval(self_hull.widen(&other_hull))
    }
}

/// Convex hull of an `IntervalSet`: the smallest `Interval`
/// containing every component. Empty set returns the empty
/// interval.
fn hull(s: &IntervalSet) -> Interval {
    let mut acc = Interval::empty();
    for i in &s.intervals {
        acc = acc.union(i);
    }
    acc
}

/// Compare two lower bounds with `None` as `-infinity`.
fn cmp_lo(a: Option<i64>, b: Option<i64>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => x.cmp(&y),
    }
}

/// If two intervals overlap or are adjacent (touching with no
/// gap), produce the merged interval; otherwise `None`. The
/// touch test treats `[a, b]` and `[b+1, c]` as adjacent because
/// no integer lies strictly between them.
fn merge_if_touching(a: &Interval, b: &Interval) -> Option<Interval> {
    if a.is_empty() {
        return Some(*b);
    }
    if b.is_empty() {
        return Some(*a);
    }
    // Order so that `lo` is the one with the smaller lower bound
    // (the input is sorted in `from_intervals`, but be defensive).
    let (lo_iv, hi_iv) = if cmp_lo(a.lo, b.lo) == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    };
    let touch = match (lo_iv.hi, hi_iv.lo) {
        (None, _) => true,
        (Some(_), None) => true,
        (Some(h), Some(l)) => h >= l || h.checked_add(1) == Some(l),
    };
    if !touch {
        return None;
    }
    let new_lo = lo_iv.lo;
    let new_hi = match (lo_iv.hi, hi_iv.hi) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(max(x, y)),
    };
    Some(Interval {
        lo: new_lo,
        hi: new_hi,
    })
}

/// Apply a binary interval operation to every pair of components
/// and renormalise.
fn pairwise(
    a: &IntervalSet,
    b: &IntervalSet,
    op: fn(&Interval, &Interval) -> Interval,
) -> IntervalSet {
    let mut parts: Vec<Interval> = Vec::with_capacity(a.intervals.len() * b.intervals.len());
    for ai in &a.intervals {
        for bi in &b.intervals {
            parts.push(op(ai, bi));
        }
    }
    IntervalSet::from_intervals(parts)
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

    #[test]
    fn mul_of_positives_yields_positive_range() {
        let a = Interval::range(2, 5);
        let b = Interval::range(3, 7);
        assert_eq!(a.mul(&b), Interval::range(6, 35));
    }

    #[test]
    fn mul_of_mixed_signs_spans_zero() {
        let a = Interval::range(-2, 3);
        let b = Interval::range(-4, 5);
        // Corner products: 8, -10, -12, 15. Range: [-12, 15].
        assert_eq!(a.mul(&b), Interval::range(-12, 15));
    }

    #[test]
    fn mul_overflow_widens_to_full() {
        let a = Interval::singleton(i64::MAX);
        let b = Interval::singleton(2);
        assert_eq!(a.mul(&b), Interval::full());
    }

    #[test]
    fn mul_of_unbounded_is_full() {
        let a = Interval::at_least(0);
        let b = Interval::singleton(3);
        assert_eq!(a.mul(&b), Interval::full());
    }

    #[test]
    fn div_of_positives_yields_quotient_range() {
        let a = Interval::range(20, 100);
        let b = Interval::range(2, 5);
        // Corner quotients: 10, 4, 50, 20. Range: [4, 50].
        assert_eq!(a.div(&b), Interval::range(4, 50));
    }

    #[test]
    fn div_by_zero_containing_divisor_widens() {
        let a = Interval::range(10, 20);
        let b = Interval::range(-1, 1);
        assert_eq!(a.div(&b), Interval::full());
    }

    #[test]
    fn div_min_by_neg_one_overflow_widens() {
        let a = Interval::singleton(i64::MIN);
        let b = Interval::singleton(-1);
        assert_eq!(a.div(&b), Interval::full());
    }

    #[test]
    fn mod_positive_divisor_nonneg_dividend() {
        let a = Interval::range(0, 100);
        let b = Interval::singleton(7);
        assert_eq!(a.rem(&b), Interval::range(0, 6));
    }

    #[test]
    fn mod_positive_divisor_full_dividend() {
        let a = Interval::full();
        let b = Interval::singleton(10);
        assert_eq!(a.rem(&b), Interval::range(-9, 9));
    }

    #[test]
    fn mod_by_zero_containing_divisor_widens() {
        let a = Interval::range(0, 100);
        let b = Interval::range(-2, 2);
        assert_eq!(a.rem(&b), Interval::full());
    }

    #[test]
    fn interval_set_empty_and_full() {
        assert!(IntervalSet::empty().is_empty());
        assert!(!IntervalSet::full().is_empty());
        assert!(IntervalSet::full().contains(0));
        assert!(IntervalSet::full().contains(i64::MAX));
    }

    #[test]
    fn interval_set_normalises_overlapping_components() {
        let s =
            IntervalSet::from_intervals(
                alloc::vec![Interval::range(0, 5), Interval::range(3, 10),],
            );
        assert_eq!(s.parts(), &[Interval::range(0, 10)]);
    }

    #[test]
    fn interval_set_normalises_adjacent_components() {
        // [0, 5] and [6, 10] are adjacent (no integer between);
        // they merge to [0, 10].
        let s =
            IntervalSet::from_intervals(
                alloc::vec![Interval::range(0, 5), Interval::range(6, 10),],
            );
        assert_eq!(s.parts(), &[Interval::range(0, 10)]);
    }

    #[test]
    fn interval_set_keeps_disjoint_components_separate() {
        // [0, 5] and [10, 20] have a gap of [6, 9]; they stay
        // as two components.
        let s = IntervalSet::from_intervals(alloc::vec![
            Interval::range(10, 20),
            Interval::range(0, 5),
        ]);
        assert_eq!(s.parts(), &[Interval::range(0, 5), Interval::range(10, 20)]);
    }

    #[test]
    fn interval_set_union_combines_components() {
        let a = IntervalSet::from_interval(Interval::range(0, 10));
        let b = IntervalSet::from_interval(Interval::range(20, 30));
        let u = a.union(&b);
        assert_eq!(
            u.parts(),
            &[Interval::range(0, 10), Interval::range(20, 30)]
        );
    }

    #[test]
    fn interval_set_intersect_pairwise() {
        let a = IntervalSet::from_intervals(alloc::vec![
            Interval::range(0, 10),
            Interval::range(20, 30),
        ]);
        let b = IntervalSet::from_intervals(alloc::vec![Interval::range(5, 25),]);
        let i = a.intersect(&b);
        assert_eq!(
            i.parts(),
            &[Interval::range(5, 10), Interval::range(20, 25)]
        );
    }

    #[test]
    fn interval_set_complement_of_singleton() {
        let s = IntervalSet::singleton(5);
        let c = s.complement();
        assert_eq!(c.parts(), &[Interval::at_most(4), Interval::at_least(6)]);
    }

    #[test]
    fn interval_set_complement_of_bounded_range() {
        let s = IntervalSet::from_interval(Interval::range(0, 100));
        let c = s.complement();
        assert_eq!(c.parts(), &[Interval::at_most(-1), Interval::at_least(101)]);
    }

    #[test]
    fn interval_set_complement_of_full_is_empty() {
        let c = IntervalSet::full().complement();
        assert!(c.is_empty());
    }

    #[test]
    fn interval_set_complement_of_empty_is_full() {
        let c = IntervalSet::empty().complement();
        assert_eq!(c.parts(), &[Interval::full()]);
    }

    #[test]
    fn interval_set_subset_across_components() {
        // [0, 5] is a subset of {[0, 10], [20, 30]}.
        let a = IntervalSet::from_interval(Interval::range(0, 5));
        let b = IntervalSet::from_intervals(alloc::vec![
            Interval::range(0, 10),
            Interval::range(20, 30),
        ]);
        assert!(a.is_subset_of(&b));
    }

    #[test]
    fn interval_set_not_subset_when_straddles_gap() {
        // [5, 25] crosses the gap between [0, 10] and [20, 30];
        // it is NOT a subset because [11, 19] are missing.
        let a = IntervalSet::from_interval(Interval::range(5, 25));
        let b = IntervalSet::from_intervals(alloc::vec![
            Interval::range(0, 10),
            Interval::range(20, 30),
        ]);
        assert!(!a.is_subset_of(&b));
    }

    #[test]
    fn interval_set_neg_distributes() {
        let s = IntervalSet::from_intervals(alloc::vec![
            Interval::range(0, 5),
            Interval::range(10, 20),
        ]);
        let n = s.neg();
        assert_eq!(
            n.parts(),
            &[Interval::range(-20, -10), Interval::range(-5, 0)]
        );
    }

    #[test]
    fn widen_stops_when_bounds_stable() {
        let a = Interval::range(0, 100);
        let b = Interval::range(0, 100);
        assert_eq!(a.widen(&b), Interval::range(0, 100));
    }

    #[test]
    fn widen_extends_to_positive_infinity_on_upper_growth() {
        let a = Interval::range(0, 10);
        let b = Interval::range(0, 11);
        assert_eq!(a.widen(&b), Interval::at_least(0));
    }

    #[test]
    fn widen_extends_to_negative_infinity_on_lower_growth() {
        let a = Interval::range(0, 100);
        let b = Interval::range(-1, 100);
        assert_eq!(a.widen(&b), Interval::at_most(100));
    }

    #[test]
    fn widen_to_full_on_both_directions() {
        let a = Interval::range(0, 10);
        let b = Interval::range(-1, 11);
        assert_eq!(a.widen(&b), Interval::full());
    }

    #[test]
    fn interval_set_widen_takes_hull() {
        let a = IntervalSet::from_interval(Interval::range(0, 10));
        let b = IntervalSet::from_interval(Interval::range(0, 100));
        // Hull of a is [0, 10]; hull of b is [0, 100]; widening
        // extends the upper bound to +infinity.
        assert_eq!(a.widen(&b).parts(), &[Interval::at_least(0)]);
    }

    #[test]
    fn interval_set_add_pairwise_unions() {
        // {[0, 1], [10, 11]} + {[0, 0], [100, 100]}:
        //   [0, 1] + [0, 0] = [0, 1]
        //   [0, 1] + [100, 100] = [100, 101]
        //   [10, 11] + [0, 0] = [10, 11]
        //   [10, 11] + [100, 100] = [110, 111]
        // Normalised: [0, 1], [10, 11], [100, 101], [110, 111]
        let a = IntervalSet::from_intervals(alloc::vec![
            Interval::range(0, 1),
            Interval::range(10, 11),
        ]);
        let b = IntervalSet::from_intervals(alloc::vec![
            Interval::singleton(0),
            Interval::singleton(100),
        ]);
        assert_eq!(
            a.add(&b).parts(),
            &[
                Interval::range(0, 1),
                Interval::range(10, 11),
                Interval::range(100, 101),
                Interval::range(110, 111),
            ]
        );
    }
}
