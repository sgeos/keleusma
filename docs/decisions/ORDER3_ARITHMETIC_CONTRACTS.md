# Order 3 legacy arithmetic outcomes

## Scope

This increment closes the remaining integer zero-divisor faults in legacy Div and
Mod during V0.2.3-WIP. BYTECODE_VERSION remains 2, FORMAT_FINGERPRINT remains
0x4327_63E1 and the public instruction count remains 66. Development artifacts
must be rebuilt. No publication is authorized.

## Instruction contracts

Both instructions consume two matching numeric operands and return
[low, high, flag]. The flag is Word. Integer and Byte zero divisors return the
numerator in low, typed zero in high and flag three. Nonzero integer division
uses the same wrapped low result, high result and overflow classification as
CheckedDiv with fraction count zero. Word classification uses the module width.
Byte division and remainder have no nonzero-divisor overflow. Integer remainder
uses the existing CheckedMod result, including the minimum integer modulo minus
one case. Div shares the CheckedDiv zero-fraction implementation. Mod shares the
CheckedMod implementation for Word, Byte and Fixed.

Float Div retains the quotient and CheckedDiv floating-point classification.
Float Mod retains the remainder and classifies that result in the same way.
Its high result is Float zero. Flags are zero for a finite result, one for positive
infinity, two for negative infinity and four for not-a-number. Bare floating-point
source operations discard high and flag, preserving their numeric result.
CheckedMod continues to reject Float operands because its source outcome family
does not admit them. Operand-kind mismatches remain runtime contract errors.

The depth requirement is two and net growth one. Growth is one and shrink zero.
The nominal cost is five, adding two result pushes to the previous estimate of
three. This is not a measured hardware timing bound. Both verifier paths and
resource analyses must use the same effects. No runtime type guard is removed.

## Compiler lowering

Bare Word and Byte operators already use CheckedDiv or CheckedMod followed by a
conditional explicit zero-divisor Trap. That lowering remains valid. Bare Float
legacy instructions now discard two additional outputs. Fixed remainder selects
total Mod and dispatches flag three to the explicit trap. The previous compiler
emitted Mod for Fixed while the runtime rejected that operand kind. Execution
controls must cover both successful and zero-divisor Fixed remainder.

Multiword division and remainder currently use integer Div only to provoke a
zero-divisor fault. Replace that guard with an explicit equality test of the
combined divisor limbs and a conditional ZeroDivisor Trap. Do not silently lose
its source failure when the legacy instruction becomes total.

## Proof boundaries

Non-first-pass indexed value forms still require explicit
availability boundaries in trap analysis. Malformed bytecode and host-contract
failures remain separate from language traps. These arithmetic contracts do not
by themselves establish trap freedom, host load policy or safe check elision.
The full Order 3 completion plan remains authoritative. Raw FixedDiv was in
this class until 2026-10-22, when a zero divisor became the outcome triple of a
Fixed remainder and source Fixed division gained the default zero-divisor Trap.

## Classification audit after the contract change

A source inspection of the virtual machine still finds internal language-fault
returns in non-scalar or boxed GetIndex forms, boxed GetField,
GetTupleField and GetEnumField. These forms cannot be admitted to the first-pass
trap-free profile merely because their module contains no Trap instruction.
FixedDiv was listed here until it became total on 2026-10-22.
The upcoming classifier must distinguish unsupported analysis from a completed
possibly-trapping verdict. Flat scalar accesses retain runtime representation,
operand-kind and lifetime checks even when the language operation is total.

The roadmap explicitly defers native-error and newtype-refinement widening to
Workstream F. Native calls still expose separate host-contract and result-shape
obligations. A first-pass verdict must not turn an unknown native return shape
into a proof for removing its runtime checks. The same applies to values supplied
on initial calls and coroutine resumes. These findings constrain the classifier
and proof-preserving host policy without reducing their required deliverables.
