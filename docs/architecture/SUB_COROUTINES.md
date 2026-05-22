# Sub-Coroutines

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

**Status**: Preliminary specification. The model is settled: asymmetric coroutines with call-down and yield-up semantics, arena-resident state, declared lifetime category. Surface syntax, exact opcode names, and several edge-case rules are open. Implementation is gated on V0.5.0 prerequisites landing.

## Goal

Specify the asymmetric coroutine primitive that allows ephemeral `loop` constructs to be invoked as sub-coroutines within a single VM or native binary. The primitive is the runtime mechanism by which V0.5.0's Keleusma-hosted host orchestrates multiple computations (the lexer, parser, and compiler pipeline stages, plus additional subcommand handlers) inside a single top-level entry point.

This document is the authoritative description of the model. The V0.5.0 roadmap document cross-references it; the language design document and the execution model document will be updated to reference it once the model is settled.

## Why this primitive is needed

Before V0.5.0, Keleusma programs have exactly one top-level `loop main`. Multi-task coordination requires either multiple VM instances (each with its own arena) or host-side coordination of a single VM through yield and resume calls. Both patterns are workable when the host is Rust because Rust is a fully concurrent host language.

V0.5.0 migrates the host to Keleusma. A Keleusma host that wants to coordinate the lexer, parser, and compiler stages cannot host them as separate VM instances without reintroducing host-language coordination, defeating the migration. Sub-coroutines are the alternative: ephemeral `loop` constructs callable from inside the host's top-level entry point, each with its own program counter, call-frame stack, operand stack, and arena slot.

The primitive is independently useful beyond V0.5.0. RTOS-style task pools, event-driven dispatch, and game-loop stage decomposition all benefit from the same mechanism.

## The asymmetric coroutine model

Sub-coroutines transfer control along a strict parent-child hierarchy.

- **Call down.** The parent invokes the child via a spawn or resume operation. Control transfers from parent to child. The parent pauses at the spawn or resume site.
- **Yield up.** The child yields a value back to the parent. Control transfers from child to parent. The child pauses at the yield site. The parent receives the yielded value.
- **Parent decides next step.** On receiving a yielded value, the parent may resume the child with a resume value, abandon the child (releasing its slot), or yield further upward to its own parent.

Sibling-to-sibling transfer is not supported. A coroutine cannot transfer control directly to another coroutine that shares its parent; control always returns to the parent, which then resumes whichever child is next. This is the *asymmetric* coroutine model, distinguished from the *symmetric* model where any coroutine may transfer to any other coroutine.

The asymmetric model is the right choice for Keleusma because:

- WCMU and WCET compose by sum along the hierarchy. A parent's bound includes the bounds of its called children. The verifier sums over the call-and-yield graph as a tree.
- Termination and productivity reasoning is local to each parent-child pair. The parent's productivity claim is either "I yield directly" or "I call a child that yields." The child's productivity is its own concern, verified independently.
- Hot replacement quiescence is well-defined. A sub-coroutine's quiescent point is exactly when it has yielded to its parent. The parent observes the quiescent boundary.

Symmetric coroutines, while strictly more expressive, break each of these properties.

## Sub-coroutine state

Every sub-coroutine instance carries the following state for the duration of its life:

- **Program counter.** Position in the chunk's bytecode (or the native code address in V0.4.0-compiled code).
- **Call-frame stack.** Local variables and return addresses for `fn` calls within the coroutine.
- **Operand stack.** Used by the VM when the coroutine executes in bytecode form. Empty or absent in native-compiled code.
- **Arena slot.** A reserved region of the program's master arena, containing the above three plus any heap-allocated data the coroutine produces during its life.

All four are *co-located in the arena slot*. The arena slot is the single allocation unit for the coroutine; releasing the slot releases all four.

## Arena slot reservation

The arena slot is reserved for the entire lifetime of the sub-coroutine. It cannot be reassigned while the sub-coroutine is alive. This invariant applies to both ephemeral and persistent sub-coroutines.

The distinction between ephemeral and persistent is what happens *after* the sub-coroutine completes:

| Lifetime category | Slot behaviour at completion |
|---|---|
| Ephemeral | Slot is released. The next ephemeral sub-coroutine assignable to the same pool may claim the slot. |
| Persistent | Slot is reserved for the program's lifetime. No reuse. |

