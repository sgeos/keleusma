# Order 3 completion plan

## Objective

Complete the V0.2.X roadmap's first-pass trap analysis, including total partial
operations, both compiler lowerings, reference and Keleusma verdicts, and a host
no-check mode gated on valid proof. The first pass covers the Word and Byte
arithmetic and bounds classes used by the toolchain. The goal remains active.

The starting commit is 1cfb6dd9 on feat/v02-order3-trap-analysis. Order 2 is complete.
Its selected typed widening and retained runtime guards remain binding until Order 3
provides evidence sufficient to remove a particular check.

## Current evidence

The baseline census passed three tests and found DivisionByZero and
IndexOutOfBounds without Trap. The expanded census has seventeen cases, including
Word, Byte and indexed shared/private storage. The focused post-change run passes
and finds only bounds faults without an explicit Trap. This remains a source-family
census, not a proof about arbitrary bytecode or host-contract failures.

Both bare Word/Byte division and modulo lowerings now reuse CheckedDiv or CheckedMod
and branch to an explicit ZeroDivisor Trap for flag three. Overflow keeps the
wrapping low result. Seven focused lowering tests, three census tests and eight tag
controls pass. A narrow-module test exposed CheckedDiv classifying against the host
width. It now uses the module-declared width through the shared arithmetic helper.

The Rust compiler omits the zero-divisor fallback for nondivision arithmetic and
unguarded variable or wildcard handlers covering that class. Guards and literal
patterns retain a possible trap. Keleusma source parsing now emits the explicit
checked-outcome protocol for Word and Byte arithmetic, including Word negation.
Source controls cover bindings, guards, literal patterns, nested expressions and
independent source rejection. Checked arithmetic left shifts now retain their
source distinction from logical shifts and lower through CheckedMul with a proved
positive constant multiplier on the supported 64 bit host path.
Debugger controls pass for the arithmetic operation and its explicit trap.

A full regression exposed lost Float division/remainder and Fixed remainder
acceptance. The parser now carries the left scalar kind through a high lane of the
binary-node argument. Code generation preserves those legacy paths. Word and Byte
use the new explicit-trap lowering. Nested noninteger expressions, locals and calls
have artifact controls. Public opcode identifiers remain unchanged.

BoundsCheck currently leaves the stack unchanged and faults on an invalid index.
GetIndex, GetDataIndexed and SetDataIndexed also fault internally on bounds errors.
Total bounds outcomes therefore require an instruction-contract change. The current
TrapKind table has no indexed-bounds trap kind. The checked-index fallback still
reissues a trapping GetIndex. No trap-free verdict or no-check mode is implemented.

Legacy Div and Mod still fault on integer zero divisors in hand-built bytecode.
The direct instruction-contract tests retain that negative control. A scan-based
verdict must address these contracts as well as compiler output before admitting
arbitrary verified modules as trap-free.

## Operator decision recorded on 2026-10-13

The operator explicitly declined advancing BYTECODE_VERSION. It remains 2.
FORMAT_FINGERPRINT remains 0x4327_63E1 throughout V0.2.3-WIP and changes only
when publishing a new release. V0.2.2 is the last published version. Its tagged
source declares bytecode version 1. The already-authorized version 2 substrate
belongs to the unpublished V0.2.3 development cycle.

The fingerprint already occupies a reserved auxiliary-header word. Its release
boundary and deliberate lack of within-cycle drift detection are documented in
src/bytecode.rs and docs/process/RELEASE_PROCESS.md. Contract changes within this
work-in-progress cycle must neither advance the version nor roll that fingerprint.
Keep the instruction count at 66 by reusing existing operations. No publication
is authorized by this work.

The intended first-pass contract changes are as follows.

| Surface | Required behavior |
|---|---|
| Bounds predicate | BoundsCheck produces an in-band outcome flag and preserves the index for dispatch. It cannot raise a language bounds fault. |
| Indexed reads | An invalid index produces a deterministic scalar fallback and an outcome flag, without an invalid memory access. Word and Byte results retain their kinds. |
| Indexed writes | An invalid index performs no write and produces an outcome flag. |
| Unhandled bounds | Both compilers emit an explicit indexed-bounds Trap with preserved source diagnostics. |
| Handled bounds | Both compilers branch on the outcome and omit the fallback trap only when handler coverage is syntactically complete. |
| Compatibility | Version and fingerprint remain fixed during V0.2.3-WIP. Development artifacts must be rebuilt after contract changes. The fingerprint changes only at the next publication boundary. |

Exact stack layouts, diagnostic payloads and effects on analysis tables must be
specified and checked before implementation of these contracts. Compound and other
full-language value forms must have explicit availability boundaries rather than
being silently treated as trap-free.

The ruling supersedes the earlier version 3 proposal. No version decision remains
pending. The broad Order 3 objective authorizes implementation under these limits.

## Required deliverables and acceptance evidence

