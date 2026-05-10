# Backlog Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Deferred decisions for future consideration. These are explicitly out of scope for the current development phase.

## ~~B1. Hindley-Milner type inference~~ (Resolved)

Hindley-Milner is in place in `src/typecheck.rs`.

Foundation. The `Type` enum carries a `Var(u32)` variant. `Subst` maps type variables to types. `unify` implements Robinson's algorithm with the occurs check. `VarGen` allocates fresh type variables. The typing context carries the substitution and variable allocator across a function check.

Integration. `types_compatible` calls `unify` and records relationships in the substitution. Unannotated positions that previously returned `Type::Unknown` now allocate fresh type variables, so constraints propagate across let bindings, function calls, returns, and conditional branches. The substitution-application pass at end of `check_function` resolves locals to their inferred types and rolls back per-function variables so cross-function checking remains independent.

Generic functions (B2) reuse the same machinery: each generic call site instantiates the function's abstract type variables with fresh per-call variables before unifying with actual arguments.

The `Type::Unknown` sentinel is retained as a permissive transitional anchor for runtime-only dispatch positions (such as native function call results without declared signatures). Removing it would require declaring native signatures, which is recorded as future work in the typecheck module documentation.

## ~~B2. Generic type parameters and trait bounds~~ (Resolved for declaration, bound enforcement, and impl validation)

Generic functions, structs, enums, traits, trait bounds, and impl signature validation are all in place. Impl methods register as compiled chunks under their mangled name `Trait::TypeHead::method`.

Surface syntax. `fn name<T, U>(args) -> ret { body }`, `struct Name<T, U> { fields }`, `enum Name<T, U> { variants }`, `trait Name { fn method(args) -> ret; }`, `impl Trait for Type { method definitions }`, and `fn name<T: Trait1 + Trait2>(...)` for bounds.

AST. `FunctionDef`, `StructDef`, and `EnumDef` carry `type_params: Vec<TypeParam>`. `TypeParam` carries `bounds: Vec<String>`. `TraitDef`, `ImplBlock`, and `TraitMethodSig` are top-level declarations. `TypeExpr::Named` carries `Vec<TypeExpr>` for generic instantiation references.

Type checking. Generic declarations record abstract `Type::Var` per type parameter. Call sites instantiate fresh per-call variables, unify with arguments, and validate trait bounds against the `impls` registry. Impl method signatures are validated against the trait declaration: arity match, name match. Each impl method is also registered as a compiled chunk under its mangled name `Trait::TypeHead::method`.

Compilation and runtime. Keleusma's runtime-tagged `Value` enum dispatches polymorphically. Generic chunks work for any concrete type. Impl methods are emitted as regular chunks under mangled names. Method call surface syntax `x.method(args)` is parsed as `Expr::MethodCall` and resolved at compile time after monomorphization makes the receiver type concrete. The parser distinguishes method calls from field access by lookahead for `(` after `expr.name`.

No remaining work under this entry. The originally deferred method call surface syntax landed in V0.1-M3-T18 and is now exercised by the monomorphization pipeline end to end.

## ~~B2.4 Compile-time monomorphization~~ (MVP plus inference reach extension)

Monomorphization specializes generic functions per concrete type instantiation. The MVP is implemented in `src/monomorphize.rs` and runs between type checking and compilation in `compile()`.

What lands.

- Call-graph traversal from non-generic functions. The pass walks every call site of a generic function and infers the concrete type arguments from literal arguments and locals with declared types.
- Specialization generation. Each `(function, type_args)` pair clones the generic function and substitutes the abstract type-parameter names with the concrete `TypeExpr` throughout the parameter list, the return type, and the function body (let bindings, casts, struct constructions, and so on).
- Trait method resolution within specializations. After substitution, the receiver of a method call has a concrete type. The compiler's existing `MethodCall` resolution path looks up the impl's mangled name `Trait::TypeHead::method` in the function map and emits a direct call.
- Output. The compiler emits the monomorphic specializations and drops the original generic functions whose specialization was generated. Calls in the program are rewritten to point to the specializations through the mangled names.
- Re-typecheck after monomorphization validates the specialized bodies under their concrete types, which is what allows generic-receiver method calls to resolve.

