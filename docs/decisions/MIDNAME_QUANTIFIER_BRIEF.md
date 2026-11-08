# BRIEF — finish the audited classes, and scale the method

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 27 | yes |
| 2 | Audit the mid-name quantifier class, the last defined set | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

Two classes are audited, **3 of 22 overclaimed**. The third and largest defined class — names carrying
a quantifier somewhere other than the front — is **36 of 325**, too many to read one by one at the
care the first two received.

**So the method scales rather than the effort.** A universal claim resting on a body that never
iterates is the shape most likely to overclaim, and that is mechanically detectable. Triaged: **29
iterate, 7 do not.** The seven are the review set.

**Non-iteration is a signal, not a verdict.** A name like *"reports each of its answers"* over a
three-valued classifier is sound with three explicit cases and no loop, and a quantifier scoped inside
the subject — *"a module with no float anywhere"* — is a property of one program rather than a claim
over many. **The triage narrows reading; it does not replace it.**

**Diminishing returns are real and worth stating.** This is the third naming audit. The first found
two defects, the second one. If this class yields nothing, that is itself the signal to stop auditing
names and return to backend questions.

## Prior failures to avoid repeating

1. **A name asserting more than its body proves**, five increments running now.
2. **A rate without its blind spot reads as complete.**
3. **An edit that silently did nothing** because its assertion was omitted.
4. **Renaming a test breaks citations of the old name.** The workspace citation guard caught this
   last increment and blocked the push, correctly. Any rename here must de-cite the old name in the
   documents that mention it.
5. **Sixteen recorded premises found false or overstated.** Predict: at most one of the seven
   overclaims.

## Specific wrong turns to avoid

- **Do not treat "does not iterate" as "overclaims".** The triage is a filter with known false
  positives, and reporting its output as a defect count would be the same error as trusting a proxy.
- **Do not skip the 29 silently.** Say that they were triaged rather than read, so the coverage claim
  is honest about its method.
- **Do not keep auditing if this yields nothing.** Three classes with a falling hit rate is evidence
  the habit is bounded; continuing past that is momentum rather than judgement.
- **Do not rename without checking the citation guard**, which gates the push.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
