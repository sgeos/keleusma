# Frequently Asked Questions

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document collects surprises that early adopters have run into. The intent is to answer the questions that the rest of the documentation does not yet anticipate, not to be exhaustive.

## Strings

**Strings are not the Keleusma value proposition.** The language's value proposition is definitive Worst-Case Execution Time and Worst-Case Memory Usage verification for embedded real-time scripting. For string-heavy work, a dynamic language with a rich standard library is the better tool. Python, Ruby, JavaScript, or any of the many shell-and-text-processing languages will all handle strings more ergonomically and with more built-in utility than Keleusma. Strings in Keleusma exist as a host-boundary type and as a debugging convenience; they are not the surface to optimise for.

That framing acknowledged, the following items collect the string-related rough edges visible in V0.1.x.

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

`f"hi {}"` lexes successfully and emits a `to_string()` call with zero arguments, which produces a runtime error during execution rather than a parse error at compile time. This is a known V0.1.x rough edge; future releases should reject empty interpolation at the lexer.

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
