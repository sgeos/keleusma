# Order 3 handler surface plan

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**. Scope resolved by the operator on 2026-10-20. Not yet implemented.
This document resolves the open question that the 2026-10-20 handoff recorded as
the first audit priority, which asked whether source-level indexed-write handlers
belong to Order 3. They do. It extends [the completion plan](./ORDER3_COMPLETION_PLAN.md)
and adds acceptance rows to it.

## Operator rulings, 2026-10-20

The rulings below were given directly by the operator in the Opus 5.5 audit
session of 2026-10-20. They are first-party, not relayed. Quoted text is the
operator's wording.

| Topic | Ruling |
|---|---|
| Indexed-write handlers | "Yes, add indexed-write handlers." |
| The ok arm | "The `ok` arm should bind the old and new values, either of which can be ignored. `ok(old, new) => ...`" The compiler generates code only for captured values, so the four capture patterns emit different amounts of code. |
| Reading the old value | The read is emitted only when `old` is bound. "In short, only conditionally emit the read. The condition is whether it is needed or not." |
| Evaluation order | "Index should be evaluated first." |
| Short circuit | "A bad index should always short circuit, even if the default trap logic is used." A program that must always evaluate the value binds it with `let` before the write. |
| Expression form | "A handled write should be an expression that yields a value." Its type is the common type of its arms, discarded when used as a statement. |
| Scope | "I think the scope change is everything from surface grammar to codegen." |
| Composite reads | Composite elements have no default fallback. "Require the handler and otherwise fall back to the trap." |
| Host-contract faults | Native-call handlers are in scope. Faults detectable at load are rejected at load. Runtime host failures are reported as a handled outcome, `host_error => ...`, with a compiler-default trap when no handler is written. |
| Non-trap faults generally | "Host-contract faults and indexing forms outside the first pass should not explicitly trap. They should set VM flags on failure where the compiler defaults to a trap if there is no explicitly provided logic." |
| Failure signal mechanism | A processor-style flags register replaces every in-band outcome flag. The diagnostic payload stays on the stack only where a handler binds it. Flags behave as on a processor. A flag-select form of the existing conditional branch reads them, with the exact encoding recorded in a design document before implementation. **Deferred to V0.2.4.** "Switching from in-band to flags can happen in V0.2.4." |
| Bytecode version | Frozen with the fingerprint until the next public version. "A version bump will occur when there is evidence of traction. I have no evidence anyone is using Keleusma." |
| Unchecked execution | "The Keleusma project rejects undefined behavior, but it does not reject platform specific behavior." Proven execution keeps a real length check on the fast path. |

## Consequences for V0.2.3

V0.2.3 keeps the in-band outcome pattern that already exists. The new handler
forms below use it, so that V0.2.3 is internally consistent. All of them move to
the flags register in V0.2.4. Some work is therefore done twice. That cost is
accepted in exchange for a consistent V0.2.3.

The ruling on short circuiting changes current bare-write behaviour. Today a bare
write evaluates its value before the bounds outcome is known. After this change a
bad index skips the value on both the handled and the bare forms. The existing
control at tests/indexed_write_outcomes.rs line 102 observes evaluation order
through private data and uses a valid index, so its expectation is unaffected.
A new control must show that an invalid index skips a value with an observable
effect.

## Surface forms

```
// Handled indexed write. An expression of the arms' common type.
d.x[i] = v { ok(old, new) => old + new, invalid_index(j) => j }
d.x[i] = v { ok(_, _) => (), invalid_index(_) => () };

// Native call with a handled host failure.
f(x) { ok(r) => r, host_error => 0 }
```

Keleusma has no `nop` keyword. The unit literal `()` serves that role.

### Lowering of a handled write

With `old` bound the compiler emits the equivalent of the following.

```
if bounds_check_ok(index) {
    old = read(index)
    write(index, new)
    ok_arm(old, new)
} else {
    invalid_index_arm(index)
}
```

Without `old` it omits the read. With `new` unbound it keeps no copy of the
value. The value expression is evaluated inside the success branch, after the
index and its bounds check, which is what makes the short circuit hold. No new
opcode is required. BoundsCheck already produces the outcome and SetDataIndexed
already performs a checked write.

## Remaining non-trap faults found by this audit

The 2026-10-20 audit found that source division on Fixed operands emits a bare
FixedDiv, which still raises DivisionByZero at run time without a Trap. The trap
analysis classifies FixedDiv as an unsupported operation in both
implementations, so no unsound trap-free verdict results. Under the ruling above,
FixedDiv must report a zero divisor as an outcome, and the compiler must default
to a trap when no handler is written. Indexing forms outside the first pass and
FixedDiv are the known members of this class. The runtime fault census in
tests/runtime_fault_census.rs should gain a Fixed division row, since its twenty
source cases did not include one.

## Acceptance rows added to the completion plan

| Row | Required evidence |
|---|---|
| Write handler grammar | Both parsers accept the four `ok` capture patterns and `invalid_index` and reject malformed shapes, with byte-identical rejection families. |
| Write handler lowering | Both compilers emit byte-identical modules. The read appears only when `old` is bound. A covered handler emits no fallback Trap. |
| Short circuit | An invalid index skips a value expression whose effect is observable, on both the handled and the bare form. |
| Native-call handlers | Load-time rejection of statically detectable host faults. A runtime host failure reaches the handler, or the default trap when none is written. |
| Composite reads | A composite read without a handler lowers to the default trap. With a handler, no fabricated value is produced. |
| FixedDiv totalization | A zero divisor yields an outcome. Source Fixed division reaches an explicit Trap by default, and the census includes the case. |
| Trap analysis | Every new form classifies identically in the Rust and Keleusma analyses, and a fully handled program classifies as trap-free. |

## Out of scope for V0.2.3

The flags register, its flag-select branch encoding and the migration of every
in-band outcome. These are V0.2.4 work and need their design document first.
