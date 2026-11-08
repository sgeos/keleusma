# BRIEF — I under-counted the class by more than half, one commit after writing the rule against it

## What happened

Last increment I audited the virtual machine's mis-compilation refusals — the ones whose message
says an opcode **should never have been emitted** — and recorded **three sites, two opcodes**. The
extraction filtered on the message phrase `is a compile-time constant`.

**There are seven.** The four I missed say the same thing in different words:

```
GetField      operand form does not match struct body
GetIndex      operand form does not match array body
GetTupleField operand form does not match tuple body
GetEnumField  operand form does not match enum body
```

Their comment reads *"a form mismatch is a corrupted or **mis-compiled** artefact rather than a
script error"* — the same class, stated plainly.

## Why I missed them, and it is the rule I had just written

That same commit records:

> *A machine-checked marker written in a form prose can take will eventually match prose.*

I fixed the **syntactic** half — requiring the marker at line start so a comment could not
impersonate a message. **I did not consider the semantic half.** My grep for the concept searched
`mis-compilation` and the four missed sites say `mis-compiled`. One word stem apart.

**Fifth instance of the marker family, and the first where I wrote the rule and violated it in the
same file.** The lesson generalises further than I put it: a marker can be too loose *and* too tight,
and I only guarded one direction.

## What the correction has to be, and what it must not be

**Not a wider grep.** Replacing one hand-chosen phrase with a better hand-chosen phrase repeats the
method that just failed. The next site will be phrased a sixth way.

The honest structure separates what is measured from what is editorial:

- **MEASURED, and it is the claim that matters**: extract EVERY `InvalidBytecode` raise site's
  message, and check whether any real program reaches any of them. That needs no classification at
  all.
- **EDITORIAL, and labelled as such**: which of those are "mis-compilation" rather than
  "corrupt-artefact" guards. Say the classification is a reading, print the full list so it can be
  audited, and stop presenting a comment-derived count as a measurement.

**The reachability result survives the correction and should be stated first**: `Op::IsStruct` is
reachable, the two `Op::Len` sites are not reached and the mechanism is the bound coupling. Adding
four sites to the class does not change any of that — it changes the denominator, which is exactly
why the denominator should not have been asserted from a grep.

## Specific wrong turns to avoid

- **Do not quietly amend "three" to "seven".** The commit that said three is pushed; the correction
  must say what was wrong and why, or the next reader inherits a number with no history.
- **Do not claim the four new sites are reachable or unreachable without measuring.** They require a
  form mismatch between construction and access, which the compiler chooses by static type. That
  suggests unreachability, and suggestion is not evidence — this line has been wrong that way before.
- **Do not treat "no corpus module reaches one" as "unreachable".** Sixty-six programs is a bounded
  search, and the `Op::IsStruct` counter-example came from a construct outside the corpus.
- **`src/vm.rs` is the `v0.2.3` line's.** Read, never written.

## What a good outcome looks like

The full `InvalidBytecode` surface is listed from source; the reachability claim is stated over a
named population; the mis-compilation subset is presented as a reading with its stem shown; and the
tree records that the earlier count was wrong, by how much, and why.

**The count being wrong is the more useful half of this increment.** A measured reachability result
survives a bad denominator; a denominator asserted from a grep does not survive contact with a
synonym.
