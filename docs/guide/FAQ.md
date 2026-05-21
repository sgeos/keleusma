# Frequently Asked Questions

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document collects surprises that early adopters have run into. The intent is to answer the questions that the rest of the documentation does not yet anticipate, not to be exhaustive.

## Strings

**Strings are not the Keleusma value proposition.** The language's value proposition is definitive Worst-Case Execution Time and Worst-Case Memory Usage verification for embedded real-time scripting. For string-heavy standalone work, a dynamic language with a rich standard library is the better tool. Python, Ruby, JavaScript, or any of the many shell-and-text-processing languages will all handle strings more ergonomically and with more built-in utility than Keleusma. Strings in Keleusma exist as a host-boundary type and as a debugging convenience; they are not the surface to optimise for.

### Text surface in V0.2.0

V0.2.0 ships only the static-string surface at the script level. String literals (`"..."`) compile to `Value::StaticStr` constants in the bytecode's read-only constant pool. The `Text` primitive type names the surface type for static strings, host-produced dynamic strings (`Value::KStr` arena handles), and string-typed parameters across the host boundary. The bundled `to_string`, `concat`, `slice`, and `length` utility natives retired alongside f-string interpolation in the V0.2.0 Phase 3.5 text-composition removal. The runtime still distinguishes static (`StaticStr`) and dynamic (`KStr`) variants behind `Text`; the cross-yield prohibition continues to apply to dynamic strings.

The recommended pattern is to register native Rust functions that perform the string work and expose them to the script. Rust's standard library handles formatting, splitting, regex, encoding conversion, and Unicode operations far better than anything reasonable to build inside the script. The host writes a small Rust function, registers it with one `register_fn` call, and the script gets a single `use` declaration that yields native performance and full Rust ecosystem access.

````rust
// Rust host code.
use keleusma::{Arena, Value, vm::Vm};

let mut vm = Vm::new(module, &arena)?;

// Host-defined string helpers using Rust's standard library.
vm.register_fn("text::upper", |s: String| -> String {
    s.to_uppercase()
});
vm.register_fn("text::trim", |s: String| -> String {
    s.trim().to_string()
});
vm.register_fn_fallible(
    "text::split_first_word",
    |s: String| -> Result<String, keleusma::VmError> {
        s.split_whitespace()
            .next()
            .map(|w| w.to_string())
            .ok_or_else(|| keleusma::VmError::NativeError("empty input".into()))
    },
);

// The script imports each native by name.
//
//     use text::upper
//     use text::trim
//     use text::split_first_word
//
//     fn greet(name: Text) -> Text {
//         text::upper(text::trim(name))
//     }
````

The host owns the string-handling vocabulary; the script consumes it through `use` declarations. See [EMBEDDING.md](./EMBEDDING.md) for the full native-registration surface.

### Where text works

- **Static string literals.** Compiled to `Value::StaticStr` and reside in the bytecode's constant pool. May flow anywhere admissible, including across the yield boundary in the dialogue type.
- **Arena-resident dynamic strings.** Produced by host-registered native functions through the `KString::alloc` arena boundary. Carried as `Value::KStr` handles that resolve through the host-owned arena and become stale on the next arena reset. Subject to the cross-yield prohibition. See [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) for the full text-type discipline.

The `Value::DynStr` global-heap variant present in V0.1.x was removed in V0.2.0. All dynamic text is arena-resident.

### Escape table for static string literals

| Escape | Result |
|--------|--------|
| `\n`   | newline (`U+000A`) |
| `\t`   | tab (`U+0009`) |
| `\r`   | carriage return (`U+000D`) |
| `\\`   | literal backslash |
| `\"`   | literal double quote |
| `\0`   | null byte |

All other characters appear directly without escaping. Any other backslash sequence is a lex error. V0.2.0 retired the f-string-specific `\{` and `\}` escapes alongside f-string interpolation; `{` and `}` are ordinary characters inside `"..."`.

## WCMU Coverage

### Exponential string concatenation bypasses the WCMU bound

A program like

