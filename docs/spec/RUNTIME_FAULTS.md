# Runtime Faults and the Native Code Generation Contract

This document specifies how Keleusma handles the operations that are mathematically partial, namely operations that are undefined on some inputs. It states the two-backend contract that governs every partial operation, the per-operation behavior on the verifying virtual machine, and the contract that a future native backend must honor. It is authoritative for the B35 Partial Operation Handling design.

## Scope and status

The partial operations are integer and fixed-point division and modulo on a zero divisor, array indexing out of bounds, refinement-newtype construction whose predicate fails, the discriminant-to-enum conversion on an invalid discriminant, and fallible native function failures. Floating-point operations are total under the Institute of Electrical and Electronics Engineers 754 standard and produce signed infinities and not-a-number values rather than faulting.

The virtual-machine side of this contract is implemented. B35 phases P1 through P7 deliver the specific trap variants, the canonical zero value and lowest-valid resolution, and the six source-level handling constructs. The native side of this contract, namely the inserted guards and the platform-specific default values produced during native code generation, is deferred to V0.4.0, where native code generation via the Low Level Virtual Machine is introduced. See [`../roadmap/V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md). This document specifies that native contract in full so the V0.4.0 backend has a complete, reviewable target before any code generation is written.

## The two-backend contract

Every partial operation has a defined contract on both execution backends, and the two backends intentionally diverge on an unhandled partial operation.

The virtual machine is the safe reference interpreter. It traps on any unhandled partial operation. A trap is a recoverable error returned to the host, not a process abort, consistent with the existing treatment of arena exhaustion. The host categorizes the fault through the structured `VmError` variant.

Native code is the as-fast-as-hardware deployment target. It produces a defined, non-crashing value for any unhandled partial operation. It uses the hardware result where the hardware does not fault, and an inserted guard that yields a defined value where the hardware would fault. The unhandled native value is therefore platform-specific, so handling the outcome arm is what makes a program portable in value as well as in behavior.

Verification on the virtual machine does not by itself establish the values a native build produces for unhandled partial operations. A program that must produce identical results on both backends handles every partial outcome at the source level through the construct family, which removes the divergence.

## Source-level handling removes the divergence

Each partial operation has an opt-in source-level construct, a match block over the fallible operation distinguished by a fixed vocabulary of outcome-arm keywords. When a program handles an outcome, both backends run the handler and neither the trap nor the native default occurs. The constructs are specified in [`GRAMMAR.md`](./GRAMMAR.md).

| Operation | Construct | Outcome arms | Unhandled VM behavior |
|-----------|-----------|--------------|------------------------|
| Division, modulo | checked arithmetic | `ok`, `overflow` where it can arise, `zero_divisor` | trap |
| Add, subtract, multiply, negate | checked arithmetic | `ok`, `overflow`, `underflow` where each can arise | wrap (two's complement) |
| Array indexing | indexing | `ok`, `invalid_index` | trap |
| Newtype construction | newtype construction | `ok`, `invalid_newtype` | trap |
| Discriminant-to-enum | discriminant conversion | `ok`, `payload_discriminant`, `invalid_discriminant` | trap |
| Native call | native error | `ok`, `error` | propagate the host failure |
| `for ... limit` overrun | loop outcome block | `ok`, `break`, `limit`; `overflow` is inadmissible | trap |

Overflow and underflow on the wrapping arithmetic operators default to two's-complement wrapping rather than a trap, so an arithmetic construct that omits those arms is equivalent to the bare wrapping operation. Every other unhandled partial outcome traps on the virtual machine.

## Virtual-machine trap variants

The virtual machine surfaces each unhandled partial operation through a specific `VmError` variant so the host's error-category mechanism can map outcomes to policy without parsing a message string. These are implemented (B35 P1).

| Operation | `VmError` variant |
|-----------|-------------------|
| Division or modulo by zero, unhandled | `DivisionByZero` |
| Array index out of bounds, unhandled | `IndexOutOfBounds(index, length)` |
| Newtype refinement predicate failed, unhandled | `RefinementFailed` |
| Discriminant matches no variant, unhandled | `EnumVariantUnmapped` |
| Native failure with a reported code, unhandled | `NativeErrorCode { code, message }` |
| Native failure with a message only, unhandled | `NativeError(message)` |
| Debug `assert` condition false (debug build only) | `AssertionFailed` |
| `for ... limit` reached the cap before the range end, unhandled | `LoopLimitExceeded` |

The compiler-emitted traps for partial operations whose unhandled case has no in-band result are encoded through the `TrapKind` encoding, distinct from the data faults above that already carry their own variant. The `assert` trap is also a `TrapKind` (`AssertionFailed`), but it is reachable only in debug builds, which compile the assert check in; a release build compiles the check out entirely. The failing assertion's source span and message, when present, ride in the strippable `AssertionContext` debug record (B29), not in the `VmError`. Restoring the dropped dynamic detail, namely the failing predicate, newtype, or function name, is tracked under the debug-information work and is independent of this contract.

## Native default values

Where a program does not handle a partial outcome, native code produces the following defined value rather than trapping. This table is the normative native contract.

| Operation | Native default where the hardware does not fault | Native default where the hardware faults |
|-----------|---------------------------------------------------|-------------------------------------------|
| Division by zero | hardware result | zero |
| Modulo by zero | hardware result | the numerator |
| Out-of-bounds index | not applicable | the element type's zero-or-lowest-valid value |
| Newtype predicate failure | not applicable | the lowest-valid value, per the precedence below |
| Discriminant-to-enum, invalid | not applicable | the zero-discriminant variant, or the lowest-valid variant when zero is not a discriminant |
| Native error | not applicable | trap, since a host failure has no safe default |

Modulo by zero defaults to the numerator rather than to zero, which matches the Reduced Instruction Set Computer Five remainder convention and the value the Advanced Reduced Instruction Set Computer Machine derives, so it is closer to portable than zero would be. A native error is the single partial operation whose unhandled outcome traps on both backends, because a host failure has no value that is safe to fabricate, consistent with the rule that an operation receives a defined non-trapping default only when a total result exists.

## Hardware basis

The native defaults above follow from how each target's hardware treats the partial operation. A native backend consults this basis when deciding whether to emit the bare hardware instruction or an inserted guard.

| Target family | Integer division or modulo by zero |
|---------------|-------------------------------------|
| Intel x86 and x86-64 | Raises a divide-error fault. Native code inserts a guard to avoid the fault and supplies the default. |
| Advanced Reduced Instruction Set Computer Machine | Returns zero for the quotient and does not fault. |
| Reduced Instruction Set Computer Five | Returns all-ones for the quotient and the dividend for the remainder, and does not fault. |
| Mostek 6502 | Has no divide instruction. Division is a software routine, so the routine defines the result and there is no hardware fault. The 6502 also has no arithmetic-fault mechanism and no memory protection, so a trap there can only be a compiler-emitted software check. |
| Institute of Electrical and Electronics Engineers 754 floating point | Already defines non-trapping results, namely signed infinity, not-a-number, and a defined zero-divisor result. The arms intercept those special results rather than avert a fault. |

Out-of-bounds indexing, newtype-predicate failure, and the invalid discriminant have no hardware-fault analog. They are software conditions whose default value comes from the canonical zero value or the lowest-valid resolution below, guarded unconditionally by the native backend.

## The canonical zero value

A single zero value is defined once for every type and supplies the out-of-bounds, newtype, and conversion native defaults. It is implemented (B35 P2) by `zero_value` in `src/zero_value.rs`.

| Type | Canonical zero |
|------|----------------|
| `Word` | `0` |
| `Byte` | `0` |
| `Float` | `0.0` |
| `Fixed<N>` | `0` |
| `Bool` | `false` |
| `Text` | the empty string |
| Tuple or struct | each field's canonical zero |
| Enum | the zero-discriminant variant, or the lowest-discriminant variant when zero is not present |
| Refined newtype | the lowest-valid value below |

## The lowest-valid precedence

Several native defaults need the lowest valid value of a refined type. It is resolved in this order, implemented (B35 P2) by `lowest_valid` in `src/zero_value.rs`.

1. The value declared by the newtype's `with saturate_min` clause, which is predicate-checked.
2. The lowest valid value computed by the interval and lattice analysis, when the valid set is analyzable. This tier is integer-domain. A refined newtype over `Float` falls through to the next tier.
3. Where neither exists, the virtual machine traps and native code uses a hard zero even where it violates the predicate, because a bare-metal target has no recovery context and no better option.

## What V0.4.0 native code generation must do

The native backend lowers each partial operation to the bare hardware instruction where the hardware does not fault, and to a guarded sequence where it would. The guarded sequence tests the partial condition, branches to the hardware instruction on the safe path, and produces the default value above on the unsafe path. Where the source handles the outcome through a construct arm, the backend lowers the arm body in place of the default and the guard branches into it.

The worst-case execution time and worst-case memory usage analyses bound the guarded sequence the same way they bound the virtual-machine construct, namely a small fixed number of operations on the longest path, so the native guards do not invalidate the bounds the verifier proves. The values produced differ between backends only for unhandled partial operations, which a fully handled program does not have.

This native lowering is deferred to V0.4.0 and is not implemented in the bytecode runtime.