End-to-end example. `examples/monomorphize_generic_method.rs` compiles and executes `fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }` where the body's method call resolves only after monomorphization specializes `use_doubler` for `T = i64`.

Inference reach extension. `infer_arg_type` now resolves the type of function calls (through a function-return-type map), tuple and array literals, cast expressions, enum variants, the first-arm of if/match expressions, field access expressions, tuple-index expressions, array-index expressions, method calls, unary operator expressions, and binary operator expressions. Generic call sites whose arguments use these shapes specialize correctly. Field-access inference threads a struct table through the rewrite chain and resolves `o.field` against the struct's declared field type, applying per-instance type-argument substitution when the receiver carries concrete type arguments. Abstract field types (those whose declared type is exactly one of the struct's type parameters and the receiver has no type arguments) are guarded against erroneous propagation. Tuple-index inference reads the indexed element type from the inferred tuple type. Array-index inference returns the array's element type regardless of the index value. Method-call inference looks up the impl method's declared return type under a `<head>::<method>` mangling in the function-return map, populated from `program.impls` at the top of monomorphize. Unary-operator inference recurses on the operand for negation and returns Bool for logical-not. Binary-operator inference recurses on the left operand for arithmetic operators and returns Bool for comparison and logical operators.

Generic struct specialization. `specialize_structs` runs after function specialization. For each `Expr::StructInit` whose target struct has type parameters, the pass infers the type arguments by matching declared field types against provided field values' types and emits a specialized `StructDef` with the field types substituted. The `StructInit`'s name is rewritten to the mangled form (for example `Cell__i64`). Subsequent compilation sees the specialized struct as a regular non-generic struct, which lets compile-time field-type inference resolve method dispatch on field-typed receivers. Example: `c.value.double()` where `c: Cell<i64>` now compiles correctly.

Generic enum specialization. `specialize_enums` runs after `specialize_structs` and mirrors that pass for `Expr::EnumVariant` whose target enum has type parameters. The payload values' inferred types determine the type arguments, and the pass emits a specialized `EnumDef` with payload types substituted. Subsequent compilation sees the specialized enum as a regular non-generic enum, which closes the same compile-time inference gap for enum-payload method dispatch that the struct pass closes for fields.

Pruning policy. Generic functions whose specializations were generated are dropped from the program output. Generic functions with no specializations are retained because they continue to execute correctly through runtime tag dispatch on Value tags. This is the safe default for cases like first-class closure arguments where the concrete type cannot be inferred but the function still runs.

Polymorphic recursion cycle detection. Two complementary bounds guard the fixed-point loop. The global `SPECIALIZATION_LIMIT` caps the total number of specializations. The `PER_FUNCTION_LIMIT` caps the number of specializations any single generic function may produce, which is the structural signature of polymorphic recursion. When the per-function bound is reached, the loop exits early and the remaining work is left unspecialized; subsequent compilation will surface the truncation through the bytecode chunk count limit, which produces a clearer error path than infinite expansion.

## ~~B3. Closures and anonymous functions~~ (Resolved with environment capture)

Surface syntax `|args| body` and `|args| -> ret { body }`. Closures capture outer-scope locals and execute end to end through hoisted chunks plus the indirect-call mechanism.

What lands.