````
fn main() -> Text {
    let s = "a";
    let s = s + s;
    let s = s + s;
    /* sixty doublings later */
    s
}
````

would in V0.1.x have compiled, been admitted by `Vm::new`, and exhausted the host process at runtime. V0.2.0 addresses both the allocator and the static-analysis dimensions.

**Issue one: string `+` previously allocated from the global allocator, not the arena.** Resolved in V0.2.0. `Op::Add` on text operands now produces a `Value::KStr` allocated through `KString::alloc` in the arena's top region. Allocation failure surfaces as `VmError::OutOfArena` rather than exhausting the host process. The `Value::DynStr` variant has been removed entirely.

**Issue two: the WCMU pass did not previously track text sizes statically.** Resolved in V0.2.0. The verifier now runs a text-size abstract interpretation pass over each chunk that tracks a per-slot `TextSize::{NotText, Known(u32), Unbounded}` lattice through the bytecode, evaluating the `OpCost::Dynamic` cost of `Op::Add` on text against the operand bounds and accumulating the result into the chunk's WCMU heap total. Programs that doubly grow a text value cumulatively saturate the bound to `u32::MAX`, which the safe constructor rejects under the default `OverflowPolicy::Reject`. The doubling-string example above is now rejected at `Vm::new` when expressed as a Stream block.

**Limitations of the V0.2.0 text-size analysis.**

- **Loops widen text values to `Unbounded`.** Text operations inside a `for` or `loop` body produce conservative `u32::MAX` contributions because the pass is linear, not iterative. Programs whose text concatenation happens once per stream iteration are handled precisely; programs that mix loops and text are conservative.
- **Branches widen text values to `Unbounded`.** Text values written conditionally inside an `if`/`else` lose their precise bound. The pass continues to correctly bound text written outside conditionals.
- **Native return values are `Unbounded`.** Text returned from a registered native function is tracked as unbounded; any subsequent `Op::Add` against it saturates the contribution. Hosts that need a tighter bound for their natives supply per-native heap attestations through `Vm::set_native_bounds`.
- **Atomic-total programs (no Stream block) are not subject to the per-iteration WCMU bound.** A `fn main() -> Text { let s = "a"; let s = s + s; ... }` compiles and runs because the resource-bounds check applies only to Stream chunks. The arena exhaustion path through `VmError::OutOfArena` provides the graceful-failure guarantee for these programs.

**Recommendation for text-heavy work.** Treat heavy text work as host-attested and out-of-band. Register native Rust functions (see the section above) that perform the work in a bounded way and let scripts consume them. Host-side text helpers can be implemented to fail safely on large input rather than allocate unboundedly.

A worked example of the host-attestation pattern for arena-resident allocations lives in [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs).

### V0.2.0 fail-fast on too-small arenas

The previous releases admitted an `Arena::with_capacity(0)` through `Vm::new` and then aborted the host process via `handle_alloc_error` on the first push. V0.2.0 changes this in two layers.

**Construct-time minimum reservation.** `Vm::new` and `Vm::new_unchecked` pre-reserve a small minimum operand-stack and call-frame allocation in the arena's bottom region. If the arena cannot hold the minimum, both constructors return the new `VmError::OutOfArena` variant rather than aborting. The minimum is conservative (four stack slots and one call frame); arenas around five hundred bytes or larger pass.

**Run-time push paths return `OutOfArena`.** Every operand-stack and call-frame push in the VM execution loop now routes through internal `sp!` and `fp!` macros that call `Vec::try_reserve` first and return `VmError::OutOfArena` on allocation failure. Programs whose runtime usage exceeds the arena no longer abort the host process; the host gets a typed error and can decide how to handle it (drop the VM, reset state via `Vm::reset_after_error`, retry with a larger arena, or surface the error to the user).

The combination means the arena-resident parts of execution — the operand stack and call frames — are now fully arena-bounded with graceful failure.

```rust
let arena = Arena::with_capacity(2 * 1024);
let mut vm = Vm::new(module, &arena)?;
// ...
match vm.call(&[]) {
    Ok(state) => /* handle state */,
    Err(VmError::OutOfArena(msg)) => {
        eprintln!("arena exhausted: {}", msg);
        // recover or reconfigure
    }
    Err(other) => /* handle other errors */,
}
```