| Requirement | Evidence required |
|---|---|
| Complete first-pass fault inventory | Executed Word and Byte cases for bare and handled division, modulo, local-array bounds, indexed shared/private reads and writes, nested dimensions, negative indices and exact bounds. Valid controls are mandatory. |
| Total first-pass primitive contracts | Opcode-level positive and negative execution tests show outcomes instead of internal language faults. Memory access is valid on every outcome. Opcode count remains 66. |
| Compiler lowering | Rust and Keleusma outputs are byte-identical on the current stage corpus and focused cases. Bare failures reach explicit Trap. Exhaustive handlers contain no unreachable fallback trap. Refutable or guarded handlers remain possibly trapping. |
| Reference and Keleusma verdicts | Both report matching completed classifications for trapping and non-trapping corpora. Unsupported or incomplete analysis remains distinct. A stage-decision intervention proves that Keleusma determines its verdict. |
| Host load policy | A host can require trap-free input. Rejection occurs before subject execution. Existing trap-permitting behavior remains an explicit policy. Load, resume and replacement paths preserve the policy. |
| No-check execution | Identify the exact checks removed and the proof for each. Compare checked and no-check execution on valid inputs. Negative controls must prevent unproved check elision. The mode must do actual supported work. |
| Safety and trust boundary | Trap absence alone does not justify removing malformed-bytecode, unknown-shape or host-contract guards. Order 2's Top widening is not a proof of concrete operand types. Required validation or a stronger bounded certificate must precede any relevant elision. |
| Compatibility and resource analysis | Stack effects, abstract interpretation, resource costs, wire codecs, disassembly, documentation and version checks agree with every changed contract. Version 2 and the current fingerprint remain unchanged throughout WIP. |
| Verification and audit | All five feature configurations, current-stage artifacts, detached compiler, shipping path, compatibility checks and relevant memory-safety checks pass on a fixed tree. Record an explicit completion audit and scoped commit. |

## Implementation order

1. Extend the executed fault inventory and implement compatible Word and Byte bare
   division/modulo lowering in both compilers, preserving diagnostics and wrapping.
2. Audit exhaustive checked-outcome lowering, including guarded and refutable arms.
3. Specify and implement total bounds contracts and explicit bounds traps within
   V0.2.3-WIP, with all instruction consumers updated together and both format
   identifiers unchanged.
4. Add reference and Keleusma trap classifications and proof-preserving host policies.
5. Establish the stronger safety preconditions for real no-check execution and test
   each removed check with a changing control.
6. Complete the full corpus, feature and compatibility gates and the completion audit.

A blacklist of existing partial opcodes is not the requested total-opcode design.
A zero-Trap scan of current version 2 bytecode must not be advertised as trap freedom.
An unsafe host promise or a flag which removes no checks cannot stand in for the
required no-check proof and execution path.

## Additional parser finding

The following executed source exposes an expression-order defect.

```keleusma
fn main(op: Word, scalar: Word) -> Word {
    let legacy = scalar == 5 orelse (scalar == 4 andalso op == 5);
    3 + (op + if legacy { scalar * 64 } else { 0 }) * 64
}
```
The original parser emitted addition before the conditional, and the shipping type
stage rejected the resulting facts. The current parser adds an internal IfMark
boundary and retains enclosing operators until the complete conditional is emitted.
Artifact and execution controls cover arithmetic, negation, nested conditions, call
arguments, branch statements, Boolean expressions, side effects and an explicit
zero-divisor trap. The parser helper retains its local-binding formulation. The diagnostic and whole-source comparison are retained in
`tmp/order3-minimal-diagnosis.log` and `tmp/order3-parse-diagnosis.log`. The latter's
unfused parsing attempts exceeded that helper's token cap and are not evidence
about the shipping fused path. Its type-stage comparisons use the fused path.

## Checked-outcome record protocol

This is implemented in parse.kel, reconstruct.kel and codegen.kel for the arithmetic
families described above. The unit tests in src/selfhost/checked_outcomes_tests.rs
inject records directly and compare instructions, constants, frame size, complete
artifacts and execution with reference-compiled checked expressions. Separate
source controls in tests/selfhost_checked_outcomes.rs and
 tests/selfhost_checked_patterns.rs establish source admission and rejection.

