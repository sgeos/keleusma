# Why Was My Program Rejected?

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Keleusma's verifier rejects programs that the WCET and WCMU analyses cannot prove bounded. This is intentional. The language's value proposition is definitive bounds on execution time and memory, and the safest place to draw the boundary is the analysis's current capability. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification) for the full statement.

This document maps verifier error messages to root causes and proposes rewrites. The error messages are the actual strings produced by `src/verify.rs` and `src/compiler.rs`. When the verifier rejects a program, search this document for a substring of the error message.

## Rejection Taxonomy

Rejected programs fall into two categories, distinguished by whether the rejection is fundamental or analytical.

**First category: provably unbounded.** The construct admits unbounded execution at runtime by construction. No future verifier improvement will admit it without an external attestation, because the bound does not exist. The remedy is to rewrite the program in a bounded form.

**Second category: bounded but not yet proven.** The runtime behavior is bounded in fact, but the static proof has not been implemented. A future analysis can move such programs into the admitted set without changing the surface language. The remedy is to rewrite the program in a form the present analysis can handle, or to wait for a future verifier extension.

The categories are coherent because the language treats rejection as the safety property: a program admitted by `Vm::new` is one whose bound is proved, not one whose bound exists in principle. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification) for the architectural rationale.

## Common Rejection Messages

### MakeRecursiveClosure

````
WCET verification: <chunk>: MakeRecursiveClosure(chunk=N) introduces unbounded
recursion that cannot be statically bounded for WCET/WCMU analysis. Recursive
closures are not admitted by the safe build pipeline.
````

**Category.** First. Self-referential dispatch admits unbounded recursion within a single Stream-to-Reset slice by construction.

**Trigger.** A `let` binding refers to itself by name, producing a closure value that captures its own environment slot.

````
let factorial = |n: i64| if n <= 1 { 1 } else { n * factorial(n - 1) };
````

**Rewrite.** Locals in Keleusma are immutable; accumulation across a loop requires either the data segment, which is itself accessible only from a `loop`-classified entry point, or a host-supplied native that performs the fold. Two structural rewrites apply.

The first is to reclassify the entry point as `loop` and accumulate across iterations through a data block.

````
data { result: i64 }

loop main(input: i64) -> i64 {
    data.result = data.result * input;
    let _next = yield data.result;
    data.result
}
````

The second is to register a host-side fold native and call it from a `fn`.

````
use math::fold_product

fn main() -> i64 {
    math::fold_product([1, 2, 3, 4, 5])
}
````

The choice depends on whether the iteration count is unbounded (the host drives `loop`) or finite and known at compile time (the host registers a fold native).

### CallIndirect

````
WCET verification: <chunk>: CallIndirect resolves its target chunk at runtime
and cannot be statically bounded for WCET/WCMU analysis. First-class function
dispatch is not admitted by the safe build pipeline. Restrict the program to
direct calls.
````

**Category.** Second. Non-recursive closure invocation is bounded in fact, but the present verifier rejects it because indirect dispatch through `Op::CallIndirect` requires a flow analysis that has not been implemented.

**Trigger.** A `let` binding holds a closure value, and a subsequent expression invokes it.

````
let f = |x: i64| x + 1;
f(5)
````

**Rewrite.** Replace the closure with a top-level function and call it directly.

````
fn increment(x: i64) -> i64 { x + 1 }

increment(5)
````

Closures are useful at the surface for compositional patterns, but the safe verifier rejects all `Op::CallIndirect` invocation regardless of whether the closure is recursive. A future flow-analysis extension can admit the non-recursive cases.

### Loop Iteration Bound Not Extractable

````
loop at instruction <ip> has no statically extractable iteration bound; strict
mode requires loops with fall-through bodies to match the canonical for-range
pattern
````

**Category.** Second. The runtime loop count may be bounded by a runtime-known value, but the present verifier extracts the iteration count only from the canonical `for i in 0..N` shape with `N` a compile-time constant.

**Trigger.** A `for` loop iterates over a range whose end is a parameter or a function-call result.

````
fn process(n: i64) -> i64 {
    for i in 0..n { ... }
    0
}
````

**Rewrite.** Use a compile-time constant bound, or iterate over an array whose length is known.

````
fn process() -> i64 {
    for i in 0..10 { ... }
    0
}
````

When the bound is genuinely runtime-known, the program is outside the safe-verification surface today and may either wait for the loop-bound inference to extend or ship through `Vm::new_unchecked` with the host accepting the unbounded risk.