Ephemeral sub-coroutines live inside *pools*. A pool has a declared capacity of N slots. The arena reserves N times the per-slot bound. At any given moment, up to N ephemeral sub-coroutines may be alive within the pool. Slot reuse happens at completion boundaries.

Persistent sub-coroutines each have their own dedicated slot, reserved at compile time and never reassigned.

Both categories are master-WCMU-based: the total arena size is computed at compile time as the sum of all reserved capacity. Mutual-exclusivity refinement may share slots between sub-coroutines proven to be statically disjoint in lifetime, but the underlying invariant (arena reserved for life) holds at every instant.

### Spawn-time slot availability

A spawn operation requires an available slot. Three possible policies:

1. **Static verification.** The verifier proves at compile time that the pool's capacity is sufficient for the worst-case concurrent demand. Spawn cannot fail. This is the V0.5.0 default for pools whose demand is statically bounded by the program structure.

2. **Runtime fallibility.** Spawn returns `Result<Handle, OutOfSlots>`. The caller handles the failure path. This is necessary when the static analysis cannot prove sufficient capacity, but it complicates the call site and is not preferred.

3. **Compile-time rejection.** A program whose pool capacity cannot be statically verified is rejected. This is the strictest stance.

V0.5.0 ships with policy 1 for programs the verifier admits and policy 3 for programs it cannot admit. Policy 2 is available as an opt-in for programs that wish to handle the fallibility explicitly, but the verifier prefers the strict stance for certification-adjacent use cases.

## Coroutine handle

A *coroutine handle* is a value-typed reference to a sub-coroutine instance. The handle carries the slot identifier and the coroutine state pointer. It is passed to resume and release operations.

Handle lifetime rules:

- A handle is valid only within the scope of the parent that spawned it.
- A handle cannot be stored in long-lived heap data outside the parent's arena.
- A handle cannot be transferred between parents; sibling-to-sibling transfer is prohibited.
- A handle is invalid after the corresponding sub-coroutine completes and its slot is released.

The handle's type carries the coroutine's signature: input type, yield type, resume type, and completion type. The type system enforces correct typing of resume values and yielded values.

## Surface syntax (resolved, R5.1)

Resolved by R5.1 (`tmp/research/r5_1_sub_coroutine_surface_syntax.md`). Four keywords plus signature clauses on the `loop` declaration.

### Spawn

```
let handle = spawn lexer_loop(source_bytes);
```

The `spawn` keyword. The right-hand side is the coroutine being instantiated; the parenthesised arguments are the input type. The keyword conflicts with concurrency connotations from Erlang, Go, and Rust, but R5.1 affirmed `spawn` after considering alternatives because the asymmetric semantics are clarified by the surrounding syntax (handle locality, signature clauses).

### Resume

```
let outcome = resume handle, next_value;

match outcome {
    yielded(token) => ...,
    completed(result) => ...,
}
```

`resume handle, value` returns a tagged outcome distinguishing yielded values from completion values. Pattern matching at the use site is the typical shape.

### Release

```
release handle;
```

Explicit early termination. Implicit on scope exit or parent completion.

### Status query

```
if alive(handle) { ... }
```

A built-in `alive(handle) -> bool` predicate. The handle remains a value after the sub-coroutine completes, but operations on a non-alive handle other than `alive` and `release` are rejected by the verifier.

### Signature clauses

A `loop` declaration intended for sub-coroutine use carries explicit type clauses:

```
loop lexer_loop(source: [Byte; N])
    yields Token
    accepts ResumeKind
    completes Summary
{
    ...
}
```

The four types (input, yield, resume, completion) are statically known at every spawn and resume site.

### Handle storage

Handles may live only in local variables. Storing a handle in a `data` block, returning it from a `fn`, or passing it across a yield boundary is rejected by the verifier. This rule ensures the handle does not escape the parent's scope.

### Atomic functions are not sub-coroutines

`fn` sub-coroutines are not admitted in V0.5.0. R5.1 considered the case and concluded that the use cases are sufficiently rare that the spec surface does not justify inclusion. Future releases may revisit.

## New opcodes

Three new opcodes are required at the bytecode level. Names are preliminary.

| Opcode | Operands | Effect |
|---|---|---|
| `SpawnCoroutine` | chunk index, input arguments | Allocates an arena slot, initialises the coroutine state, pushes a handle to the operand stack. |
| `ResumeCoroutine` | handle, resume value | Transfers control to the coroutine, returns when the coroutine yields or completes. Pushes the yielded value or completion marker to the operand stack. |
| `ReleaseCoroutine` | handle | Releases the slot, invalidates the handle. Implicit at parent scope exit if not called. |

