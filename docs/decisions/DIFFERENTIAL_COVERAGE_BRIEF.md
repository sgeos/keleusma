# BRIEF — lowered is not executed, and executed is not the same population

## The gap

Every coverage instrument in this package measures **lowering**:
`isa_coverage_census` asks which opcodes nothing has ever lowered;
`spike_corpus_coverage` reports opcode instances lowering; `opcode_denominator`
classifies all 66 and says in its own header that a `Lowered` verdict *"does not
mean the emitted code is correct"*.

**None asks which opcodes are lowered but never differentially EXECUTED.** That is
where `Fixed % Fixed` hid: it lowered without refusal, in an ordinary-looking
module, and was wrong only when something ran both implementations and compared.

## The measurement

The corpus differential classifies 74 modules as **63 executed and agreeing, 1
agreed-but-vacuous, 10 exempt**. Intersecting that classification with per-module
opcode sets gives:

**Four opcodes have no executed witness in the corpus: `BitAnd`, `BitOr`,
`BitXor`, `Shr`.** All four appear only in `wire.kel`, which the virtual machine
refuses to resume (`IndexOutOfBounds`), so it is compared by the FAULT observable
rather than by a result.

My module counts (63 executed, 11 not) match the differential's own report
exactly, which is what licenses the exclusion list.

## They are covered — elsewhere — and that must be CHECKED

`scalar_operator_matrix.rs` drives `word band`, `word bor`, `word bxor` and
`word asr`, and those compile to exactly `BitAnd`, `BitOr`, `BitXor` and `Shr`.
So the evidence exists; it simply is not in the corpus.

**"Covered elsewhere" is precisely the kind of claim this session has repeatedly
found to be false**, so the instrument must not assert it. It must compile the
named witness, assert that witness emits the opcode, and assert that the witness
is actually present in the file said to drive it. A three-link chain, each link
checked.

This also revises what the matrix is for. It was built to find type-dispatch
defects. It turns out to be the **sole** differential evidence for four opcodes,
which raises the cost of ever trimming it.

## Wrong turns to avoid

- **Reimplementing the differential's exemption logic.** The exclusion list is
  transcribed from its own report and validated by the module counts matching. A
  private copy of a canonical walk already cost this project once, reporting five
  scripts as compiler failures when the harness was the cause.
- **Claiming an executed module executes all its opcodes.** It does not — a branch
  may never be taken. The claim here is the weaker, checkable one: an opcode
  appearing only in never-executed modules has no differential evidence from the
  corpus at all.
- **Treating `fault-compared` as no evidence.** A fault IS an observable, and the
  differential compares it. The four orphans are not unevidenced because
  `wire.kel` faults; they are unevidenced by RESULT comparison.
- **Letting the orphan list be patched rather than explained.** A new orphan means
  either the corpus lost a witness or an opcode arrived; both deserve a sentence.