For sizing the arena, use `auto_arena_capacity_for` plus a host-side margin, or the bundled `DEFAULT_ARENA_CAPACITY` of sixty-four kilobytes for typical embedded scripting.

## Other Surprises

### `Vm::call` rejects wrong arg count or type up front

Hosts that drive Keleusma scripts from Rust pass arguments through `vm.call(&[Value::Int(1), Value::Int(2)])`. The runtime validates the argument count against the entry chunk's `param_count` and each argument's runtime type against the parameter's declared `TypeTag` before any bytecode runs. Too few or too many arguments, or a wrong-typed argument, produces a `VmError::TypeError` at the call boundary rather than a confusing arithmetic error later.

Typical reproduction:

````rust
// Script: fn main(a: Word, b: Word) -> Word { a + b }
vm.call(&[Value::Int(1)])
// -> VmError::TypeError("function `main` expected 2 arguments, got 1")

vm.call(&[Value::Int(1), Value::Float(2.5)])
// -> VmError::TypeError("function `main` parameter 1 expected Word, got Float")
````

Hosts that genuinely want to pass an opaque or composite value receive `TypeTag::Composite` validation, which accepts any `Value`. The `param_types` field of each chunk is the source of truth for what the runtime will accept; the compiler populates it from the function's declared parameter types.

### `Vm::resume` validates the resume value's type for Stream blocks

A `loop main(x: T) -> R` script yields a value of type `R` and resumes with the next iteration's value of type `T`. The host calls `vm.resume(value)` to drive the next iteration. The runtime validates `value` against the loop's parameter type before pushing it into the parameter slot.

````rust
// Script: loop main(x: Word) -> Word { let z = yield x; z }
vm.call(&[Value::Int(11)])      // Ok(Yielded(Int(11)))
vm.resume(Value::Float(1.5))    // VmError::TypeError(
                                //   "loop `main` resume expected Word, got Float")
````

The yield expression's type and the resume value's type are the same by language design (the parameter type), so a single tag at the chunk level covers both directions of the dialogue.

### Parser rejects deeply nested expressions

The parser is a recursive-descent walker. Deeply nested parens (around a thousand or more) used to overflow the host process's stack. The parser now bails with a typed `ParseError` at `MAX_PARSE_DEPTH = 32` levels of nesting. The limit applies at the three recursive entry points (`parse_expr`, `parse_type_expr`, `parse_pattern`).

Hosts that produce Keleusma source programmatically (templating, code generation) should keep expression nesting well under thirty-two levels. Realistic hand-written source rarely approaches the limit; the bound exists to prevent a malicious or accidental input from killing the host process.

### Local bindings are immutable

`let` bindings cannot be rebound or mutated. The data segment is the only region of mutable state observable to a script, and it is accessible only from a `loop`-classified entry point. Accumulation across a loop iteration in an atomic-total `fn` is therefore not possible without either (a) a `loop main` script using the data segment, or (b) a host-side fold native. See [WHY_REJECTED.md](./WHY_REJECTED.md) under the recursive-closure entry for examples of both rewrites.

### Closures are rejected at the type-checker stage

V0.2.0 Phase 4 retired the closure family: the `Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`, and `Op::CallIndirect` opcodes are gone, the `Value::Func` runtime variant is gone, and the closure-hoisting compiler pass is gone. The type checker now rejects `Expr::Closure` with the diagnostic `closures are not supported; V0.2.0 admits only direct calls and trait dispatch under the conservative-verification stance. Rewrite as a top-level fn or trait method.` First-class function references (e.g. `let f = my_func;`) are likewise rejected by the compiler. This is the conservative-verification stance documented in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification). The valid form of unbounded execution is the top-level `loop` block enforced by the productivity rule.

### Pipeline operator requires parentheses

The right-hand side of `|>` must be a function call with parentheses, even when the function takes no additional arguments. `expr |> f` is a parse error; `expr |> f()` is correct.

