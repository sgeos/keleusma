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
