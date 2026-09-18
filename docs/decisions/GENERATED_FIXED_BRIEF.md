# BRIEF — generated breadth over the Fixed surface

**Filed 2026-09-18, against `af51d7d7`, backlog 0, suite 648.**

## What is left uncovered

Four generators now exist: word and byte expressions, composites, nesting, float
expressions, float streams. **`Fixed` appears in none of them.** It is the last
scalar type with no generated coverage, and it is the one with the most
arithmetic machinery behind it — a Q-format scale, `FixedMul` and `FixedDiv` with
their own opcodes, and a `%` that the reference virtual machine TRAPS on.

`Fixed % Fixed` is also **the one real divergence this line has ever found**: the
reference compiler accepted it, the virtual machine trapped, and this backend
returned an arithmetically correct answer. That history is the argument for
generated breadth here rather than against it.

## The hazards are different from the float generators', and that is the point

**The f32-versus-f64 constraint does not apply at all.** `Fixed` is exact integer
arithmetic on `i64` bits in both implementations; there is no configured width to
disagree about. Copying the float generators' exactness machinery would be
cargo-culting a guard against a hazard this surface does not have.

**What replaces it is overflow.** `Op::Add`, `Op::Sub` and `Op::Mul` are the
UNCHECKED opcodes — the compiler reserves them for `Byte`, `Fixed` and `Float` —
so a `Fixed` overflow is a **wrong number rather than a trap**. That is worse than
the word generator's hazard, where a checked overflow kills the process with
`SIGTRAP` and cannot be mistaken for a result.

So the magnitude bound is load-bearing in a new way: it does not prevent a crash,
it prevents a silently wrong comparison against a reference that would wrap
identically. **Both sides wrapping the same way would AGREE and prove nothing.**

- Keep leaves and depth small enough that no intermediate approaches the `i64`
  range once scaled by the Q-format shift.
- **Assert the bound in the tree**, not in a comment.
- And prefer a bound that makes the assertion meaningful: if both implementations
  wrap identically, the differential cannot see the overflow, so the guard is the
  only thing standing between a green run and a vacuous one.

## The surface, measured rather than assumed

From `scalar_operator_matrix.rs`'s recorded cells: `+`, `-`, `*`, `/`, unary `-`
and the six comparisons all AGREE. **`%` is `VmTrapsNativeRefuses`** — the
reference traps and the backend refuses, which is sound and must stay. Shifts and
the bitwise family are `RefRejects` — the reference compiler declines them.

**So the generator's operator set is `+`, `-`, `*`, `/` and nothing else**, with
`/` taking a non-zero literal divisor exactly as the word generator does.

## Wrong turns, named

1. **Do not include `%`.** It is a recorded divergence that is now correctly
   refused. Generating it would turn a sound refusal into a stream of failures.

2. **Do not copy the float exactness guard.** There is no configured float width
   here. A guard against a hazard the surface does not have is noise that makes
   the real guard harder to see.

3. **Do not assume a trap will catch an overflow.** Fixed arithmetic is unchecked.
   The word generator's header reasons from `SIGTRAP`; that reasoning does not
   transfer.

4. **Do not let a divisor be anything but a non-zero literal.** Division by zero
   is the same hazard by another route.

5. **Verify the scale.** Bare `Fixed` is `Fixed<32>`, not `Fixed<16>` — a previous
   increment's prose read raw operands as Q16.16 and printed wrong decimal figures.
   The differential is unaffected by the reading, but any figure written down is
   not.

6. **If a file is added, check the host-buffer census before committing.** A
   targeted run selected by binary name cannot see a census in another binary; that
   turned the gate red once this session.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment, written before the work

- **If the generator finds a divergence, check the magnitude bound first**, not the
  emitter. An overflow on one side only is the likeliest cause and it is a
  generator defect.
- **If it finds nothing, say so plainly.** Three of the four generators found
  nothing; that is the expected outcome for a backend that has produced no
  incorrect result all session, and writing it up as validation would be the
  overstatement this session keeps catching.
- **Prove reach by perturbation before believing a green run** — corrupt a Fixed
  arithmetic arm in the emitter and require the generator to catch it, in both
  configurations.
