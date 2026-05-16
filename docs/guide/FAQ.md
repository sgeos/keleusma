# Frequently Asked Questions

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document collects surprises that early adopters have run into. The intent is to answer the questions that the rest of the documentation does not yet anticipate, not to be exhaustive.

## Strings

**Strings are not the Keleusma value proposition.** The language's value proposition is definitive Worst-Case Execution Time and Worst-Case Memory Usage verification for embedded real-time scripting. For string-heavy standalone work, a dynamic language with a rich standard library is the better tool. Python, Ruby, JavaScript, or any of the many shell-and-text-processing languages will all handle strings more ergonomically and with more built-in utility than Keleusma. Strings in Keleusma exist as a host-boundary type and as a debugging convenience; they are not the surface to optimise for.

### Enabling text support

Surface support for strings is gated behind the `text` cargo feature, which is disabled by default. With the feature off, the lexer rejects string literals (`"..."`) and f-strings (`f"..."`) with `string literals require the text cargo feature, which is disabled in this build`, the parser does not recognise the `Text` primitive type, and the bundled string utility natives are not useful because no script can produce a string argument.

Hosts that want script-side string concatenation, f-strings, and the bundled utility natives (`to_string`, `concat`, `slice`, `length` against text) enable the feature explicitly in their `Cargo.toml`.

````toml
[dependencies]
keleusma = { version = "0.2", features = ["text"] }
````

The `keleusma-cli` crate enables the feature for the CLI runner and the REPL, so users running scripts from the command line do not have to think about the feature. Embedding hosts that target small embedded runtimes and do not need scripts to manipulate text get a smaller compiled artifact by leaving the feature off.

That said, real applications routinely need some string work in context. **The recommended pattern is to register native Rust functions that perform the string work and expose them to the script.** Rust's standard library handles formatting, splitting, regex, encoding conversion, and Unicode operations far better than anything reasonable to build inside the script. The host writes a small Rust function, registers it with one `register_fn` call, and the script gets a single `use` declaration that yields native performance and full Rust ecosystem access.

````rust
// Rust host code.
use keleusma::{Arena, Value, vm::Vm};
use keleusma::utility_natives::register_utility_natives;

let mut vm = Vm::new(module, &arena)?;
register_utility_natives(&mut vm);

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
//         let cleaned = trim(name);
//         let first = split_first_word(cleaned);
//         f"hello, {upper(first)}!"
//     }
````

The host owns the string-handling vocabulary; the script consumes it through `use` declarations. This is the same registration pattern that exposes the bundled `concat`, `to_string`, `length`, and `slice` helpers, applied to whatever string operations the application actually needs. See [EMBEDDING.md](./EMBEDDING.md) for the full native-registration surface.

The following items collect the string-related rough edges still visible in V0.1.x for callers who do use the bundled string helpers.

### F-strings require `use concat` and `use to_string`

The f-string syntax `f"text {expr}"` desugars at lex time into a chain of `concat` and `to_string` native function calls. The desugaring runs before the type checker sees the program, so the script must import the two functions or compilation fails with `undefined function 'concat'` or `undefined function 'to_string'`.

````
use concat
use to_string

fn greet(name: Text) -> Text {
    f"hello, {name}!"
}
````

The CLI runner pre-registers both functions, but the type checker still requires the `use` declarations for the script's own type-resolution pass. A future release may auto-inject these `use` declarations when the lexer emits f-string desugaring; until then, the declarations are user-visible.

### Literal `{` and `}` in f-strings

Use backslash escapes inside an f-string.

````
use concat
use to_string

fn main() -> Text {
    f"open\{brace\}close"
}
````

This produces the literal string `open{brace}close`. Outside f-strings (in plain `"..."` strings) the braces are ordinary characters and do not need escaping.

### Empty interpolation `{}`

`f"hi {}"` is rejected at lex time with `empty f-string interpolation '{}'`. Whitespace-only interpolation such as `f"{   }"` is rejected the same way. Write an expression between the braces, or use `\{` and `\}` for literal braces.

### Complete escape table

| Escape | Result | Where |
|--------|--------|-------|
| `\n` | newline (`U+000A`) | string and f-string |
| `\t` | tab (`U+0009`) | string and f-string |
| `\r` | carriage return (`U+000D`) | string and f-string |
| `\\` | literal backslash | string and f-string |
| `\"` | literal double quote | string and f-string |
| `\0` | null byte | string and f-string |
| `\{` | literal `{` | f-string only |
| `\}` | literal `}` | f-string only |

All other characters that are not special in source (single quotes, dollar signs, hash marks, ordinary Unicode) appear directly without escaping. Any other backslash sequence is a lex error.

### Where text works

- **Static string literals.** Compiled to `Value::StaticStr` and reside in the rodata region of the loaded image. May flow anywhere admissible, including across the yield boundary in the dialogue type.
- **Arena-resident dynamic strings.** Produced by `Op::Add` on text operands and by the bundled `concat`, `slice`, and `to_string` natives. Carried as `Value::KStr` handles that resolve through the host-owned arena and become stale on the next arena reset. Subject to the cross-yield prohibition. See [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) for the full text-type discipline.

The `Value::DynStr` global-heap variant present in V0.1.x was removed in V0.2.0. All dynamic text is now arena-resident.

For text operations beyond what the bundled `register_utility_natives` provides (`to_string`, `length`, `concat`, `slice`), host applications register their own natives.

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

### Local bindings are immutable

`let` bindings cannot be rebound or mutated. The data segment is the only region of mutable state observable to a script, and it is accessible only from a `loop`-classified entry point. Accumulation across a loop iteration in an atomic-total `fn` is therefore not possible without either (a) a `loop main` script using the data segment, or (b) a host-side fold native. See [WHY_REJECTED.md](./WHY_REJECTED.md) under the recursive-closure entry for examples of both rewrites.

### Closures compile but the safe verifier rejects them

The compile pipeline accepts closures with environment capture; the safe constructor `Vm::new` rejects programs that invoke them through `Op::CallIndirect` because indirect dispatch cannot be statically bounded. This is the conservative-verification stance, documented in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification). The valid form of unbounded execution is the top-level `loop` block enforced by the productivity rule. Closures exist in the language so the rejection can be precise.

### Pipeline operator requires parentheses

The right-hand side of `|>` must be a function call with parentheses, even when the function takes no additional arguments. `expr |> f` is a parse error; `expr |> f()` is correct.

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

### Bytecode 0.1.0 was yanked

`keleusma 0.1.0` was yanked from crates.io within hours of publication because its declared MSRV of 1.87 conflicted with let-chain syntax in the source that requires Rust 1.88. `keleusma 0.1.1` is the corrected initial release; `Cargo.lock` files referencing 0.1.0 continue to resolve but new `cargo add keleusma` invocations pick 0.1.1.

## Where to look for more

- The full language reference is [GRAMMAR.md](../design/GRAMMAR.md) (descriptive; the normative reference is the parser at `src/parser.rs`).
- The verifier rejection catalog is [WHY_REJECTED.md](./WHY_REJECTED.md).
- The embedding API surface is [EMBEDDING.md](./EMBEDDING.md).
- The conservative-verification stance is in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification).

When in doubt about whether a behaviour is intended or a bug, the parser, type checker, and verifier are authoritative; the documentation is descriptive.
