# BRIEF — witness the unexercised arms that ordinary source can reach

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## What this is, and what it deliberately is not

The kind-arm census left **twenty-six (family, kind) combinations unexercised**. They are not one
kind of thing, and treating them as one is the error to avoid:

| group | what it is | what to do |
|---|---|---|
| `Unit`, `Text`, `Opaque` | **refused** by the lowering | leave recorded; a contrived witness would be worse than an honest gap |
| `Byte`, `Bool`, `Fixed` in the read families | **accepted**, reachable from ordinary source, driven by nothing | **witness them** |

**The second group is the dangerous class this line keeps rediscovering.** A refusal is loud. An
accepted path no test executes ships a plausible wrong number, and the backend's whole correctness
argument is a differential worth exactly as much as the paths it drives.

**The census brief warned against a coverage-closing spree.** That warning was about doing this
INSIDE the census, where it would have buried which gaps were real. As a separate increment it is
the finding acted on.

## Why these arms are not cosmetic

The narrow arm ZERO-extends, and the tree already records what happens when that is wrong:
*changing the narrow load from zero-extension to sign-extension left every other test passing.* The
`Fixed` arm reads eight raw Q-format bytes where the bits ARE the value, with no rescale — a
lowering that zero-extended or masked would be wrong in a way no uniform-word composite reveals.

## Prior failures to avoid repeating

- **Pick values that discriminate.** A byte below 128 agrees under zero- and sign-extension and
  proves nothing. Use values above 127. For `Fixed`, use a value whose bit pattern differs from the
  integer reading. This package has produced three vacuous tests by ignoring exactly this.
- **Confirm each new test can FAIL.** A witness that passes against a broken lowering is worse than
  no witness, because it converts a gap into a false claim of coverage. Mutate and show the changed
  line.
- **Attribute in the census table as each lands**, or the table becomes stale in the direction it
  cannot self-check: it fails when the CORPUS starts reaching an attributed row, but a newly covered
  row needs a human.
- **Do not manufacture a witness for a refused kind.** If a combination cannot be reached without
  contortion, record it as unexercised and say why. A test that exists to move a number is the thing
  this whole census was built to detect.
- **Check the binary count, not just the pass count.** A SIGTERM produced a plausible
  "398 passed, 0 failed" this session and only the short binary count betrayed it.
- **Do not name a test in a comment before writing it.** The citation guard caught that twice this
  session, both times in prose about my own work.

## The wrong turn most likely here

**Reporting the residue as closed when only the easy half moved.** The deliverable includes the
UNCLOSED remainder, named, with the reason each stays open.

## Outcome, written after the measurement

**The reachable-and-accepted group is TWO combinations, not the eight or so the split implied.**
`probe_float_composite::which_narrow_and_fixed_composite_reads_are_reachable_from_source` asked the
reference compiler and the backend rather than reasoning about them:

| shape | result |
|---|---|
| `Byte` tuple member | **lowers** — witnessed |
| `Byte` enum payload | **lowers** — witnessed |
| `bool` struct field | compiles, **backend REFUSES** |
| `bool` array element | compiles, **backend REFUSES** |
| `Fixed` tuple member | compiles, **backend REFUSES** |
| `Fixed` array element | compiles, **backend REFUSES** |

**AND THE REFUSAL HAS A DIFFERENT CAUSE THAN THIS WHOLE CENSUS WAS ABOUT.** All four are refused by
`NewComposite` reporting *an operand of unknown packed width* — a CONSTRUCTION-side gap in operand
width tracking, not a kind arm on the read side. They never reach a read arm at all. That is a loud
refusal in the safe direction, and it is now recorded rather than counted as an unexercised read arm,
which is what the raw census number invited.

### Two of my six probe sources were rejected by the reference compiler, and that was my instrument

`Bool` is spelled `bool`, and `1.5 as Fixed<16>` is not the surface form. **The first run reported
"REFERENCE COMPILER REFUSES THIS SOURCE" for four shapes and two of those were my own syntax
errors.** Had the probe been trusted as written, this document would have recorded four language
limitations that do not exist. *Check the instrument* applies to the source strings a probe is built
from, not only to its counting.

### Evidence

`a_byte_tuple_member_zero_extends_like_the_vm` and `a_byte_enum_payload_zero_extends_like_the_vm`,
both using 200, where sign-extension reads −56 and anything under 128 proves nothing. The enum case
matters separately because its offset is measured PAST the discriminant word, so an error there is an
offset error rather than an extension error.

**Two mutations, each confirmed applied by printing the changed line.** Sign-extending the struct
field read fails the tuple, enum and pre-existing struct witnesses; sign-extending the array element
read fails the array witness. The two extension sites are distinct and each now has a witness that
dies with it.

### The remainder, named rather than implied closed

**Twenty-four combinations stay unexercised.** `Unit`, `Text` and `Opaque` are refused kinds across
all five families, where a contrived witness would be worse than an honest gap. The `bool` and
`Fixed` composite cases are blocked by the construction-side width gap above, which is a separate
piece of work with its own cause. **Nothing here closes them and no document says otherwise.**
