# Order 3 completion plan

## Objective

Complete the V0.2.X roadmap's first-pass trap analysis, including total partial
operations, both compiler lowerings, reference and Keleusma verdicts, and a host
no-check mode gated on valid proof. The first pass covers the Word and Byte
arithmetic and bounds classes used by the toolchain. As of the 2026-10-20 handoff
to Opus 5.5, the goal is paused and incomplete. The receiving session should begin
with the audit recorded in docs/process/REVERSE_PROMPT.md. The expectation of
primarily audit work does not waive any acceptance requirement below.

The starting commit is 1cfb6dd9 on feat/v02-order3-trap-analysis. Order 2 is complete.
Its selected typed widening and retained runtime guards remain binding until Order 3
provides evidence sufficient to remove a particular check.

## Current evidence

The baseline census passed three tests and found DivisionByZero and
IndexOutOfBounds without Trap. The expanded census has twenty cases, including
Word, Byte and indexed shared/private storage. The post-change data-read run passes
and finds no source census faults without an explicit Trap. This remains a source-family
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

BoundsCheck now preserves the index and appends four diagnostic byte limbs and an
outcome flag. It performs no indexed data access and returns an invalid-index outcome
instead of a language bounds fault. TrapKind code eight consumes the index and four
limbs, preserving the unsigned 32 bit bound even on an 8 bit Word runtime. Rust
per-dimension lowering emits that explicit trap and retains an exact index source
span. Both verifier implementations check terminal payload availability and loop
frame floors. The [bounds contract](ORDER3_BOUNDS_CONTRACTS.md) records exact effects.

SetDataIndexed now consumes the value and index and returns the portable diagnostic
suffix and flag. Invalid indices perform no write. Both compilers dispatch bare
failures through an explicit bounds Trap and discard successful diagnostics.
Shared and private Word and Byte controls cover failure, success and continued
execution after a handled outcome.

GetIndex now returns a typed scalar value and the diagnostic suffix for flat Word
and Byte reads. Both bare lowerings use explicit traps. Both compilers also support
checked reads of stack-resident scalar arrays, with seven temporaries, Word index
bindings for Byte arrays, guarded and literal patterns, and no fallback trap under
complete invalid_index coverage. Other GetIndex forms retain the legacy contract
and remain outside the first-pass proof.

GetDataIndexed now consumes an explicit Word, Byte or Unit fallback after its index
and returns a value with the portable diagnostic suffix. Both bare lowerings emit
an explicit bounds trap. Both checked scalar data-read lowerings dispatch the
outcome directly. The private record carries the fallback kind above its two
24-bit address lanes. Relocation preserves valid kinds and refuses reserved kinds
and oversized store lengths. The trap classifier and host policy are implemented below. The proof-gated
execution mode is implemented below.

Legacy Div and Mod now return low, high and flag outcomes in hand-built bytecode.
Div shares CheckedDiv with fraction count zero. Mod shares CheckedMod for Word,
Byte and Fixed and retains Float remainder with a floating-point outcome flag.
Both compilers discard outcomes for bare Float operations and dispatch bare Fixed
remainder zero divisors through an explicit trap. Multiword division and remainder
now use an explicit zero-divisor test and trap. The
[arithmetic contract](ORDER3_ARITHMETIC_CONTRACTS.md) records the stack, cost and
remaining proof boundaries. Direct instruction and source execution controls cover
these paths. Classification and host policy are implemented below. Proof-gated
execution and the full completion audit remain required.

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

For checked Word and Byte array reads, record 118 also accepts a scalar-index root
node 31. Seven temporaries follow the operands in flag, value, index and bound-limb
order. Class zero is ok and class one is invalid_index. Only one pattern is allowed.
The ok binding uses the element kind, while invalid_index always binds Word.
Reconstruction checks those kinds and source slots independently. The parser retains
this family in its context form and uses the same guard and coverage protocol.

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
1024, with measured parser usage of 1023 names and 30199 blob bytes. Two unused
private scalar fields were removed before adding the indexed-outcome keyword.
Only one name remains free.

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

## Remaining contracts and analysis

The predicate, indexed-write, flat scalar array-read, indexed data-read and
diagnostic contracts are specified and implemented in
[ORDER3_BOUNDS_CONTRACTS.md](ORDER3_BOUNDS_CONTRACTS.md). Data reads use a stack
fallback, preserving the public base and length operands. The verifier opcode
profile is now 2048 because the expanded stage corpus exceeds 1536. Selected type
capacities remain 640. Selected verification and its limits are recorded in the handoff.

The legacy arithmetic contracts are now total for their numeric operands. The
remaining internal language faults include raw FixedDiv and non-first-pass indexed
or boxed field forms. Classifications must distinguish these unsupported forms
from completed trap-free or possibly-trapping verdicts. The roadmap defers native
error and newtype-refinement widening to Workstream F. Host-contract and unknown
shape checks remain separate obligations.

Matching reference and Keleusma classifications are implemented in
[ORDER3_TRAP_CLASSIFICATION.md](ORDER3_TRAP_CLASSIFICATION.md). Both validate before
scanning the complete module and distinguish unsupported analysis from completed
trap-free and possibly-trapping verdicts. The Keleusma structural stage owns its
profile and Trap decisions through a separate raw-opcode mode. Verification is
recorded in the handoff.

Host policy is implemented in [ORDER3_HOST_POLICY.md](ORDER3_HOST_POLICY.md).
RequireTrapFree rejects possibly-trapping and unavailable analysis before execution
and persists across resume and all replacement routes. The default Allow behavior
is explicit. Borrowed and trust-skip instances can request policy through a common
builder, which requires real verification support. Signed replacement now clears
its internal signature flag only after authentication while retaining host keys.

Proved check elision is implemented in
[ORDER3_PROVEN_EXECUTION.md](ORDER3_PROVEN_EXECUTION.md). A private image-bound
certificate adds a bounded lower-depth proof with actual control-flow and reset
edges. Runtime protocol states preserve its preconditions across entry, suspension,
errors, unwinding and replacement. Actual pop, peek and counted-operand guards are
removed only in that mode. The command-line runner and both validator backends
expose the corresponding policy and proof boundaries.

The full completion audit is next. A public trap verdict alone remains insufficient
for unchecked execution. No new public opcode or version advance is authorized.

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

## Execution proof obligations identified during the arithmetic audit

The current run wrapper preserves frames and operands after an error. For example,
an indexed data read can consume an invalid fallback and return a type error
before consuming the index. A module-level stack-depth proof does not describe
that partially executed frame. Any future unchecked pop path must invalidate its
execution-state proof on error and establish safe reentry behavior before allowing
resume. A fresh call or replacement needs an explicitly justified proof transition.
Breakpoint suspension, yields, resets, normal returns and failed replacement also
need separate controls. If an embedding host catches a panic, reentry must not
reuse a proof from a partially executed frame. Proof invalidation must therefore
cover unwinding as well as returned errors. Unknown operand kinds must retain
their runtime guards.

The implemented operand certificate strengthens the ordinary depth model with
minimum joins, a real Return requirement and reset reentry edges. It binds to the
installed image and instruction cache. Runtime state guards invalidate partial
execution across errors and unwinding. Shape, lifetime, host-result, storage and
other unproved checks remain. Negative controls and memory evidence are recorded
in the proven-execution contract and current handoff. The complete final audit
still remains required.
