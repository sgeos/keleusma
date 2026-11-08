# `docs/proofs/`

Mathematical proofs of properties the implementation relies on but does not check at run time.

**A proof lands here when an optimization or bound would otherwise rest on informal reasoning.**
Keleusma's value proposition is *definitive* WCET and WCMU; a memory-reuse strategy adopted on the
grounds that it "appears safe" is exactly the thing this directory exists to prevent.

## Convention

- **Mathematical notation, not prose argument.** LaTeX inside Markdown.
- **State the model first.** A proof about memory reuse is meaningless without the memory model it
  quantifies over.
- **State what is assumed and what is discharged**, separately. An assumption that turns out false
  later should be findable without re-reading the whole proof.
- **A counterexample is a result.** If a property does not hold, the document records the
  counterexample and the restriction under which it does hold, rather than being deleted.

## Index

| document | status |
|---|---|
| [`COMPOSITE_REGION_REUSE.md`](COMPOSITE_REGION_REUSE.md) | **OBLIGATION STATED, PROOF NOT WRITTEN** |
