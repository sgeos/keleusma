# BRIEF — generated expression trees, differentially tested

## Why this, and why now

**Nothing in this package generates a program.** Every subject is hand-written:
74 corpus modules, 90 same-type operator cells, 42 mixed cells, four opcode
witness families, three stream shapes. All of them enumerate a surface someone
thought of.

**The backend has produced no defect since `Fixed % Fixed`** — through a 200-tick
stream sweep, an arena high-water measurement, a corpus-wide region census, 64 of
66 driven opcode witnesses, and a full corpus execution under `default<O2>`. That
is a real result. It is also the classic signal that **hand-written tests have
been exhausted and the next defect is in a composition nobody wrote down.**

The fixed matrices test operators **one at a time**. Nothing here tests
**composition**: an expression tree where a checked multiply feeds a divide feeds
a comparison, with operand widths mixing along the way. That is where a lowering
bug that survives every single-operator cell would live.

## The design, and the traps in it

**Seeded, reproducible, bounded.** A failure that cannot be reproduced is an
anecdote. Every program comes from a seed that is printed on failure, and the seed
set is fixed so the run is deterministic across machines and days.

**Trapping is the hazard that makes this harder than it looks.** Keleusma's `+`,
`-`, `*` are CHECKED: on overflow the reference returns an error and the native
side executes `llvm.trap`, which **kills the process with SIGTRAP**. A harness
that generates overflowing arithmetic does not get a comparison, it gets a dead
test binary. Division and modulo by zero are the same hazard by another route.

So the generator must produce expressions whose evaluation **cannot** trap:
bound the constants and the parameter values so that the worst-case magnitude of
any subexpression stays far inside the word, and make every divisor a non-zero
constant. **This narrows what is explored, and the narrowing must be stated** —
an untrapped-arithmetic generator says nothing about the trap paths, which the
existing witnesses cover separately.

**A generator that produces trivial programs is the failure mode that reads as
success.** If depth collapses to one node, this is a slower version of the
operator matrix. Assert a floor on the distinct shapes produced, and on the number
of operators actually appearing across the run.

## Prior failures in this package to avoid repeating

- **A probe implicating itself, five times so far** — a float argument passed as
  `i64::MIN`, a float return read as an integer, a hand-named signature taking a
  SIGBUS, a degenerate stream handed to the general driver, and a subject written
  with `let mut` in a language with no mutable local. **Any program the generator
  emits that fails to compile is the GENERATOR's defect until proven otherwise**,
  and must fail loudly rather than being skipped — a generator that silently
  discards what it cannot compile will quietly shrink to the trivial subset.
- **Believing a green run before asking what it covered.** Count the programs
  actually compared and assert a floor.
- **A guard with no reach.** Before reporting "no divergence", show the comparison
  can fail — perturb one operator's lowering or compare against a deliberately
  wrong expected value and watch it fire.
- **Unbounded runtime.** The gate's worst phase is already 380s and had to be
  split. This must cost seconds, not minutes; prefer a few hundred small programs
  over a long sweep.

## What a green result may and may not claim

**May**: no divergence over N generated expression trees of bounded depth, over
non-trapping arithmetic, at this commit. **May not**: that the lowering is
correct, that composition is exhaustively covered, or anything about the trap
paths, floats, composites, or streams — none of which this generator emits.


---

## OUTCOME

**300 generated programs, 0 divergences, 0.58 seconds.** Depth 3, 300 distinct
sources, seven distinct operators appearing, deepest nesting 3 levels.

### Reach proven at the SUBJECT, not at the assertion

`Op::BitXor` was temporarily lowered as `build_or` in the emitter — a real defect
in the real lowering, not an edited expectation. **The differential caught it on
program 0**:

```
fn main(a: Word, b: Word) -> Word { (((b * 7) - (b bxor b)) * ((1 bor b) band (a + a))) }
  reference = 42
  native    = 36
```

`b bxor b` is zero; `b bor b` is `b`. The failure names the program and both
values, and the generator is seeded, so it reproduces by re-running.

**This is the first instrument in the package whose reach was demonstrated by
breaking the lowering itself** rather than by shrinking a buffer or stubbing a
helper. Every operator in the tree was already covered one-at-a-time by
`scalar_operator_matrix.rs`; what this adds is composition.

### No defect found, and what that is worth

The backend survived 300 composed trees on the first attempt. **That is a
genuinely informative negative** — the fixed matrices could not have told us
whether composition holds — but it is bounded: depth 3, `Word` only,
non-trapping arithmetic, one seed.

**The cheapest way to make this stronger is more seeds, not more machinery.**
The run costs 0.58s; a future increment could sweep several seeds for a few
seconds more. Deliberately not done now: one seeded run that is understood beats
a wider one added in the same breath as the harness.
