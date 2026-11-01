# Runtime Faults and the Native Code Generation Contract

This document specifies how Keleusma handles the operations that are mathematically partial, namely operations that are undefined on some inputs. It states the two-backend contract that governs every partial operation, the per-operation behavior on the verifying virtual machine, and the contract that a future native backend must honor. It is authoritative for the B35 Partial Operation Handling design.

## Scope and status

The partial operations are integer and fixed-point division and modulo on a zero divisor, array indexing out of bounds, refinement-newtype construction whose predicate fails, the discriminant-to-enum conversion on an invalid discriminant, and fallible native function failures. Floating-point operations are total under the Institute of Electrical and Electronics Engineers 754 standard and produce signed infinities and not-a-number values rather than faulting.

The contract is implemented. B35 phases P1 through P7 delivered the specific trap variants, the canonical zero value and lowest-valid resolution, and the six source-level handling constructs. Since the operator's ruling of 2026-10-20, recorded in [`../decisions/ORDER3_HANDLER_SURFACE_PLAN.md`](../decisions/ORDER3_HANDLER_SURFACE_PLAN.md), only the `Trap` opcode traps, and the compiler emits that `Trap` itself when a program writes no handler. Native code generation follows the same `Trap`, so the two backends no longer diverge on an unhandled partial operation. This revision, made on 2026-10-28 at the operator's request, replaces the earlier contract under which native code produced platform-specific default values.

## The two-backend contract

Every partial operation has the same contract on both execution backends.

A partial operation is compiled as its checked form, which leaves the result and an outcome flag, followed by a test of the flag. Where the program handles the outcome, the test branches into the handler. Where it does not, the compiler emits a `Trap` carrying the outcome's `TrapKind`, so the bare division `a / b` compiles to `CheckedDiv`, a comparison of the flag with the zero-divisor outcome, and `Trap(5)` under it. Only that `Trap` opcode traps.

On the virtual machine a trap is a recoverable error returned to the host, not a process abort, consistent with the existing treatment of arena exhaustion. The host categorizes the fault through the structured `VmError` variant. Native code follows the bytecode's `Trap` rather than fabricating a value, so a program's behavior on an unhandled partial operation does not depend on the backend. Keleusma rejects undefined behavior but not platform-specific behavior, and the one platform-specific result that remains is what the hardware's own instruction gives on a path where the program has handled the outcome itself.

## Source-level handling removes the divergence

Each partial operation has an opt-in source-level construct, a match block over the fallible operation distinguished by a fixed vocabulary of outcome-arm keywords. When a program handles an outcome, both backends run the handler and no trap occurs. The constructs are specified in [`GRAMMAR.md`](./GRAMMAR.md).

| Operation | Construct | Outcome arms | Unhandled behavior, both backends |
|-----------|-----------|--------------|------------------------|
| Division, modulo | checked arithmetic | `ok`, `overflow` where it can arise, `zero_divisor` | trap |
| Add, subtract, multiply, negate | checked arithmetic | `ok`, `overflow`, `underflow` where each can arise | wrap (two's complement) |
| Array indexing | indexing | `ok`, `invalid_index` | trap |
| Newtype construction | newtype construction | `ok`, `invalid_newtype` | trap |
| Discriminant-to-enum | discriminant conversion | `ok`, `payload_discriminant`, `invalid_discriminant` | trap |
| Native call | native error | `ok`, `host_error` or its alias `error` | trap with the host's own error |
| `for ... limit` overrun | loop outcome block | `ok`, `break`, `limit`; `overflow` is inadmissible | trap |

Overflow and underflow on the wrapping arithmetic operators default to two's-complement wrapping rather than a trap, so an arithmetic construct that omits those arms is equivalent to the bare wrapping operation. Every other unhandled partial outcome traps, through the compiler-emitted `Trap`, on both backends.

## Virtual-machine trap variants

The virtual machine surfaces each unhandled partial operation through a specific `VmError` variant so the host's error-category mechanism can map outcomes to policy without parsing a message string. These are implemented (B35 P1).

| Operation | `VmError` variant |
|-----------|-------------------|
| Division or modulo by zero, unhandled | `DivisionByZero` |
| Array index out of bounds, unhandled | `IndexOutOfBounds(index, length)` |
| Newtype refinement predicate failed, unhandled | `RefinementFailed` |
| Discriminant matches no variant, unhandled | `EnumVariantUnmapped` |
| Native failure with a reported code, unhandled, through `Trap(HostError)` | `NativeErrorCode { code, message }` |
| Native failure with a message only, unhandled, through `Trap(HostError)` | `NativeError(message)` |
| Debug `assert` condition false (debug build only) | `AssertionFailed` |
| `for ... limit` reached the cap before the range end, unhandled | `LoopLimitExceeded` |

The compiler-emitted traps for partial operations whose unhandled case has no in-band result are encoded through the `TrapKind` encoding, distinct from the data faults above that already carry their own variant. The `assert` trap is also a `TrapKind` (`AssertionFailed`), but it is reachable only in debug builds, which compile the assert check in; a release build compiles the check out entirely. The failing assertion's source span and message, when present, ride in the strippable `AssertionContext` debug record (B29), not in the `VmError`. Restoring the dropped dynamic detail, namely the failing predicate, newtype, or function name, is tracked under the debug-information work and is independent of this contract.

## Native default values, superseded

An earlier version of this document specified platform-specific values that native code would produce for an unhandled partial operation, for example zero for a division by zero and the numerator for a modulo by zero, chosen from each target's hardware behavior. **That contract is superseded** by the ruling of 2026-10-20 that only `Trap` traps and that the compiler emits the default `Trap` when no handler is written. Native code follows the bytecode `Trap`, so no native default value exists. A program that wants a value in place of the trap handles the outcome and writes the value in its arm.

## The canonical zero value

A single zero value is defined once for every type. It is implemented (B35 P2) by `zero_value` in `src/zero_value.rs`, a public library module. It supplied the superseded native defaults, and no part of the compiler or runtime consumes it now, so it remains a definition a host may use rather than a value either backend produces on its own.

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

The lowest valid value of a refined type is resolved in this order, implemented (B35 P2) by `lowest_valid` in `src/zero_value.rs`. Like the canonical zero, it supplied the superseded native defaults and is not consumed by either backend now.

1. The value declared by the newtype's `with saturate_min` clause, which is predicate-checked.
2. The lowest valid value computed by the interval and lattice analysis, when the valid set is analyzable. This tier is integer-domain. A refined newtype over `Float` falls through to the next tier.
3. Where neither exists, there is no lowest valid value.

## What native code generation must do

The native backend lowers each partial operation to its checked form and the flag test that the bytecode carries, and lowers the compiler-emitted `Trap` to the target's trap. Where the source handles the outcome through a construct arm, the backend lowers the arm body and the test branches into it. Where the hardware instruction would fault on the unsafe input, as integer division by zero does on Intel x86, the guard that the flag test represents runs before the instruction, so the fault never occurs and the program reaches the `Trap` or its handler instead.

The worst-case execution time and worst-case memory usage analyses bound the checked form, the test and the `Trap` the same way on both backends, namely a small fixed number of operations on the longest path, so native code does not invalidate the bounds the verifier proves.