| Record | Meaning |
|---|---|
| 118 | Open after the normal operation node. Payload is flag-slot plus scalar-kind times 65536. Scalar kind is 3 for Word or 2 for Byte. Reserve flag, low and high slots in that order. |
| 125 | Typed low or high local read for a pattern binding. Payload is source-slot plus scalar-kind times 65536. |
| 119 | Wrap the arm body and optional guard. Payload is class plus guard-present times 4 plus literal-pattern flags times 8 and 16. Class values are 0 ok, 1 overflow, 2 underflow and 3 zero divisor. |
| 120 and 121 | Supply the first and second literal-pattern values when their flags are set. |
| 126 and 127 | Supply the nonnegative bitwise complement of a negative first or second literal. This avoids the legacy transport sentinel at negative one. |
| 128 | Supply the nonnegative complement of a signed ordinary literal, reconstructed as a literal node. Negative complements are malformed. |
| 5 | Existing LetIn wrappers bind variable patterns after record 119, in reverse pattern order. Source parsing must also emit binding-name metadata record 90 immediately before each wrapper. |
| 123 | Finish an arm after its binding wrappers and literal records. Payload is zero. |
| 124 | Close the checked construct. Payload is zero. An unguarded ok catch-all is mandatory. |

Reconstruction produces checked-expression node 77, arm-result node 78 and typed
binding-value node 80. Its four-word header carries packed slots/type, the reverse
arm list, arm count and covered classes. Each five-word arm descriptor carries
metadata, two literal values, the wrapped result node and the preceding-arm link.
Guard and literal flags determine coverage inside the stage. The host does not
supply a trap-free claim. Header and arm storage share the existing 256-word side
table. An otherwise empty table fits fifty arms and refuses the fifty-first.
Checked contexts have a guarded depth of 32. Existing unrelated unguarded side
arrays remain explicitly registered.

Code generation schedules the reverse list onto its work stack to preserve source
order. Deferred integers share immutable scratch slots so repeated outcome flags
do not exhaust the 64-entry table. Actual constant-pool interning remains deferred
until emission. No capacity was raised.

The source parser reserves three temporaries after operands and independently
checks class admissibility, arity, unreachable arms and unguarded ok coverage.
A checked marker protects enclosing expression operators, and a bounded context
stack accepts 32 and refuses 33 before writing. Guards, binding scope and nested
expression boundaries have source controls. The source type channel recognizes
node 80 scalar types, node 78 guards and node 77 arm-result joins. Checked shifts
have source and record-level controls. Reconstruction independently requires Word
operands and a literal amount from zero through 62 on the supported 64 bit host
path. The multiplier is interned before the left operand, preserving the reference
constant-pool order. All original Order 3 deliverables remain required.

Two source controls exposed defects subsequently repaired. Negative one collided
with the parser transport sentinel, so records 126 and 127 now carry complements.
Byte bindings lost their scalar classification during local lookup, so lookup now
uses the recorded scalar kind. Reconstruction also rejects reserved record 122.

The expanded parser exceeded the lexer source buffer. Its capacity is now 524288
bytes, with a driver guard before copying source and aligned detached-harness
checks. The source-cap boundary has positive and negative controls. The separate
operator-selected type capacities remain 640. The module-input name cap remains
1024, with measured parser usage of exactly 1024 names and 30050 blob bytes.
The name pool has no remaining headroom.

The observed packed-token defect is repaired. Large integer values use lexer token
65 and a shared value table keyed by the interned spelling. The parser receives the
stage-computed values through a matching table. Both tables retain the existing
1280-entry interner bound. The host transports values without parsing their source
text. Integer accumulation checks its range before multiplication, and token 66
reports overflow or a radix prefix without digits. Ordinary literals use full-word
records, with record 128 avoiding the negative-one transport sentinel.

Source controls exercise decimal, hexadecimal and binary limits, end of input,
positive and negative checked patterns, constant fields, a string spelling collision,
collected and fused feeds, and the shared interner capacity. The detached lexer and
parser commands accept the maximum integer and reject overflow. Constant-field
parsing now retains its leading minus sign. Bare Byte shifts after a cast also retain
the scalar kind. The initial failures and final receipts are in REVERSE_PROMPT.md.

## Bounds contract work next

Define the exact stack effects and diagnostic payload before changing the four
bounds operations. Current GetDataIndexed operands carry base and length, without
an element kind. Its invalid-index fallback therefore needs an explicit rule for
recovering Word or Byte kind, including the zero-length case. Current TrapKind codes
zero through seven are occupied. Numeric index and length diagnostics must account
for narrow runtime words, alongside preserved source fault locations. These are
implementation constraints found by source inspection, not completed contracts.

The required work remains total bounds operations, explicit bounds traps, both
compiler lowerings, matching classifications, host policy, proved check elision and
the full completion audit. No trap-free or no-check claim follows from this
arithmetic frontend increment.

## Debug verification recovery

The pre-push debug suite exposed stack overflow in the reference parser used to
bootstrap stages. Primary-expression helper extraction repairs the observed paths
while preserving the depth limit. Fixed two-mebibyte tests cover every stage and
nine nesting shapes at both sides of the existing boundary. The debug quick suite
reported no remaining stack aborts. Its single command-timeout failure was followed
by a successful measured invocation and a finite debug-only deadline adjustment.
All four command tests then passed. The full quick suite was not rerun after that
test-only adjustment. This recovery enables further verification and does not
complete any of the outstanding trap-analysis deliverables.