### What does the `signed` modifier do?

V0.2.0 introduces a `signed` modifier on the entry function declaration (`signed fn main`, `signed yield main`, `signed loop main`). It sets `FLAG_REQUIRES_SIGNATURE` in the framing header so the load-time runtime refuses the module unless a cryptographic signature is attached and verifies against the host's trust matrix.

The signing operation itself is a toolchain step independent of the compiler. `keleusma compile script.kel --signing-key seed.bin` produces an Ed25519-signed bytecode file; the consumer registers the matching public key on the VM (`Vm::register_verifying_key`) and loads through `Vm::load_signed_bytes` or hot-swaps signed updates through `Vm::replace_module_from_bytes`. `Vm::load_bytes` refuses signed modules with a diagnostic that names the alternate entry point.

The feature requires the `signatures` cargo feature, which is off by default and pulls in `ed25519-dalek`. Builds without the feature accept unsigned modules normally and reject signed modules with `LoadError::SignaturesUnsupported`. The `signed` surface keyword still parses without the feature so source files remain portable.

Use case: multi-party module delivery to embedded targets. A mothership compiles per-mission scripts and signs them; a daughtership flashed with the mothership's public key verifies before loading. See the [Distributing signed bytecode](./COOKBOOK.md#distributing-signed-bytecode) cookbook recipe and `R42` in [RESOLVED.md](../decisions/RESOLVED.md).

### If-else at statement position requires a trailing semicolon

The parser does not auto-insert semicolons. An `if-else` expression used as a statement (followed by another statement) requires `;` even though the expression evaluates to unit.

````
if state.rem0 == 0 {
    /* ... */
} else {
    state.rem0 = state.rem0 - 1;
};   // <-- this semicolon is required
state.rem1 = state.rem1 - 1;
````

### Opaque types compile but cannot cross the native boundary as themselves

The type checker tracks opaque types correctly, but the marshalling layer in V0.1.x has no path for opaque host values to flow across the native function boundary as themselves. The recommended pattern is to pass opaque values through a primitive handle (typically `Word`) that the host translates to and from its real Rust type at the boundary. See [GRAMMAR.md §3](../design/GRAMMAR.md) and §9.

### Integer arithmetic wraps to the target word width

Keleusma's `Word` is a fixed-width signed integer whose width is declared by the target descriptor. Arithmetic operations mask the result to that width using a sign-extending shift on every step. Overflow does not produce a typed error; the result silently wraps in the modular sense the declared width permits.

````
fn main() -> Word {
    let max = 9223372036854775807;
    max + 1
}
// On a sixty-four bit target this returns -9223372036854775808.
````

This choice is intentional. The Worst-Case Execution Time and Worst-Case Memory Usage bounds the language guarantees depend on every arithmetic operation having a fixed step count. A trapping-overflow semantics would either inflate the worst-case cost of every operation or introduce a control-flow edge that the static analysis would have to enumerate. The wrapping semantics gives a predictable step count and a closed result domain that the analysis can reason about uniformly.

Hosts that need overflow detection register a native that performs the checked operation against a wider Rust integer and surfaces an error through `VmError::NativeError`. The host owns the checked-arithmetic vocabulary; the script consumes it through `use` declarations.

### Loop-calls-loop is rejected by lexical productivity

The productivity rule that admits `loop` blocks is enforced by a purely lexical structural check. The verifier walks the syntactic body of each `loop` and requires that every control-flow path through one iteration contains at least one `yield`. A `loop` block whose body's only `yield` is inside a function it calls is rejected because the structural pass does not chase the call.

````
yield helper() -> Word { yield 1 }

loop main() -> Word {
    let v = helper();   // <-- structural pass does not see the yield
    v
}
````

This program is rejected with `loop body has no yield on at least one path`. The rule errs conservative on purpose. A semantic check that chased calls would be unsound for parameter-dependent dispatch or trait method resolution and would also have to handle mutually recursive call graphs. The lexical check is sound, fast, and easy to explain at the cost of forcing the `yield` to appear at the top level of the `loop` body.