### Recursive Call Detected

````
recursive call detected during WCMU topological sort
````

**Category.** First. Direct or mutual recursion in `fn` or `yield` functions is rejected by language design; only `loop` admits cyclic execution and only through the productive RESET cycle.

**Trigger.** A `fn` calls itself directly or transitively through another `fn`.

````
fn count_down(n: i64) -> i64 {
    if n <= 0 { 0 } else { count_down(n - 1) }
}
````

**Rewrite.** As with the recursive-closure case above, the rewrite depends on whether the iteration count is bounded. For a compile-time-bounded count, use a `for` loop and structure the computation so the result is determined by the iteration count rather than by accumulation. For an unbounded count, move the cyclic behavior into the top-level `loop` block, where the productivity rule admits it.

The pure-functional rewrite for a count-down is a no-op when the script does not need the per-step output.

````
fn count_down(n: i64) -> i64 {
    for _ in 0..n { let _step = 1; }
    0
}
````

When the per-step output is needed, accumulate through the data segment in a `loop` script as shown in the recursive-closure rewrite above.

### Stream Block Missing Yield

````
Stream block must contain at least one Yield
````

**Category.** First. A `loop` function must yield on every iteration to satisfy the productivity guarantee. A loop function whose body contains no `yield` admits unbounded silent computation.

**Trigger.** A `loop` declaration with a body that does not call `yield`.

````
loop main(input: i64) -> i64 {
    input * 2
}
````

**Rewrite.** Add a `yield` expression to the loop body.

````
loop main(input: i64) -> i64 {
    let doubled = input * 2;
    let _next = yield doubled;
    doubled
}
````

### Reentrant Block Missing Yield

````
Reentrant block must contain at least one Yield
````

**Category.** First. A `yield`-classified function must contain a `yield` expression on every path or the classification is wrong.

**Trigger.** A function declared `yield` but whose body never yields.

**Rewrite.** Either add a `yield` expression to the body, or change the classification to `fn` if the function actually returns directly.

### Resource Bounds Exceeded

````
verify_resource_bounds: arena capacity <cap> bytes is below WCMU bound
of <wcmu> bytes
````

**Category.** Second in spirit, first in effect. The program is bounded in memory, but the configured arena is too small for the bound.

**Trigger.** Either the arena was configured by hand and is too small, or the script's WCMU exceeds expectations.

**Rewrite.** Use `auto_arena_capacity_for` to size the arena from the module, or increase the explicit capacity.

````
let cap = keleusma::vm::auto_arena_capacity_for(&module, &[])?;
let arena = Arena::with_capacity(cap);
let vm = Vm::new(module, &arena)?;
````

When the WCMU is itself surprising, inspect `verify::module_wcmu` output per chunk to identify the high-cost path. See [`examples/wcmu_basic.rs`](../../examples/wcmu_basic.rs) for the inspection pattern.

### Block Boundary Errors

````
EndIf at <ip> with no matching If
EndLoop at <ip> with no matching Loop
Break at <ip> outside any Loop block
````

**Category.** First. Bytecode-level block boundaries are inconsistent. These messages indicate a bug in the source-to-bytecode pipeline rather than a user-program issue. If a Keleusma user encounters one of these, the issue is a compiler bug; please file an issue against the project.

## When the Surface Compiles but the Verifier Rejects

The conservative-verification stance accepts that the surface language is broader than the verifier's admittance set. A program that lexes, parses, type-checks, and compiles successfully may still be rejected at `Vm::new`. This is the second category in action: the language describes the construct so the verifier can reject it precisely, rather than approximately.

The standard response is to rewrite the program. The alternative responses are these.

- Use `Vm::new_unchecked` and accept the unbounded risk explicitly. This is intentional misuse outside the WCET contract and is documented as such.
- Wait for a future verifier improvement. The B3 closure entry in [BACKLOG.md](../decisions/BACKLOG.md) tracks the closure-related extensions.
- File an issue with the rejected program if you believe the analysis should admit it. Worked examples are valuable for prioritizing analysis improvements.

## Cross-References

- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification) is the canonical statement of the conservative-verification stance.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the structural verifier's operation.
- [BACKLOG.md](../decisions/BACKLOG.md) tracks future verifier extensions.
- [`src/verify.rs`](../../src/verify.rs) contains the actual rejection logic and error message strings.
