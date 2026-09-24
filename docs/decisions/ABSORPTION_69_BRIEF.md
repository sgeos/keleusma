# ABSORPTION 69 — the ownership check stops being vacuous

**Computed against** `8957a89f3faf8430ee51bcc5c8b68608a734816a`, with **2 unabsorbed**
commits on `origin/v0.2.3` (`521d19b9`, `53d9cd70`). Every figure below was derived at
that tree, in the same session that files this brief.

## What is arriving

| file | why it matters here |
|---|---|
| `tests/claimed_counts.rs` | **root `tests/`, which this line may not touch** |
| `keleusma-arena/tests/public_api.rs` | the integration tests the instructions claimed existed |
| `CLAUDE.md` | the arena count, corrected DOWNWARD to the truth |
| `CHANGELOG.md`, `docs/process/DESIGN_JOURNAL.md` | their records |

## Predictions

1. **The merge is clean.** `git merge-tree --write-tree` at the stamp above computes no
   conflict. Their `CLAUDE.md` hunks are the Status paragraph and the Technology Stack
   bullet; this line's edits today were the green-run catalogue table and its summary
   count, which do not overlap.

2. **THE OWNERSHIP CHECK BECOMES NON-VACUOUS, after three absorptions in which it was
   an absent check reading like a pass.** Root `tests/` moves — `tests/claimed_counts.rs`
   — and it moves by THEIR hand. This line's seven commits at this stamp touch **0**
   files under root `src/` and root `tests/`, which is the property the check exists to
   establish and has not been able to test since absorption 65.

3. **`gate-status.sh` will NOT flag this absorption.** None of the five arriving files
   lies in the reach set widened earlier today. **This is a prediction of a NEGATIVE, and
   it is a weakness, not a reassurance**: the widening's most important member is
   `examples/scripts`, the differential's subjects, and this absorption will not exercise
   it. The widening remains untested against another line's work after this lands.

4. **The backend gate stays green and the record's reach is unmoved by the merge
   itself**, because no `native_codegen/` file arrives.

5. **A false claim in this tree becomes true.** This line's `CLAUDE.md` says the arena has
   59 tests, 51 lib plus **8** integration. Their tree says 57, 51 plus **6**, and their
   `public_api.rs` contains **6** `#[test]` functions. The claim of 8 is false in THIS
   tree right now, inherited from the merge-base, and the absorption repairs it. Worth
   stating plainly: this line has been carrying a wrong figure it did not write and did
   not check.

6. **Their new root guard runs in this line's pre-push workspace gate** and should pass.

## The convergence worth noticing

Their commit writes tests the documentation claimed existed, and adds a guard pinning
claimed counts to the tree. This session independently built a guard pinning a gate's
claimed verdict to the tree, and found four wrong figures doing it. **Two lines reached
the same instrument on the same day from opposite ends** — one from documentation that
over-claimed, one from a verdict that was never written down. That is evidence the class
is structural to the project rather than incidental to either line.