The recommended pattern is to keep `yield` at the top of the `loop` body and call helpers around it.

````
yield helper() -> Word { yield 1 }

loop main() -> Word {
    let v = yield helper();   // direct yield satisfies the rule
    v
}
````

The same constraint applies to `if`/`else` and `match` branches inside the `loop` body. Every fall-through path must contain a `yield`, or the branch must `break` out.

### V0.2.0 boundary diagnostics

The construction and call surfaces were tightened in V0.2.0 so that several previously silent or misleading cases now produce typed diagnostics at the appropriate boundary.

- **Integer literals that overflow `i64` are now `LexError`.** The previous behaviour silently produced `Value::Int(0)` for literals such as `99999999999999999999999999999`. The lexer now reports `integer literal does not fit in i64` with the source span of the literal.
- **Untyped parameters are inferred when context resolves them.** Writing `fn main(x) -> Word { x }` previously parsed and registered `x` with no inferred type, then tripped a confusing error later. The type checker now writes inferred primitive types back into the AST. For `fn main(x) -> Word { x }` the return-type constraint forces `x: Word`, and the chunk's `param_types` carries `TypeTag::Word` so `Vm::call(&[Value::Float(1.5)])` is rejected at the API boundary. If inference does not resolve the parameter (no constraint), the chunk records `TypeTag::Composite` and the runtime accepts any value.
- **Duplicate function heads are rejected, entry point or not.** Two function definitions that share the same name whose parameter signatures cannot be disambiguated as multi-headed pattern matching (same shape, no guard) used to keep the first and silently discard the rest. The compiler now reports `function head is dead code` at the second definition. The rule applies to all categories (`fn`, `yield`, `loop`) and to every function, not just the entry point.
- **Multi-headed entry points are accepted for `fn`, `yield`, and `loop`.** All three function categories admit pattern-matched entry points. Multi-headed `loop main(...)` Stream blocks compile to a single `Op::Stream` and single `Op::Reset` envelope around a dispatch wrapped in `Op::Loop`/`Op::EndLoop`; each matched head's body ends with `Op::Pop` and `Op::Break` so the structural verifier's Stream invariants hold.
- **Modules without an entry point are now `VmError::VerifyError`.** A module compiled from source that omits `fn main`, `yield main`, or `loop main` previously surfaced as `VmError::InvalidBytecode("no entry point")` at the first `Vm::call`. The constructor `Vm::new` (and `Vm::new_unchecked`) now rejects the module with `module has no entry point` at the API boundary.
- **Premature `Vm::resume` is now `VmError::NotSuspended`.** Calling `vm.resume(value)` before `vm.call(args)` previously surfaced as `VmError::InvalidBytecode("cannot resume: VM not suspended")`, which conflated API misuse with corrupt bytecode. The runtime now returns the dedicated `VmError::NotSuspended` variant.
- **Structural-verification rejections now carry source spans.** Compile-pipeline rejections for `CallIndirect` and `MakeRecursiveClosure` used to attach `Span::default()`, which hid the offending source position. Each rejection now points at the originating function or closure declaration so editors can underline the construct.

### Bytecode 0.1.0 was yanked

`keleusma 0.1.0` was yanked from crates.io within hours of publication because its declared MSRV of 1.87 conflicted with let-chain syntax in the source that requires Rust 1.88. `keleusma 0.1.1` is the corrected initial release; `Cargo.lock` files referencing 0.1.0 continue to resolve but new `cargo add keleusma` invocations pick 0.1.1.

## Where to look for more

- The full language reference is [GRAMMAR.md](../design/GRAMMAR.md) (descriptive; the normative reference is the parser at `src/parser.rs`).
- The verifier rejection catalog is [WHY_REJECTED.md](./WHY_REJECTED.md).
- The embedding API surface is [EMBEDDING.md](./EMBEDDING.md).
- The conservative-verification stance is in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification).

When in doubt about whether a behaviour is intended or a bug, the parser, type checker, and verifier are authoritative; the documentation is descriptive.
