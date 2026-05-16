# Frequently Asked Questions

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document collects surprises that early adopters have run into. The intent is to answer the questions that the rest of the documentation does not yet anticipate, not to be exhaustive.

## Strings

**Strings are not the Keleusma value proposition.** The language's value proposition is definitive Worst-Case Execution Time and Worst-Case Memory Usage verification for embedded real-time scripting. For string-heavy standalone work, a dynamic language with a rich standard library is the better tool. Python, Ruby, JavaScript, or any of the many shell-and-text-processing languages will all handle strings more ergonomically and with more built-in utility than Keleusma. Strings in Keleusma exist as a host-boundary type and as a debugging convenience; they are not the surface to optimise for.

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
//     fn greet(name: String) -> String {
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

fn greet(name: String) -> String {
    f"hello, {name}!"
}
````

The CLI runner pre-registers both functions, but the type checker still requires the `use` declarations for the script's own type-resolution pass. A future release may auto-inject these `use` declarations when the lexer emits f-string desugaring; until then, the declarations are user-visible.

### Literal `{` and `}` in f-strings

Use backslash escapes inside an f-string.

````
use concat
use to_string

fn main() -> String {
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

### Where strings work

- **Static string literals.** Compiled to `Value::StaticStr` and reside in the rodata region of the loaded image. May flow anywhere admissible, including across the yield boundary in the dialogue type.
- **Dynamic strings.** Produced by native functions that allocate, or by `concat` and `slice`. Reside in the arena's top region or the global allocator. Subject to the cross-yield prohibition: a value whose static type contains a dynamic string cannot appear in a yield expression. See [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) for the full two-string-type discipline.

For string operations beyond what the bundled `register_utility_natives` provides (`to_string`, `length`, `concat`, `slice`), host applications register their own natives.

## WCMU Coverage

### Exponential string concatenation bypasses the WCMU bound

A program like

````
fn main() -> String {
    let s = "a";
    let s = s + s;
    let s = s + s;
    /* sixty doublings later */
    s
}
````

compiles and is admitted by `Vm::new` even though the value of `s` after sixty doublings would be 2^60 bytes. At runtime the program will exhaust the arena (or the global heap) and produce an allocation failure. The cause is that the WCMU pass does not track string-length information through string-concatenation operations; the operation is modelled as a constant-cost call rather than a doubling of the operand bound.

This is **naive misuse, not a verifier soundness violation against the analysed surface**. The headline guarantee is that the verifier proves a bound for programs that fall within its analysis; string-concatenation length propagation is not yet in that analysis. The honest framing for V0.1.x is:

- **The verifier is sound for what it analyses.** Arithmetic, control flow, calls (with host attestation), array and tuple construction, and the operand stack are tracked.
- **String size growth across `+` and `concat` is not tracked.** Programs that exponentially grow a string through repeated concatenation are not currently rejected at verification time even if their runtime memory exceeds the arena.
- **Recommendation.** Treat string operations as untrusted from the verifier's perspective and bound them at the host. Native attestation of `concat` and any host-side string helpers, plus a host-side margin on the arena capacity, addresses the concern in V0.1.x. Future analysis improvements can move string-length tracking into the verifier.

A worked example of the host-attestation pattern lives in [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs).

### V0.2.0 fail-fast on too-small arenas

The previous releases admitted an `Arena::with_capacity(0)` through `Vm::new` and then aborted the host process via `handle_alloc_error` on the first push. V0.2.0 changes this:

- `Vm::new` and `Vm::new_unchecked` pre-reserve a small minimum operand-stack and call-frame allocation in the arena's bottom region.
- If the arena cannot hold the minimum, both constructors return the new `VmError::OutOfArena` variant rather than aborting.
- The minimum is conservative (four stack slots and one call frame); programs that need more still grow at runtime. Larger programs may still abort on a later push when the arena is exhausted; **full OOM-safe push paths for arbitrary workloads is tracked for V0.2.x**.

For now, the recommendation is to size the arena either through `auto_arena_capacity_for` plus a host-side margin, or with the bundled `DEFAULT_ARENA_CAPACITY` of sixty-four kilobytes for typical embedded scripting.

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

The type checker tracks opaque types correctly, but the marshalling layer in V0.1.x has no path for opaque host values to flow across the native function boundary as themselves. The recommended pattern is to pass opaque values through a primitive handle (typically `i64`) that the host translates to and from its real Rust type at the boundary. See [GRAMMAR.md §3](../design/GRAMMAR.md) and §9.

### Bytecode 0.1.0 was yanked

`keleusma 0.1.0` was yanked from crates.io within hours of publication because its declared MSRV of 1.87 conflicted with let-chain syntax in the source that requires Rust 1.88. `keleusma 0.1.1` is the corrected initial release; `Cargo.lock` files referencing 0.1.0 continue to resolve but new `cargo add keleusma` invocations pick 0.1.1.

## Where to look for more

- The full language reference is [GRAMMAR.md](../design/GRAMMAR.md) (descriptive; the normative reference is the parser at `src/parser.rs`).
- The verifier rejection catalog is [WHY_REJECTED.md](./WHY_REJECTED.md).
- The embedding API surface is [EMBEDDING.md](./EMBEDDING.md).
- The conservative-verification stance is in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification).

When in doubt about whether a behaviour is intended or a bug, the parser, type checker, and verifier are authoritative; the documentation is descriptive.