The yield opcode already exists; its semantics extend to "yield up the parent-child chain." When a sub-coroutine yields, its yield value lands on the parent's operand stack at the `ResumeCoroutine` site.

### Lowering to LLVM coroutine intrinsics in V0.4.0

When the V0.4.0 native code generator processes bytecode that contains these opcodes, each opcode lowers to a corresponding LLVM coroutine intrinsic call. The mapping uses the returned-continuation family (`@llvm.coro.id.retcon`) per R4.1's corrected recommendation; earlier drafts referenced the switched-resume family in error.

| Bytecode opcode | LLVM coroutine intrinsic |
|---|---|
| `SpawnCoroutine` | `@llvm.coro.id.retcon` with Keleusma-provided allocator and deallocator function pointers, followed by `@llvm.coro.begin`. The fixed-size buffer is the arena slot. |
| `ResumeCoroutine` | Indirect call through the current continuation pointer stored in the arena slot. The yielded value is returned alongside the next continuation pointer. |
| `ReleaseCoroutine` | The deallocator function pointer is invoked, returning the slot to the arena's free list (for ephemeral pools) or marking it dormant (for persistent slots). |

The continuation pointer changes on each resume; the arena slot stores the current pointer so the Keleusma-level handle remains stable across resumes. This indirection wraps the retcon mechanics behind the stable handle abstraction at the Keleusma surface.

The same surface syntax compiles to either bytecode opcodes (executed by the VM) or LLVM coroutine intrinsics (lowered to native code). Operators select the deployment shape per build; the sub-coroutine semantics are identical across shapes.

R4.1's milestone M1 (a minimal LLVM IR fragment using `coro.id.retcon` with a Keleusma-shaped allocator) is the load-bearing technical risk in V0.4.0. The bytecode-shape implementation is not affected.

See [V0_4_0_NATIVE_CODEGEN.md](../roadmap/V0_4_0_NATIVE_CODEGEN.md) for the native lowering strategy in full.

## Verifier extensions

The verifier gains the following responsibilities:

- **Per-coroutine WCMU.** Each sub-coroutine has its own WCMU bound, computed structurally over its body. The bound includes any further sub-coroutines it spawns.
- **Per-coroutine WCET.** Each sub-coroutine has its own per-resume WCET bound. The parent's per-iteration WCET sums over called sub-coroutines.
- **Productivity.** A `loop` sub-coroutine must yield on every path, recursively. A `fn` sub-coroutine (if such a thing exists; see open questions) must terminate.
- **Slot reservation.** At each spawn site, the verifier confirms a slot is available. For pool spawns, the pool's capacity is checked against the worst-case concurrent demand at that site.
- **Handle scoping.** A handle cannot escape its parent's scope; the verifier rejects programs that store handles in long-lived locations.
- **Type compatibility.** Resume values, yielded values, input arguments, and completion values must match the coroutine's signature.

These extensions are mechanical given the existing verifier infrastructure. The new analysis cost is per-spawn-site rather than per-program.

## Hot replacement quiescence

A sub-coroutine is *quiescent* when it has yielded to its parent and the parent has not yet called resume. At a quiescent boundary, the sub-coroutine's chunk is eligible for hot replacement.

The interface-fingerprint check from the V0.5.0 live code update model applies at the chunk level. A swap is accepted when:

- The new chunk's interface-fingerprint matches the in-place chunk's (or is a documented compatible extension).
- The new chunk's declared bounds are at least as tight as the in-place chunk's declared bounds (no relaxation).
- The new chunk's signature is valid.

Live sub-coroutines whose chunks are swapped retain their PC and stack state. The new chunk must be able to accept the existing state. State migration (transforming the existing state to match a structurally changed chunk) is V0.5.x work, not V0.5.0.

Persistent sub-coroutines may be quiescent for long periods between activations; they are eligible for hot replacement at any such boundary. Ephemeral sub-coroutines are typically replaced by replacing their pool's chunk; live instances finish their current life cycle on the old chunk, and subsequent spawns use the new chunk.

## Relationship to existing constructs