- New `Value::Func { chunk_idx: u16, env: Vec<Value> }` runtime-only variant. The `env` carries captured values for closures with capture; non-empty `env` is produced by `Op::MakeClosure`, empty `env` by `Op::PushFunc`.
- New `Op::PushFunc(u16)`, `Op::MakeClosure(u16, u8)`, and `Op::CallIndirect(u8)` instructions.
- Closure hoisting pass walks the program before compilation. For each `Expr::Closure`, the pass collects free variables (identifiers referenced in the body but not bound by the closure's parameters), filters out names declared as natives or qualified with `::`, prepends the remaining names as parameters of the synthetic function, and replaces the closure expression with `Expr::ClosureRef { name, captures, span }`.
- Compiler emits captures: for each name in the `ClosureRef`'s captures list, `GetLocal(slot)` if local, `PushFunc(chunk_idx)` if a top-level function. Then `MakeClosure(synth_idx, n)` if any captures, otherwise `PushFunc(synth_idx)`.
- VM execution. `Op::MakeClosure` pops `n` captures and pushes `Value::Func` with the captured env. `Op::CallIndirect` pops args plus the `Func` value, then pushes the env values back onto the operand stack as implicit arguments before the explicit ones, and invokes the referenced chunk.
- Type checker accepts `ClosureRef` and indirect-call call sites with fresh type variables.

End-to-end. `examples/closure_basic.rs` demonstrates `let f = |x: i64| x + 1; f(41)` returning 42. `examples/closure_capture.rs` demonstrates `let n: i64 = 10; let f = |x: i64| x + n; f(5)` returning 15.

First-class closures as function arguments now work end to end. A generic function `fn apply<F>(f: F, x: i64) -> i64 { f(x) }` accepts a closure and invokes it through the indirect-call mechanism. The compiler resolves the parameter `f` as a local and emits `Op::CallIndirect`. Monomorphization leaves the call generic when the argument's concrete type cannot be inferred (closure types are opaque); the runtime polymorphic dispatch handles invocation.

Nested closures with transitive capture work end to end. When a closure is hoisted, the resulting `Expr::ClosureRef` carries the inner closure's free-variable list. The free-variable analysis for any enclosing closure now treats each entry of an inner `ClosureRef`'s captures list as a free variable of the enclosing expression unless it is bound in the enclosing scope. This propagation lets an inner closure capture a name from an outer-function local through an outer closure's synthetic chunk: the outer closure's hoisted body is given the name as an additional implicit parameter, and at the inner closure's construction site that local is in scope and is captured normally.

Recursive closures via let-binding work end to end. The form
`let f = |...| ... f(...) ...` declares a closure whose body
references its own let-binding name. The hoist pass detects this
pattern in `Stmt::Let` and synthesizes a chunk whose parameter list
is `(other_captures..., self_param, explicit_params...)` where
`self_param` carries the binding name. The compiler emits the new
`Op::MakeRecursiveClosure(chunk_idx, n_captures)` instead of
`Op::MakeClosure`, producing a `Value::Func { recursive: true }`. At
each invocation through `Op::CallIndirect`, the runtime pushes the
closure value itself into the self slot before the explicit
arguments, so references to the binding name inside the body
resolve to the closure value and dispatch through indirect call.
The type checker registers a fresh type variable for the binding
before checking the closure value, allowing the body's
self-reference to type-check rather than failing as undefined.
Recursive closures also support regular captures: the synthetic
chunk's parameter order places captures before the self slot, and
`MakeRecursiveClosure` pops the captures into the env identically
to `MakeClosure`. End-to-end demonstration:
`examples/closure_recursive.rs` computes `fact(5) == 120` using
`Vm::new_unchecked` to opt out of resource-bounds verification.
Bytecode version is bumped to `7`.

WCET and WCMU implications. Recursive closures introduce unbounded
recursion that cannot be statically bounded by the present analysis.
The pre-existing `topological_call_order` walk over the call graph
detects recursion through `Op::Call` only and never traverses
`Op::CallIndirect`, so a self-referential closure escapes the
recursion check by construction. To preserve the soundness of the
WCET and WCMU analyses, `verify::module_wcmu` rejects any module
that contains `Op::MakeRecursiveClosure` with a clear error
message. The safe constructors `Vm::new` and `Vm::load_bytes`
therefore reject recursive-closure programs by default. Hosts that
need recursive closures and accept the unbounded-recursion risk
must construct the VM through `Vm::new_unchecked` or
`Vm::load_bytes_unchecked`. A future refinement could add a
recursion-depth attestation API analogous to `set_native_bounds`,
multiplying the closure's per-invocation WCET and WCMU by the
declared depth, but that is not implemented and is recorded as a
follow-on item. For real-time embedding without an external
attestation, recursive closures are out of scope and the safe
constructor's rejection is the correct contract.

Capture by reference disposition. Capture by reference is not meaningful in Keleusma's pure-functional surface. The language's `let` bindings are immutable by design. There is no surface assignment operator that mutates a previously bound local, which means a captured local cannot diverge from the captured snapshot regardless of whether the capture is by value or by reference. The only mutable mechanism is the data segment, which is accessed through `data.field` and `data.field = expr` syntax that is independent of closure capture. A closure that wants to mutate shared state therefore reads and writes data segment slots directly. Capture by reference would only matter in a language with mutable locals, which Keleusma intentionally does not have. The item is therefore closed as not applicable rather than deferred.

## ~~B4. Hot code swap implementation~~ (Resolved as R29)

Hot code swap is implemented through `Vm::replace_module`. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. See R29 in [RESOLVED.md](./RESOLVED.md).

## ~~B5. Structural verification implementation~~ (Resolved as R22, R23)

Structural verification is implemented. See R22 and R23 in [RESOLVED.md](./RESOLVED.md).

## ~~B5b. Static string discipline extensions~~ (Resolved as utility natives)

String values use the two-string-type discipline of `Value::StaticStr` and `Value::DynStr` with the host-owned arena boundary type `Value::KStr` for stale-pointer detection.

Concatenation and slicing land as utility natives in both context-aware and non-context variants:

- `concat(s1: String, s2: String) -> String`
- `slice(s: String, start: i64, end: i64) -> String`

The non-context variants return `Value::DynStr` allocated through the global allocator. The context-aware variants `concat_with_ctx` and `slice_with_ctx` return `Value::KStr` allocated through the host-owned arena's top region. The `_with_ctx` variants resolve `Value::KStr` operands through the supplied arena. Helper functions `string_view_no_arena` and `string_view_with_arena` factor the value-to-string projection. `slice` indexes by Unicode code points, matching the existing `length` semantics, so multi-byte characters are not split. Out-of-range indices return a `NativeError` with a descriptive message.

Formatting beyond `to_string(value)` is provided through f-string interpolation, recorded in B6.

WCET and WCMU implications. Concat and slice produce dynamic strings whose worst-case output length is the sum of operand lengths (`concat`) or `end - start` (`slice`). The verifier treats native function allocations as the per-native attestation supplied through `Vm::set_native_bounds`. Hosts that rely on `verify_resource_bounds` for real-time embedding must declare heap bounds for the registered string natives before constructing the VM through the safe constructor. Without an attestation, the analysis treats the natives as zero-cost, which is unsound for unbounded inputs. This trade-off is consistent with the existing native-attestation contract.

## ~~B6. String interpolation~~ (Resolved as f-string desugaring)

f-string interpolation lands as a lex-time desugaring. The surface syntax `f"text {expr} more {expr2}"` produces a left-associative chain of `concat` and `to_string` calls.

Mechanism. The lexer recognizes `f"..."` ahead of regular identifier lexing. Inside the f-string body, `{...}` markers delimit interpolated expressions. The lexer scans the body, collects alternating literal and interpolation parts, and emits a desugared token stream:

- An empty f-string `f""` produces a single `StringLit("")`.
- A literal-only f-string `f"abc"` produces the bare `StringLit("abc")`.
- A single-interpolation f-string `f"{x}"` produces the tokens for `to_string(x)`.
- A mixed f-string folds left through `concat`, so `f"a{x}b"` produces `concat(concat("a", to_string(x)), "b")`.

Interpolated expressions are recursively tokenized through `tokenize`; the trailing `Eof` is dropped at the splice. Lex errors inside an interpolation propagate to the outer call. The lexer uses a `pending: VecDeque<Token>` buffer so multi-token paths can return through the standard `next_token` interface.

Escape sequences. `\{` and `\}` produce literal braces in the output. The other existing string escapes (`\n`, `\t`, `\r`, `\\`, `\"`, `\0`) work identically to regular string literals.

Limitations. Newlines inside an f-string body or an interpolation are rejected with a clear error message. Unmatched `}` is rejected. Format specifiers (`{x:.2}` and similar) are not supported; only the bare expression form is accepted. Hosts that want richer formatting should provide additional natives.

Dependency note. f-strings desugar to references to the registered `to_string` and `concat` natives. Programs that use f-strings must register the corresponding natives at runtime. The compile pipeline does not detect missing native registrations until VM construction.

## ~~B7. Error propagation through yield~~ (Resolved as resume value pattern)

Bidirectional error handling between host and script does not require runtime mechanism beyond what the existing yield/resume cycle already provides. The host can resume with any `Value`, and the script's yield expression takes that `Value` as its result. Scripts can therefore implement error propagation by typing the resumed value as a script-defined Result-shaped enum or as `Option<T>` and pattern-matching on the variant.

Surface pattern. The script declares an enum like `enum Reply { Ok(i64), Err }` (or any structurally appropriate variant union) and matches on the resumed value:

```text
loop main(input: Reply) -> i64 {
    let reply = yield request;
    match reply {
        Reply::Ok(v) => { /* use v */ }
        Reply::Err => { /* recover */ }
    }
}
```

Host pattern. The host calls `Vm::resume(Value::Enum(...Ok...))` for success and `Vm::resume_err(Value::Enum(...Err...))` for failure. Both are routed through the same operand-stack mechanism. `Vm::resume_err` is a thin wrapper that documents intent and provides a clear API name for the failure case; functionally it is equivalent to `resume`.

Recovery semantics. If the script does not handle the error variant in its match arms, the next operation that consumes the value traps with a runtime type error. This matches Keleusma's general dynamic-tag dispatch contract; it is not a new failure mode introduced by this design. Scripts that want strict recovery wrap their dialogue logic in an exhaustive match.

WCET implications. The pattern introduces no new bytecode or runtime mechanism. Match-arm dispatch is bounded by the number of arms at compile time. The verifier's existing analysis applies unchanged. Hosts that need automatic propagation analogous to Rust's `?` operator can implement that pattern in the script through pattern matching and early `return`; no language extension is required.

## ~~B8. VM allocation model~~ (Resolved as not-applicable)

The originally framed question was whether multiple `Vm` instances should share an arena. Analysis shows that this is incompatible with several existing contracts and unnecessary for the legitimate use cases.

Why a shared arena does not fit. (1) `verify_resource_bounds` checks that a single VM's worst-case memory fits in the arena it was constructed against; sharing the arena across VMs invalidates the per-VM contract and forces budget arithmetic at the host level. (2) `KString` epoch-based stale detection is per-arena, so a reset by one VM would invalidate handles that another VM still holds. (3) `Op::Reset` advances the arena epoch and clears top-region allocations, so two VMs sharing an arena would clobber each other on every reset. (4) The arena is single-threaded by ownership; "many concurrent scripts" implies parallel access, which would require a thread-safe arena and contradicts the bounded-cost design. (5) The cross-yield prohibition on dynamic strings is per-VM and would not extend across VMs sharing an arena.

What the use cases actually need. Allocation overhead amortization across sequential scripts is already supported: the host constructs an `Arena` once and reuses it across successive `Vm::new` calls between full resets. Pooling memory across short-lived scripts under a fixed budget is the same. Reducing global allocator pressure is solved by choosing an allocator, not by sharing an arena. True concurrent multi-tenant scripting on shared memory is incompatible with Keleusma's analysis model and would belong in a different abstraction layer (a global allocator pool), not as an extension of the `Arena` type.

Conclusion. The existing pattern of constructing one `Arena` and reusing it across sequential `Vm` lifecycles covers the practical case without requiring new API. No code change is recorded under this entry; the entry is closed as not-applicable.

## ~~B9. Hot update of yielded static strings~~ (Resolved structurally)

The lifetime concern is structurally avoided in the current implementation. `Value::from_const_archived` materializes archived `StaticStr` constants into owned `String` values at the moment they are pushed onto the operand stack. Yielded values that contain a `Value::StaticStr` therefore hold owned heap data that is independent of the bytecode buffer. A hot update that swaps the buffer through `Vm::replace_module` does not affect the host's retained yield value because the string bytes were already copied out at the lift boundary.

Eager resolution at the lift boundary is the resolution path B from the original design. The trade-off is a heap allocation per `StaticStr` push, which is acceptable for the dialogue surface where yielded values cross out of the VM. Future zero-copy yield paths that retain `&ArchivedString` references in `Value` would re-introduce the concern; if they are pursued, the host-responsibility model from path A is the alternative.

## ~~B11. Per-op decode optimization for zero-copy execution~~ (Resolved as cached Vec)

Option A landed. The VM caches a per-chunk `Vec<Op>` populated at construction and at every `replace_module`. The hot dispatch loop reads from this slice directly through `chunk_op`, which is now a constant-time load. The previous hot-path call to `op_from_archived` for every fetch is gone; that conversion now runs once at construction time.

Implementation. `Vm::decoded_ops: Vec<Vec<Op>>` indexed as `decoded_ops[chunk_idx][ip]`. A new `decode_all_ops` helper walks the archived module's chunks and decodes every op into the cache. Both the owned-bytecode constructor (`Vm::construct`) and the borrowed-bytecode zero-copy constructor (`Vm::view_bytes_zero_copy`) populate the cache. `Vm::replace_module` re-decodes for the new module.

Trade-offs. Cost is one heap allocation per chunk at construction, proportional to the program's total op count. Constants and string data continue to be read on demand from the archived form, so the zero-copy contract for those is preserved. The `Op` type is `Copy`, so the slice access is a trivial load on the hot path. For one-shot scripts the cost is roughly equal to the previous per-fetch decoding; for hot-loop scripts the saving compounds with the iteration count.

Option B (specialized dispatch tables for hot opcodes) was not pursued. The simpler cache approach removes the per-fetch decode cost without the codegen complexity, and benchmark-driven workload analysis would be needed to identify which opcodes are hot enough to merit specialization.

Deferred until profiling identifies the dispatch as a hot path on real workloads. The current implementation is correct and the cost is bounded by the structural verifier's per-op accounting, so this is a performance enhancement rather than a correctness concern.

## ~~B10. Portability and target abstraction~~ (Foundation in place)

Foundation. The compiler now accepts a `Target` descriptor through `compile_with_target`. The target's word, address, and float widths are baked into the resulting module's wire-format header, and the compiler rejects programs that use features unsupported by the target. The current 64-bit runtime accepts bytecode whose declared widths are at most its own; emitting for a narrower target produces bytecode the runtime can still load, with integer arithmetic masked to the declared width via `truncate_int`.

Surface. `crate::target::Target` carries the three width fields and two capability flags (`has_floats`, `has_strings`). Const presets cover common cases: `host` (64-bit, all features), `wasm32` (32-bit word and address, 64-bit floats, full features), `embedded_32` (32-bit, all features), `embedded_16` (16-bit, no floats), `embedded_8` (8-bit-word with 16-bit address space matching the 6502 class, no floats, no strings). Hosts construct a custom `Target` directly when none of the presets fit.

Compile-time validation. `validate_program_for_target` walks the AST looking for float types, string types, float literals, and string literals; programs that use features absent from the target are rejected with descriptive error messages pointing at the offending source span. `Target::validate_against_runtime` rejects targets whose declared widths exceed the runtime's, so a narrower-runtime build can refuse oversized bytecode by construction.

What remains open. The runtime continues to be 64-bit. Target-specific runtime builds (a 16-bit or 8-bit native runtime) are not implemented. The `Value` representation is unchanged; targeting an 8-bit native runtime would require a different `Value` layout and a corresponding execution-loop variant. Target-defined primitive types (`byte`, `bit`, `word`, `address`) are not added to the type system; the existing `i64` continues to be the integer type, with target-declared width controlling arithmetic masking. Cross-target codegen (emitting native 6502 or ARM64 assembly) is out of scope and has not been pursued. The synchronous-language tradition's approach of target-independent intermediate representations feeding target-specific backends is referenced in `RELATED_WORK.md`.

This entry's interaction with B5 (static strings), B9 (hot update of yielded static strings), and the precompiled-code question remains. R39 and the wire format established there cover the cross-environment portability of bytecode artifacts. Full zero-copy execution from `.rodata` is tracked under P10.