- **Top-level `loop main`.** Remains the program's entry point. A top-level `loop main` may spawn sub-coroutines. The V0.5.0 `impure loop main` driver is a top-level loop that spawns the compiler pipeline stages as sub-coroutines.
- **Top-level `impure fn main`.** May spawn sub-coroutines. The V0.5.0 CLI driver (the compiler's `keleusma compile` entry point) is an `impure fn main` that spawns the lexer, parser, and compiler pipeline stages.
- **`yield` functions.** Continue to exist as the within-loop yielding mechanism. They are not coroutines in their own right; they are syntactic forms inside a `loop`'s body. A sub-coroutine call within a `yield` function's body is permitted, as long as the resulting yield-up traffic terminates at the enclosing `loop`.
- **Recursion prohibition.** Sub-coroutines do not relax the recursion prohibition. A `loop` invoked as a sub-coroutine cannot directly recurse into itself, though it can spawn another `loop` (potentially a different chunk) as a further sub-coroutine. Depth bounds apply at the spawn-site analysis.

## Out of scope (for V0.5.0)

- **State migration across hot swaps.** Changing the in-memory state of a live sub-coroutine to match a structurally evolved chunk. V0.5.x.
- **Symmetric coroutine transfer.** Direct sibling-to-sibling control transfer. Possibly V0.6+, possibly never.
- **Multi-CPU coroutine scheduling.** Running sub-coroutines on separate cores. V0.7+ at earliest.
- **Reflection or introspection.** Programmatic inspection of a coroutine's state beyond its yielded values. Not contemplated.
- **First-class continuations.** Capturing a coroutine's current state as a value and resuming it later from a different context. Not contemplated.

## Resolved design questions

The seven open questions from the preliminary specification were addressed in the 2026-05-21 research pass. Resolutions:

1. **Surface syntax**. Resolved by R5.1. Keywords `spawn`, `resume`, `release`. Signature clauses on the `loop` declaration. See "Surface syntax" above for the worked specification.

2. **Handle storage discipline**. Resolved by R5.1. Local variables only. Storage in `data` blocks, return from `fn`, or transfer across yield boundaries is rejected by the verifier.

4. **`fn` sub-coroutines**. Resolved by R5.1. Not admitted in V0.5.0. Future releases may revisit.

5. **Pool dormancy and mutual-exclusivity refinement**. Resolved by R5.4. V0.5.0 ships with simple-sum allocation. Interval-graph refinement lands in V0.5.x, with pool dormancy folded in as a special case.

6. **Yield and resume value types**. Resolved by R5.1. The signature clauses (`yields T accepts R completes C`) provide distinct types for each direction. Tuples are admissible within each clause; the Lua-style multiple-value shape is the natural default.

7. **Parent observation of completed sub-coroutines**. Resolved by R5.1. The built-in `alive(handle) -> bool` predicate answers the question without resuming.

## Open questions

These remain unresolved after the 2026-05-21 research pass.

1. **Maximum spawn depth.** A sub-coroutine may spawn its own sub-coroutines; the verifier needs a static bound. A natural answer is "as deep as the static call graph admits"; another is "fixed depth limit declared per program." The spec surface for the bound is open.

2. **Sub-coroutine and hot-swap interaction.** Surfaced by R5.2 and `tmp/research/IMPLEMENTATION_ORDER.md`. When a module hosting a live sub-coroutine is hot-replaced (matching fingerprint), the continuation pointer in the slot may reference invalidated code. The migration table semantics, the resumability of in-flight sub-coroutines after swap, and the parent's observation of the swap event need specification.

## References

- [V0_5_0_KELEUSMA_HOST.md](../roadmap/V0_5_0_KELEUSMA_HOST.md) for the V0.5.0 deployment context that motivates the primitive.
- [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) for the existing function categories and coroutine model.
- [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the arena memory model and the two temporal domains.
- [WIRE_FORMAT.md](../spec/WIRE_FORMAT.md) for the interface-fingerprint header field that hot replacement depends on.
- Lua 5.x reference manual, chapter on coroutines, for the canonical asymmetric coroutine model.
- Marlin and de Moura, "Revisiting Coroutines," *ACM TOPLAS*, 31(2), 2009, pp. 6:1-6:31. The reference treatment of asymmetric versus symmetric coroutines.
- Joe Armstrong, *Programming Erlang: Software for a Concurrent World*, Pragmatic Bookshelf, second edition 2013. The Erlang spawn and process model, contrasted with the asymmetric coroutine model adopted here.
