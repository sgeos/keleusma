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

End-to-end example. `examples/monomorphize_generic_method.rs` compiles and executes `fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }` where the body's method call resolves only after monomorphization specializes `use_doubler` for `T = Word`.

Inference reach extension. `infer_arg_type` now resolves the type of function calls (through a function-return-type map), tuple and array literals, cast expressions, enum variants, the first-arm of if/match expressions, field access expressions, tuple-index expressions, array-index expressions, method calls, unary operator expressions, and binary operator expressions. Generic call sites whose arguments use these shapes specialize correctly. Field-access inference threads a struct table through the rewrite chain and resolves `o.field` against the struct's declared field type, applying per-instance type-argument substitution when the receiver carries concrete type arguments. Abstract field types (those whose declared type is exactly one of the struct's type parameters and the receiver has no type arguments) are guarded against erroneous propagation. Tuple-index inference reads the indexed element type from the inferred tuple type. Array-index inference returns the array's element type regardless of the index value. Method-call inference looks up the impl method's declared return type under a `<head>::<method>` mangling in the function-return map, populated from `program.impls` at the top of monomorphize. Unary-operator inference recurses on the operand for negation and returns Bool for logical-not. Binary-operator inference recurses on the left operand for arithmetic operators and returns Bool for comparison and logical operators.

Generic struct specialization. `specialize_structs` runs after function specialization. For each `Expr::StructInit` whose target struct has type parameters, the pass infers the type arguments by matching declared field types against provided field values' types and emits a specialized `StructDef` with the field types substituted. The `StructInit`'s name is rewritten to the mangled form (for example `Cell__Word`). Subsequent compilation sees the specialized struct as a regular non-generic struct, which lets compile-time field-type inference resolve method dispatch on field-typed receivers. Example: `c.value.double()` where `c: Cell<Word>` now compiles correctly.

Generic enum specialization. `specialize_enums` runs after `specialize_structs` and mirrors that pass for `Expr::EnumVariant` whose target enum has type parameters. The payload values' inferred types determine the type arguments, and the pass emits a specialized `EnumDef` with payload types substituted. Subsequent compilation sees the specialized enum as a regular non-generic enum, which closes the same compile-time inference gap for enum-payload method dispatch that the struct pass closes for fields.

Pruning policy. Generic functions whose specializations were generated are dropped from the program output. Generic functions with no specializations are retained because they continue to execute correctly through runtime tag dispatch on Value tags. This is the safe default for cases like first-class closure arguments where the concrete type cannot be inferred but the function still runs.

Polymorphic recursion cycle detection. Two complementary bounds guard the fixed-point loop. The global `SPECIALIZATION_LIMIT` caps the total number of specializations. The `PER_FUNCTION_LIMIT` caps the number of specializations any single generic function may produce, which is the structural signature of polymorphic recursion. When the per-function bound is reached, the loop exits early and the remaining work is left unspecialized; subsequent compilation will surface the truncation through the bytecode chunk count limit, which produces a clearer error path than infinite expansion.

## ~~B3. Closures and anonymous functions~~ (Removed in V0.2.0 Phase 4)

**V0.2.0 status.** Closures are no longer part of the language. V0.2.0 Phase 4 retired the closure surface syntax, the closure-hoisting compiler pass, the `Value::Func` runtime variant, and the `Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`, and `Op::CallIndirect` opcodes. The type checker now rejects `Expr::Closure` and first-class function references with a diagnostic naming the construct. The historical V0.1 implementation is described below for context; the present runtime no longer admits any of it.

---

Surface syntax `|args| body` and `|args| -> ret { body }` parses, type-checks, monomorphizes, and emits bytecode. The runtime supports first-class function values through `Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`, and `Op::CallIndirect`.

WCET status. **Programs that invoke closures through `Op::CallIndirect` are rejected by the safe verifier.** The static WCET and WCMU analysis cannot follow indirect-dispatch edges through the call graph. `verify::module_wcmu` rejects any module containing `Op::CallIndirect` or `Op::MakeRecursiveClosure`. The construction ops `Op::PushFunc` and `Op::MakeClosure` remain admissible because they produce values that can be yielded or stored without invocation. Only dispatch through `Op::CallIndirect` is the load-bearing rejection. The valid form of unbounded execution is the top-level `loop` block, which the structural verifier admits through the productivity rule.

The presence of the closure feature in the language pipeline despite its rejection by the verifier follows the [Conservative Verification stance](../architecture/LANGUAGE_DESIGN.md#conservative-verification). Closures are described in the surface so that the verifier can reject their invocation definitively. As analysis techniques mature, the second-category rejection of non-recursive closure invocation may be lifted by a flow analysis that admits programs whose indirect-dispatch targets are statically known. Recursive closure construction through `Op::MakeRecursiveClosure` remains in the first category and is rejected without recourse to future analysis.

`Vm::new_unchecked` and `Vm::load_bytes_unchecked` exist for trust-skip of precompiled bytecode that was verified during the build pipeline. Using them to admit unbounded programs at runtime is intentional misuse outside the WCET contract. The closure feature is therefore not part of the WCET-safe surface; programs that need definitive bounds must restrict themselves to direct calls.

What lands.

- New `Value::Func { chunk_idx: u16, env: Vec<Value> }` runtime-only variant. The `env` carries captured values for closures with capture; non-empty `env` is produced by `Op::MakeClosure`, empty `env` by `Op::PushFunc`.
- New `Op::PushFunc(u16)`, `Op::MakeClosure(u16, u8)`, and `Op::CallIndirect(u8)` instructions.
- Closure hoisting pass walks the program before compilation. For each `Expr::Closure`, the pass collects free variables (identifiers referenced in the body but not bound by the closure's parameters), filters out names declared as natives or qualified with `::`, prepends the remaining names as parameters of the synthetic function, and replaces the closure expression with `Expr::ClosureRef { name, captures, span }`.
- Compiler emits captures: for each name in the `ClosureRef`'s captures list, `GetLocal(slot)` if local, `PushFunc(chunk_idx)` if a top-level function. Then `MakeClosure(synth_idx, n)` if any captures, otherwise `PushFunc(synth_idx)`.
- VM execution. `Op::MakeClosure` pops `n` captures and pushes `Value::Func` with the captured env. `Op::CallIndirect` pops args plus the `Func` value, then pushes the env values back onto the operand stack as implicit arguments before the explicit ones, and invokes the referenced chunk.
- Type checker accepts `ClosureRef` and indirect-call call sites with fresh type variables.

Implementation surface. The language continues to support closures end to end at the parse, type-check, monomorphize, and runtime levels. First-class function arguments, environment capture, transitive nested capture, and recursive let-binding self-reference all work through the language pipeline. The runtime executes closures correctly when constructed through `Vm::new_unchecked` because the unsafe path skips the resource-bounds rejection while preserving structural verification. Hosts that have non-real-time requirements may use the unsafe constructor at their own risk, but the language does not advertise closures as part of the WCET-safe surface. The repository does not include closure examples because all such examples either fail at the safe constructor or require the unsafe constructor, and the latter would model a usage pattern outside the language's contract.

Capture by reference disposition. Capture by reference is not meaningful in Keleusma's pure-functional surface. The language's `let` bindings are immutable by design. There is no surface assignment operator that mutates a previously bound local, so a captured local cannot diverge from the captured snapshot regardless of whether the capture is by value or by reference. The only mutable mechanism is the data segment, which is accessed through `data.field` and `data.field = expr` syntax independent of closure capture. The item is closed as not applicable rather than deferred.

## ~~B4. Hot code swap implementation~~ (Resolved as R29)

Hot code swap is implemented through `Vm::replace_module`. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. See R29 in [RESOLVED.md](./RESOLVED.md).

## ~~B5. Structural verification implementation~~ (Resolved as R22, R23)

Structural verification is implemented. See R22 and R23 in [RESOLVED.md](./RESOLVED.md).

## ~~B5b. Static string discipline extensions~~ (Removed in V0.2.0 Phase 3.5)

**V0.2.0 status.** The bundled `to_string`, `concat`, `slice`, and `length` utility natives are no longer registered by `register_utility_natives`. The `Value::DynStr` global-heap variant is gone; all dynamic strings live in the arena as `Value::KStr`. Dynamic-text composition is the host's responsibility through `register_verified_native` or the `register_fn` marshalling layer. The historical V0.1 implementation is described below for context.

---

String values use the two-string-type discipline of `Value::StaticStr` and `Value::DynStr` with the host-owned arena boundary type `Value::KStr` for stale-pointer detection.

Concatenation and slicing land as utility natives in both context-aware and non-context variants:

- `concat(s1: String, s2: String) -> String`
- `slice(s: String, start: Word, end: Word) -> String`

The non-context variants return `Value::DynStr` allocated through the global allocator. The context-aware variants `concat_with_ctx` and `slice_with_ctx` return `Value::KStr` allocated through the host-owned arena's top region. The `_with_ctx` variants resolve `Value::KStr` operands through the supplied arena. Helper functions `string_view_no_arena` and `string_view_with_arena` factor the value-to-string projection. `slice` indexes by Unicode code points, matching the existing `length` semantics, so multi-byte characters are not split. Out-of-range indices return a `NativeError` with a descriptive message.

Formatting beyond `to_string(value)` is provided through f-string interpolation, recorded in B6.

WCET and WCMU implications. Concat and slice produce dynamic strings whose worst-case output length is the sum of operand lengths (`concat`) or `end - start` (`slice`). The verifier treats native function allocations as the per-native attestation supplied through `Vm::set_native_bounds`. Hosts that rely on `verify_resource_bounds` for real-time embedding must declare heap bounds for the registered string natives before constructing the VM through the safe constructor. Without an attestation, the analysis treats the natives as zero-cost, which is unsound for unbounded inputs. This trade-off is consistent with the existing native-attestation contract.

## ~~B6. String interpolation~~ (Removed in V0.2.0 Phase 3.5)

**V0.2.0 status.** The f-string interpolation surface (`f"text {expr}"`) is no longer part of the language. The lexer-level desugaring to `concat` / `to_string` calls is gone. Hosts compose dynamic text through a registered native (typically named `format`) that returns `Value::KStr`. The historical V0.1 implementation is described below for context.

---

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

Surface pattern. The script declares an enum like `enum Reply { Ok(Word), Err }` (or any structurally appropriate variant union) and matches on the resumed value:

```text
loop main(input: Reply) -> Word {
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

What remains open. The runtime continues to be 64-bit. Target-specific runtime builds (a 16-bit or 8-bit native runtime) are not implemented. The `Value` representation is unchanged; targeting an 8-bit native runtime would require a different `Value` layout and a corresponding execution-loop variant. Target-defined primitive types (`byte`, `bit`, `word`, `address`) are not added to the type system; the existing `Word` continues to be the integer type, with target-declared width controlling arithmetic masking. Cross-target codegen (emitting native 6502 or ARM64 assembly) is out of scope and has not been pursued. The synchronous-language tradition's approach of target-independent intermediate representations feeding target-specific backends is referenced in `RELATED_WORK.md`.

This entry's interaction with B5 (static strings), B9 (hot update of yielded static strings), and the precompiled-code question remains. R39 and the wire format established there cover the cross-environment portability of bytecode artifacts. Full zero-copy execution from `.rodata` is tracked under P10.

## ~~B12. WCMU analysis precision for helper-function calls inside short hot loops~~ (Resolved)

Resolved by extending the text-size analysis pass in
`src/text_size.rs` to track per-callee text-ness and propagate
it through the module-level WCMU pass in `src/verify.rs`.

### What landed

Three changes.

1. `src/text_size.rs` adds `ChunkTextAnalysis` (a struct carrying both `heap_alloc: u32` and `returns_text: bool`) and a new public function `analyze_chunk_text(chunk, callee_returns_text) -> ChunkTextAnalysis`. The existing `chunk_text_heap_alloc(chunk) -> u32` is preserved as a thin wrapper that calls `analyze_chunk_text` with an empty `callee_returns_text` slice (the conservative default that assumes every callee may return text). The wrapper is kept for backward compatibility with internal tests.

2. The internal `TextAnalysis::apply_op` method gains a `callee_returns_text: &[bool]` parameter. For `Op::Call(callee_idx, _)`, the method consults this slice. If the callee is recorded as not returning text, the call pushes `TextSize::NotText` to the abstract stack. If the callee is unknown, out-of-range, or marked as returning text, the call falls back to the previous `TextSize::Unbounded` policy. This restores the type-checker's invariant ("either operand NotText implies result NotText" under `Op::Add`) for integer-arithmetic call chains.

3. `src/verify.rs::module_wcmu` maintains a `chunk_returns_text: Vec<bool>` populated in topological order. `compute_chunk_wcmu` now returns the chunk's `returns_text` flag alongside its WCMU. As each chunk is analysed, its flag enters the array so subsequent callers (which appear later in topological order) see it. The text-size analysis for each chunk uses the partially-populated array, which is safe because the topological order guarantees that every callee appears before its callers.

### Soundness

The change preserves soundness. A chunk's `returns_text` flag is set if and only if a non-`NotText` value can reach a `Return` op or the end of the chunk's ops on any execution path. The flag is therefore an upper bound on whether the chunk may return text. Calls to chunks with `returns_text = false` push `NotText` correctly, because the static analysis has proved no text path reaches a return. The host's runtime arena still bounds actual allocation, so any analysis error surfaces as `VmError::OutOfArena` rather than as silent corruption.

### Impact

Song 9 (`examples/scripts/piano_roll/piano_roll_9.kel`) was the load-bearing case. The initial draft used helper functions for `scale_offset`, `chord_degree`, `iter_lead_waveform`, `scale_tonic`, `section_for_iter_tick`, `perc_pattern`, plus a nested `match` computing a scale-aware diatonic third interval through repeated calls to `scale_offset`. The previous policy rejected this with `top = u32::MAX` because two `Op::Call` returns met at an `Op::Add` and both were `Unbounded`. With B12 resolved, the helper-based structure verifies correctly. Song 9's final implementation restores the helper functions per its specification and uses the scale-aware diatonic intervals.

### Tests

The full workspace test suite passes (`cargo test --all-features`). The text-size module's existing tests (`add_known_values`, `add_saturates_to_unbounded_on_overflow`, and the chunk-level tests) continue to pass because `chunk_text_heap_alloc` preserves its API and calls `analyze_chunk_text` with an empty callee-info slice (the conservative default). Verification of all ten bundled songs (`piano_roll_0.kel` through `piano_roll_9.kel`) succeeds.

### Limitation context (for historical reference)

The worst-case-memory-usage analysis in `src/verify.rs` previously rejected some bounded programs whose bound is provable in principle. A short hot per-tick body that calls helper functions returning primitive scalars could produce a verifier rejection with `top = u32::MAX`, even when the helper functions performed only bounded integer arithmetic and allocated no top-of-arena resources. Inlining the same computation directly into the call site eliminated the rejection. The script's semantics and the script's actual worst-case top-of-arena allocation were identical in both versions.

This was a category-2 limitation under the conservative-verification stance documented in `LANGUAGE_DESIGN.md`. The verifier produced no incorrect acceptance and the rejection was safe; the analysis simply failed to compute a tighter bound that the program supported.

### Reproducer

The first two drafts of `examples/scripts/piano_roll/piano_roll_5.kel` triggered the limitation. The minimal pattern is approximately:

```keleusma
fn pattern_position(channel: Word, input: Word) -> Word {
    let base = input / 2;
    let dp = drift_period(channel);
    let extra = if dp > 0 { input / dp } else { 0 };
    (base + extra) % 12
}

fn drift_period(channel: Word) -> Word {
    match channel {
        0 => 0,
        1 => 7200,
        _ => 480,
    }
}

loop main(input: Word) -> Word {
    if input % 2 == 0 {
        for ch in 0..8 {
            let pos = pattern_position(ch, input);
            host::play(ch, pattern_pitch(pos));
        }
    };
    let _ = yield 0;
    0
}
```

Replacing the `for` loop with an unrolled sequence of statements does not resolve the rejection. Replacing the helper-function calls with inlined arithmetic does resolve it. The accepted form replaces the per-channel function-call sequence with:

```keleusma
loop main(input: Word) -> Word {
    if input % 2 == 0 {
        let base = input / 2;
        host::play(0, pattern_pitch(base % 12));
        host::play(1, pattern_pitch((base + input / 7200) % 12));
        // ... etc unrolled, all inlined ...
    };
    let _ = yield 0;
    0
}
```

Both versions have the same number of total instructions, the same arithmetic, the same arena footprint, and the same external behaviour. The verifier accepts the second and rejects the first.

### Hypothesis

The WCMU analysis likely composes the per-call return-stack usage of helper functions with the loop iteration count or with the surrounding scope's allocation profile in a way that produces an upper bound dominated by an unbounded term. Songs 3 and 4 use many helper-function calls successfully, so function calls in general are not the trigger. The trigger appears to be the specific combination of the for-loop iterating across channels, helper functions called per channel, and the relatively small surrounding context in a minimalist script. The combination apparently passes through an analysis branch that does not return a tight bound.

A more rigorous investigation would identify which analysis step in `verify.rs` returns the `u32::MAX` sentinel for this case. The investigation is left to whoever implements the fix.

### Impact

Composers will naturally factor their per-tick bodies into helper functions for clarity. When the verifier rejects this pattern, the only workaround is to inline the computation, which damages code readability. The lesson is also non-obvious; a future author will not know to inline unless they have read the song 5 spec or have hit the failure themselves.

The mitigation cost is low for compositions whose hot bodies are small (song 5 has eight inlined statements per tick), but the cost scales unfavourably for compositions with larger per-tick bodies. A future composer hitting this limitation in a more complex piece would face a substantially worse refactor.

### Recommendation

Improve the WCMU analysis to recognise this pattern. The class of programs to support is helper functions that take primitive arguments, return primitive values, perform only integer arithmetic, and call no allocating natives. These should be analysed as zero-top-allocation regardless of how many times they are called. The improvement is composition-friendly and preserves the verifier's correctness guarantees.

Deferred until the verifier's WCMU pass is revisited. The current workaround (inline the computation) is documented in `docs/extras/SONG_5_SPEC.md` so future authors of minimalist scripts can avoid the failure path without first triggering it.

## ~~B13. Refinement-type compile-time elision through range analysis~~ (MVP resolved)

The MVP landed: literal-argument refinement elision. When a refined newtype constructor is called with a direct integer literal that the compile-time evaluator can prove satisfies the predicate, the runtime call and trap are skipped and the constructor reduces to the inner value. When the literal provably fails the predicate, the compiler rejects the construction at the source span with a diagnostic naming the predicate, the newtype, and the argument.

### What landed

- `TypeInfo::refinement_bodies` caches each predicate's parameter name and tail expression for the eligible subset (single bare-variable parameter, no statements, a tail-expression body).
- `eval_predicate_at_int(body, param_name, value)` is a small structural evaluator over `Expr` that handles literals, identifier substitution for the parameter, integer arithmetic (`+`, `-`, `*`, `/`, `%`), comparison (`==`, `!=`, `<`, `<=`, `>`, `>=`), logical operators (`and`, `or`, `not`), and unary negation. Anything outside this subset returns `None` and the runtime path is preserved.
- The constructor emission path at `compile_call` consults the evaluator before emitting the runtime check. On `Some(true)` the inner value is emitted bare; on `Some(false)` the compile fails with a span-localized diagnostic; on `None` the existing runtime check is emitted.
- Three new tests cover: elision on a provably-true literal, compile-time rejection on a provably-false literal, and continued runtime trap for an out-of-range non-literal argument.

### Follow-ons resolved

- **Tier 1** (constant-folded argument expressions). `Counter(2 + 40)`, `Counter(0 - 1)`, and arbitrary literal-only arithmetic chains fold through the evaluator and route through the predicate check.
- **Tier 2** (let-bound integer constants). `let n = 42; Counter(n)` resolves through a per-function `local_const_values` map populated at let-stmt emission. Chained constants walk the recorded values.
- **Tier 3** (interval lattice on Word). New `src/interval.rs` carries the lattice (closed signed intervals on `i64`, with bounds as `Option<i64>` for the infinity directions). Constructors: `full`, `empty`, `singleton`, `at_least`, `at_most`, `range`. Predicates: `is_empty`, `contains`, `is_subset_of`. Lattice operations: `intersect`, `union`. Transfer functions for `neg`, `add`, `sub`. The predicate decomposer at `compiler::predicate_true_set` handles `true` / `false` literals, comparison against the parameter (on either side), `and` (intersection), and `not` over a single comparison (operator inversion). The constructor emission path consults the lattice when constant-fold fails: `infer_arg_range` walks the argument expression through literals, identifiers, unary negation, addition, subtraction, and newtype-to-underlying casts; when the inferred range is a subset of the predicate's true set, the runtime check is elided; when the ranges are disjoint, the compile fails. Function parameters declared as refined newtypes populate `local_ranges` with the predicate's true set, providing the principal source of non-singleton ranges. Three integration tests cover parameter-range elision, disjoint-range rejection, and graceful fall-through on a predicate the decomposer cannot reduce to a single interval (e.g. one containing `or`).

### Follow-ons resolved (continued)

- **Sign-aware multiplication, division, and modulo transfer.** The lattice gains `Interval::mul`, `Interval::div`, and `Interval::rem`. Multiplication uses corner-product bounding-box analysis (exact for convex intervals on signed integers). Division handles zero-containing divisors by widening to `full()`; `i64::MIN / -1` overflow widens through `checked_div`. Modulo handles the positive-singleton-divisor case tightly and widens otherwise.
- **Disjoint-interval support.** New `IntervalSet` type holds a sorted list of disjoint non-empty `Interval`s with no touching gaps. Parallel constructors, predicates, lattice ops (`intersect`, `union`, `complement` all exact), and transfer functions (`neg`, `add`, `sub`, `mul`, `div`, `rem` distribute pairwise). Predicates with `or`, `!=`, and `not` over compound subexpressions now decompose to convex sets; the constructor-emit site routes through `IntervalSet::is_subset_of` for the subset check.
- **Byte natural-range parameter tracking.** Byte-typed parameters populate `local_ranges` with `[0, 255]`. A cast `b as Word` carries the range through; constructors whose predicate's true set covers `[0, 255]` admit. Byte and Fixed&lt;N&gt; refinement predicates over the source surface remain blocked by a separate type-checker gap (integer literals default to Word; `x >= 0` on a Byte does not type-check). Closing that gap is recorded as a type-checker follow-up; the lattice infrastructure is ready when the surface supports it.
- **Cross-function return-range summaries.** New per-function `function_return_ranges` map computed at the top of `compile` through a fixed-point pass. Each function's summary is the `IntervalSet` covering every value its body might return under the parameters' natural / refinement ranges. The summary computation respects existing summaries on calls within the body, so chained calls converge in one or two sweeps. The constructor-emit site's `infer_arg_range` consults the summary at `Expr::Call` sites; the customer is `Counter(some_function())` where `some_function`'s body is decidable under the lattice.

### Follow-ons resolved (final pass)

- **Byte/Fixed literal coercion.** The type checker rewrites integer literals at binary-operator sites when one operand is Byte (and the literal fits in `[0, 255]`) or Fixed&lt;N&gt;. The literal is wrapped in an `Expr::Cast` so the existing same-type dispatch and downstream emit produce the correct conversion. Byte-typed refinement predicates now compile.
- **Match-arm conditional range narrowing.** `infer_arg_range_with` gained a shadow-map parameter and a new `Expr::Match` arm; each arm intersects the scrutinee range with the arm's pattern range and binds a variable pattern to the narrowed range. The summary-pass equivalent in `eval_expr_to_range` gained matching `Expr::If` and `Expr::Match` arms.
- **Widening for recursive function summaries.** New `Interval::widen` and `IntervalSet::widen` operators (Cousot-Cousot style) widen growing bounds to infinity. The function-summary pass seeds every function to `IntervalSet::empty` and uses widening after `WIDEN_AFTER_ITERATIONS` rounds to converge on recursive bodies. Recursive functions get a sound compile-time summary even though the WCMU verifier rejects them at load time in V0.2; the widening infrastructure is in place for future passes that admit a relaxed WCMU bound or trust-skip recursion.

### No remaining B13 follow-ons

All open items in the B13 entry are now resolved. The refinement-elision pass admits literal-folded arguments, let-bound integer constants, arithmetic on the above, function-parameter ranges (including refined-newtype and primitive Byte natural ranges), function-call return ranges (computed through a fixed-point pass with widening), and match-arm narrowed bindings.

## ~~B14. CallIndirect flow analysis for non-recursive closure invocation~~ (Closed; not applicable after V0.2.0 Phase 4)

V0.2.0 Phase 4 retired the closure surface and the four closure opcodes (`Op::CallIndirect`, `Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`) along with the `Value::Func` runtime variant. There is no `Op::CallIndirect` site left for a flow analysis to admit; closure-shaped expressions are rejected at the type-checker stage with a diagnostic that names the construct. The historical investigation is preserved below for context.

---

The structural verifier's conservative-verification stance rejects programs containing `Op::CallIndirect` because the dispatch target is not statically known. For closures whose call sites are reachable from a finite set of `Op::PushFunc` and `Op::MakeClosure` instructions, a flow analysis could lift the rejection by tracking the points-to set of every indirect-dispatch site.

### Approach

- Per-stack-slot abstract interpretation. The text-size analysis pass already tracks lattice values per operand-stack position through the compiled bytecode; extend the lattice with a "function pointer" element that carries the set of `chunk_idx` values it might refer to.
- Per-call-site target set. At each `Op::CallIndirect`, the points-to set of the topmost operand-stack slot is the set of possible target chunks. When the set is finite and the WCET and WCMU of each target is statically bounded, the call site can be admitted with cost equal to the maximum over targets.
- Conservative fallback. When the points-to set is unbounded or contains a cycle through `Op::MakeRecursiveClosure`, retain the current rejection.

### Out of scope

- Recursive closures (`Op::MakeRecursiveClosure`). The first-category rejection (unbounded recursion through indirect dispatch) remains; the analysis cannot lift it without separate termination arguments.
- Closure dispatch through data segments or yields. The MVP analyses purely intra-chunk flow; closures stored in data segments or passed across yields require an interprocedural extension.

## ~~B15. Remove `Type::Unknown` entirely~~ (Resolved)

The `Type::Unknown` variant is gone from the runtime crate. The refactor proceeded in three phases:

1. **Drop the placeholder underlying from `Type::Newtype`.** The variant changed from `Newtype(String, Box<Type>)` to `Newtype(String)`. The authoritative underlying lives in `Ctx::newtypes`; the boxed placeholder was dead weight. This eliminated the largest `Type::Unknown` producer (the resolver at `from_expr_with_params_and_frac`).

2. **Convert remaining producers to fresh type variables.** The two `unwrap_or(Type::Unknown)` sites in the newtype-construction paths now route through `ctx.fresh()` when the newtype is not yet recorded in `ctx.newtypes`.

3. **Remove the `types_compatible` wildcard short-circuit and drop the variant.** Every `Type::Unknown` consumer arm collapsed into the surrounding `Type::Var(_)` arm; standalone `Type::Unknown => ctx.fresh()` arms were deleted because the now-uniform `Type::Var(_)` arm already covers them. The cast wildcard `(Type::Unknown, _) | (_, Type::Unknown) => to_ty.clone()` became `(Type::Var(_), _) | (_, Type::Var(_)) => to_ty.clone()`.

All 642 lib tests pass under the new shape. Workspace and doctests clean. Bare-metal STM32N6570-DK full-pipeline build verified.

The companion type-system invariant tightens accordingly: every unannotated position now produces a fresh `Type::Var` through `Ctx::fresh`, and inference proceeds uniformly through unification.

## B16. Parametric `Vm<W, A, F>` for sub-64-bit native runtimes

The chosen design: parameterize `Vm` over three traits — `Word`, `Address`, and `Float` — corresponding to the wire-format header's `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` declared widths. The bundled runtime defaults to `Vm<i64, u64, f64>`. Sub-64-bit hosts instantiate `Vm<i16, u16, f32>`, `Vm<i8, u8, f32>`, etc. The bytecode-level `Target` descriptor and the existing `truncate_int` machinery handle cross-compilation; this entry covers the *runtime* parameterization so the `Value` memory footprint and arithmetic semantics actually shrink on narrow targets.

### Steps (multi-pass)

1. **Word trait foundation.** `src/word.rs`. Signed-integer abstraction with `wrapping_add`/`sub`/`mul`/`div`/`rem`/`neg`, an associated `Wide` type for the i128-style multiplication intermediate, and `BITS_LOG2` matching the bytecode header. Impls for `i8`, `i16`, `i32`, `i64`. *Resolved in commit `a820607`.*

2. **Address trait.** `src/address.rs`. Unsigned-address abstraction with `BITS_LOG2` matching the wire-format `addr_bits_log2`. Impls for `u8`, `u16`, `u32`, `u64`. *Resolved in commit `af6a307`.*

3. **Float trait.** `src/float.rs`. Floating-point abstraction with `add`/`sub`/`mul`/`div`/`neg` and `BITS_LOG2` matching `float_bits_log2`. Impls for `f32` and `f64`. The trait module is ungated so the parametric `Vm<W, A, F>` shape carries an `F` parameter regardless of the `floats` cargo feature. *Resolved in commits `af6a307`, `25e4a39`.*

4. **`GenericValue<W, F>` parametric runtime value.** `src/bytecode.rs`. Renames the `Value` enum to `GenericValue<W: Word, F: Float>` and adds `pub type Value = GenericValue<i64, f64>` so every existing call site compiles unchanged. Recursive variants (`Tuple`, `Array`, `Struct`, `Enum`, `Func`) carry `Vec<GenericValue<W, F>>`. The `Float(F)` variant remains gated by the `floats` feature; when off, a hidden `_PhantomFloat(PhantomData<F>)` variant satisfies Rust's "type parameters must be used non-recursively" rule. *Resolved in commit `dbd9594`.*

5. **Parameterize `Vm<'a, 'arena, W, A, F>`.** Add the three type parameters with `i64`/`u64`/`f64` defaults. The operand stack becomes `StackVec<'arena, GenericValue<W, F>>`. The data segment becomes `Vec<GenericValue<W, F>>`. The Drop impl, every `impl<'a, 'arena>` block, and every method signature gain the type parameters. Each arithmetic site in the dispatch loop switches from concrete `i64::wrapping_add` etc. to `W::wrapping_add`. The checked-arithmetic opcodes (`CheckedAdd`/`Sub`/`Mul`/`Neg`/`Div`/`Mod`) switch from concrete `i128` to `W::Wide` via `W::widen` and `W::Wide::wide_mul`/`wide_add`. The phantom `_phantom_a: PhantomData<A>` field carries the unused `A` parameter (no Value variant carries an address payload; the parameter is consumed by future opcode evolution and host-side `A::MAX` bound checks). *Resolved on the `V0.2.0-parametric-vm` branch and merged to `v0.2.0` in merge commit `fa68a3f`. Six WIP checkpoints land as a fast-forward-free merge; final commit `e79b91f`. 734 lib tests pass; clippy clean; STM32N6570-DK full pipeline builds. Marshall-tied methods deferred to step 6.*

   **Cascade map** (recorded from an exploratory pass; lets a future session move quickly):
   - Rename `pub struct Vm<'a, 'arena>` → `pub struct GenericVm<'a, 'arena, W, A, F>` with three defaults; add `pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>` alias. Stack/data fields use `GenericValue<W, F>`. ~50 lines.
   - Drop impl gains the three type parameters.
   - `NativeFn`, `NativeEntry`, `NativeCtx` (the latter only if it later carries values) become generic over `(W, F)`. The native registry's `func: NativeFn<W, F>` boxed-closure type uses parametric value slices.
   - Three `impl<'a, 'arena> Vm<'a, 'arena>` blocks become `impl<'a, 'arena, W: Word, A: Address, F: Float> GenericVm<'a, 'arena, W, A, F>`.
   - **F-name collision.** Methods like `register_native_closure<F>`, `register_native_with_ctx_closure<F>`, `register_fn<F, Args, R>` use `F` as their *closure-type* parameter. With the impl-level `F: Float`, the inner `F` shadows or conflicts. Rename inner `F` → `Func` (or any non-conflicting name) in each such method's signature, where-clause, and body. ~7 sites.
   - **Value → GenericValue<W, F>** substitution inside generic impls. The alias `Value = GenericValue<i64, f64>` is still correct in the public API and in the type-checker tests; the substitution is needed only inside the generic impl bodies where the current code uses `Value::Int(n)` patterns. Roughly 150 sites in `vm.rs` (across the dispatch loop, the register / call / return paths, and the natives interface). Mechanical but tedious.
   - **Arithmetic-site retargeting** in the dispatch loop. Patterns like `let (Value::Int(x), Value::Int(y)) = (a, b) => sp!(self, Value::Int(x.wrapping_add(y)))` become `(GenericValue::Int(x), GenericValue::Int(y)) => sp!(self, GenericValue::Int(W::wrapping_add(x, y)))`. The `W::wrapping_add(x, y)` call works because `W: Word` brings the trait method into scope; `x` and `y` are `W` from the pattern. ~30 arithmetic sites including the checked-arithmetic opcodes.
   - **Checked-arithmetic widening**. Currently `let r = (x as i128) + (y as i128); let high = (r >> 64) as i64; let low = r as i64;`. Becomes `let r = x.widen().wide_add(y.widen()); let high = W::from_wide_wrap(r.high_half()); let low = W::from_wide_wrap(r);`. The flag computation `if r >= i64::MIN as i128 && r <= i64::MAX as i128` becomes a range check against `W::MIN.widen()..=W::MAX.widen()`.
   - Construction sites: `Vm::new`, `Vm::new_unchecked`, etc. The two construction-site errors flagged in the exploratory pass are missing-field initializers for `_phantom_a: PhantomData`. Mechanical addition.
   - Iterator-collect mismatches (8 sites): code like `items.iter().map(...).collect::<Vec<Value>>()` becomes `Vec<GenericValue<W, F>>`.
   - Final validation: 734 lib tests pass; no-floats variant passes; bare-metal STM32N6570-DK build.

   **Second-order cascade** (discovered in a deeper exploratory pass):
   - `VmState` itself needs parameterization: it carries `Value` payloads in the `Yielded` and `Finished` variants. Mirror the `GenericValue` pattern: rename to `GenericVmState<W, F>` and add `pub type VmState = GenericVmState<i64, f64>`. Inside generic Vm methods, use `GenericVmState::Foo`.
   - **Marshall layer is concrete on `Value`**. The `IntoNativeFn`, `IntoFallibleNativeFn`, and `KeleusmaType` traits all use the concrete `Value` alias in their signatures. Two options: (i) lift the marshall layer to be parametric over `(W, F)` in a coordinated pass with step 6; (ii) **temporary**: move `register_fn`, `register_fn_fallible`, and `register_library` into a separate `impl<'a, 'arena> Vm<'a, 'arena>` block specialized to the default. Hosts wanting these methods on a narrow Vm wait for step 6.
   - `bytecode::TypeTag::admits` takes `&Value`; lift to be generic over `(W, F)` taking `&GenericValue<W, F>`. The body switches `Value::` patterns to `GenericValue::`.
   - `bytecode::value_from_archived` returns concrete `Value`; lift to be generic, returning `GenericValue<W, F>` via `GenericValue::from_const_archived`.
   - `required_persistent_capacity_for` uses `size_of::<Value>()`; add a generic counterpart `required_persistent_capacity_for_generic::<W, F>(...)` that uses `size_of::<GenericValue<W, F>>()`. The non-generic entry point stays for default hosts.
   - **Operator-trait bounds on `Word` and `Float`.** The current `Word` trait carries `wrapping_*` methods but not the standard `core::ops` operators. Code like `bits >> frac_bits`, `i & 0xFF`, and `x + y` for Float requires the operator traits to be in scope. Add bounds: `Word: ... + BitAnd<Output=Self> + BitOr<Output=Self> + BitXor<Output=Self> + Shr<u32, Output=Self> + Shl<u32, Output=Self>`. Float: `... + Add + Sub + Mul + Div + Rem + Neg<Output=Self>`. These all hold for the i8/i16/i32/i64 and f32/f64 impls.
   - **`as` casts to non-primitive types fail on generic `W`**. Sites like `i as usize`, `x as i128`, `i as f64`, `f as i64`, `(i & 0xFF) as u8` need rewriting to trait-method calls. For `as i128`: use `W::widen()`. For `as i64`: use `W::to_i64()`. For `as usize`: add a `Word::to_usize_checked()` or use `W::to_i64() as usize` with bounds check. For `as f64`: use `F::from_f64(W::to_i64(i) as f64)`. For `as u8` after a mask: use a new helper or `W::to_i64() as u8`.
   - **Type-mismatch volume**. After the mechanical substitutions described above, expect ~60 remaining type mismatches centered on the dispatch loop's match arms and the `set_data` / `replace_module` constructor variants. Most are fixable by ensuring `val` variables come from `self.pop()` (which is generic and returns `GenericValue<W, F>`) rather than concrete `Value` literals.

   **Estimated effort**: 4-8 hours of focused work in a dedicated session, given the cascade map. Two exploratory passes (in commits `6ffa770` and the present commit) demonstrated the work is mechanical but voluminous; a single conversational turn cannot complete it safely.

6. **Parameterize the marshall layer and `KeleusmaType`.** The native ABI (`IntoNativeFn`, `IntoFallibleNativeFn`, `KeleusmaType`, `stddsl::Library`) becomes generic over `(W, F)` (the marshall layer does not depend on `A`; the `Library` trait additionally takes `A` so library impls can opt their bundle into a specific shape). Existing impls quantify universally over `<W: Word, F: Float>` and use trait methods (`W::to_i64`, `W::from_i64_wrap`, `F::to_f64`, `F::from_f64`) to bridge the canonical Rust types (`i64`, `f64`) to the script word and float. The `#[derive(KeleusmaType)]` macro emits universal impls. The standard `stddsl` bundles (`Math`, `Audio`, `Text`, `Shell`) implement `Library<i64, u64, f64>` because their inner closures pin `f64`; hosts targeting narrow runtimes write their own `Library<W, A, F>` impls. The `register_fn`, `register_fn_fallible`, and `register_library` methods move back into the generic `impl<W, A, F> GenericVm` block. *Resolved on `v0.2.0`. 734 lib tests pass; 17 marshall integration tests pass; clippy clean; STM32 build clean.*

   **Ergonomic note**: at host call sites that use the bundled `Vm<'a, 'arena>` (= `GenericVm<i64, u64, f64>`), `vm.register_fn("name", |x: i64| ...)` continues to work with no turbofish because `W` and `F` are concrete from the receiver's type. Free-standing test calls like `let v = p.into_value();` need a type ascription `let v: Value = p.into_value();` so Rust can pick the `KeleusmaType<i64, f64>` impl from the universal family.

7. **Demonstrator `Vm<i16, u16, f32>` plus cookbook recipe.** Worked example at `examples/narrow_runtime.rs` exercises `GenericVm<i16, u16, f32>` against bytecode compiled with `Target::embedded_16()`. Three scenarios: plain arithmetic (1 + 2 = 3 as i16), wrapping at the word boundary (30_000 + 10_000 = -25_536 in i16), and host-side `register_fn` with a natural Rust `i64` closure that the marshall layer truncates to `i16`. Integration test at `tests/narrow_vm.rs` pins all three. Cookbook recipe added at `book/src/COOKBOOK.md` under *Narrow-runtime type alias*, documenting the `type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>` pattern, the host-function marshall-widening behaviour, the standard-library-bundle bound to the default shape, and the word-width arithmetic discipline. *Resolved on `v0.2.0`.*

8. **Soundness-closure follow-up pass.** After steps 1-7 closed the public API, three residual gaps remained from the gap-audit pass and now land together. *Resolved on `v0.2.0`.*

   - **Load-time width validation.** `Vm::new`, `Vm::new_unchecked`, and `Vm::view_bytes_zero_copy` now validate that the bytecode's declared `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` are each at most the runtime's `<W as Word>::BITS_LOG2`, `<A as Address>::BITS_LOG2`, and `<F as Float>::BITS_LOG2`. The narrow Vm previously admitted wider bytecode and silently truncated constants through `Word::from_i64_wrap`; the new check rejects the mismatch as `VmError::VerifyError`. The `Address` parameter `A` now carries runtime semantics through this check. `replace_module_inner` also runs the check so hot-swap respects the same soundness property.
   - **Standard-library bundle lift.** `stddsl::Math` and `stddsl::Audio` lift to `impl<W: Word, A: Address> Library<W, A, f64>`. The inner `math::register` and `register_audio_natives` quantify over `W` and `A` and pin `F = f64` because their closures use `f64` arguments. Hosts that combine a narrow `Word` (i16, i32) with `f64` floats can now register these bundles directly.
   - **`Word::to_usize_checked`** helper added with a default impl that delegates to `to_i64` and `usize::try_from`. Mirrors `Address::to_usize_checked` and gives custom `Word` impls a uniform conversion path. Two new unit tests pin the conversion for the positive and negative branches across `i8`, `i16`, and `i64`.

   Integration coverage in `tests/narrow_vm.rs`: width-mismatch rejection (`narrow_runtime_rejects_wider_word_bytecode`, `wider_float_bytecode_rejected_by_f32_runtime`), lifted Math bundle on `GenericVm<i16, u16, f64>` (`narrow_runtime_can_register_math_library_via_lifted_impl`), a runtime whose Float type is f32 running matching bytecode through `register_fn` (`narrow_float_runtime_runs_f32_bytecode`), checked-arithmetic on the narrow i16 runtime exercising `Word::widen`/`WideWord` (`narrow_runtime_checked_arithmetic_exercises_word_widen`), and hot-swap width-mismatch rejection (`narrow_runtime_rejects_hot_swap_to_wider_bytecode`).

9. **Text and Shell library lift.** `stddsl::Text` and `stddsl::Shell` move from `Library<i64, u64, f64>` to `impl<W: Word, A: Address, F: Float> Library<W, A, F>`. `register_utility_natives` and `stddsl::shell::register` quantify over `(W, A, F)`. Every utility native (`native_to_string_with_ctx`, `native_length_with_ctx`, `native_concat_with_ctx`, `native_slice_with_ctx`, `native_println`, plus the helpers `render_value_to_string`, `read_string_arg`, `check_arity`, `read_i64_arg`, `finalize_string_result`) quantifies the same way. Pattern arms switch from `Value::` to `GenericValue::`; integer-payload formatting bridges through `Word::to_i64` so any narrow word type produces the same numeric rendering; length values returned by `length` wrap through `Word::from_i64_wrap` so they fit the runtime's word width. The same lift applies to all five shell natives (`getenv`, `has_env`, `run`, `run_checked`, `exit`); the exit-code argument bridges through `Word::to_i64` and the `(exit_code, stdout)` tuple's word component is wrapped through `Word::from_i64_wrap`. A narrow runtime test (`narrow_runtime_can_register_text_library_via_lifted_impl`, gated on the `text` feature) pins the lift on `GenericVm<i16, u16, f64>` calling `length("hello")` to obtain `5_i16`. *Resolved on `v0.2.0`.*

10. **Math and Audio lift to generic Float; documentation pass.** `stddsl::Math` and `stddsl::Audio` move from `Library<W, A, f64>` to `impl<W: Word, A: Address, F: Float> Library<W, A, F>`. The inner `math::register` and `register_audio_natives` quantify the same way. The closures still use `f64` arguments and returns; on a runtime whose `F` is `f32`, every closure argument and return passes through `Float::from_f64` / `Float::to_f64` at the marshall boundary, narrowing constants and intermediates. The narrowing is mathematically defined and silent; programs that require full `f64` precision should select an `f64`-Float runtime rather than relying on the narrowing. A new test (`f32_narrow_runtime_can_register_math_library_via_lifted_impl`) pins `math::sqrt(9.0) = 3.0_f32` on a `GenericVm<i64, u64, f32>`. Documentation pass on the architecture and design knowledge graph: the *Narrow-runtime type alias* recipe in `book/src/COOKBOOK.md` is rewritten to reflect that all four `stddsl` bundles work on narrow runtimes; the stale "current 64-bit Keleusma runtime" prose in `docs/architecture/LANGUAGE_DESIGN.md` is replaced with parametric-aware text; the `i128` literal in the checked-arithmetic section becomes `W::Wide` with a concrete mapping table; the bytecode-load section in `docs/architecture/EXECUTION_MODEL.md` distinguishes the binary's framing-level upper bound from the per-Vm bound and explains how the two compose; the primitive-type tables in `docs/spec/TYPE_SYSTEM.md` and `docs/spec/GRAMMAR.md` annotate the `Word` and `Float` sizes as defaults that vary under the parametric shape. *Resolved on `v0.2.0`.*

11. **Verifier precision and ancillary test coverage.** Four post-audit follow-ups land together.

    - **Verifier `value_slot_bytes` threading.** The WCMU analysis previously used the hard-coded `VALUE_SLOT_SIZE_BYTES = 32` constant regardless of the runtime's chosen `(W, F)`. The `verify_resource_bounds_with_cost_model` entry point accepted a `CostModel` but ignored its `value_slot_bytes` field. The internal `wcmu_region`, `wcmu_subregion`, and `compute_chunk_wcmu` functions now thread `value_slot_bytes: u32` through; new public variants `module_wcmu_with_value_slot_bytes`, `wcmu_stream_iteration_with_value_slot_bytes`, and `verify_resource_bounds_with_natives_and_value_slot_bytes` expose the parameter. The cost-model entry point now honors `cost_model.value_slot_bytes` through this plumbing. `Vm::new_with_options` and `replace_module_inner` pass `core::mem::size_of::<GenericValue<W, F>>() as u32` so the WCMU bound matches the runtime's actual slot footprint. On a `GenericVm<i16, u16, f32>` whose `Value` enum is materially smaller than 32 bytes, the verifier now admits programs that would previously have been rejected as exceeding the conservative bound. The public-API functions `module_wcmu`, `wcmu_stream_iteration`, `verify_resource_bounds_with_natives`, and `verify_resource_bounds` retain their signatures and delegate with the 32-byte default.
    - **Audio bundle narrow-runtime test.** New `narrow_runtime_can_register_audio_library_via_lifted_impl` pins `audio::midi_to_freq(69) = 440.0_f64` on `GenericVm<i16, u16, f64>`. Belt-and-suspenders coverage of the lift code path (was previously verified by symmetry with the Math bundle test).
    - **Zero-copy regression tests.** Two new tests pin the load-time width check on the `view_bytes_zero_copy` path. `narrow_runtime_view_bytes_zero_copy_runs_embedded_16_bytecode` runs a narrow runtime against narrow precompiled bytes through the zero-copy entry point. `narrow_runtime_view_bytes_zero_copy_rejects_wider_bytecode` confirms that wider bytecode is rejected on the same path (matching the `Vm::new` rejection behavior).
    - **`Vm<i8>` end-to-end smoke tests.** Two new tests exercise an 8-bit signed-Word runtime end-to-end against `Target::embedded_8()` bytecode. `i8_narrow_runtime_runs_embedded_8_bytecode` confirms `100 + 27 = 127_i8` (fits `i8::MAX`); `i8_narrow_runtime_wraps_at_i8_boundary` confirms `100 + 28 = -128_i8` (wraps via `Word::wrapping_add`). *Resolved on `v0.2.0`.*

12. **Binary-build narrowing features for runtime maximums.** The framing-level constants `RUNTIME_WORD_BITS_LOG2`, `RUNTIME_ADDRESS_BITS_LOG2`, and `RUNTIME_FLOAT_BITS_LOG2` in `src/bytecode.rs` previously held the build-time-fixed value `6` (i64, u64, f64). The change introduces seven Cargo features that lower the constants on builds shipping only narrow runtimes. The feature set is `narrow-word-8`, `narrow-word-16`, `narrow-word-32`, `narrow-address-8`, `narrow-address-16`, `narrow-address-32`, and `narrow-float-32`. The narrowest enabled feature wins per dimension; absence of any narrowing feature retains the default of `6`. The narrowing affects the framing-level check inside `Module::access_bytes` and `Module::from_bytes`, the widths reported by `Target::host()`, and the binary's compile-time admissibility through `Target::validate_against_runtime`. It does not change opcode dispatch or the parametric `GenericVm<W, A, F>` shape; the per-Vm width check at `<W as Word>::BITS_LOG2` continues to apply on top of the framing-level rejection. Tests that exercise i64-boundary behavior (Q31.32 fixed-point, i64 checked-arithmetic overflow, golden bytecode bytes, saturate-keyword newtype contracts, embedded_16 admissibility tests) are gated on the absence of the relevant narrowing features so they remain in the default build's matrix but are skipped on narrowed builds. A new test `runtime_width_constants_track_narrowing_features` in `cost_model_tests` pins the constants per feature combination. *Resolved on `v0.2.0`. 737 lib tests pass in the default configuration; 725 on `narrow-word-16`; 720 on `narrow-word-8`. Clippy clean; STM32N6570-DK full pipeline check clean.*

### Status snapshot

All twelve steps complete. Steps 1-4 landed in commits `a820607`, `af6a307`, `25e4a39`, `dbd9594`. Step 5 landed on the `V0.2.0-parametric-vm` feature branch and merged to `v0.2.0` in merge commit `fa68a3f`; six WIP checkpoints from the feature branch travel into trunk as one merge. Step 6 landed on `v0.2.0` in commit `4f7be84`. Step 7 landed on `v0.2.0` in commit `d33fc9d`. Step 8 landed on `v0.2.0` in commit `a89582d`, with a follow-up hot-swap fix and checked-arithmetic test in `9c40b35`. Step 9 landed on `v0.2.0` in commit `e9166fb`. Step 10 landed on `v0.2.0` in commit `89892e4`. Step 11 landed on `v0.2.0` in commit `71095e2`. Step 12 lands alongside this BACKLOG update.

The bundled `Vm<'a, 'arena>` aliases `GenericVm<'a, 'arena, i64, u64, f64>`, so every pre-existing call site compiles unchanged. Hosts targeting narrower runtimes instantiate `GenericVm<i16, u16, f32>` (or any other admissible combination) directly. The worked demonstrator at `examples/narrow_runtime.rs` and the cookbook recipe at `book/src/COOKBOOK.md` document the host-side ergonomics.

Out of scope (recorded for completeness):

- Target-defined primitive surface syntax (`byte`, `bit`, `word`, `address` as user-facing types beyond the existing `Byte`, `Word`, `Fixed<N>`).
- Cross-target native codegen (emitting 6502 or ARM Thumb assembly from Keleusma source).

## ~~B17. Embassy feature trimming~~ (Resolved as not actionable)

A `cargo-bloat` profiling pass measured the microkernel's bare-metal `.text` per feature combination on the STM32N6570-DK target. The findings: **`embassy-stm32` is not the trimming target**. Its symbol contribution is essentially negligible in all three feature modes:

| Mode | Total `.text` | `embassy_stm32` | Share |
|------|--------------:|----------------:|------:|
| Trust-load | 125.5 KiB | 1.9 KiB | 1.5% |
| Verifier-only | 142.5 KiB | 1.9 KiB | 1.3% |
| Full pipeline | 654.8 KiB | 1.9 KiB | 0.3% |

The crate-level breakdown (full pipeline) is dominated by `keleusma` itself at 530.4 KiB (81%). The largest contributors inside the runtime are:

| Symbol | Size | Share of `.text` |
|--------|-----:|-----------------:|
| `Vm::run` | 49.8 KiB | 7.6% |
| `compile_with_target` | 38.4 KiB | 5.9% |
| `typecheck::run_check` | 23.5 KiB | 3.6% |
| `typecheck::type_of_expr` | 22.3 KiB | 3.4% |
| `monomorphize::monomorphize` | 14.7 KiB | 2.2% |
| `compiler::compile_expr` | 12.8 KiB | 2.0% |
| `parser::parse_program` | 12.2 KiB | 1.9% |
| `compiler::compile_block` | 11.4 KiB | 1.7% |
| `parser::parse_postfix_expr` | 10.4 KiB | 1.6% |
| `Vm::new_with_options` | 9.6 KiB | 1.5% |

In verifier-only mode (the production embedded mode), `keleusma` is 95.7 KiB (67%) of 142.5 KiB total; `Vm::run` alone is 49.8 KiB (35%) and `Vm::new_with_options` is another 15.7 KiB. Several BTreeMap monomorphizations at 2-3 KiB each appear across compiler / typechecker / monomorphizer; reducing the variety of key-value type pairs might claw back a few KiB.

### Rationale for closing as not actionable

The V0.2 deferred-items pass already dropped `embassy-stm32`'s `exti` and `unstable-pac` features. The remaining default-on features account for less than 2 KiB total. Disabling more would risk breaking the platform impl without measurable benefit. The profiling concretely demonstrates that further size reduction work belongs against the runtime VM (`Vm::run`) and the compile pipeline (`compile_with_target`, the parser, the type checker), not against embassy.

### Recorded for future passes

Future size-reduction work targeting the runtime should:

- Investigate splitting `Vm::run`'s opcode dispatch (a giant `match` over `Op`) into per-opcode functions. Each opcode's body could become a `#[inline(never)]` function dispatched through a small jump table. This would trade some inline expansion for shared code, potentially shrinking the dispatch surface.
- Audit BTreeMap key-value monomorphizations: 4 distinct instances at 2-3 KiB each (`compiler.rs` produces several). Consolidating to a smaller set of `(K, V)` pairs would reduce the duplication.
- Consider gating the source compiler's monomorphization pass behind a feature when the workload is known not to use generics. The microkernel scripts don't use generics; saving the 14.7 KiB of `monomorphize` would be useful.
- The `Debug` impls visible in verifier-only mode (1.8 KiB for `&T: Debug::fmt`) suggest some inadvertent format-machinery retention even with `floats` off and `text` off. Worth investigating where the residual Debug derives originate.

None of the above require embassy changes.

## ~~B18. Big-number arithmetic worked example using the pattern-arm form~~ (Resolved)

A worked example landed at [`examples/scripts/09_big_numbers.kel`](../../examples/scripts/09_big_numbers.kel) demonstrating two patterns:

- **Full 64x64 -> 128-bit multiplication.** `mul_full(a, b)` reads the high half of the `Op::CheckedMul` i128 intermediate directly. Worked input: `2^32 * 2^32 = 2^64` produces `(high=1, low=0)`.
- **Addition with carry-out.** `add_with_carry(a, b)` returns the wrapped sum together with a carry-out flag derived from the overflow class. Worked input: `i64::MAX + 1` produces carry=1, low=i64::MIN.

The example is exercised by the integration test [`tests/big_number_arithmetic.rs`](../../tests/big_number_arithmetic.rs) (`big_number_example_returns_1`) and documented in the guide at [`book/src/BIG_NUMBERS.md`](../../book/src/BIG_NUMBERS.md) with a discussion of the signed/unsigned caveats, the chained two-digit addition pattern, and the cross-references to the grammar and language-design sections.

Follow-on items that interact but remain out of scope:

- A standard-library `BigInt` type with arbitrary precision and the full arithmetic surface. The worked example demonstrates the underlying pattern; a fully-featured `BigInt` is its own subsystem.

The `Op::CheckedDiv` and `Op::CheckedMod` follow-on landed separately: the checked construct's `/` and `%` paths now route through dedicated opcodes that surface the `i64::MIN / -1` and `i64::MIN % -1` corners through the standard pattern-arm dispatch.

## B19. `Multiword<N, F>` parametric multi-word fixed-point type

> **Status: implemented (V0.2.1).** Originally deferred (the paragraph below is retained as the original rationale), the type was implemented on the `feat-const-generics-bignum` branch. Construction and indexing, scale-independent addition and subtraction, the six comparisons, integer and fixed-point multiply, divide and modulo at every scale, the four shift operators with a constant or runtime-variable amount, and the per-limb bitwise operators (`band`/`bor`/`bxor`/`bnot`) are all in place; the surface operator redesign (`lsl`/`asl`/`lsr`/`asr`, `band`/`bor`/`bxor`/`bnot`, and the boolean `and`/`or`/`xor`/`andalso`/`orelse`) rides on top. `Byte` is admitted by the scalar shift and bitwise operators through promote-operate-truncate masking. The one remaining generality item, recognising `N`/`F` as general const generic parameters rather than a special case, was implemented under B40 (V0.2.1).

> **Reframing (implemented).** Per the pilot decision, Multiword is implicitly fixed-point. The type carries a word count N and an optional fraction-bit count F, written `Multiword<N>` for the integer case (F = 0) or `Multiword<N, F>` for F fractional bits over the same N-word layout. Addition and subtraction are scale-independent and unchanged; the fixed-point shift enters at multiply, which shifts the double-width product right by F, and divide, which shifts the dividend left by F. The integer-only description below is the original design and is superseded on the fixed-point point by this note and by Standard 5.1.2.

### Goal

Provide a first-class fixed-width bignum type whose surface syntax carries the digit count as a const parameter and whose operations compile to per-N inline cascades over the existing checked-arithmetic opcodes. The user-facing surface is the operator set on multi-word signed integers; the compiler emits the bytecode that performs the cascading carry / borrow / partial-product / quotient-estimation work at each use site.

Deferred to a future language version. The V0.3.0 milestone is committed to self-hosting Keleusma's lexer, parser, and compiler as stream processors in Keleusma itself, which is the more load-bearing language-validation target. `Multiword<N>` does not interact with self-hosting; the lexer / parser / compiler bootstrap exercises string handling, pattern matching, bounded iteration, and the coroutine model rather than arithmetic. Implementing `Multiword<N>` in the current Rust compiler now and re-implementing it in the bootstrapped Keleusma compiler later is wasteful; specifying the design now retains the design value at near-zero cost. The mechanical-ness of the per-operation cascades (recorded below) means future implementation will be straightforward in either compiler.

### Surface syntax

```keleusma
// Type expression. N is a positive integer literal, validated at
// type-check time.
fn add(a: Multiword<4>, b: Multiword<4>) -> Multiword<4> { a + b }

// Construction. Two equivalent forms.
let x: Multiword<4> = Multiword::<4>(42, 0, 0, 0);
let y: Multiword<4> = (42, 0, 0, 0) as Multiword<4>;

// Indexing. m[i] returns the i-th Word digit in little-endian
// order (digit 0 is the least significant). Index i is a runtime
// Word with a bounds check `0 <= i < N` that traps on violation.
let lo = x[0];
let hi = x[3];

// Arithmetic operators on Multiword<N> with the same N.
let s = a + b;       // wrapping at the Multiword<N> boundary
let d = a - b;
let p = a * b;       // truncated to Multiword<N>; full 2N width
                     // requires a separate widening multiply op
let q = a / b;
let r = a % b;

// Shift operators with a compile-time-constant Word amount. The
// keyword mnemonics name the arithmetic-versus-logical choice: `lsl`
// logical left, `asl` arithmetic left, `lsr` logical right, `asr`
// arithmetic right.
let l = a lsl 8;
let r2 = a asr 16;

// Bitwise operators, applied limb by limb.
let m = a band b;
let c = bnot a;

// Comparison operators. The result is `Bool`.
let eq = a == b;
let lt = a < b;
let le = a <= b;
let gt = a > b;
let ge = a >= b;
```

### Internal representation

A `Multiword<N>` value lives at runtime as a `Value::Array([Value::Int; N])`. The N entries are signed `Word` digits in little-endian order: digit 0 is the least significant. The top digit's sign bit is the overall sign of the Multiword<N> value. Construction through the tuple-like constructor or the `(tuple) as Multiword<N>` cast emits `Op::NewArray` populated with the N digit values. Indexing `m[i]` reuses the existing `Op::ArrayIndex` opcode; the bounds check is the same as for any array.

The type checker tracks `Multiword<N>` as a distinct nominal type. The runtime representation as `Value::Array` is implementation detail; the surface does not admit treating a `Multiword<N>` as an `[Word; N]` array without an explicit cast.

### Per-operation compilation approach

The compiler emits a specialised inline bytecode sequence per `(operation, N)`. No new opcodes are required; all per-digit work routes through the existing `Op::CheckedAdd`, `Op::CheckedSub`, `Op::CheckedMul`, `Op::CheckedDiv`, `Op::CheckedMod` and the existing array opcodes.

| Operation | Algorithm | Linear / quadratic | Approximate op count at N = 4 |
|-----------|-----------|--------------------|-------------------------------|
| `+`, `-` | N-step cascade. Each digit: `Op::CheckedAdd` (or Sub), unpack `(high, low, flag)`, fold the carry / borrow into the next digit. | Linear in N | ~50 |
| `==`, `!=` | N digit-wise equality reduction via the existing `Op::Eq` and AND. | Linear in N | ~30 |
| `<`, `<=`, `>`, `>=` | Compare from most-significant to least-significant digit. The top digit is signed; lower digits are unsigned. Break on first inequality. | Linear in N | ~80 |
| `*` | Schoolbook: N² partial products via `Op::CheckedMul`, each depositing `(high, low)` at adjacent digit positions, then a carry-propagation pass. The result is truncated to Multiword<N>; full 2N width requires a separate widening multiply op not in this spec. | Quadratic in N | ~250 |
| `/`, `%` | Knuth Algorithm D long division. N quotient-digit estimations, each followed by a multiply-by-divisor and subtract-from-remainder, with a small adjustment step when the estimate is off by 1. Loop bound is N, statically extractable. | Quadratic in N | ~500-800 |
| `<<`, `>>` | Constant shift amount K: split into K/W full-digit shifts and K%W bit-level shifts, unrolled at compile time. Variable amount: bounded runtime loop that consults a Word-typed shift count and applies the digit-level and bit-level steps in turn. | Linear in N for constant K; bounded for variable | ~80 for constant K |
| `m[i]` | Direct `Op::ArrayIndex` with the existing array-bounds-check semantics. | Trivial | ~3 |

### Carry and borrow semantics (correction)

The per-digit cascade must propagate the **unsigned** carry and borrow, not the signed-overflow flag that `Op::CheckedAdd` / `Op::CheckedSub` raise. The `examples/scripts/10_multbyte.kel` worked example uses the signed-overflow flag, which coincides with the true multi-word carry only at the signed-range boundary and gives wrong results otherwise. Concretely, for `Multiword<2>`, `(Word::MAX, 0) + (1, 0)` is `2^63`, whose correct representation is `(Word::MIN, 0)`; a signed-flag cascade would wrongly yield `(Word::MIN, 1)`. The implemented lowering (phase 2) takes the wrapping result limb from the checked opcode and derives the carry from the bit formula `top_bit((x & y) | ((x ^ y) & ~s))` for addition and its dual `top_bit((~x & y) | (~(x ^ y) & s))` for subtraction, extracting the top bit by an arithmetic right shift of `word_bits - 1` and a mask of `1`. The `10_multbyte.kel` example is retained as a signed-carry demonstration and is not the `Multiword` semantics.

### Why no new opcodes are needed

- The checked-arithmetic opcodes (`Op::CheckedAdd` / Sub / Mul / Div / Mod) already produce the `(high, low, flag)` triple that the cascade consumes. The pattern-arm `ok` / `overflow` / `underflow` dispatch surfaces the flag through bytecode without an extra opcode.
- `Op::ArrayIndex` and `Op::NewArray` handle the internal array storage.
- Local slots and `Op::GetLocal` / `Op::SetLocal` carry the intermediate carries, borrows, and partial products between digit steps.
- `Op::If` and `Op::Loop` are sufficient for the comparison short-circuit, the Knuth D adjustment step, and the variable shift loop.

### Phased implementation plan (for the eventual implementation)

| Phase | Scope | Approximate Rust-side effort | Status |
|-------|-------|----------------------------|--------|
| 1 | Lexer + parser + AST + type checker for `Multiword<N>`, tuple constructor, `(...) as Multiword<N>` cast, indexing | ~600 lines | Implemented |
| 2 | `+`, `-`, all six comparison operators | ~500 lines | Implemented |
| 3a | `*` integer, F = 0 (unrolled schoolbook, truncated to N words) | ~300 lines | Implemented |
| 3b | `*` fixed-point, F > 0 (full product, signed correction, right shift by F) | ~200 lines | Implemented |
| 4a | `/`, `%` integer, F = 0 (branchless binary long division, unrolled) | ~350 lines | Implemented |
| 4b | `/` fixed-point, F > 0 (dividend pre-shift left by F) | ~150 lines | Implemented |
| 5 | `lsl`, `asl` (left), `asr` (arithmetic right), `lsr` (logical right), constant amount | ~300 lines | Implemented |
| 6 | Variable (runtime) shift amount, scalar and Multiword (unrolled-over-N runtime-index lowering) | ~250 lines | Implemented |
| 7 | Per-limb bitwise `band`/`bor`/`bxor`/`bnot`; `Byte` shift and bitwise via promote-mask-truncate | ~150 lines | Implemented |

The surface operator spelling was settled in the operator redesign: shifts are the assembly mnemonics `lsl`/`asl`/`lsr`/`asr` (replacing the unpublished symbolic `<<`/`<<<`/`>>`/`>>>`), the bitwise operators are the Erlang-style keywords `band`/`bor`/`bxor`/`bnot`, and the boolean operators are the eager `and`/`or`/`xor`/`not` with the short-circuit control forms `andalso`/`orelse`. Selection is by operator name, never by operand type. The remaining generality item, general const generics for `N`/`F`, was implemented under B40 (V0.2.1).

Each phase ends with end-to-end integration tests at N = 2 (128-bit on the default i64 runtime), N = 3 (192-bit), and N = 4 (256-bit). Earlier phases unblock testing of later phases.

Phase 2 status detail. Addition and subtraction lower to the unsigned carry and borrow cascade recorded in the correction note above. The six comparison operators lower to a branch-free limb-wise fold: two accumulators, one for less-than and one for greater-than, are updated from the least significant word upward so the most significant differing word decides, the top word compared signed by XOR-ing both operands with the word sign bit and the lower words compared unsigned. An unsigned word less-than is exactly the subtraction borrow-out, so the same carry helper serves both arithmetic and comparison, and no new opcode is introduced. The fraction-bit count F is not yet consumed by any implemented operator, so the bound F no greater than N times the word width is deferred to the multiply and divide lowering of phase 3, where F first becomes a shift amount and the target word width is concretely available.

Phase 3a status detail. The integer multiply (F = 0) lowers to an unrolled schoolbook product truncated to the low N words, which for two's-complement operands equals the unsigned product of the same bit patterns truncated to N words. Each word-by-word partial product needs the unsigned double-word result, but `Op::CheckedMul` computes the signed widening product and returns the signed high word, so the high word is corrected to the unsigned high word by the identity `unsigned_high = signed_high + (x < 0 ? y : 0) + (y < 0 ? x : 0)` evaluated mod the word width, the conditional add done branch-free with the arithmetic-shift sign mask `w >> (word_bits - 1)`; the low word is interpretation-independent and taken directly. Partial products are summed by column in the Comba scheme with a two-word accumulator, the low word of which becomes each result word before the accumulator shifts down. A column sum is at most `(2N + 1) * 2^word_bits`, so the two-word accumulator is exact only while `2N + 1 < 2^word_bits`; this admits the full word-count range for every word width of seventeen bits or more and is a real constraint only on eight- and sixteen-bit words, where it excludes only word counts so large that the N-squared unrolling would be impractical anyway. A word count that would overflow the accumulator is rejected at compile time rather than lowered to a silently wrong product, so the failure mode is a clean diagnostic rather than an incorrect result. The correction is exercised on negative digits and negative multi-word values, and the multiply is verified at N = 2 and N = 3 and at the 16-bit word width in `tests/narrow_vm.rs`, including a real digit-product carry into the high word and the accumulator-capacity rejection.

Phase 3b status detail. The fixed-point multiply (F > 0) forms the full 2N-word signed product and shifts it right by F, taking the low N words, because two same-scale operands a and b represent a/2^F and b/2^F, so their raw result is (a*b) >> F. The full product is built in three steps. First the unsigned 2N-word product is accumulated by the same Comba scheme and per-digit unsigned-high correction as the integer multiply, but over all 2N columns. Second the unsigned product U is corrected to the signed product S by `S = U - 2^(N*word_bits) * (a * [b < 0] + b * [a < 0])`, realised as two conditional in-place subtractions from the high N words, each gated by an operand's arithmetic-shift sign mask so a non-negative operand contributes a no-op. Third S is shifted right by F with an arithmetic (sign-extending) shift and the low N words are taken; the shift splits F into a word offset and a bit offset, and because `Op::Shr` is arithmetic the logical part of the sub-word shift is synthesised as an arithmetic shift masked to the low `word_bits - r` bits, with words at or beyond index 2N reading the sign-extension of the top product word. The fraction-bit bound F no greater than N times the word width is enforced here, where F first becomes a shift amount and the target word width is concretely known, so a `Multiword<N, F>` whose F exceeds the value's total bit width is rejected. The arithmetic shift floors toward negative infinity, so a negative product rounds down rather than toward zero, and a product that does not fit in N words wraps, matching the wrapping default of the other multi-word operations; both are documented in Standard 5.1.2 and pinned by tests. Because F is bounded by N times the word width, the shift window never reaches beyond the 2N-word product, so no sign-extension word past the product is materialised. The multiply is verified for the whole-word shift (F = 64), the sub-word shift (F = 32), a purely fractional result, one and two negative operands, the negative-infinity rounding direction, the single-word case (N = 1), the maximum admissible F, the over-bound rejection, and the positive and negative cases at the 16-bit word width in `tests/narrow_vm.rs`.

Phase 4a status detail. Integer divide and modulo (F = 0) are signed with truncation toward zero, matching the scalar Word division: the quotient takes the sign of the operands' exclusive-or and the remainder takes the sign of the dividend. The operands are reduced to their magnitudes by a conditional two's-complement negate gated on each sign mask, an unsigned division runs, and the sign is reapplied to the quotient (for divide) or the remainder (for modulo). A zero divisor traps as a division by zero by dividing one by the bitwise-or of the divisor words through the scalar `Op::Div`, which is zero exactly when the divisor is zero, reusing the existing scalar trap rather than adding an opcode. The unsigned core is branchless binary long division rather than Knuth Algorithm D: the earlier plan named Knuth D, but the constrained instruction set has no mutable array indexing, so every word and bit index must be a compile-time constant, which forces the bit loop to be unrolled and makes the per-digit estimation of Knuth D no simpler than the bit-at-a-time method while being far harder to get right. Each of the N*word_bits bit-steps shifts the running remainder left by one and injects the next dividend bit, tentatively subtracts the divisor, and uses the subtraction's final borrow both as the remainder-less-than-divisor test and as the mask that either keeps the old remainder or takes the difference and sets the quotient bit, so the comparison and the conditional subtraction are one operation and no branch is emitted. Because the loop is unrolled, a division emits code proportional to N*word_bits, so a word count whose bit total exceeds a fixed bound is rejected up front; the bound admits the practical word counts and rejects only values so wide the unrolling would be impractical. Beyond that bound, the compiler's per-chunk op-count cap (`CHUNK_SIZE_HARD_LIMIT`, the u16 control-flow target width) is the backstop: a function whose emitted ops exceed it, whether from one very wide division or several together, is rejected with a clean "decompose into helpers" diagnostic rather than a panic, and an 80% soft-warning precedes it. A runtime-loop or Comba-style rewrite that would remove the code-size scaling is a deferred efficiency optimization, gated on either a mutable-indexing primitive or an N-way selection, and is not a correctness matter. The signed division has the same MIN-over-minus-one wrapping edge as the scalar wrapping division, since the magnitude of the most negative value is not representable; MIN / -1 wraps back to MIN and MIN % -1 is 0, matching `Op::Div`'s `wrapping_div`, and this is pinned by a test. The divide and modulo are verified for the four sign combinations, an exact quotient, a dividend smaller than the divisor, a quotient spanning a word boundary, a three-word case, the zero-divisor trap, the fixed-point rejection, the MIN-over-minus-one wrapping edge, and the divide-and-modulo and word-boundary cases at the 16-bit word width in `tests/narrow_vm.rs`.

Phase 4b status detail. The fixed-point divide (F > 0) and modulo reuse the phase 4a division, generalised with a fraction-bit parameter, rather than a separate lowering. Two same-scale operands a and b represent a/2^F and b/2^F, so the raw quotient representing their ratio is (a << F) / b: the dividend is pre-shifted left by F. The shift is folded into the bit loop rather than materialised, since bit i of the shifted dividend is bit i - F of the magnitude; the loop runs over the N*word_bits + F widened dividend and stores only the low N words of the quotient, the higher bits being the overflow that a fixed-point quotient may produce. The fixed-point modulo needs no shift at all, because a same-scale remainder keeps the scale: raw_a mod raw_b already represents the fixed-point remainder, identical to the integer modulo, so the modulo path ignores F. A fraction-bit count wider than the value is rejected, and the widened dividend is subject to the same unroll-size bound and chunk-op backstop as the integer division. The fixed-point divide is verified for a basic quotient, a purely fractional result, a negative operand, a whole-word dividend shift (F = word_bits), and the zero-divisor trap, the fixed-point modulo for the scale-preserving remainder within a word and across a word boundary, and the divide and modulo at the 16-bit word width in `tests/narrow_vm.rs`.

Phase 5 status detail. The shift operators are a language-wide surface addition, not a Multiword-internal one, because Keleusma had no shift or bitwise operators at all before this phase. Four operators are added, named after the assembly mnemonics: `lsl` logical left shift, `asl` arithmetic left shift, `lsr` logical (zero-fill) right shift, and `asr` arithmetic (sign-preserving) right shift. They apply to a `Word` or a `Multiword<N, F>` value shifted by a `Word` amount, and bind below the comparisons and above the additive operators. The amount is a compile-time constant within the value's total bit width in this increment; a variable amount is a stretch item, and `Byte` shifts are not yet supported.

The keyword-mnemonic naming was chosen after an extended comparative review. The high-assurance language cluster the Justification compares against, Ada and the instruction sets Keleusma lowers toward, makes the arithmetic-versus-logical distinction explicit by name rather than inferring it from operand signedness or distinguishing it by a confusable glyph pair such as the shift-right against shift-right-arithmetic forms. A symbolic first pass used the Verilog convention, but naming the operations after the mnemonics is more legible under a convention where the plain right shift is logical, it dissolves the shift-versus-nested-generics parsing conflict entirely since a keyword never collides with a generic close, and it keeps the whole operator surface consistent with the language's existing keyword operators. The names `asl`, `asr`, `lsl`, `lsr` are the 68000's four shift mnemonics verbatim.

A left shift is a single operation on the data, so `lsl` and `asl` produce the same value, and the arithmetic one earns its distinct name by carrying overflow. `asl` is the value `x * 2^k`, which can exceed the range, so on the word-width type it admits the `overflow` and `underflow` arms of the checked-arithmetic construct, lowering to a checked multiply by the constant `2^k`, while `lsl` is a plain wrapping bit operation. The right shifts and the logical left shift never overflow. On the multi-word type, which has no checked construct, `asl` wraps identically to `lsl`. The overflow capture reuses the existing checked-multiply machinery in full, so no new opcode or fault path is added; the amount for a checked `asl` is bounded to `word_bits - 2` because the multiplier `2^k` must be a positive Word. The capture is verified by tests for the `ok` outcome, the `overflow` and `underflow` outcomes, the low-half binding of the wrapped result, and the `saturate_max` resolution in the overflow arm.

The lowering adds no opcode. A scalar `Word` shift maps to `Op::Shl`/`Op::Shr`, with the logical right shift synthesised as an arithmetic `Op::Shr` masked to the low `word_bits - k` bits since `Op::Shr` is arithmetic. A `Multiword` shift unrolls a per-word shift that splits the amount into a word offset and a bit offset, zero-filling and truncating for the left shift and filling the vacated top with the sign word (arithmetic) or zero (logical) for the right shifts. The shifts are verified for the scalar `Word` (including the arithmetic-versus-logical distinction, the precedence below the additive operators, and the variable-amount and out-of-range rejections) and for `Multiword` (within-word and cross-word left shift, the arithmetic-versus-logical right-shift distinction, a whole-word shift, and the out-of-range rejection), at both the 64-bit and 16-bit word widths.

Fixed-point rounding consistency with the scalar Fixed type. The multi-word fixed-point multiply rounds toward negative infinity and the divide truncates toward zero, an asymmetry that at first reads like an inconsistency between the two. It is not one: the scalar `Fixed<N>` type has exactly the same asymmetry, because its checked multiply shifts the signed product right by the fraction-bit count arithmetically (a floor) while its checked divide computes `(x << frac) / y` with the truncating integer division (toward zero). The `Multiword` lowerings mirror both by construction, so the two families round alike where their ranges overlap, and the multiply-floors, divide-truncates behaviour is an inherited property of the Fixed family rather than a `Multiword` divergence. A fixed-point result that overflows N words wraps, matching both the wrapping default of the other multi-word operations and the wrapped low slot of the scalar Fixed checked arithmetic. The divide's truncation direction is pinned by a test against the scalar-consistent value.

Phase 2 residual closure. The narrow-word case is verified rather than assumed. The lowering computes its sign-bit shift amount from `word_bits - 1` and its sign constant as `1 << (word_bits - 1)`, both derived from the target word width, and on a 16-bit target the sign constant `32768` narrows through `Word::from_i64_wrap` to the i16 sign pattern `0x8000`. The `tests/narrow_vm.rs` suite exercises construction, the unsigned carry, the signed-carry counterexample, and the signed-top-word and unsigned-lower-word comparisons on a `GenericVm<i16, u16, f32>` runtime, so no digit masking is required and none is present. The scratch locals each lowered operation declares (named with a `__mw_` prefix) draw a fresh frame slot per `declare_local` call and are therefore never aliased across nested or sequential operations, verified by a nested-operation test; the cost is that the slots are not reclaimed within a function, so a function performing many multi-word operations inflates its frame slot count and thus its worst-case memory bound. Scratch-slot reuse across operations is a deferred efficiency optimization, not a correctness matter.

### Interaction with other backlog items

- **B16 (parametric Vm).** `Multiword<N>` operations are defined per script-visible Word width, so a `GenericVm<i32, ...>` runtime with `Multiword<4>` carries 128-bit values rather than 256-bit. The compilation cascade is identical; only the digit type changes.
- **B14 (CallIndirect flow analysis).** No interaction. Bignum operations compile to direct bytecode; no indirect dispatch.
- **B10 (target portability) remaining target-defined primitive types.** Independent. Bignum surface does not depend on target-defined primitives.

### Surface-syntax precedent

The `Multiword<N>` syntax is a forward-looking grammar that the rest of the type system does not generalise to (no general const generics on types). The type checker treats `Multiword<N>` as a recognised type-name pattern, not as an instance of a broader parametric-types mechanism. A future grammar pass that adds general const generics could subsume the `Multiword` recognition into a uniform mechanism without changing the user-visible surface.

## B20. V0.2.0 ISA and wire format implementation

R40 in [RESOLVED.md](RESOLVED.md) describes the V0.2.0 ISA and wire format. This backlog entry tracks the implementation work that lands the design in code.

### Scope

The implementation effort spans the compiler, verifier, and runtime. Approximate Rust-side effort:

- **Wire format types and serializer/deserializer.** Define the fixed-size opcode record and operand pool entry layouts. Implement encoding from the in-memory `Module` representation and decoding from the byte stream. Replace the rkyv-based execution wire format with the new layout; retain the rkyv-archived encoding only as an internal cross-process transport mechanism. ~600 lines.
- **Compiler emission updates.** Rewrite `compile_with_target` to emit the new opcode set. Implement the `PushImmediate` encoding (small-integer literals), the `CallVerifiedNative` / `CallExternalNative` split with source-level `use external` parsing, the `PopN(u8)` consolidation, and the consolidated checked-arithmetic emission with `PopN(2)` for the wrapping cases. ~500 lines.
- **Verifier updates.** Update the WCET, WCMU, and ephemerality passes to use the new opcode set. Wire the new opcodes' cost contributions through the cost model. Update the structural verifier to walk the opcode stream and operand pool independently. ~300 lines.
- **Runtime decoder.** Rewrite `Vm::run` to dispatch from the new wire format. Implement the per-record parity check, the per-pool-entry parity check, and the inline-versus-pool operand fetch. ~400 lines.
- **Hot swap and zero-copy paths.** Update `replace_module`, `view_bytes`, `view_bytes_zero_copy`, and the related entry points to consume the new wire format. ~200 lines.
- **Examples, tests, documentation.** Update every example, every test, and the reference documentation to match the new ISA and wire format. The existing test surface is approximately 750 tests; many will require minor updates, a small number will require rewriting. ~400 lines.

Total estimated implementation effort: ~2,400 lines across the workspace.

### Migration

V0.1.x bytecode artefacts cannot be loaded by V0.2.0 runtimes. Hosts that have V0.1.x bytecode in flight at publication time must recompile against the V0.2.0 toolchain. The framing-header `version` field resets to `1` to signal the discontinuity; V0.2.0 runtimes reject V0.1.x bytecode at the framing-level check.

The implementation lands as a sequence of commits on the V0.2.0 publication branch. The ISA and wire format are the published artefact; the implementation work is operational.

### Phase status

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Op enum additions (`PushImmediate`, `PopN`, `BitAnd`/`BitOr`/`BitXor`/`Shl`/`Shr`, `CallVerifiedNative`/`CallExternalNative`) | Done |
| 2 | Source-level f-string and bytecode small-integer push consolidations | Done |
| 3 | Op enum removals (`PushTrue`/`PushFalse`/`PushUnit`/`PushNone`/`Pop`/`WrapSome`) | Done |
| 3.5 | Text-composition removal (utility natives, f-string desugaring, `stddsl::Text`); `Op::Add` text branch retired from VM dispatch | Done |
| Consolidation B | `Int` arithmetic folded into `CheckedAdd` / `CheckedSub` / `CheckedMul` / `CheckedNeg` followed by `PopN(2)`; `Op::Add` / `Sub` / `Mul` / `Neg` narrowed to `Byte` / `Fixed` / `Float` operand types | Done |
| 4 | Closure opcode removal (`CallIndirect`, `PushFunc`, `MakeClosure`, `MakeRecursiveClosure`) plus `Value::Func` retirement; closure-hoisting pass dropped; closures rejected at the type-checker stage | Done |
| 5 | Native ABI split: source-level `use external` keyword; compiler emits `CallVerifiedNative` versus `CallExternalNative`; `Op::CallNative` retired; `Vm::register_verified_native` and `Vm::register_external_native` host registration methods with classification cross-check via `Vm::verify_native_classifications`; external natives' per-call WCMU explicitly zeroed at the verifier handoff | Done |
| 6 | Control-flow operand narrowing `u32` → `u16` with hard cap at `u16::MAX` ops as `CompileError` and 80% soft warning surfaced via `compile_with_warnings` | Done |
| 7a | Wire format specification (`docs/spec/WIRE_FORMAT.md`) plus `wire_format` module: framing header layout, four-byte opcode records with parity, eight-byte operand pool entries with type tag and parity, opcode-id table, encoder, decoder, round-trip tests. Execution path remains on rkyv. | Done |
| 7b | `wire_format::module_to_wire_bytes` and `module_from_wire_bytes` round-trip an entire `Module` through the section-partitioned body (opcode stream + operand pool + rkyv-archived auxiliary body). New `WireChunk` and `WireAuxBody` types. Round-trip tests cover empty/minimal/branchy/pool-using/Stream programs plus BadMagic, BadChecksum, Truncated, and shebang paths. `Module::to_bytes` and `Module::from_bytes` continue to route through rkyv pending the Phase 7c cutover. | Done |
| 7c | Default `Module::to_bytes` / `Module::from_bytes` / `Module::access_bytes` cut over to the wire-format codec; `access_bytes` returns `&ArchivedWireAuxBody`; VM zero-copy and ops-decode walk the section-partitioned body. `op_from_archived`, the legacy 32-byte framing header, and the legacy CRC residue constants retired. Golden bytes test refreshed to the V0.2.0 byte sequence; `zero_copy_demo.kel.bin` fixture regenerated. | Done |
| 8 | Documentation alignment with the V0.2.0 ISA: FAQ and cookbook text section rewritten; closure entries in FAQ and WHY_REJECTED point at the type-checker rejection rather than the load-time verifier; bundled-natives surface updated to reflect `register_utility_natives` shrinking to `println`. `BYTECODE_VERSION` re-affirmed at 1. `Archive`, `Serialize`, `Deserialize` derives dropped from `Module`, `Chunk`, and `Op` now that the wire format owns serialization. Stale piano_roll `.kel.bin` fixtures removed from the repo. | Done |

## B21. Value-side negative information-flow labels via product lattice

V0.2.0 ships negative information-flow labels at function parameter and return type positions only (`R43` in [`RESOLVED.md`](./RESOLVED.md)). A negative label is a boundary clause: the type checker rejects values flowing across a parameter, return, or yield position that carry any of the listed labels. The clause is checked at the boundary; negative labels do not propagate through arithmetic, branching, or classify/declassify operations on values inside the function body.

A natural extension makes negative labels propagate through the lattice on values themselves: a value of type `Word@!Secret` would carry the guarantee "this value's provenance is free of the Secret label" through every operation. The two values' guarantees combine compositionally under the lattice's join (intersection of negative sets: combining two values produces a value that retains only the negatives both already had) and meet (union of negative sets: the meet of two values' guarantees keeps every negative either had).

**Mathematical formulation.** The label space becomes a product lattice:

- Positive component: the existing lattice over subsets of label identifiers, ordered by ⊆, with join = ∪ and meet = ∩. Unchanged from V0.2.0.
- Negative component: the dual lattice over subsets of label identifiers, ordered by ⊇, with join = ∩ (a value combined from two operands retains only the negatives both already had) and meet = ∪ (the meet of two guarantees keeps every negative either had). The dual ordering is correct because a stronger negative guarantee carries more labels.

The flow rule at every position becomes:

- Positive subset: `source.positive ⊆ target.positive` (existing).
- Negative superset: `source.negative ⊇ target.negative` (new; the source guarantees at least every negative the target requires).

The two clauses run independently. The product lattice composes algebraically.

**Use cases.**

1. **Sanitization audits.** A value that has gone through a `declassify` step can carry an explicit `!Secret` guarantee that the type system propagates compositionally. Without value-side negatives, the post-declassify guarantee is lost the moment the value participates in another operation.

2. **Open-world reasoning.** Saying "no value derived from Secret may reach this sink" is currently expressible only by enumerating every alternative label. Value-side `!Secret` says the property directly. The label universe in Keleusma is open; enumerative expression is fragile against future label additions.

3. **Compositional absence proofs.** Two values that both carry `!Secret` combine into a value that still carries `!Secret`. Static type checking propagates the guarantee through chains of operations.

4. **Deep trust chains.** A device deployed many delegation hops downstream of an originating signer ought to carry compositional provenance: "this command value was never derived from a contaminated sensor reading," "this code segment's signature path never passed through a compromised intermediate." The product lattice expresses this directly; positive labels alone require enumeration.

**Why deferred.** The V0.2.0 parameter-position form covers the immediate signing-and-sanitization use cases. The product-lattice extension adds doubled per-value state, more delicate declassify semantics (a `re-attest` operator that re-establishes a negative guarantee after declassify is its own surface question), and conceptual surface for regular programmers ("how does a value know what it doesn't have?"). The deferral keeps V0.2.0 minimal without preventing the eventual extension: value-side negatives are a strict superset of parameter-position negatives, so a V0.2.0 program will not need to change when the extension lands.

**Forcing case.** Awaits a concrete customer use case. The trust-chain aspects of the fleet delivery scenarios are the strongest candidate; audits that want compositional absence proofs would also qualify. Without a concrete forcing case, designing the value-side semantics risks committing to a model that the eventual case will need to revise.

**Compatibility.** Value-side negatives can land as a backwards-compatible feature addition. Every V0.2.0 program parses unchanged; every existing test continues to pass; the AST gains an internal-only extension to `TypeExpr::NegativeLabelled` that the parser starts to produce at additional positions, and the type checker propagates the negative component through the lattice. No surface syntax changes are required.

**Implementation sketch when forcing case appears.**

1. Promote the AST `TypeExpr::NegativeLabelled` to admit nesting and combine with `TypeExpr::Labelled` at every type position.
2. Extend the type-checker `Type` enum with `Type::Labelled(Box<Type>, BTreeSet<String>, BTreeSet<String>)` or a parallel `Type::DualLabelled` variant.
3. Add the negative component to the lattice operations in arithmetic, branching, classify, and declassify.
4. Introduce a `re-attest` operator that re-establishes a negative guarantee after a declassify step. The audit-point semantics is the same as the existing `declassify`.
5. Update the boundary clauses to check both components.
6. Documentation pass.

The work is mechanical once the design endpoint is pinned by a real customer use case. The V0.2.0 parameter-position form does not prevent the eventual extension.

## B22. Structural-recursion relaxation of the recursion prohibition

R4 prohibits all recursion at compile time. The prohibition is broader than strictly necessary for totality. Rocq (formerly Coq) admits structural recursion via the `Fixpoint` mechanism: a recursive function whose recursive call passes a syntactically smaller argument is admitted as a sound terminator. Agda and Idris use the same mechanism. A structurally-recursive function over a statically-sized data type has a static depth bound (bounded by the size of the input structure) and would not break Keleusma's worst-case execution time and worst-case memory usage analyses.

**Motivation.** The blanket prohibition forces operators to write explicit work-stack iteration (the R3.1 pattern) for cases where structural recursion would be ergonomically natural. Examples: walking a recursive abstract syntax tree, traversing a fixed-depth nested option, performing Robinson unification over a recursively-shaped type term. The work-stack pattern is correct but verbose; structural recursion would express the same algorithm more directly.

**Scope of the relaxation.** Admit recursion that satisfies all of the following.

1. The recursive call passes a strictly structurally-smaller argument of the same type. "Structurally smaller" means the argument is a sub-term of one of the original arguments, accessed through destructuring patterns (match arms over enum variants, fixed-index access into arrays of statically-known length, projection into tuple components).
2. The recursive function operates over a type whose recursive depth is statically bounded. The depth bound is derived from the type definition; for example, `enum Tree { Leaf, Node(Box<Tree>, Box<Tree>) }` would NOT qualify (unbounded depth) but `enum Depth3 { A(Depth2), B }` would qualify (bounded at depth 3).
3. The verifier extracts the depth bound from the type and folds it into the per-call worst-case execution time and worst-case memory usage analyses.

**Effort estimate.** Moderate. Adding a structural-recursion checker to the call-graph verifier is a few hundred lines of Rust plus tests. The depth-bound extraction from types is the harder part because it requires the type system to recognise which types are recursively bounded.

**Prior art.** Rocq's `Fixpoint` and `Function` mechanisms. Agda's termination checker. Idris's `total` keyword. The Coq Reference Manual's "Recursive Functions" chapter is the canonical reference. Friedman and Eastlund's *The Little Prover* introduces the technique pedagogically. See `docs/reference/RELATED_WORK.md` § 6 for the prior-art positioning.

**Why deferred.** R3.1's work-stack pattern is adequate for the V0.3.0 self-hosted compiler. The structural-recursion relaxation is an ergonomic refinement, not a correctness or expressiveness requirement. Likely V0.3.x or V0.4.0 scope.

**Forcing case.** A real Keleusma program where the work-stack pattern produces measurably worse code (more lines, more bugs in operator-authored versions, harder to read) than structural recursion would. The self-hosted compiler is the most likely forcing case; once V0.3.0 lands, the operators writing compiler stages may find specific algorithms (Robinson unification, structural pattern matching, syntactic decomposition) where the work-stack form is genuinely worse.

**Compatibility.** Backwards-compatible feature addition. Programs that compile under the blanket prohibition continue to compile under the relaxed rule. The verifier rejection set strictly shrinks.

## B23. Coinductive productivity formalism

Keleusma's productivity rule (every `loop` iteration must yield) is currently a syntactic check. Rocq's `CoInductive` types and `cofix` corecursion provide a more rigorous formalism for productive divergent computation. Studying Rocq's machinery could refine Keleusma's productivity analysis for cases where data-dependent control flow makes the syntactic check too conservative.

**Motivation.** The syntactic productivity rule rejects some programs that are provably productive but not syntactically obvious. Example: a `loop` that yields conditionally based on a data-driven decision, where the verifier cannot statically prove all paths reach a yield. Today these programs are rejected; under a coinductive analysis they might be admitted.

**Scope.** Refine the productivity rule to accept programs whose productivity argument is coinductively sound but not syntactically obvious. Define the precise admission criteria; pin the analysis algorithm; document the trade-off against the syntactic check.

**Effort estimate.** Substantial. Coinductive productivity proofs are an active research area; integrating them into Keleusma's verifier would require careful design. Probably weeks of design plus weeks of implementation. The analysis would need to walk the loop body's control flow under a coinductive abstract interpretation.

**Prior art.** Rocq's `CoInductive` types, `cofix` corecursion, and the `Guarded` keyword for productivity proofs. Agda's `Codata` and `Coinductive` mechanisms. Idris's stream productivity rules. Turner's `Strict-Total` discipline. The Coq Reference Manual's "Inductive and Co-Inductive Types" chapter is the canonical reference. Abel and Pientka's "Well-founded Recursion with Copatterns and Sized Types" provides the formal foundation. See `docs/reference/RELATED_WORK.md` § 6 for the prior-art positioning.

**Why deferred long-horizon.** The syntactic rule is conservative but correct; rejecting some provably-productive programs is preferable to admitting some non-productive ones. The coinductive refinement is a long-horizon item for cases where operator demand surfaces a real ergonomic gap.

**Forcing case.** A V0.3.0-or-later customer program that gets rejected by the syntactic productivity check despite being provably productive under a more careful analysis. The case has not yet appeared; the V0.2.0 surface admits most natural productive-loop shapes.

**Compatibility.** Backwards-compatible feature addition. Programs that pass the syntactic check continue to pass. The verifier rejection set strictly shrinks.

## B24. Hardware-isolation integration for Cortex-M targets

Keleusma's layered-security posture combines four protective layers: cryptographically signed modules (R42), statically verified information flow (R43), encrypted artefacts (in-flight at `tmp/encrypted_signed_modules.md`), and hardware-isolated execution (this entry). The first three layers are language-level features; the fourth requires platform support that Cortex-M55 provides through TrustZone-M and ARMv8-M Memory Protection Units. This backlog entry documents the integration direction.

**Scope: narrow integration only.** Keleusma provides primitives that the host can use to mark arena regions as secure-world only, configure the MPU for arena protection, and store decryption keys in secure flash. The runtime does not manage secure-world entry points itself; secure-world control remains the host's responsibility. The narrow scope keeps the work bounded and avoids substantial architectural changes to the runtime.

The broad scope alternative, in which Keleusma manages secure-world execution directly and configures TrustZone-M as a first-class language feature, is out of scope. The broad scope would require redesign of the arena memory model, the dual-end stack-and-heap discipline, and the call frame layout to accommodate secure-world transitions. Substantial work with assurance implications. Not contemplated.

**Components of the narrow integration.**

1. **Host-supplied secure-flash key storage.** The host stores the runtime's X25519 decryption private key (per the encrypted-modules spec) in secure flash, not in normal flash or RAM. The Keleusma runtime accesses the key only through a host-registered native function whose implementation enters secure-world for the actual key material. The bytecode never sees the key as a plaintext value.

2. **MPU-configured arena protection.** The host configures the ARMv8-M MPU to mark the arena's memory region as accessible only to specific privilege levels. Keleusma runtime code executes at a known privilege level; host code at higher privilege. Bytecode-level attacks that escape the verifier (a hypothetical zero-day in the structural verifier, for example) cannot access memory outside the configured MPU regions.

3. **Secure-world entry points for decryption.** The host's native function for module decryption is implemented in secure-world. The encrypted bytecode arrives in non-secure memory, the host transitions to secure-world via the standard SG (Secure Gateway) instruction, the secure-world routine decrypts the body, and the plaintext bytecode is placed in MPU-protected memory before transition back to non-secure execution.

**Effort estimate.** Substantial. Each Cortex-M variant has distinct TrustZone-M and MPU configurations. The work splits into:

- Host-side TrustZone-M plumbing: roughly two to four weeks per platform.
- Secure-world routines: one to two weeks per cryptographic primitive (X25519 unwrap, AES decryption).
- MPU configuration helpers: one week per platform.
- Testing across the platform's evaluation requirements if applicable: weeks to months depending on the assurance level required.

Total per-platform integration cost is therefore in the range of one to three months. Multiple platforms compound accordingly.

**Prior art.** ARM's documentation for the Cortex-M55 TrustZone-M architecture is the canonical reference. The Keil RTX5 RTOS and the FreeRTOS-Plus-TrustZone integrations provide working open-source examples of secure-world entry-point design. Several embedded firmware vendors (NXP, ST, Renesas) ship platform-specific TrustZone-M templates. High-assurance evaluation schemes typically require this kind of hardware isolation; specific requirements vary by evaluation scheme and protection profile.

**Composition with existing infrastructure.** The four-layer posture composes cleanly:

- Ed25519 signed modules authenticate the source.
- X25519 hybrid encryption protects the artefact contents.
- IFC labels statically verify data flow within the authenticated code.
- TrustZone-M plus MPU isolate the execution from compromise via channels outside the runtime's awareness.

Each layer addresses a distinct threat. The combination is materially stronger than any subset. The encrypted-modules spec at `tmp/encrypted_signed_modules.md` is the immediate predecessor in this chain; the hardware-isolation work is the natural successor.

**Why deferred.** The first three layers, namely signed modules, IFC labels, and encrypted modules, are operational improvements that do not require platform-specific work. They land as V0.2.0 and V0.2.x. The hardware-isolation work is necessarily platform-specific, substantial in scope, and pre-requires the encrypted-modules infrastructure to exist. The natural sequencing is V0.4.x for initial Cortex-M55 integration, with other Cortex-M variants following based on operator demand.

**Forcing case.** A concrete customer use case that requires hardware-isolated execution and a high-assurance evaluation posture. Without such a forcing case, the platform-specific engineering investment is hard to justify against the alternative of operator-managed hardware integration outside the Keleusma runtime.

**Compatibility.** Backwards-compatible feature addition. The work extends the host-interface surface with optional native functions that hosts may register or ignore. Programs written without hardware-isolation awareness continue to run identically. Hosts that opt in gain the additional isolation layer.

**Cross-references.**

- R42 (Ed25519 module signing) is the first protective layer.
- R43 (information-flow labels with negative variants) is the second.
- `tmp/encrypted_signed_modules.md` (the in-flight spec) is the third.
- R4.5 (cross-platform target order) places Cortex-M55 in Tier 2 of V0.4.x, which is the natural delivery window for the initial hardware-isolation integration.
- The fleet delivery scenarios, together with the related long-running deployment scenarios, are the operational shape that the four-layer combination addresses end to end.

## B25. Directional information-flow labels on data field types

V0.2.x admits negative information-flow labels at three boundary-position categories: function parameters and returns, `shared` data field types, and `private` data field types. The data-field cases landed in the 2026-05-23 IFC extension after the operator's observation that shared and private data sections are return values and inputs masquerading as storage. Each field carries one label set that governs both directions: a positive label propagates from writes to reads, while a negative label is checked at writes and cleared at reads. This single-set design works but conflates two genuinely distinct flows.

A data field has two boundary characters in tension. A `shared` field is simultaneously a sink for inbound writes and a source for outbound reads; a `private` field plays both roles across the yield-resume boundary. A richer model would admit *directional* labels: a separate label set for each direction.

**Surface form (proposed).**

```keleusma
shared data state {
    sanitised_command: T @ in: {Untrusted, !Secret}, out: Trusted,
}
```

Reading: the host writes values that may carry the `Untrusted` label and must not carry `Secret`; the script reads values typed as `T @ Trusted`. The storage acts as an explicit classifier at the boundary, audit-tracked the same way a `declassify` operator is.

The four directional combinations the model would enable:

| Pattern | Inbound label | Outbound label | Use case |
|---------|---------------|----------------|----------|
| Sanitiser-by-storage | `Untrusted` | `Trusted` | Network bytes verified by an intermediate sanitiser land in this field; downstream code reads them as trusted |
| Classifier-by-storage | `Trusted` | `Untrusted` | Internal data is intentionally declassified by the act of writing to a public-output field |
| Symmetric | `Label` | `Label` | The field carries a single label in both directions; current V0.2.x form `T @ Label` already expresses this |
| Asymmetric exclusion | `!Label` | (any) | The field rejects values carrying `Label` on writes; reads produce values with no labels. Current V0.2.x form `T @ !Label` expresses this for the in-strict, out-loose case |

The mixed positive-and-negative parse-time rejection from V0.2.0 remains for each direction independently. Mixed sets within one direction are redundant under the same analysis that applies to function parameters and returns.

**Mathematical formulation.**

The label declaration becomes a pair `(in_set, out_set)`. The check rules:

- At every write: `source.labels ⊆ in_set.positive` AND `source.labels ∩ in_set.negative = ∅`.
- At every read: the value is typed as `T @ out_set.positive`; negative labels in `out_set` clear at the boundary (read returns the inner type with the positive labels).

The two directions are independent. The current single-set semantics is the special case where `in_set = out_set`.

**Use cases.**

1. **Audit clarity at trust transitions.** A signed-update channel that lands in a specific `shared data` field. The directional form makes the trust transition visible at the field declaration rather than at scattered `classify` call sites.

2. **Hot-swap robustness on trust transitions.** A hot-swap that changes a verifier function cannot accidentally bypass classification if the field's `out:` label is statically declared. The trust elevation is bound to the storage, not to a code path.

3. **Source-level density at verify-then-store boundaries.** Applications with many ingress channels (multi-tier cross-link, multi-sensor types, multi-operator-update channels) repeat the verify-classify-store pattern. Directional labels collapse three operations to one declaration. The benefit compounds with the number of trust transitions.

4. **Storage-boundary sanitisers and classifiers.** The two unexpressible cases under V0.2.x (sanitiser-by-storage and classifier-by-storage) become directly expressible. Today both have function-based equivalents via `verify` plus `classify`/`declassify`, but those are scattered across call sites rather than declared at the boundary.

**Why deferred.**

The function-based sanitiser and classifier pattern (verify-then-classify; declassify-then-store) works today for every probe-application scenario examined. The expressiveness gap is genuine but its load-bearing weight is low. The cost-benefit assessment for a probe codebase, where IFC discipline is enforceable through code review, is "modest ergonomic improvement, modest audit clarity, no new capability". For a generalist embedded RTOS API (RA.14 in the rtos_api research loop) the value is higher because operators of those systems may not adopt the function-discipline rigorously. For a single-team-owned codebase the discipline is enforceable at code review and the language-level extension is correspondingly less urgent.

The forcing case is a concrete application with many verify-then-store boundaries (a generated or transformed codebase, a high-IFC-density application with dozens of cross-tier links, or an audit regime that prefers storage-boundary trust transitions over code-path ones).

**Compatibility.**

Backwards-compatible feature addition. Every V0.2.x program parses unchanged; the existing single-set form `T @ Label` is the special case `in: Label, out: Label`. The parser gains an alternative form when a `@` is followed by `in:` or `out:`; the type checker tracks the directional pair instead of a single set. Existing tests continue to pass.

**Implementation sketch when forcing case appears.**

1. Extend the AST `TypeExpr::Labelled` and `TypeExpr::NegativeLabelled` with optional directional tags, or add a third variant `TypeExpr::DirectionalLabelled(Box<TypeExpr>, in: LabelSet, out: LabelSet, Span)`.
2. Parser admits `@ in: {...}, out: {...}` syntax in addition to the existing `@ Label`, `@ {Labels}`, `@ !Label`, `@ {!Labels}` forms.
3. Type checker stores per-field `in_set` and `out_set` separately in `Ctx::data_negative_labels` (rename or split as appropriate).
4. Boundary check at writes uses the field's `in_set`. The `check_negative_labels_against_data_write` helper extends with the positive-subset check on the in direction.
5. Reads produce a value typed as `inner @ out_set.positive`; the existing resolve_type call grows a parallel "outbound" path.
6. Documentation passes in `docs/architecture/LANGUAGE_DESIGN.md`, `docs/spec/GRAMMAR.md`, and the `TypeExpr` AST comment.
7. Unit tests for accept and reject paths in each of the four directional combinations.

The work is mechanical once the forcing case pins the design endpoint. The V0.2.x single-set form does not prevent the eventual extension.

**Cross-references.**

- R43 (information-flow labels with negative variants) defines the positive and negative semantics this entry generalises.
- R51 (negative labels on data field types) is the immediate predecessor in V0.2.x; B25 generalises its single-set form to a directional pair.
- The 2026-05-23 commit `0262634` (data-field negative labels) is the implementation landing of R51.
- B21 (value-side negative labels via product lattice) is the larger generalisation. B25 is strictly narrower; it remains a boundary clause rather than a value-side property.
- `RA.14` in `tmp/research/rtos_api/ra_14_ifc_labels.md` outlines the RTOS-level IFC discipline this entry would compose with.
- The fleet delivery scenarios are the operational shape whose audit and hot-swap concerns this entry would address.

## ~~B26. Arena-resident persistent region for composite data values~~ (Resolved through B28, V0.2.1)

> **Status: resolved through B28 (V0.2.1, 2026-06-24).** B28's flat-byte composite representation makes a private `.data` composite pure bytes in the arena's persistent region (P3 item 3a baked the per-slot persistent pool offsets, 6A generalised them to array elements, and 6B removed the last owned-bytes form). The persistent region is now byte-self-contained, so the byte-snapshot patterns this entry wanted are sound and B26's Path C is the natural outcome. Retained as the symptom-level design record.

V0.2.x stores `.data` slot values as `GenericValue<W, F>` enum instances inline in the arena's persistent region. The enum's variant payload for composite types (`Tuple(Vec<Value>)`, `Array(Vec<Value>)`, `Struct { fields: Vec<(String, Value)> }`, `Enum { fields: Vec<Value> }`) holds a heap-allocated `Vec` whose body lives on the global allocator's heap, not in the arena. The slot's bytes contain the `Vec`'s `(ptr, len, cap)` triple; the elements live elsewhere. The KString machinery resolves the same problem for variable-length strings (`Value::KStr` is arena-backed via `ArenaHandle<str>`), but no analogous machinery exists for the composite variants.

This implementation choice creates a mismatch between the language guarantee and the runtime reality. The language admits only fixed-size types in `.data` fields and forbids references at any source position. The runtime nevertheless places heap pointers in `.data` slots for any composite-typed field. Operators reading the language design correctly expect "fixed size, no references, byte-portable storage" and reach for byte-snapshot patterns that the runtime does not support without additional plumbing.

**Immediate manifestation: REPL persistence.**

The 2026-05-23 REPL persistence work (commit `92b994c`) snapshots `shared data` slots through the per-slot `Vm::set_data` and `Vm::get_data` Value-clone API, which works because shared slots are host-visible and `Value::clone()` deep-clones the heap data. Private data slots have no equivalent host-side API and the persistent region's byte content includes the heap pointers, so byte-snapshot of the private region is unsound. Private data persistence in the REPL is therefore deferred. A scalar-only allowlist (Word, Float, Bool, Byte, Fixed) is the tactical workaround; a representation that places composite bodies in the arena rather than the global heap is the structural fix.

**Future manifestation: live migration and cross-process state transfer.**

A V0.4.x or V0.5.x feature that wants to migrate a Vm's persistent state across processes (signer-to-device update delivery, checkpoint-resume on embedded targets with battery-backed RAM, hot-swap onto a new module via an opaque blob) would hit the same heap-pointer problem. Today these features require per-Value serialisation walks. A persistent region whose every byte is self-contained would let the feature treat the region as a flat opaque byte buffer.

**Design space.**

Three paths solve the problem with different trade-offs.

**Path A: inline representation per slot type.** The persistent region's layout becomes per-slot-type-sized rather than uniform `size_of::<Value>()` per slot. A `Word` slot is 8 bytes, a `(Word, Word)` slot is 16 bytes, a `[Word; 8]` slot is 64 bytes. The `Op::GetData(slot)` opcode reads the slot's typed bytes and constructs a `Value` view on the operand stack; `Op::SetData(slot)` takes a `Value` and packs the bytes into the slot. The `DataSlot` layout grows to carry a type hint; the bytecode compiler emits the hint at codegen time; `required_persistent_capacity_for` becomes a per-slot sum.

This is the architecturally cleanest answer. Byte-snapshot becomes sound trivially because the bytes are self-contained for every admissible field type. Embedded targets without a global allocator can host arbitrary admissible data without any global-heap traffic. WCMU accounting becomes more precise because the persistent region's per-slot footprint is exactly what the slot's type requires.

The cost is a non-trivial refactor: data layout extension, compiler emit change, opcode handler rewrite for `GetData`/`SetData`, WCMU accounting adjustment, migration of existing tests that assume slot uniformity, and a documentation pass. Estimated one to two weeks of focused work. Operand-stack hot path is unchanged because the `Vec`-based `Value` shape remains for transient values.

**Path B: parameterise `Value`'s composite variants over an allocator (the `allocator-api2` approach).** Rust admits parameterised collections through the `Allocator` trait, polyfilled in stable Rust by the `allocator-api2` crate that the workspace already depends on for `keleusma-arena`. The composite variants become `Tuple(Vec<Value, ArenaAllocator>)`, `Array(Vec<Value, ArenaAllocator>)`, et cetera. The arena exposes an `Allocator` impl; allocations happen in the arena rather than the global heap.

This is a less invasive code-level change because `Vec` and `String` shape are preserved and only the allocator parameter changes. The persistent region's bytes still contain a `(ptr, len, cap)` triple, but the `ptr` now points into the arena rather than the global heap. For byte-snapshot purposes this does not directly help: the `ptr` is an absolute address, and copying the persistent bytes into a `Vec<u8>` captures an absolute pointer that becomes stale once the source arena is dropped, just like the heap-pointer case. The byte-snapshot pattern would additionally need an offset-based pointer encoding (relocate the inner pointers relative to the arena base) and a relocation pass on restore.

Compared with Path A, Path B keeps the operand-stack and persistent-region representations uniform (still `Vec`-shaped) but introduces arena-relative pointer encoding. The implementation cost is comparable; the architectural cleanliness is lower because composite layouts retain a level of indirection that the language did not require. Path B does have one operational advantage: embedded targets with no global allocator can use the arena as the sole allocator without changing operand-stack representations or hot-path code.

**Path C: hybrid.** Persistent region uses Path A's inline layout; operand stack continues to use the `Vec`-backed `Value` shape. `Op::GetData` materialises the inline bytes to a heap-Vec-backed `Value` for the stack; `Op::SetData` packs the stack's heap-Vec back into the inline slot. The boundary between persistent and transient representations is at the data-segment opcodes. Operand-stack hot path is unchanged. Persistent-region byte-snapshot is sound. Estimated cost is one to two weeks, same as Path A.

This is the recommended path. It captures Path A's architectural benefit at the persistent region and Path B's hot-path preservation at the operand stack.

**Required properties any path must satisfy.**

The operator's design intent is that all private data lives in and fits in the persistent region such that an opaque byte snapshot can be taken and restored. Five concrete properties follow:

1. **No pointers in the persistent region's bytes.** Either inline storage (Path A or C) or offset-relative pointers (a refinement of Path B that replaces `Vec<T, A>`'s absolute `NonNull<T>` with arena-base-relative offsets). The current `Vec<T>`-with-global-heap and the straightforward `Vec<T, ArenaAllocator>` both fail this property because both use absolute pointers.

2. **The persistent region is sized to hold all private values including composite bodies.** `required_persistent_capacity_for(&module)` walks the type info and sums per-slot bytes (`Word` = 8, `Float` = 8, `Bool` = 1 padded, `(Word, Word)` = 16, `[Word; 8]` = 64, `struct Point { x: Word, y: Word }` = 16, et cetera). The current `slot_count * size_of::<Value>` formula is replaced.

3. **WCMU accounting integrates the persistent size.** `wcmu_stream_iteration` continues to return per-iteration `(stack_bytes, body_heap)`. The persistent footprint becomes a separate, statically-knowable quantity: `required_persistent_capacity_for(&module)` reports the total persistent bytes the module requires, computed from the per-slot type info. The host adds the two: `total_wcmu = persistent_bytes + max(stack_bytes, body_heap_in_transient_region)`. The composite-body bytes currently counted under `body_heap` (when those bodies live on the global heap) shift into the persistent count instead, because those bodies now live in the persistent region and persist across iterations.

4. **The arena allocator is given both numbers.** Already true via `Arena::with_capacity(total)` plus `Arena::resize_persistent(persistent_bytes)`. The contract does not change; only the per-slot byte sum that feeds the persistent number grows from `slot_count * size_of::<Value>` to a per-type sum.

5. **Opaque byte snapshot of the persistent region is sufficient to restore.** A `Vec<u8>` snapshot from `arena.persistent_ptr()` of `persistent_capacity()` bytes captures everything needed to reconstruct the private data. Restoring on a fresh arena of the same persistent size makes the new Vm observe identical private state. This property follows directly from property 1.

Path A and Path C satisfy all five properties directly. Path B as documented in this entry satisfies properties 2 through 4 but fails property 1 (absolute pointers) and therefore property 5; closing the gap requires the offset-relative-pointer refinement, which is more work than Path C's inline approach. Path D (Vm-side deep-clone API) does not satisfy property 1 or 5 by design; it provides a typed alternative to byte-snapshot rather than enabling byte-snapshot.

**Path D: keep current design; add a deep-clone API for private slots.** `Vm::private_data_snapshot(&self) -> Vec<Value>` and `Vm::private_data_restore(&mut self, values: Vec<Value>) -> Result<(), VmError>` walk the persistent region slot by slot, cloning each `Value` (whose `Clone` impl deep-clones the heap-resident bodies). The host snapshots the resulting `Vec<Value>` rather than the raw bytes. Byte-snapshot remains unsound; per-slot Value-snapshot is the supported pattern.

This is the smallest change. It does not align the runtime with the language guarantee; it just adds a typed-clone API for private slots equivalent to what shared slots already have through `set_data`/`get_data`. Estimated cost is a few hours. Suitable as a stopgap if Path C is not pursued in V0.2.x.

**Why deferred from V0.2.x.**

The REPL persistence work covers the operationally relevant case (shared data with `set_data`/`get_data`) through Path D's already-existing shared-slot equivalent. Private data REPL persistence is restricted to scalar types as a tactical workaround. No live migration or cross-process checkpoint feature is in V0.2.x scope. The forcing case for Path C is a concrete need to checkpoint or migrate a Vm's full persistent state across processes, or a concrete embedded-target deployment without a global allocator that needs composite data field support.

**Forcing case.**

A V0.4.x or V0.5.x feature that requires opaque-buffer persistent-state transfer. Candidates include hot-swap blob delivery, multi-tier update propagation in the fleet delivery scenarios, embedded-target battery-backed RAM checkpoints, or a generated codebase that produces many module variants whose persistent state must round-trip without a typed walk.

**Compatibility.**

Path C is a breaking change at the bytecode-version level. The `DataSlot` layout grows; `required_persistent_capacity_for` returns different values for composite-bearing modules. Modules compiled under V0.2.x would not load on a runtime that implements Path C, and vice versa, unless a compatibility shim is added. The `BYTECODE_VERSION` constant would advance.

Path D is a backwards-compatible API addition. No version change required.

**Implementation sketch when forcing case appears (Path C).**

1. `DataSlot` gains a `type_hint: TypeRepr` field where `TypeRepr` is a stable bytecode-friendly enum (`Word`, `Float`, `Bool`, `Byte`, `Fixed(u8)`, `Tuple(Vec<TypeRepr>)`, `Array(Box<TypeRepr>, u32)`, `Option(Box<TypeRepr>)`, `Struct(Vec<TypeRepr>)`, `Enum { variants: Vec<Vec<TypeRepr>> }`). The compiler emits this from the resolved type at codegen time.

2. `required_persistent_capacity_for` walks the layout, summing per-slot bytes via a `TypeRepr::byte_size()` recursive computation.

3. `Op::GetData(slot)` reads the slot's inline bytes through a `TypeRepr::decode_inline_bytes()` routine that returns a `Value`. The returned `Value` uses the existing `Vec`-backed composite variants for the operand stack.

4. `Op::SetData(slot)` takes a `Value` from the operand stack and encodes it through a `TypeRepr::encode_inline_bytes()` routine that writes the slot's inline bytes.

5. The arena's `persistent_ptr()` exposure remains unchanged. The persistent region's byte semantics become "POD bytes, no pointers, byte-snapshot-portable".

6. WCMU accounting in `verify::wcmu_stream_iteration` adjusts per-slot per-type rather than per-slot-uniform.

7. `BYTECODE_VERSION` advances by one. The wire format documentation records the layout change.

8. Tests: per-`TypeRepr` round-trip (encode then decode produces the original `Value`), byte-snapshot round-trip across Vm boundaries for every admissible composite type, WCMU accuracy regression against the prior uniform-size accounting.

The work is mechanical once the design decision is locked in. The forcing case determines whether V0.2.x scope absorbs it or whether it lives in V0.3.x or beyond.

**Cross-references.**

- R32 (dual-end arena, established the persistent versus transient split).
- R29 (hot code swap, the precedent for cross-Vm data transfer; currently uses Value-walking and would benefit from inline storage).
- B16 (parametric Vm for sub-64-bit native runtimes; intersects with the `TypeRepr` design because byte-size computation depends on the target's word and float widths).
- B27 (arena-resident transient region for composite Value bodies) is the complementary entry for non-`.data` composite values.
- The 2026-05-23 commit `92b994c` (REPL shared data persistence) is the operational use case that surfaced the mismatch.
- `keleusma-arena` already depends on `allocator-api2`, so Path B's plumbing prerequisite is already in place if the project pursues that route instead.

## ~~B27. Arena-resident transient region for composite Value bodies~~ (Resolved through B28, V0.2.1)

> **Status: resolved through B28 (V0.2.1, 2026-06-24).** B28 packs ephemeral composite bodies directly into the arena's top region (`FlatComposite::build_in_arena` / `pack_flat_in_arena`); the `Vec`/`String` global-heap bodies this entry describes are gone, so composite construction is bump-allocator-style with no global allocator, and the arena-as-sole-allocator property R32 promised is delivered. Embedded targets without a global allocator can run composite-building scripts. Retained as the symptom-level design record.

The persistent counterpart to this entry is B26. The architectural intent that motivates both: the arena is the sole allocator the Keleusma runtime uses; the global allocator is unused. The persistent region (B26) holds `.data` values inline; the transient region (this entry) holds ephemeral composite Value bodies via arena-backed `Vec` and `String` rather than the std-global-allocator counterparts.

**Current state.** The composite `Value` variants (`Tuple(Vec<Value>)`, `Array(Vec<Value>)`, `Struct { fields: Vec<(String, Value)> }`, `Enum { type_name: String, variant: String, fields: Vec<Value> }`) use std `Vec` and `String` with the global allocator. A script that constructs a tuple in expression position allocates the tuple's body from the global heap. The body is dropped when the operand stack pops the value or when the iteration ends. WCMU's `body_heap` counter in `wcmu_stream_iteration().1` accounts for the bytes correctly. The mechanism is functionally fine. The locational fact (global heap rather than arena transient region) is the gap.

**Operational consequences of the gap.**

1. **Embedded targets without a global allocator are blocked.** Cortex-M targets that disable `alloc::alloc::GlobalAlloc` or configure a fixed-size heap separate from the Keleusma arena cannot run scripts that build composite values. The arena is sized to bound the script; the global allocator is separate and either absent or independently sized. This is a real obstacle to V0.4.x cross-target deployment for any script touching composite types.

2. **WCMU bounds are not equivalent to the arena's bound.** A script's WCMU report says "this iteration peaks at N bytes of operand stack and M bytes of heap". The arena's `with_capacity(total)` is sized to satisfy operand-stack peak plus persistent region. The M bytes of `body_heap` come from the global allocator. An operator verifying the script's memory bound must add the global-heap quota to the arena bound. The two-allocator accounting is correct but awkward.

3. **Allocator behaviour bleeds in.** Global allocators on different platforms have different fragmentation behaviour, different time-to-allocate, different failure modes. A Keleusma script's runtime behaviour gains a dependency on the host platform's allocator even though every per-op cost is bounded. The arena's bump allocator is statically predictable; the global allocator is not.

4. **The persistent versus transient split is half-arena, half-global.** R32 established the dual-end arena to handle both ends of the memory lifetime spectrum with one allocator. Composite bodies leak past this design by going to the global allocator. The arena-as-sole-allocator property R32 implicitly promises is not actually delivered.

**Proposed change.**

The composite `Value` variants become parameterised over an allocator:

```rust
Tuple(Vec<GenericValue<W, F>, ArenaAllocator>)
Array(Vec<GenericValue<W, F>, ArenaAllocator>)
Struct {
    type_name: String<ArenaAllocator>,
    fields: Vec<(String<ArenaAllocator>, GenericValue<W, F>), ArenaAllocator>,
}
Enum {
    type_name: String<ArenaAllocator>,
    variant: String<ArenaAllocator>,
    fields: Vec<GenericValue<W, F>, ArenaAllocator>,
}
```

`ArenaAllocator` is a wrapper around the arena's transient bump allocator implementing the `Allocator` trait from `allocator-api2` (which the workspace already depends on). Allocations in the transient region are bump-only and freed wholesale on RESET; the `Drop` impl for `Vec<T, ArenaAllocator>` is a no-op because the underlying memory is reclaimed by the arena's RESET, not by the collection.

The arena exposes an `Allocator` impl scoped to its transient region. The operand stack and other transient working memory continue to use the arena's existing dual-end bump regions; composite Value bodies now share the same region.

**Compatibility with B26.**

B26 covers the persistent region (inline storage, no pointers, byte-snapshot-portable). B27 covers the transient region (arena-backed Vec and String, absolute pointers into the arena's transient bytes, no byte-snapshot requirement because transient is cleared on RESET).

Both land independently. Order is flexible. Landing B27 alone gives the embedded-target benefit but leaves the persistent region's heap-pointer issue unresolved for cross-Vm checkpoint patterns. Landing B26 alone gives the byte-snapshot property but leaves composite-construction allocations on the global heap. Landing both gives the architecturally complete picture: the arena holds everything; the global allocator is unused; WCMU's persistent and transient bytes sum to the arena's total capacity.

**Trade-offs.**

| Property | Current | After B27 |
|----------|---------|-----------|
| Operand-stack composite allocation source | Global allocator | Arena transient region |
| Embedded target with no global allocator | Blocked | Supported |
| RESET behaviour | Composite bodies dropped individually via `Drop` | All composite bodies released together with the arena's transient reset |
| Allocation cost per composite | Allocator-dependent (varies by platform) | Bump allocator, one pointer increment |
| Fragmentation hazard | Yes (allocator-dependent) | No (bump allocator is fragmentation-free) |
| WCMU accounting | `body_heap` counter, global-heap bytes | `body_heap` counter, arena-transient bytes (same number, different location) |
| `Vec<T, A>` API ergonomics in runtime code | Standard `Vec<T>` | `Vec<T, ArenaAllocator>` (requires `allocator-api2` syntax everywhere) |

**Effort estimate.**

Moderate. Smaller than B26 because the bytecode layout does not change. The work splits into:

1. **Define `ArenaAllocator`** as a wrapper around the arena's transient bump allocator implementing `allocator_api2::Allocator`. Roughly one to two days.

2. **Migrate `GenericValue` composite variants** to use `Vec<T, ArenaAllocator>` and `String<ArenaAllocator>`. Touches every site that constructs or consumes a composite Value. Ripple through the VM's instruction handlers (`NewArray`, `NewTuple`, `NewStruct`, `NewEnum`, `GetField`, et cetera) and the native-marshalling layer. Two to four days for the core migration; longer if `KeleusmaType` derive needs extending.

3. **Update WCMU accounting** to record bytes against the arena's transient region rather than the global heap. The numerical accounting does not change; only the location label does. One day including tests.

4. **Test all admissible composite expressions** under the new representation. Ensure RESET correctly releases all composite bodies. One to two days.

5. **Documentation pass**: update R32 to note that the transient region now hosts composite bodies; update the architecture narrative for the "arena is the sole allocator" property. Half a day.

Total: one to two weeks.

**Compatibility.**

Backwards-compatible at the bytecode level. The compiled bytecode is unchanged; only the runtime's memory layout changes. Programs do not need recompilation. The `BYTECODE_VERSION` constant does not advance.

The Rust API for embedders changes if they construct `Value::Tuple` or similar directly. Embedders that use the `KeleusmaType` derive and `register_fn` ergonomics are insulated from the change. Embedders constructing `Value` enum variants by hand (rare) must update to the `Vec<T, ArenaAllocator>` shape.

**Forcing case.**

A V0.4.x or V0.5.x deployment to an embedded target with no global allocator (or a deliberately-undersized global allocator) and a script that constructs composite values. The STM32N6570-DK reference platform from the RTOS work and the perpetual-operational deployable-platform scenarios are the natural candidates. Without such a forcing case the global-allocator path continues to work on hosted targets and the V0.2.x scope does not require this change.

**Cross-references.**

- B26 (arena-resident persistent region for composite data values) is the complementary entry. B26 covers `.data` slots; B27 covers non-`.data` composite values.
- R32 (dual-end arena) is the prior decision that B27 completes.
- B24 (hardware-isolation integration for Cortex-M targets) is the deployment family that benefits from arena-as-sole-allocator.
- `keleusma-arena` already implements an `Allocator` shape for KString; B27 generalises the same mechanism to composite Vec and String bodies.
- The fleet delivery scenarios are the operational shape that benefits from the deterministic-allocator property.

## ~~B28. Runtime composite Value representation aligned with the language guarantee~~ (Resolved for V0.2.1; all phases P0-P5 complete)

> **Status: resolved (V0.2.1, 2026-06-24). All phases complete; the entry is retained as the design record.** P0 through P4 landed, P3's reference-field and representation items (1 thin-box boxed bodies, 2 the `FlatComposite::Inline` deletion and `Value` 40-to-32 collapse, 3a persistent composite data slots, 4 `StaticStr` to rodata, 5 typed codegen) all landed, the shared-data re-architecture replaced the `set_data`/`get_data` slot vector with a host-owned borrowed `&mut [u8]` buffer, and 6A baked the private-composite layout table so 6B could delete `Inline`. The live ISA is **66 opcodes** (P4 retired the four V0.2.0 construct opcodes, wire ids 34-37, in favour of `NewComposite`, wire id 69; maximum live id 69), `BYTECODE_VERSION` stays at 1, and `Value` is 32 bytes (pinned by a `const` assertion). P5 (this closure) reconciled the documentation and marked B26 and B27 resolved through B28; the hot-swap migration shipped as the strict-schema-check plus host-owned Replace model documented in `EXECUTION_MODEL.md`, which superseded the offset-to-offset migration-table sketch below. The composite runtime now matches the language's fixed-size guarantee. The plan was revised after a design pass that established four things. No opcode is added or removed; the operands of the composite ops are re-specified so the compiler can bake field offsets and kinds into the access instructions, which is permitted because byte-code compatibility is not a goal. No layout table or template is emitted into the artifact and none lives at run time; the compiler's transient layout (`layout_pass`) is dissolved into the baked offsets and the worst-case-memory-usage bound, and the composite value is pure bytes. Byte-code compatibility with V0.2.0 is **not** a goal; the byte code may break freely and is simply recompiled, and `BYTECODE_VERSION` stays at 1 only for lack of production traction. Because offsets are baked, the only remaining suboptimality of the Rust runtime is the tagged scalar operand stack; making that untyped is the one flat-machine step deferred to the V0.4 native-code-generation ISA redesign, recorded below under *Deferred ISA redesign*.

The language admits only fixed-size types in composite positions. The verifier proves WCMU bounds assuming fixed sizes. The runtime contradicts the guarantee at the storage layer: `Value::Tuple(Vec<Value>)`, `Value::Array(Vec<Value>)`, `Value::Struct { fields: Vec<(String, Value)> }`, and `Value::Enum { type_name: String, variant: String, fields: Vec<Value> }` use heap-allocated `Vec` and `String` indirection. The script's promise is "fixed size, no references"; the runtime's reality is "heap pointer to dynamically-sized backing, plus string keys for structs".

This is a runtime defect, not an ISA defect. The V0.2.0 ISA's composite opcodes (`Op::NewTuple`, `Op::NewArray`, `Op::NewStruct`, `Op::NewEnum`, `Op::GetField`, `Op::GetTupleField`, `Op::GetEnumField`, `Op::GetIndex`, `Op::Len`) carry sufficient information for a flat-byte runtime: the count of operand-stack values to consume, the struct-template index, or the field name index in the constant pool. Each keeps its current signature; only the handlers change, to pack and read bytes rather than build `Vec`s. Offsets are resolved at dispatch from the struct template or from the layout reference the composite value carries, not from an emitted table, since there is no layout table in the artifact or at run time. (This paragraph predates P4. The refactor stayed runtime-side, but P4 did consolidate the four V0.2.0 construct opcodes into `NewComposite`, taking the live ISA from 69 to 66; `BYTECODE_VERSION` stays at 1.)

B26 and B27 are local fixes for two symptoms of this defect (persistent region byte portability, transient bodies on the global heap). B28 fixes the root cause and subsumes both.

A prior framing of B28 proposed to consolidate the composite opcodes around a single `AllocTransient(byte_size)` plus offset-and-kind read/write opcodes. The reverse-engineered conclusion is that the V0.2.0 ISA is already sufficient. Opcode consolidation does not reduce the per-program opcode count materially (the same byte writes happen either way) and does not facilitate WCET or WCMU analysis (per-opcode cost summation produces equivalent totals). The cost of consolidation is the wire-format churn and the loss of ISA stability; the benefit is small. Keep the V0.2.0 ISA.

**Design model: ASM section layout.**

The runtime memory layout mirrors the section model that an ASM file (or any ELF binary, or an NES ROM) uses:

- `.text` analogue: bytecode in `Module::chunks[].ops`. Read-only at runtime.
- `.rodata` analogue: the constant pool in `Module::constants`, the `const data` section declarations, and static string bodies. Read-only. The compiler may emit a warning when a `private data` section is never mutated within the program, suggesting the operator move it to `const data` for `.rodata` placement.
- `.data` analogue (mutable storage):
  - `private data` sections are materialised in the arena's persistent region. The persistent region survives RESET.
  - `shared data` sections are materialised in a host-passed struct external to the arena. The host owns the storage; the VM reads and writes through host-supplied APIs.
- Arena ephemeral region (no direct ASM analogue; functionally similar to the .data plus stack plus heap of a running process): two bump heads sharing one middle region.
  - Bottom head (currently used as the "stack"): operand stack vectors (`StackVec<'arena, T>` in `src/vm.rs:21`). Grows up from the boundary with the persistent region.
  - Top head (currently used as the "heap"): dynamic string bodies (`KString::alloc` in `src/kstring.rs:39`). Grows down from the arena's capacity boundary. Under B28, composite bodies and additional dynamic content join the top head.
  - Cleared on RESET. Cleared again on every reset issued by the host.
- Heap (global allocator): not used by the runtime for composite or string storage. Opaque values are the one exception because they are host-managed `Arc<dyn HostOpaque>` references.

The bottom-versus-top assignment of stack and heap is a convention that the existing consumer code established. Both heads are interchangeable bump allocators from the arena's perspective. The WCMU bound is the sum of the high watermarks of both heads plus the persistent-region capacity, so the bottom-versus-top split does not affect the bound itself.

Every value has a fixed byte size known at compile time. Scalars are inlined into their slot bytes. Strings, opaque references, and nested composites occupy fixed-size slots whose bytes are either inline (the scalar case) or a handle pointing to bytes elsewhere (the reference case).

**RESET semantics.**

RESET clears both ephemeral heads and increments the arena's epoch counter. The persistent region is untouched. The Keleusma `loop main(...) -> ReturnType { ... }` construct pairs each iteration with a RESET at the closing brace of the loop body. This is the structural source of the productive-divergent loop's bounded memory profile: within one iteration the ephemeral region accumulates up to its high watermark, then RESET releases all of it at the iteration boundary. WCMU is computed over the single-iteration high watermark, not the cumulative usage across iterations.

**Fixed byte size by type.**

| Type | Byte representation |
|------|---------------------|
| Unit | 0 bytes |
| Bool | 1 byte |
| Byte | 1 byte |
| Int | word_bytes (8 by default, 4/2/1 for narrow runtimes) |
| Fixed | word_bytes |
| Float | float_bytes (8 for f64, 4 for f32) |
| StaticStr | fixed-size reference into `.rodata` (rodata offset plus length, or pointer plus length) |
| KStr | fixed-size arena handle (pointer plus epoch in V0.2.x; a tighter packed form is a future optimisation) |
| Opaque | fixed-size `Arc<dyn HostOpaque>` pointer |
| Tuple, Array, Struct, Enum | arithmetic sum of constituent byte sizes (compile-time computed) |

Composites are byte-sums of their fields. A `(Word, Word)` tuple is `2 * word_bytes`. A `struct Point { x: Word, y: Word, name: Text }` is `2 * word_bytes + sizeof(StaticStr_or_KStr_handle)`. An `[Word; 8]` array is `8 * word_bytes`. An `enum Color { Red, Green, Blue }` is `1` (discriminant byte) plus `0` (largest variant payload). An `enum Maybe<Word> { None, Some(Word) }` is `1 + word_bytes`.

**ISA stays unchanged (zero opcode delta).**

An opcode analysis over the composite, data-segment, and type-test ops confirmed that no opcode is added or removed; the operands of the composite ops are re-specified, which is permitted because byte-code compatibility is not a goal. The construct ops (`NewStruct`, `NewTuple`, `NewArray`, `NewEnum`) carry a count, and a variant for the enum, and pack from the tagged operands they pop. The access ops (`GetField`, `GetTupleField`, `GetEnumField`, `GetIndex`) carry the compiler-baked field offset and kind, read the bytes at that offset, and push the correctly tagged scalar; a scalar field fits `(offset, kind)` in the four-byte record, a nested-composite field uses `(offset, size)` through the operand pool. `Op::GetData`/`Op::SetData` read or write a composite slot as a flat byte range, and `Op::GetDataIndexed`/`Op::SetDataIndexed` compute `base + index * element_bytes`.

Offsets are baked, not resolved at dispatch. The compiler already computes the layout in `layout_pass` and bakes each field offset and kind directly into the access instruction, the way an assembler resolves a struct equate. Construction infers field sizes from the kinds of the tagged operands it packs. The composite value is therefore pure bytes, with no template, no layout reference, and no layout table in the artifact or at run time. `layout_pass` is the compiler's transient symbol table, used to bake the offsets and to compute the worst-case-memory-usage bound, and is never written into the artifact. A tuple is just an anonymous struct and packs and reads identically.

**What stays the same.**

- The opcode set, 69 variants with their numeric encoding and semantic contracts, is preserved.
- `BYTECODE_VERSION` stays at 1, for lack of production traction, not because compatibility is maintained.
- The framing header, operand pool, chunk metadata, and constant pool are structurally unchanged.
- The public `Value` enum surface at the host API is preserved. `Value::Tuple`, `Value::Array`, `Value::Struct`, `Value::Enum` keep the same constructors and accessors; only the internal payload changes from `Vec<Value>` to flat bytes. The host marshalling boundary interprets those bytes from the static type a native function declares, not from anything on the value.

**Byte-code compatibility is not a goal.** V0.2.0 and V0.2.1 byte code may differ and need not interoperate. There is nothing deployed to break, so a program is simply recompiled. The version is not bumped because the number is not worth bumping without traction, not because the format is held stable.

**What changes (entirely runtime-side).**

- **Composite internal representation.** `Value::Tuple(Vec<Value>)` becomes a flat byte buffer, and the same for `Array`, `Struct`, `Enum`. A struct's `String` field names and a tuple's element list are gone from the value; the value carries no layout reference, template index, or `Arc`. It is pure bytes.
- **Op handler logic.** `NewStruct`, `NewTuple`, `NewArray`, `NewEnum` pack field bytes inline from the tagged operands. `GetField`, `GetTupleField`, `GetEnumField`, `GetIndex` read at the baked offset and push the tagged scalar. `SetData` for a composite slot copies the flat bytes into the slot's byte range.
- **Composite body storage.** Composite bodies move from the global heap to the arena's top ephemeral head alongside dynamic string bodies. Bump-allocated, mark-reclaimed at scope boundaries, cleared at the RESET that closes each `loop main()` iteration. The single-iteration high-water mark is the transient-region worst-case-memory-usage bound.
- **Worst-case-memory-usage calculation.** The verifier computes the precise byte cost from the compile-time layout. The V0.2.0 over-approximations from `Vec` headers, `String` keys, and per-element indirection disappear, so the numbers shrink and sharpen on recompilation.

**Documented suboptimality of the Rust model.**

Baking the offsets and reading composites as pure bytes removes what would otherwise have been the first two compromises, dispatch-time offset resolution and a value-carried layout reference. One compromise remains, recorded so the interim is not mistaken for the intended end state.

- **The operand stack stays tagged.** A composite is pure bytes, but a scalar on the operand stack still carries its kind as a tag, so the arithmetic and comparison ops dispatch on that tag rather than the kind living purely in the opcode. Making the stack untyped bytes would force every currently tag-dispatching op, such as `Op::Add`, to become kind-determined, which is the one change that would actually stress the opcode set, so it is deferred wholesale to the V0.4 native-code-generation redesign below.

**Deferred ISA redesign (V0.4 native code generation).**

The interim already bakes composite field offsets into the access instructions and reads composites as pure bytes, so the assembler model is partly in place. What remains for the V0.4 native-code-generation pass, where the instruction set is designed for rad-hard silicon rather than borrowed from the Rust host, is to make the operand stack itself untyped bytes so that scalars carry no kind tag either. Then every operation, not just composite access, is statically kind-determined and verified, and there is no run-time type dispatch anywhere. The opcode-set consequence, kind-specific opcodes versus a kind operand for the arithmetic and comparison ops that today dispatch on the value tag, is the design question for that pass and is captured here as input to it. See [`docs/roadmap/V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md).

**Information the compiler and runtime use.**

The compile-time layout pass (`src/layout_pass.rs`, landed in P1) walks every Keleusma type and produces a `LayoutDescriptor` describing its byte size and field offsets. The compiler uses this during emission to validate consistency. The runtime computes the same layouts at chunk-load time from the struct templates and the type declarations carried in the existing wire format. No additional wire-format bytes are required.

**Operational consequences.**

| Property | V0.2.x (current) | After B28 |
|----------|------------------|-----------|
| Composite construction cost | One `Vec` allocation plus N pushes; struct adds per-field `String` clone | Bump pointer increment plus N byte writes; no allocation |
| Heap allocation per composite | Yes (multiple `Vec` and `String` allocations) | Zero. Composites live in the arena's transient region |
| Embedded target without global allocator | Blocked | Supported |
| Byte-snapshot of persistent region | Captures stale pointers | Captures self-contained bytes (subsumes B26's Path C) |
| Arena-as-sole-allocator (R32's implicit promise) | Not delivered | Delivered (subsumes B27) |
| WCMU bound precision | Imprecise (over-approximates by `Vec`/`String` overhead the language does not require) | Precise (reflects the language's fixed-size guarantee) |
| Cache locality | One indirection per composite access plus another for elements | Locality follows source-level structure; LLVM register-allocation under V0.4.x lowering can see individual fields |
| Hot code swap migration | Walk `Value` trees, rebuild composites | Migration table maps offset-to-offset; flat-byte transfer |
| ISA opcode count | 69 | 66 (P4 consolidated the four construct opcodes into `NewComposite`) |
| Byte-code compatibility | not a goal; recompile | not a goal; recompile (no opcode change, but the value representation and worst-case-memory-usage numbers change) |

**Phased implementation plan.**

Revised plan. P0 and P1 landed. The remaining phases are resliced by field-type difficulty rather than by composite kind, because the difficulty is the field type, not the kind, so all four kinds migrate together on the shared flat-byte machinery and the hard reference-field case is isolated. This supersedes the prior P2 through P9, which cut by composite kind and met the reference-field problem four times.

| Phase | Scope | Status |
|-------|-------|--------|
| P0 | `LayoutDescriptor` and the flat-byte and scalar helpers | Complete (`45df5bf`). `FlatComposite` is refit in P2 to a pure byte buffer; the value carries no layout reference. |
| P1 | Compile-time layout pass (`layout_pass`), the compiler's transient symbol table | Complete (`0fc5950`) |
| P2 | Flat representation for fixed-scalar and nested-composite fields, all four composite kinds together. The composite value becomes pure flat bytes; the compiler bakes field offset and kind into the access ops and a count into the construct ops (operand re-spec, no opcode added or removed); the operand stack stays tagged; bytes are arena-resident in the end state, with temporary scaffolding permitted within the phase; the public `Value` surface and host marshalling keep working throughout. Composites containing a reference field (`Text`, `Opaque`) retain the boxed `Vec` representation until P3. | Complete on `feat-flat-memory-arena` (merged-and-pushed through Phase 1; Phase 2 local). All four composite kinds use the flat byte body through single construction choke points (`tuple_with_widths`/`array_with_widths`/`struct_with_widths`/`enum_with_widths`) with baked access at every emission site; the operand re-spec added `TupleField`/`StructField`/`EnumField`/`ArrayElem` with `Flat`, `FlatNested`, and boxed forms, no opcode added or removed. Nested-composite fields inline recursively, including nested enums (uniformly-flat enums padded to one fixed size, padding-tolerant equality preserving per-variant standalone behaviour). The compiler's flat-layout arithmetic is folded onto `LayoutContext`/`LayoutDescriptor`. Bytes are arena-resident: composites built in the VM live on the arena top ephemeral head (`FlatComposite` is `Inline(Vec<u8>)` / `Arena(ArenaHandle<[u8]>)`); reads go through `resolve(arena)`, equality through materialise-then-content (`if_exists` then `if_equals`), and returned, yielded, persistent-data, and native-boundary values materialise to inline so they survive `RESET` or the arena being dropped. Host marshalling reads and writes flat bodies through the element types (`KeleusmaType::flat_byte_size`/`from_flat_bytes`). Two deviations carried forward: float fields stay boxed pending a kind-aware equality (raw-byte equality would change plus-zero, minus-zero, and NaN comparison), and the typeless `format_value` display path renders a flat composite as a byte-length placeholder, an interim limitation until the return type is threaded or the V0.4 backend bakes display. One soundness gap is deferred to P4: the WCMU verifier does not yet count composite top-head bytes, so a bound can undercount a composite-heavy program; the runtime fails safe (`OutOfArena`, not undefined behaviour). |
| P3 | Reference-typed fields as fixed-size handles. `Text` and `Opaque` become handles; a dynamic string is an arena handle, an opaque a registry index. | Substantially complete on `feat-flat-memory-refs`. `Opaque` is flat in struct and enum fields as a one-word index into the VM `ephemeral_opaques` registry (deduped by `Arc::ptr_eq`, cleared at `RESET` so `Drop` runs, pointer-identity equality). `Text` is flat in struct and enum fields as a two-word `(ptr, len)` arena handle; the epoch is supplied by the arena wrapper rather than stored in the field. Host-boundary decode of both is implemented across struct, enum, nested-container, and native-argument paths via a `RefContext` threaded through `from_value_ctx`/`from_flat_bytes_ctx` and the `Vm::decode` helper. **Operator decisions (2026-06-08), see `REVERSE_PROMPT.md`:** (1) the compiler bakes the flat access operand from the type-checked type, so tuple and array reference elements become flat and the value-driven boxing fallback is removed; (2) enum equality is compiled to compare the used bytes with `N` baked at compile time, removing the runtime padding slack zero-fill; (3) the `Text` slot stays two words and extraction reattaches the originating composite's epoch (not the current arena epoch), so a stale read is a clean `Stale` outcome, and a composite transitively containing `Text` inherits the bare dynamic-string flow restrictions (cross-yield prohibition, data-segment exclusion) enforced by the type checker. The epoch-sourcing fix, the transitive flow check, the access-operand baking, the field-wise enum equality, and the `into_value_ctx` host-return path are the remaining P3 follow-ups. |
| P4 | Worst-case-memory-usage recomputation in the verifier from the compile-time layout. Golden bounds update to the precise values; cost-model recalibration via `keleusma-bench` if cycle counts drift. | **Complete on `feat-flat-memory-wcmu`.** The four V0.2.0 construct opcodes (`NewStruct`/`NewTuple`/`NewArray`/`NewEnum`, wire ids 34-37) are consolidated into one `NewComposite` (wire id 69) carrying the exact flat allocation byte size. This is a deliberate scope expansion the operator directed (net minus-three opcodes) that supersedes the original no-opcode-change invariant: a tuple is an anonymous struct, an array a homogeneous struct, and a flat enum a struct whose first packed value is the discriminant, so flat construction is one operation. The verifier sums the operand-carried byte sizes instead of estimating `count * VALUE_SLOT_SIZE_BYTES`, closing the P2 undercount gap. The live ISA is 66 opcodes with ids 34-37 retired and reserved, maximum live id 69, `BYTECODE_VERSION` unchanged at 1. The compiler emits `NewComposite` at every construct site (struct and enum operand driven, tuple and array value driven because element-type inference can fail); the bench cost models and the authoritative `INSTRUCTION_SET.md`, `WIRE_FORMAT.md`, and `EXECUTION_MODEL.md` are reconciled. Tuple and array operand byte size is exact when inference succeeds and conservatively over-approximate otherwise, which keeps the bound sound. |
| P5 | Hot-code-swap migration over flat bytes, documentation (`EXECUTION_MODEL.md`, the dual-end-arena and hot-swap decisions; the `NewComposite` consolidation is already reflected in `INSTRUCTION_SET.md` and `WIRE_FORMAT.md` from P4), B26 and B27 marked resolved through B28, and B28 closure. | **Complete (V0.2.1, 2026-06-24).** Hot-swap migration over flat bytes shipped as the strict-schema-check plus host-owned Replace model: `replace_module` preserves a same-schema private region in place and rejects a schema mismatch unless `replace_module_unchecked` is called with fresh private initial data, and the host owns and supplies the shared buffer; `EXECUTION_MODEL.md` documents this and it superseded the offset-to-offset migration-table sketch. Documentation reconciled (the authoritative `INSTRUCTION_SET.md` already records 66 opcodes and `EXECUTION_MODEL.md` the borrowed shared-buffer model; this entry's stale `69`-opcode and implementation-status text were corrected). B26 and B27 are marked resolved through B28. B28 is closed; the entry is retained as the design record. |

No opcode changes in any phase. `BYTECODE_VERSION` stays at 1 for lack of traction, not for compatibility.

**Compatibility.** Not a goal. V0.2.0 and V0.2.1 byte code may differ and need not interoperate, and a program is simply recompiled. The recompiled program reports smaller and more precise worst-case-memory-usage bounds, since the `Vec` and `String` over-approximation is gone, so any tool that consumes those numbers should expect them to change.

**Forcing case.**

V0.4.x cross-target deployment, particularly to embedded targets without a global allocator (Cortex-M55, Cortex-M variants, rad-tolerant cores). V0.5.x self-hosted compiler that produces native code through `llvm-mos` or similar backends and wants LLVM's register allocator to see through composite values. The fleet delivery scenarios that require deterministic memory-allocation behaviour for verification.

**Composition with B26 and B27.**

B28 supersedes both. After B28 lands:

- The persistent region naturally holds `private data` composites inline because the entire composite is flat bytes. B26's Path C is the natural outcome of B28; the work in B26 is absorbed.
- The top ephemeral head naturally holds composite bodies because composite construction is bump-allocator-style and reclamation is mark-based. B27's allocator parameterisation is moot; there is no `Vec` to parameterise.

B26 and B27 stay in the backlog as documentary captures of the symptom-level fixes that were considered. When B28 lands, B26 and B27 transition to "resolved through B28" status.

**Cross-references.**

- B26 (arena-resident persistent region for composite data values) is a symptom-level fix that B28 subsumes.
- B27 (arena-resident transient region for composite Value bodies) is a symptom-level fix that B28 subsumes.
- B29 (strippable debug metadata) is independent of B28. The strippable framework still applies to debug aids like variable names, source spans, breakpoints; it no longer carries a `DataSlotAnnotation` record load-bearing for B28 because the runtime computes layouts from existing wire-format metadata.
- R32 (dual-end arena) is the prior decision; B28 delivers the arena-as-sole-allocator property R32 implicitly promised but did not achieve.
- R29 (hot code swap) interacts: the migration path needs updating because the Value tree's internal shape changes.
- B16 (parametric Vm for sub-64-bit native runtimes) intersects: byte-size computation depends on the target's word and float widths. `LayoutDescriptor` is parameterised over those widths from P0.
- B24 (hardware-isolation for Cortex-M) is the deployment family that benefits from arena-as-sole-allocator and from precise WCMU bounds.
- The fleet delivery scenarios are the operational shape that benefits most from the deterministic-allocator property and the high-precision WCMU precision.
- V0.2.0's WCMU calculation imprecision is the operational artefact of this defect; corrected runtime produces corrected numbers.

## ~~B29. Strippable debug metadata in the ISA~~ (Resolved for V0.2.1; three precision refinements deferred)

> **Implementation status (V0.2.1): complete; all twelve record kinds emit and the VM trap read path is wired; only precision refinements remain.** The chunk-local `debug_pool` section, its data model, and its canonical byte encoding are implemented ([`src/debug_meta.rs`](../../src/debug_meta.rs)), carried through the wire format as an optional per-chunk field that leaves the opcode stream byte-identical ([`src/wire_format.rs`](../../src/wire_format.rs)), emitted by the compiler under `compile_with_options` / `keleusma compile --debug`, removed by the `keleusma strip` subcommand, read back through a query API (`DebugPool::records_at`, `source_location`), and surfaced at runtime traps through `Vm::fault_source_location`. Invariants 4 and 5 are exercised end to end: a `--debug` build strips to byte-identical release bytes. The authoritative format reference is [`DEBUG_METADATA.md`](../spec/DEBUG_METADATA.md); this section records the design rationale and decision history.
>
> Emitted today (all twelve): `CallSite` (per call instruction), `SourceSpan` and `LineNumber` (per statement), `VariableName` (per declared local slot), `OptimisationMarker` (at each refinement-elision site, where the compiler proves a refinement predicate at compile time and omits the runtime check), `GenericInstantiation` (per monomorphized chunk, naming the generic origin and canonical type arguments, threaded from the monomorphizer through `monomorphize_with_provenance`), `IfcLabelAnnotation` (at each `classify`/`declassify` operation, recording the label set for the information-flow audit trail), `TypeAnnotation` (per declared local slot, carrying a string-form `TypeRepr` v1 in the type sub-pool), and `WcetMarker` (per Stream chunk, the verifier-stage per-iteration WCET appended to the built `debug_pool`, carried as two `u16` operands reconstructing the `u32` cycle count). The `TypeAnnotation` scope is the per-local declared or inferred type, not the full per-op operand-stack type model that finer-grained stack introspection would require; the latter remains future work. `AssertionContext` emits at each debug `assert`: a compile-out `assert` construct was added to the language (grammar, parser, type checker, codegen, and the `AssertionFailed` trap), and under a debug build each `assert` emits an `AssertionContext` record carrying the source span and optional message. `assert` is a contextual keyword like `classify`/`declassify`; the check compiles out entirely in a non-debug build, so a debug and a release build are distinct compilations rather than one artefact bridged by `strip`. See [`GRAMMAR.md`](../spec/GRAMMAR.md) and [`RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md). `BreakpointCandidate` emits at each statement boundary, at each block's tail expression, at the trap-bearing operator ops (`Div`/`Mod`), and at function entry (op 0 of every chunk), recording the op position and source span a debugger reads to present breakpoint choices. `VerifierWitness` emits a per-construct verification trace at the verifier stage. `verify::chunk_verification_witness` returns the per-chunk admission summary (the passes that admitted the chunk), and `verify::chunk_verification_obligations` returns the finer **structural** trace: one `VerificationObligation` per individual check the three structural passes of `verify` discharge, each keyed to the op position of the construct it concerns. The trace is produced by the same per-chunk routine that renders the verdict (`verify` and `chunk_verification_obligations` both call the private `verify_chunk`, the latter with an obligation sink), so the trace cannot drift from the checks the verifier actually performs: each obligation is recorded at the point its check is discharged, and a chunk that would fail `verify` yields a trace truncated at the first failing check rather than fabricated facts. Under a debug build the compile pipeline emits one `VerifierWitness` record per obligation, with operands `[pass, property]` and the construct's `op_index` (chunk-level facts such as `all-blocks-closed` carry `op_index = 0`). A reader groups them with `DebugPool::records_at`. Pass 1 records one obligation per individual check (`if-branch-target-in-bounds`; the three Else checks `else-preceded-by-matching-if`, `else-target-in-bounds`, `else-target-is-endif`; `endif-closes-open-if`; `loop-exit-target-in-bounds`; `endloop-closes-open-loop` and `endloop-back-edge-targets-loop-entry`; the break and data-slot checks; and the chunk-level `all-blocks-closed`); pass 2 records the block-type constraints keyed to the marker ops (`func-has-no-yield`/`-stream`/`-reset`, the reentrant and stream variants); pass 3 records `every-stream-to-reset-path-yields` at the Stream op.
>
> Beyond the structural passes, the compile pipeline emits **resource-bound** obligations (pass `resource-bounds`) on the arms where a bound is actually proven. For each Stream chunk: `wcet-per-iteration-bound-proven` when `wcet_stream_iteration` returns a finite cycle count (the count itself is carried by the adjacent `WcetMarker`), and `wcmu-per-iteration-bound-proven` when `wcmu_stream_iteration` returns a finite total. For each Func or Reentrant chunk: `wcet-per-chunk-bound-proven` and `wcmu-per-chunk-bound-proven`, computed by `wcet_whole_chunk`/`wcmu_whole_chunk` over the chunk's whole op range (the same cost walk as the Stream path). For a Func chunk the WCET is the per-call bound; for a Reentrant chunk (a `yield` function) `wcet_whole_chunk` returns the worst-case single-resumption WCET: `reentrant_segmented_wcet` gives the exact maximum inter-yield segment cost when every `Yield` is top-level, and when a `Yield` is nested in an `If`/`Loop` the whole-body cost is computed with each provably-productive yield-loop clamped to one iteration (a single resumption cannot complete more than one pass of a loop whose every body path yields). The clamp is guarded against nested loops and conditional yields (a loop that is not provably productive keeps its full iteration count), so the bound is never under-counted; it is a sound upper bound, tighter than the plain cumulative cost whenever a productive yield-loop iterates more than once. The Reentrant WCMU is the genuine persistent peak, since the coroutine frame and operand stack survive yields. These obligations are witness facts only: they are **not** folded into `module.wcet_cycles`/`module.wcmu_bytes`, which remain the per-iteration maximum across Stream chunks (a Stream-free module declares no header bound). All of these bounds are shallow with respect to calls (an `Op::Call` contributes its dispatch cycle, not the callee's body; there is no transitive WCET resolver). Admission of the program against a host's arena capacity is a separate load-time analysis (`verify_resource_bounds`) run at `Vm::new`, not at compile time, so the witness deliberately does not assert it. A chunk whose bound does not prove records no resource-bound obligation, so absence is the faithful signal. The Reentrant WCET is exact (per-segment) for top-level yields and a sound, productive-loop-tightened bound for nested yields; straight-line code summed across yields keeps the nested bound loose but sound.
>
> The structural obligations are now produced inline by the verify routine itself (one obligation per check, recorded as the check is discharged), and the resource-bound obligations are recorded on the proof arms where the WCET and WCMU bounds are established. The trace is therefore a faithful record of the verifier's own checks rather than an independent re-derivation, though it remains a record of which checks passed, not a machine-checkable proof object.
>
> **The B29 record catalogue is complete: all twelve kinds emit.** The breakpoint *runtime mechanism* is now implemented: the VM holds a `breakpoint_positions` list, suspends with `GenericVmState::BreakpointHit { chunk, op }` before executing an armed op (gated on an `is_empty` fast path so unarmed programs pay nothing), exposes `set_breakpoint`/`clear_breakpoint`/`clear_breakpoints`/`breakpoint_count` and `resume_from_breakpoint` (a value-free resume with one-shot re-trigger suppression), and arms a position from a `BreakpointCandidate` record's op index. Breakpoints are runtime state, not bytecode, so they do not affect the verified WCET or WCMU bounds. Breakpoint candidates now emit at statement boundaries, block tail expressions, the trap-bearing operator ops (`Div`/`Mod`), and function entry (op 0); a candidate at *every* operator op remains intentionally unimplemented, since an every-op candidate set is high-volume and of marginal value over the boundary and trap-operator candidates.
>
> The read path into the VM *trap* path is now wired. The VM records the op it is dispatching in a `fault_location` field (a single `Option` write per op, cleared by the `run` wrapper on success), so after a failed `call`/`resume` it names the faulting op. `Vm::fault_location() -> Option<(chunk, op)>` exposes it, and `Vm::fault_source_location() -> Option<FaultSource>` maps it back to source by decoding the faulting chunk's debug pool on demand: tier 1 returns a span-bearing record (`CallSite`, `SourceSpan`, `AssertionContext`, `BreakpointCandidate`) sitting exactly at the fault op (`exact = true`); tier 2 falls back to the nearest enclosing statement's `SourceSpan` (`exact = false`). It never fabricates a location, and the owned `FaultSource` carries the `exact` flag so a host does not over-trust a fallback. A failed debug `assert` resolves exactly through its `AssertionContext`, and the partial operations resolve exactly through the `SourceSpan` the compiler now emits at their operator op: division and modulo by zero (`Div`/`Mod`), array indexing (`GetIndex` for stack arrays, `GetDataIndexed`/`SetDataIndexed` for data arrays), and newtype-construction refinement failure (the `Trap` op). The compiler also emits a `SourceSpan` for each block's tail expression, so a fault in a function whose body is one expression resolves to that expression's span rather than to nothing. A trap at an op that still carries no operator-site span resolves to the tightest enclosing statement or expression (`exact = false`).
>
> The three precision refinements have been addressed to the extent each is cleanly actionable; what remains in each is a deliberately bounded residual rather than a coverage gap: (1) **operator `BreakpointCandidate` granularity** — candidates emit at statement boundaries, tail expressions, trap-bearing operators (`Div`/`Mod`), and function entry; a candidate at *every* operator op is left unimplemented as high-volume and marginal. (2) **per-resume Reentrant WCET** — `wcet_whole_chunk` returns the exact per-resume WCET (maximum inter-yield segment cost) for top-level yields, and for nested yields a sound bound that clamps each provably-productive yield-loop to one iteration (guarded against conditional yields and nested loops, so never under-counted). The residual is the remaining looseness of the nested bound: straight-line code summed across yields and conditional-yield-in-loop cases stay at the conservative count; tightening them fully would require a structured max-yield-free-segment (CFG longest-path) analysis whose marginal value does not justify the soundness risk, especially as the bound is not surfaced in the witness (which records presence only). (3) **per-op `SourceSpan` granularity** — operator-site spans now make every partial operation resolve exactly (division and modulo, array and data-array indexing, and newtype-construction refinement failure), and tail-expression spans cover one-expression bodies; full per-op spans for non-trapping ops remain unimplemented as high-volume and of marginal value for fault localization.

> **Design revision (supersedes the original inline-opcode framing).** B29 originally proposed strippable debug *opcodes* emitted inline in the op stream. That framing is withdrawn. Control flow in this ISA is block-structured and encodes every branch target as an op-index offset within the chunk (`If(u16)`, `Loop`, `EndLoop`, `Break`; see [`STRUCTURAL_ISA.md`](../spec/STRUCTURAL_ISA.md) and `src/bytecode.rs`). An inline debug opcode would shift the op index of every later instruction, so the debug build's control-flow offsets would differ from the release build's and the stripper would have to rewrite them. That breaks invariants 4 and 5 below. The revised design keeps every debug marker out of the op stream: markers are op-index-keyed records in the chunk-local `debug_pool` section. The op stream is then byte-identical between debug and release builds, strip is a pure subtraction (drop the section), and the invariants hold strictly. The label "debug opcodes" survives in some cross-references for continuity, but the markers are records, not opcodes.

B29 provides a strippable debug-metadata framework: compiler-emitted records that the runtime and external tooling consume for development aids (variable names, source spans, breakpoints, IFC label audit trails) and that a `keleusma strip` tool removes without affecting execution semantics or the op stream. B29 is independent of B28; an earlier framing tied them together through a `DataSlotAnnotation` marker that B28 needed for runtime data-section layout, but B28's pure-runtime-refactor framing computes data-section layouts from existing wire-format metadata and no longer requires it. The strippable framework remains useful for the rest of the catalogue. This entry catalogues the candidate debug records, names the stripper tool, and pins the wire-format invariants the design must preserve.

The general pattern: the compiler attaches debug-info records to a chunk-local `debug_pool` section. Each record names the op-stream position it annotates by op index and carries small indices into the section's variable-length sub-pools (strings, source spans, type representations). The op stream is unchanged by the presence or absence of debug info. The runtime and external tooling read the records at chunk-load time or on demand, use them to enrich diagnostics, and tolerate their absence by falling back to the V0.2.x behaviour. A `keleusma strip` subcommand drops the chunk's `debug_pool` section; the stripped artefact's op stream is byte-identical to a release build compiled without debug info. Execution semantics are identical; only debug-grade properties degrade.

**Design metaphor.** The op stream is a finished page of text. Debug metadata is a transparent overlay registered to the page by position: it marks points on the page and attaches notes (variable names, IFC labels, source spans, type representations) without altering a single character of the page. `keleusma strip` lifts the overlay off; the page underneath is byte-identical to a page that never had one. Compiling with debug info on registers an overlay to the same page.

**BYTECODE_VERSION stays at 1.** Keleusma has no production traction; backward-compatibility is not a constraint. The chunk-level `debug_pool` section is introduced without a version bump. Bytecode produced before B28 and B29 has an absent debug pool; bytecode produced after may carry one. Runtimes built after the change handle both; runtimes built before do not know the section, but no such runtimes exist in production.

Five invariants the design preserves:

1. **Debug metadata lives only in the `debug_pool` section, never in the op stream.** Strippability is a property of the section, which is removable as a unit. The op stream is never walked or rewritten by strip, and no opcode carries a strippable flag because no opcode is debug-only.

2. **Stripped and unstripped bytecode produce the same execution behaviour.** A program that compiles, signs, encrypts, and runs successfully produces identical observable outputs whether or not it was stripped before execution. The strip step is purely byte-reduction and debug-grade information loss.

3. **Debug metadata never affects the operand stack or control flow.** Records are positional annotations held outside the op stream; they neither push nor pop nor alter branch targets. The verifier's stack-effect and CFG analyses are identical with or without the section, and the WCMU computation treats the section as zero runtime cost.

4. **Debug content adds to and subtracts from the release format cleanly.** Producing debug bytecode from release bytecode appends a `debug_pool` section to each chunk and changes nothing else. Producing release bytecode from debug bytecode drops the section. No op-stream bytes and no other chunk field change in either direction. This invariant is what makes `strip` a verifiable operation rather than a lossy transformation, and it holds strictly because no debug content is ever interleaved with the program.

5. **Strip is byte-deterministic and order-preserving.** Given the same unstripped input, the strip tool produces byte-identical stripped output across runs, machines, and tool versions. Because the op stream is identical in stripped and unstripped builds, a stripped artefact is byte-identical to a release build compiled without debug info, which is what makes reproducible builds across the strip step possible. Strip is the removal of a separable section, not a transform of the program. The records within the section are emitted in a canonical order (by op index, then kind) so the section itself is byte-deterministic.

**Chosen format: chunk-local `debug_pool` section (Shape B)**

The debug records and the variable-length data they reference (variable names, source spans, type representations, IFC label sets) live in a chunk-local `debug_pool: Option<Vec<u8>>` field that the wire format gains alongside the existing per-chunk fields (`constants`, `struct_templates`). The field is optional and length-prefixed, following the existing wire-format conventions for per-chunk optional sections. Bytecode without debug info omits the field entirely; bytecode with debug info carries the section.

The section is four concatenated sub-pools: three data sub-pools holding the variable-length payloads, and one record sub-pool holding the op-index-keyed markers themselves:

```
debug_pool = {
    string_pool_length: u32,
    string_pool: Vec<String>,            // length-prefixed UTF-8 strings
    span_pool_length: u32,
    span_pool: Vec<(u16, u32, u32)>,     // (file_string_idx, byte_offset, byte_length)
    type_pool_length: u32,
    type_pool: Vec<TypeRepr>,            // compact type representations for `TypeAnnotation`
    record_pool_length: u32,
    record_pool: Vec<DebugRecord>,       // op-index-keyed markers, canonical order
}

DebugRecord = {
    op_index: u32,                       // the op-stream position this record annotates
    kind: u8,                            // CallSite, SourceSpan, VariableName, ...
    operands: [u16; k],                  // indices into the sub-pools above
}
```

Each record names the op position it annotates and carries u16 indices into the sub-pools rather than inline payloads. The record sub-pool is sorted canonically by `(op_index, kind)`, so emission is byte-deterministic and a loader can binary-search records for a given op position. The op stream carries no debug bytes and no debug opcodes; the fixed-size opcode records and the block-structured offset encoding are untouched.

`keleusma strip` is a one-step pass per chunk: set the chunk's `debug_pool` field to absent. Every other chunk byte, including the entire op stream and all control-flow offsets, is preserved exactly.

This design realises the overlay metaphor directly. The op stream is the page; the `debug_pool` section is the overlay registered to it by op index. Stripping lifts the overlay; the page is byte-identical to a release build that was compiled without debug info in the first place.

**Alternatives considered and rejected**

| Shape | Description | Reason rejected |
|-------|-------------|-----------------|
| A: Op-stream-introduced inline pool | A marker in the op stream introduces a length-prefixed payload of raw bytes inline. Records that follow reference offsets into the pool. | The data lives inside the op stream rather than alongside it. Stripping requires excising a region of the op stream and rewriting control-flow offsets, not detaching a separate field. This is the inline approach the revision note above withdraws. |
| C: Module-level debug pool | A single `debug_pool` field at the module level holds debug operand data for all chunks. Records carry module-level offsets. | Per-chunk pools mirror the natural compilation unit; module-level pools force cross-chunk reference resolution at load time. The deduplication benefit is small for typical programs because most identifier strings are local to their declaring chunk. Per-chunk pools also let the loader stream chunk loads without pre-reading a global addendum. |
| D: Reuse existing constants pool | Records reference indices in the chunk's existing constants pool; the stripper garbage-collects unreferenced constants. | Renumbering live constants after strip is invasive: every op in the chunk that references a moved constant must be rewritten. The release-format chunk of a stripped artefact would then differ from a freshly-compiled release artefact in constant indices, violating the symmetric add/subtract invariant. |

**Candidate debug records**

The following record kinds are candidates. Each is sized by its expected operational benefit against the cost of carrying it. The first three are the strongest candidates because their benefits are visible in operational artefacts that operators routinely consume (stack traces, debugger sessions, error messages with source positions). The remainder are nice-to-have for richer development experience.

| Record kind | Purpose | Operands | Strip impact |
|--------|---------|----------|--------------|
| `CallSite` | Per-call-site source position for stack traces. The proposal model is whatever shape `Rex` uses for the same purpose | Source file index plus source span (byte offset, byte length) | Stripping makes runtime stack traces position-free; error messages report function name only |
| `SourceSpan` | Per-op source position. Finer-grained than `CallSite`; allows a debugger to highlight the exact op responsible for a fault | Source file index plus source span | Stripping makes per-op debugger highlighting unavailable |
| `LineNumber` | Per-op source line. Coarser than `SourceSpan` but cheaper to carry | Line number (u16 or u32) | Stripping disables line-precise debugging |
| `VariableName` | Per-local-slot human-readable name. Lets debuggers display `count` rather than `local_3` | Local-slot index plus name index in the string sub-pool | Stripping makes debugger variable inspection numeric-only |
| `TypeAnnotation` | Per-stack-position type for debugger introspection | Stack offset plus compact type representation | Stripping makes type inspection reliant on inferred types |
| `AssertionContext` | Structured info for assertion failures: which assertion fired, source location, message | Assertion id plus source span | Stripping makes assertion errors generic ("assertion failed") rather than specific |
| `BreakpointCandidate` | A source-level position at which a developer may set a breakpoint. The compiler emits these at statement boundaries by default; coarser (function-entry) and finer (operator) granularity are configurable through a compile-time flag. Arming a breakpoint inserts the record's op index into the VM position list (see "Breakpoint mechanism") | Op index plus span sub-pool index for the corresponding source position | Stripping removes the candidate set; arbitrary-position bytecode-level debugging uses the VM's position-list mechanism directly |
| `GenericInstantiation` | Marker noting which monomorphisation this chunk was generated from. Lets debuggers show `Vec<Word>` rather than `Vec_Word__instance_7` | Source generic identifier plus instantiation arguments | Stripping makes monomorphised functions appear anonymous |
| `IfcLabelAnnotation` | Per-position IFC label info beyond what the type system already carries. Strictly compile-time today, but the annotation creates an audit trail | Position plus label set | Stripping reduces IFC audit trail granularity |
| `WCETMarker` | Per-block WCET annotation from the cost model. Lets runtime telemetry compare measured against declared bounds | Block id plus declared cycle count | Stripping disables runtime WCET telemetry |
| `OptimisationMarker` | Records which optimisations the compiler applied to a region. Pure provenance | Optimisation identifier | Stripping erases optimisation history |
| `VerifierWitness` | Trace of why the verifier accepted (or rejected) certain constructs. Audit-grade info for auditors | Witness identifier plus structured payload | Stripping reduces audit trail for audit artefacts |

**Breakpoint mechanism**

Breakpoints use the VM position-list mechanism exclusively; there is no breakpoint opcode in the op stream. Debug builds additionally carry `BreakpointCandidate` records that enumerate the source-level positions at which a developer may set a breakpoint.

*Surface-language breakpoints.* The compiler emits `BreakpointCandidate` records at statement boundaries by default, with function-entry (coarser) and operator (finer) granularity selectable through a compile-time flag. Each record pins an op index and the corresponding source position (a span sub-pool index). A host or debugger reads the candidate set to present source-level breakpoint choices. Arming a chosen breakpoint inserts its op index into the VM's `breakpoint_positions` list. The bytecode is never modified, which preserves signing, encryption, and verification invariants.

*Bytecode-level breakpoints (VM position list).* The VM holds a `breakpoint_positions: Vec<(ChunkId, OpOffset)>` list as part of its runtime state. Before each op dispatch, the VM checks whether the current position appears in the list; if yes, it yields with a `BreakpointHit { position }` reason. The cost is one branch per op dispatch when the position list is non-empty; the branch becomes a constant-folded zero when the list is empty (an `is_empty` check that the compiler hoists). The list works identically on stripped and unstripped artefacts because it is runtime state rather than bytecode. Surface-language users who set no breakpoints populate nothing and pay nothing. This single mechanism serves both the source-level case (op indices come from `BreakpointCandidate` records) and the arbitrary-position case (op indices come from a bytecode-level tool), and it replaces the withdrawn inline `Breakpoint` opcode. The trade is that breakpoint checking is a per-op list test while a breakpoint is armed, rather than a zero-cost-between-dispatches inline opcode; the cost is paid only in debug sessions with armed breakpoints, never in production stripped artefacts.

*Granularity rationale.* Statement-level is the default because most users interact through the surface language and expect statement-level breakpoints from conventional debuggers. Function-entry is appropriate for very dense code where statement-level emission would balloon the debug build. Operator-level is for fine-grained investigation of complex expressions and is the right granularity for users who think in opcodes rather than statements. The choice is per-build, not per-script, so a project can carry both a statement-level debug build for casual debugging and an operator-level debug build for deep investigation without changing the source.

**The `keleusma strip` subcommand**

The stripper is a thin tool that opens a bytecode file, sets each chunk's `debug_pool` field to absent, recomputes the chunk and module sizes, regenerates the CRC trailer, and writes the result. It does not walk or rewrite the op stream. The flow is otherwise identical to `keleusma compile -o <out>` except that the input is already-compiled bytecode and the output is a debug-info-reduced version. The strip operation preserves Ed25519 signatures only when the input was unsigned; signed artefacts cannot be re-signed by the stripper because it does not have the private key. The natural workflow is therefore "compile, strip, sign" rather than "compile, sign, strip" — the strip step runs before the signing step.

A `--keep <record-kind>` flag lets operators selectively retain specific record kinds. The default strips the entire `debug_pool`. The selective form is useful for operators who want call-site info preserved for error reports but not per-op source spans; in that case the stripper re-emits the `debug_pool` retaining only the named record kinds and the sub-pool entries they reference, in the same canonical order, so the selective output is itself byte-deterministic.

**Implementation cost**

| Phase | Effort |
|-------|--------|
| Wire format: add the optional per-chunk `debug_pool` section and the `DebugRecord` encoding | Two to three days |
| Compiler emits `CallSite`, `SourceSpan`, `LineNumber` records (the most-useful first set) | Three to five days |
| Compiler emits the remaining record kinds above | Five to ten days total |
| Runtime and tooling read the records (pass-through; B29 does not feed any load-bearing behaviour) | Two to three days |
| `keleusma strip` subcommand | Two to four days |
| Tests for stripped/unstripped equivalence and byte-deterministic strip | Two to four days |
| Documentation: wire format spec, CLI README | Two to three days |

Total estimated effort: three to five weeks for the full catalogue. The section mechanism itself is small; the volume is in emitting and consuming the individual record kinds.

A reduced first commit could land the `debug_pool` section plus the `CallSite` record (the highest-value record for stack-trace quality), then add `SourceSpan`, `LineNumber`, `VariableName` in follow-on commits.

**Compatibility**

`BYTECODE_VERSION` stays at 1. Keleusma has no production traction, so backward-compatibility is not a constraint and the version-bump cost is unjustified. The chunk-local `debug_pool` section lands as a V0.2.y or V0.3 wire-format change without a version bump. Bytecode produced before this entry has an absent debug pool; bytecode produced after may carry one. Runtimes built before this entry do not know the optional section. Because the op stream and every existing field are unchanged, a debug-carrying chunk differs from a release chunk only by the appended optional section, so the failure mode for an older runtime is limited to the new section rather than the program itself.

The wire format framing is unchanged. The chunk-level `debug_pool` field is a new optional length-prefixed section that follows the existing per-chunk optional-section convention. A chunk without debug info serialises identically to today's chunk format; a chunk with debug info appends the additional section.

**Forcing case**

Debugger quality, audit artefacts, and richer error reports each create demand for one or more debug record kinds. None individually forces the entire framework; the framework lands once and admits incremental record-kind additions thereafter. Each record kind becomes operationally important when its consuming workflow (a debugger, a auditor, a runtime introspection tool) becomes operationally important.

B35 P1 created one concrete near-term demand. The compiler-emitted traps now surface as specific `VmError` variants without the dynamic detail the prior message carried, namely the failing predicate and newtype names for a refinement trap and the function name for a no-matching-head trap, so a runtime trap identifies its cause but not its source site. The intended way to restore localization is the `SourceSpan` or `CallSite` debug record here, or a dedicated trap-context record, which would let a host map a trap back to a source position at `Word` cost rather than re-embedding a string. So a refinement or no-match trap is a consumer of this framework, and the `AssertionContext` row is the closest existing analogue for the trap case.

**Cross-references**

- B28 (runtime composite Value representation aligned with the language guarantee) is independent of B29 under the pure-runtime-refactor framing. An earlier framing tied them together; that coupling no longer applies.
- `docs/spec/WIRE_FORMAT.md` documents the optional per-chunk `debug_pool` section and its sub-pool layout including the `DebugRecord` encoding (added when the framework landed in V0.2.1).
- `docs/spec/INSTRUCTION_SET.md` and `docs/spec/STRUCTURAL_ISA.md` are unchanged by this entry, since no opcode is added or marked debug-only; the op stream carries no debug content.
- The CLI README's `compile` and `run` sections get a note about the strip workflow; a new `keleusma strip` subcommand is added.
- `Rex` is the cited prior art for `CallSite`'s shape; the specific mechanism it uses is the basis for this entry's `CallSite` design.
- B35 (Partial Operation Handling) P1 dropped the dynamic detail from runtime traps in favor of structured `VmError` variants. Restoring trap localization is a consumer of this framework's `SourceSpan`, `CallSite`, or a dedicated trap-context opcode, rather than a re-embedded message string.

## B30. CLI runner deferred work

Three items deferred during the V0.2.1 CLI runner work that are not specific to the `run-tasks` multirunner. Each is individually substantial and individually scoped. None blocks any V0.2.x release; each becomes load-bearing when a concrete operator workload calls for it. This entry consolidates the items from `docs/process/REVERSE_PROMPT.md` (which is overwritten each session) into a durable backlog record.

| # | Item | Description | Forcing case |
|---|------|-------------|--------------|
| 1 | Mutable `shared`/`private data` REPL persistence beyond scalars | The V0.2.x REPL persists shared data through the host-visible `Vm::set_data` and `Vm::get_data` Value-clone API. Scalar types round-trip correctly. Composite types (Tuple, Array, Struct, Enum) clone correctly across evaluations through the existing API. Private data slots have no equivalent host API; private data persistence requires either an arena snapshot-and-restore mechanism in the VM or incremental module loading that preserves state across recompiles. The structural fix is B28's flat-byte composite representation, which makes byte-snapshot of the persistent region sound. | Operators who want their REPL session state to survive a private-data declaration would feel this most. |
| 2 | Generic `Result<T, E>` type | A language-design question deferred deliberately. The V0.2.x bundled shell natives use the trap-on-error pattern (errors surface as `VmError::NativeError` and halt execution) rather than returning a `Result` that the script-side code unwraps. Adding `Result<T, E>` adds a sum-type pattern that the language does not currently provide; the choice between trap-on-error and explicit-Result has implications for total-functional reasoning, WCET analysis, and the verification narrative. | Operators who want to write recoverable shell pipelines rather than trap-on-first-failure would feel this most. The host-native error-handling boundary is the natural location for it. |
| 3 | `shell::read_lines` native | Returns the lines of a text file as a script-visible collection. Contingent on a dynamic-length Array type or equivalent. The V0.2.x `shell::read_string` returns the entire file contents as a single Text; line-by-line iteration in script-side code requires splitting the string, which the bundled natives do not currently provide. | Operators who want to process line-delimited inputs (logs, CSV, configuration files) inside a Keleusma script without a host-side preprocessor. |

**Cross-references**

- B28 (runtime composite Value representation aligned with the language guarantee) is the structural fix for item 1's deeper case (private-data byte snapshot).
- The Generic `Result<T, E>` decision is in the V0.3.x language-extension territory; it interacts with monomorphisation and with the host-native marshalling surface.
- The `shell::read_lines` decision is contingent on a dynamic-length Array type or a script-side string-splitting native; either is a separate decision.

## B31. run-tasks deferred work

Ten items deferred during the V0.2.1 `keleusma run-tasks <manifest.toml>` implementation. Each was explicitly marked deferred in the design proposal (`docs/architecture/RUN_TASKS.md`) and none blocks V0.2.1 landing. This entry consolidates the items into a durable backlog record and gives each a tracking row that survives REVERSE_PROMPT rewrites.

| # | Item | Description | Forcing case |
|---|------|-------------|--------------|
| 1 | Manifest signing | The TOML manifest itself is unsigned. Tasks declared in the manifest reference per-task bytecode artefacts that can be independently signed and verified, but the manifest's task list, restart limits, and per-task policy is plain text. A manifest signing scheme would Ed25519-sign the manifest body and require the runner to verify before scheduling tasks. | Operators deploying to environments where the manifest itself is a tamper target. |
| 2 | Per-task isolation through OS primitives | Tasks share the runner process's address space, file descriptors, and OS-level permissions. Per-OS isolation primitives (Linux namespaces, FreeBSD jails, OpenBSD `pledge`/`unveil`, macOS sandbox profiles, Windows job objects) would scope a task's OS-visible side effects. Substantial per-OS work because each platform's primitive set has different semantics. | Operators running mixed-trust task sets in a single runner. |
| 3 | Dynamic task addition | Tasks are declared statically in the manifest and the manifest is read once at startup. A control socket or a `kernel::add_task` native would let a running task add new tasks at runtime. | Operators whose workload shape includes spawning sub-tasks in response to events. |
| 4 | Hot reload via SIGHUP | A SIGHUP handler is installed but performs no action. Hot reload would re-read the manifest, gracefully drain removed tasks, and start added tasks without restarting the runner process. Highest-leverage item in this list per the V0.2.1 close-out assessment. | Operators who want to update task configuration without process restart. |
| 5 | Priority levels and preemption | The cooperative scheduler treats all tasks as equal-priority and runs each dispatch to completion (or to a yield). Priority levels with preemption would let a high-priority task interrupt a low-priority task. Out of scope by design for the cooperative model; operators needing preemption write their own host. | Operators whose workload includes hard-real-time tasks alongside best-effort tasks. |
| 6 | Soft resource caps beyond WCMU | The arena bounds per-task memory and the cooperative model bounds per-dispatch CPU. Neither bounds wall-clock task lifetime, cumulative output volume, or other operational ceilings. A kill-runaway-task cap would terminate a task that exceeds a declared lifetime budget. | Operators running untrusted task sets who want to prevent a misbehaving task from monopolising the runner. |
| 7 | Typed event payloads | Events currently carry a single `Word` payload. A manifest-declared event schema would let tasks declare typed payloads (structs, tuples, enums) and have the kernel validate payload conformance at `post_event` time. | Operators with rich inter-task communication patterns. |
| 8 | Task-to-task ABI compatibility checking | Tasks declare event ids by number; mismatches between producer-side and consumer-side event id assignments are silently miscommunicated rather than rejected. Schema versioning would attach a version stamp to each event id and reject cross-version mismatches at task load. | Operators whose task set evolves over time and where event-id renumbering is a real maintenance risk. |
| 9 | Native Windows Service Control Manager integration | The current Windows deployment path uses NSSM or winsw as a wrapper that runs the runner as a service. Native SCM integration would implement the SCM protocol directly so the runner can be installed as a first-class Windows service without a wrapper. | Operators deploying to Windows production environments who want first-class SCM tooling. |
| 10 | Notification-protocol conventions on non-systemd supervisors | The runner emits `NOTIFY_SOCKET` messages for Linux systemd integration. Other supervisors (OpenRC, runit, s6, launchd, FreeBSD rc.d, OpenBSD rc.d, Windows service) do not define an equivalent ready/status/stopping/watchdog protocol; the runner currently emits nothing to them. A per-supervisor convention would specify what each emits and how the supervisor consumes it. | Operators on non-systemd hosts who want runtime status visibility through their supervisor's native channel. |

**Cross-references**

- `docs/architecture/RUN_TASKS.md` is the design doc that originally captured each item under "Open questions and future work".
- Item 4 (hot reload) and B30 item 2 (`Result<T, E>`) were called out as highest-leverage in the V0.2.1 close-out assessment.
- Item 7 (typed event payloads) and item 8 (ABI compatibility) compose: typed payloads make ABI checking meaningful.
- Item 5 (preemption) is intentionally excluded from the cooperative model. The forcing case is a deployment shape that needs preemption; the response is "use a different host" rather than "extend the runner".

## ~~B32. Arena bytes-builder feature in keleusma-arena~~ (Obsolete for V0.2.1; premature spec, prototype reverted, no consumer)

> **Status: obsolete (V0.2.1).** This entry was a premature spec. It assumed B28's flat-byte composite path writes into arena memory incrementally during construction and therefore needs a stateful, bounds-checked builder. The actual consumer does not: `GenericValue::try_pack_flat` assembles the complete body in a `Vec<u8>` (the `byte_size` is known up front, baked by P4), and `FlatComposite::in_arena` then migrates it with a single one-shot `alloc_top_bytes` + `copy_nonoverlapping` + `ArenaHandle::from_raw_parts`, the same pattern `KString::alloc` uses. The existing `alloc_top_bytes` (which returns a writable `NonNull<[u8]>`) plus `ArenaHandle::from_raw_parts`/`get` already provide everything the flat-byte work needs, including the epoch-stamped stale check that is the only safety mechanism beyond raw memory. B33 and P3 only widen flat-eligibility and flow through the same pack-then-copy choke point, so they do not need a builder either. A prototype builder was implemented and then reverted (it had no consumer). The only residual is that `in_arena` and `KString::alloc` share near-identical `unsafe`; if that duplication ever justifies a fix it is a private helper extracted at the third caller, not a public arena API. No action.

The `keleusma-arena` crate exposes raw byte allocation through `Arena::alloc_top_bytes(n) -> Result<NonNull<[u8]>, AllocError>` and typed read-only handles through `ArenaHandle<str>` (used by `KString` for arena-resident strings). It does not expose a safe builder for arena-resident byte buffers that consumers can write to during construction and read from afterwards. Consumers of B28's flat-byte composite-Value representation need exactly that: a bounds-checked region of flat memory that the arena manages, suitable for packing scalar field bytes into a composite body. This entry adds the missing API.

**Design.**

A builder pattern that allocates a zeroed byte buffer in the arena's top ephemeral region, exposes bounds-checked write methods during construction, and yields a `Copy` read-only handle on finish. The handle pattern matches `KString` (epoch-tagged stale detection through `ArenaHandle<[u8]>`); the builder pattern is new.

```rust
impl Arena {
    /// Allocate `n` zeroed bytes in the top ephemeral region and
    /// return a builder for bounds-checked writes. The builder
    /// retains exclusive write access until `finish()` is called.
    pub fn alloc_top_bytes_builder(&self, n: usize) -> Result<ArenaBytesBuilder<'_>, AllocError>;

    /// Symmetric bottom-region allocator. Same shape; the bytes
    /// live in the bottom ephemeral region.
    pub fn alloc_bottom_bytes_builder(&self, n: usize) -> Result<ArenaBytesBuilder<'_>, AllocError>;
}

pub struct ArenaBytesBuilder<'arena> { /* opaque */ }

impl<'arena> ArenaBytesBuilder<'arena> {
    /// Bounds-checked write of `bytes` at `offset`. Returns
    /// `BoundsError` if the write would exceed the allocated
    /// region.
    pub fn write_at(&mut self, offset: usize, bytes: &[u8]) -> Result<(), BoundsError>;

    /// Convenience: bounds-checked write of a single byte.
    pub fn write_byte_at(&mut self, offset: usize, value: u8) -> Result<(), BoundsError>;

    /// Capacity of the buffer (the `n` passed to the builder
    /// constructor).
    pub fn capacity(&self) -> usize;

    /// Convert the builder to a `Copy` read-only handle. The
    /// builder is consumed; subsequent writes are not possible
    /// through this API surface.
    pub fn finish(self) -> ArenaBytes;
}

#[derive(Copy, Clone)]
pub struct ArenaBytes(ArenaHandle<[u8]>);

impl ArenaBytes {
    /// Resolve the handle against the arena that produced it.
    /// Returns `Stale` if the arena has been reset since the
    /// handle was issued.
    pub fn get<'a>(&self, arena: &'a Arena) -> Result<&'a [u8], Stale>;

    /// Epoch captured when the handle was issued. Useful for
    /// debugging and for consumers that want to verify
    /// freshness without dereferencing.
    pub fn epoch(&self) -> u64;
}
```

The builder's internal storage is the raw pointer returned by `alloc_top_bytes`. The `write_at` method validates `offset + bytes.len() <= capacity` and then performs a safe slice copy through the validated bounds. The `finish` method constructs `ArenaHandle::from_raw_parts(ptr, arena.epoch())` (which is the existing unsafe constructor, used safely inside the builder because the builder owned the allocation).

**Usage limitations (documented).**

The byte buffer is POD only. Storing Drop-bearing types (`Arc`, `Box`, `String`, etc.) requires consumer-managed Drop semantics; this feature does not provide them. The arena reset reclaims the bytes without running any Drop. Consumers that need Drop for individual values must store such values outside the arena byte buffer (in side tables, in the operand stack's `Value` enum directly, in the host's storage, etc.). See B33 for the opaque-value pattern that uses indices into VM-resident Vecs to handle Arc-bearing references in a POD-shaped runtime representation.

**Effort estimate.**

One to two days of focused work. The implementation is mostly an API and tests around the existing `alloc_top_bytes` and `ArenaHandle::from_raw_parts` primitives. Tests cover the round-trip (write through builder, read through handle), bounds checking, stale-handle detection across arena reset, symmetric bottom-region builder, and the documented POD-only usage limitation.

**Forcing case.**

B28 P2 onwards needs this feature to migrate composite Value internal storage from `Vec<GenericValue>` to flat bytes in the arena. The feature also benefits the broader arena-consumer audience that wants to build fixed-size byte buffers safely.

**Cross-references.**

- B28 (runtime composite Value representation aligned with the language guarantee) is the immediate consumer. The Flat path in `GenericTuple`, `GenericArray`, `GenericStruct`, `GenericEnum` uses this feature for the byte buffer.
- B33 (opaque values as indices) complements this feature for the Arc-bearing case.
- The existing `KString` API in `keleusma::kstring` is the prior art for the handle pattern; the builder pattern is new.

## ~~B33. Opaque values stored as indices into per-VM Vecs~~ (Resolved for V0.2.1; persistent registry deliberately omitted, no consumer)

`Value::Opaque(Arc<dyn HostOpaque>)` carries a fat `Arc` pointer (16 bytes on 64-bit). The `Arc` is Drop-bearing; dropping it decrements the host object's refcount. This prevents Opaque fields from being stored in arena byte buffers (B32) because the arena reset reclaims the bytes without running any Drop, leaking the refcount. For B28's "everything in arena" property to hold across all composite Value variants, Opaque needs a POD-shaped runtime representation that the arena can hold safely.

**Design.**

The VM gains two opaque registries:

```rust
pub struct GenericVm<...> {
    // ...existing fields
    ephemeral_opaques: Vec<Arc<dyn HostOpaque>>,
    persistent_opaques: Vec<Arc<dyn HostOpaque>>,
}
```

The runtime represents an opaque reference as a small index plus a tag identifying which registry it indexes:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OpaqueRef {
    /// Index into the VM's `ephemeral_opaques` Vec. Cleared on
    /// arena RESET. Used by opaque values living on the operand
    /// stack, in arena ephemeral bytes (tuple, array, struct,
    /// enum bodies), and in any other transient storage.
    Ephemeral(u32),
    /// Index into the VM's `persistent_opaques` Vec. Cleared on
    /// VM drop only. Used by opaque values held in `private data`
    /// fields and any other persistent storage.
    Persistent(u32),
}
```

`OpaqueRef` is `Copy`, POD-shaped (5 bytes plus padding to 8 bytes on 64-bit alignment), and safe to store in arena byte buffers (B32).

**Storage by context.**

| Context | Storage | Lifetime |
|---------|---------|----------|
| Operand stack | `Value::Opaque(OpaqueRef)` (internal runtime form) | Until RESET (ephemeral) or VM drop (persistent) |
| Arena ephemeral bytes (composite bodies) | `OpaqueRef::Ephemeral(u32)` inline | Until RESET |
| Arena persistent bytes (private data) | `OpaqueRef::Persistent(u32)` inline | Until VM drop |
| Yielded value crossing into host | `Value::Opaque(Arc<dyn HostOpaque>)` (materialised) | Host-managed |
| Native function argument | `Arc<dyn HostOpaque>` (materialised from index at call site) | Native-call duration |

**Lifecycle.**

- Native function returns an `Arc<dyn HostOpaque>`: the VM pushes it to `ephemeral_opaques`, returns `OpaqueRef::Ephemeral(index)` to the script.
- Native function takes an opaque argument: the VM looks up the indexed `Arc` and clones it for the native call. The clone bumps the refcount during the call; the call's drop on the cloned Arc decrements.
- Arena RESET: clears `ephemeral_opaques`. All ephemeral Arcs drop; refcounts decrement. Anything still referenced from `persistent_opaques` or by the host survives. The persistent Vec is untouched.
- Private data field write of an opaque: the VM moves the Arc from `ephemeral_opaques` to `persistent_opaques` (or duplicates if shared) and rewrites the `OpaqueRef` tag accordingly. The ephemeral index slot becomes vacant; it may be reclaimed by subsequent allocations or left vacant until RESET.
- VM drop: drops both Vecs, dropping all remaining Arcs.
- Yield boundary: the runtime walks the yielded value tree (analogous to `materialise_kstrings`) and replaces every `OpaqueRef` with the `Arc::clone(...)` from the indexed Vec. The yielded `Value` carries `Arc<dyn HostOpaque>` as today. Host code is unaffected by the internal indexed representation.
- Hot code swap: ephemeral opaques are gone (the swap restarts the iteration). Persistent opaques survive because their Vec is part of persistent state. The new module's references continue to work.

**Public API.**

The host marshalling boundary continues to use `Value::Opaque(Arc<dyn HostOpaque>)` as the public surface. Host code that pattern-matches on `Value::Opaque` is unaffected. The `OpaqueRef`-indexed form is an internal runtime representation that surfaces only on the operand stack and inside arena byte buffers. The marshall layer handles the index-to-Arc translation when crossing the host boundary; the `KeleusmaType` derive macro is updated to do the translation at native-call argument and return positions.

**Effort estimate.**

Three to five days of focused work. Touches:

- `Value` enum definition (the `Opaque` variant's payload changes internally; the public marshalling surface is preserved).
- VM construction (`Vm::new`) to initialise the two Vecs.
- VM op handlers that touch Opaque values (native call paths, equality, type predicates).
- Marshall layer to translate at host boundary.
- Yield-boundary materialisation, analogous to `materialise_kstrings`.
- Tests covering ephemeral/persistent isolation, RESET behaviour, yield materialisation, private-data write that moves opaque from ephemeral to persistent, VM drop drops all Arcs.

**Forcing case.**

B28 P2 onwards. Without this feature, B28 must use a hybrid Flat-plus-Boxed representation for composite Values, with Opaque-containing composites falling back to the heap-resident `Vec<Value>` form. With this feature, the Flat path applies uniformly, and the "everything in arena" property holds.

**Cross-references.**

- B28 (runtime composite Value representation aligned with the language guarantee) is the immediate consumer.
- B32 (arena bytes-builder) is the complement: B32 provides the byte buffer; B33 provides the POD opaque representation that fits in it.

### Resolved (2026-06-26)

The design above predated the B28 P3 "opaque registry tightening", which already built the ephemeral half: a single arena-resident `ephemeral_opaques` registry (`StackVec` in the arena, cleared at RESET), with `intern_ephemeral_opaque`/`resolve_ephemeral_opaque`, so an opaque field inside a flat composite is already a one-word byte index rather than the `Drop`-bearing `Arc`. The remaining residual, the operand stack itself still carrying `Value::Opaque(Arc)` (an arena-resident stack holding a global-heap pointer, which the snapshot and no-global-heap goals forbid), was closed here.

A new internal `GenericValue::OpaqueRef(u32)` variant is the POD index form. The operand stack and boxed-composite elements carry `OpaqueRef`; the host `Value::Opaque(Arc)` form survives only at boundaries, so host code that matches `Value::Opaque` is unaffected. Four VM boundary walks convert: a native-call argument materialises `OpaqueRef` to `Arc` (the native receives the `Arc`), a native result interns `Arc` to `OpaqueRef`, the yield/finish boundary materialises so host code can pattern-match `Value::Opaque` without `decode`, and a host `call`/`resume` argument interns. `read_flat_scalar` now pushes the index form instead of resolving, `NewComposite` packs the index directly, equality is index equality (which coincides with `Arc` pointer identity because interning deduplicates by pointer), and `type_name` is unchanged because it already returned the generic `"Opaque"`. The marshalling `Arc::from_value_ctx` resolves a bare `OpaqueRef` through the context as a defensive path for `decode`.

The two-registry design and the `OpaqueRef::Persistent` arm are deliberately **not** built: a data-segment field type rejects `String` and opaque named types at compile time (`validate_data_field_type`), so no opaque can reach `private data` and persist across a RESET. The single ephemeral registry suffices; if a future feature admits opaque in persistent data, the persistent registry and a tag are the natural extension.

Tests: `tests/opaque.rs` gains `opaque_materialises_across_the_yield_boundary` and `host_supplied_opaque_argument_round_trips`; the existing flat-composite, decode, and interning tests cover the read/pack and native paths. Validated under default features: the library suite (1116), the opaque suite with the two new boundary tests (8), and the flat-reference, decode, and interning integration suites (12), plus `cargo fmt`. Clippy, narrow-word compilation, signatures, the doc gate, and Miri over the opaque suite are run at `-j 1` in the session's memory-constrained build environment.

## ~~B34. keleusma-macros extension for shared-data flat-byte layout~~ (Resolved 2026-06-26; re-scoped against B28, both tiers implemented)

`#[derive(KeleusmaType)]` in `keleusma-macros` generates `impl KeleusmaType for T` blocks that marshall between host Rust types and the heap-allocated `Value` enum. Under B28's flat-byte composite-Value representation, the VM accesses host structs backing `shared data` declarations through byte offsets rather than through `Value` round-trips. The derive macro needs to additionally generate byte-layout information so the VM can read and write fields directly at the right offsets.

**Design.**

Extend `#[derive(KeleusmaType)]` to also emit, for struct and enum types:

1. **A compile-time byte-layout descriptor.** A `const SHARED_DATA_LAYOUT: LayoutDescriptor` (or equivalent) declaring field byte offsets, scalar kinds, byte sizes, and total struct bytes. The descriptor matches the runtime's [`crate::value_layout::LayoutDescriptor`] shape so consistency checks at VM construction time are straightforward.

2. **Direct byte-level accessor functions.** Generated methods that read or write a field at a known byte offset and scalar kind. For example, for a struct `Sensor { temperature: Float, pressure: Float, count: Word }`, the macro emits:

```rust
impl Sensor {
    pub fn get_field_at_offset(&self, offset: usize, kind: ScalarKind) -> Value { ... }
    pub fn set_field_at_offset(&mut self, offset: usize, kind: ScalarKind, value: Value) { ... }
}
```

The VM uses these accessors at every `Op::GetData(slot)` or `Op::SetData(slot)` dispatch when the slot indexes a shared-data field, avoiding the round-trip through `Value`'s heap-allocated form.

3. **A layout-consistency check.** A `validate_against_keleusma_layout(layout: &LayoutDescriptor) -> Result<(), LayoutMismatch>` method that compares the host struct's byte layout against the Keleusma script's declared shared-data layout. Called at `Vm::new` to surface mismatches as a `VerifyError` rather than at runtime.

**Constraints on the host struct.**

The macro requires the host struct to use `#[repr(C)]` or an explicit field-ordering attribute so the byte layout is stable and matches Rust's well-defined struct layout rules. Hosts that use the default Rust layout (which is unspecified) get a compile-time error from the macro pointing them at the constraint.

The macro emits a `static_assertions`-style compile-time check that the struct's size and field offsets match what the macro computed, guarding against silent drift if the struct is edited.

**Effort estimate.**

Four to six days of focused work. Touches:

- `keleusma-macros` derive implementation (most of the work; the macro grows by roughly a factor of two).
- A new `LayoutDescriptor` type in the runtime that matches the macro's emitted form, if not already a fit with `crate::value_layout::LayoutDescriptor`.
- A new `validate_against_keleusma_layout` check site in `Vm::new`.
- Tests for the generated code on representative structs (scalar-only, mixed-scalar, with Text fields, with Opaque fields under B33's representation).
- Documentation update for the `KeleusmaType` derive's expanded contract.

**Forcing case.**

B28 P5 or P6, when shared `data` access integrates with the flat-byte runtime. Until then, shared data continues to use the V0.2.x `Vec<Value>` round-trip through `Vm::set_data` and `Vm::get_data`. The macro extension is not on the critical path for P2 through P4 (which migrate tuple, array, struct, and enum to flat-byte storage for in-arena composites, not for shared data).

**Cross-references.**

- B28 (runtime composite Value representation aligned with the language guarantee) is the immediate consumer.
- The existing `keleusma-macros` crate is the implementation site.
- The `#[derive(KeleusmaType)]` macro's current shape lives in `keleusma-macros/src/lib.rs`.

### Re-scope (2026-06-26)

A premise-check against the current `v0.2.1` state shows B28 already built most of what the original design proposed, and the forcing case above is obsolete.

**Already done by B28.** Shared data is no longer a `Vec<Value>` round-trip; `set_data`/`get_data` are gone. It is a host-owned borrowed `&mut [u8]` buffer driven through `call_with_shared`/`resume_with_shared`, with `shared_data_bytes()`/`shared_data_bytes_for(module)` for sizing and per-slot scalar accessors `get_shared`/`set_shared`. The VM already holds the flat shared layout per slot (`shared_layout_entry` returns offset, kind, and size), so B34's "compile-time byte-layout descriptor" and "byte-level accessors" largely exist. The `#[derive(KeleusmaType)]` macro already generates the flat **read** side (`flat_byte_size`, `from_flat_bytes`, `from_flat_bytes_ctx`), which is effectively the byte-layout descriptor for a host type.

**The real remaining gap.** A host cannot marshal a whole struct that mirrors the `shared data` segment to or from its buffer. `set_shared` is scalar-only and rejects composite slots (a composite shared field is written from the script, not the per-slot host API), so a host cannot seed or read a composite shared field at all. Two specific holes:

1. `KeleusmaType` has no flat **write** method, only `from_flat_bytes` (read). There is no way to write a host type's flat bytes into a buffer.
2. `Vm::marshal_shared_into<T>` / `unmarshal_shared<T>` do not exist.
3. The host marshalling flat layout is **incomplete** relative to the script-side shared layout. `Option<T>` is admissible in a data segment and the script lays it out flat (discriminant plus payload), but the marshalling `Option` impl is not flat-eligible (`flat_field_kind` is `None`, and `into_value` treats `Some(t)` as the bare inner value with no discriminant). So the host and script flat layouts disagree for `Option`, and likely for any other admissible-but-not-yet-flat type.

**Re-scoped plan, two tiers.**

- **Tier 1 (about a day).** Add `KeleusmaType::to_flat_bytes` (a trait default for fixed scalars via `write_scalar_le`, erroring on the reference kinds `Text`/`Opaque` which need the arena and cannot reach a data segment anyway; overrides on `[T; N]` and the `impl_tuple` macro; generation in the derive macro for structs and enums, mirroring `from_flat_bytes`). Add `Vm::marshal_shared_into<T: KeleusmaType>(&self, value, buf)` and `Vm::unmarshal_shared<T>(&self, buf)`, each validating `T::flat_byte_size(module_word, module_float) == shared_data_bytes()` (the original "validate_against_keleusma_layout", surfaced as a clear error). Covers scalars, tuples, arrays, and nested derived structs and enums, the common shared-segment shapes.
- **Tier 2.** Make `Option`, and any other admissible-but-not-flat type, flat-eligible in marshalling (discriminant plus payload) so the host marshalling layout fully matches the script-side shared layout.

**Touchpoints.** `src/marshall.rs` (the trait method and the `[T; N]`/tuple/`Option` impls), `keleusma-macros/src/lib.rs` (generate `to_flat_bytes`), `src/vm.rs` (`marshal_shared_into`/`unmarshal_shared` plus the layout-match validation). Tests: round-trip a host struct mirroring a `shared data` segment through `marshal_shared_into` then `unmarshal_shared`, at i64/f64 and at narrow module widths, including a composite field; and a negative test for a byte-size mismatch.

The original "extend the macro by roughly a factor of two" estimate no longer holds; the read side and the layout already exist. The realizable work is the write side plus the whole-struct shared helpers plus the `Option` flat-eligibility alignment.

### Resolved (2026-06-26), both tiers

**Tier 1.** Added `KeleusmaType::to_flat_bytes`, the write mirror of `from_flat_bytes`: a trait default for fixed scalars through `write_scalar_le` (erroring on the reference kinds `Text`/`Opaque`, which need the arena and cannot reach a data segment), overrides on `[T; N]` and the `impl_tuple` macro, and generation in the derive macro for structs and enums (the enum write arms dispatch on the runtime variant, write the discriminant word, write the payload at packed offsets, and zero-fill the trailing pad to `word + payload_max`). Added `Vm::marshal_shared_into<T: KeleusmaType>` and `Vm::unmarshal_shared<T>`, each validating `T::flat_byte_size(module_word, module_float) == shared_data_bytes()` and the buffer length through a shared `check_shared_marshal_layout`, so a layout mismatch is a clear error rather than a silent mis-read or mis-write. A host can now seed or read the whole `data` segment in one call, including composite fields the scalar-only `set_shared` rejects.

**Tier 2.** Made `Option<T>` flat-eligible in marshalling so the host layout matches the script side, which lays `Option` flat as a discriminant word plus the `Some` payload (`word + payload_max`, `None` = 0, `Some` = 1; value_layout.rs). `Option` gained `flat_byte_size`, `from_flat_bytes`, `to_flat_bytes`, and `from_flat_bytes_ctx`. This also closes a latent gap: decoding a flat composite with an `Option` field at the host boundary previously errored, because `Option` had no flat methods.

Tests: `tests/marshall.rs` gains three tests over a `data state` segment whose host mirror exercises a scalar, a tuple, a fixed array, a nested flat struct, a tuple-style flat enum, and an `Option` field: a `marshal_shared_into` then `unmarshal_shared` round trip whose success proves the host flat layout equals `shared_data_bytes`, the script reading the marshalled `counter` at its offset, a second variant-and-`None` round trip, and a layout-mismatch rejection. Validated at `-j 1`: default library suite (1116), the marshall suite (34) including the three new tests, the marshall suite under `narrow-word-8` at the narrow module width (34), `signatures` compile, `cargo clippy -p keleusma --all-targets`, `cargo fmt`, `cargo doc` under the CI flags, and Miri over the three new tests under tree borrows (no UB).

The script grammar admits only unit and tuple-style enum variants, not struct-style ones, so a host enum mirroring a segment enum uses tuple variants; the derive macro itself handles all three forms.

## ~~B35. Partial Operation Handling~~ (Resolved for V0.2.x; native code generation lowering deferred to V0.4.0)

**Resolution status.** The virtual-machine side is complete. Phases P1 through P7 deliver the specific trap variants, the canonical zero value and lowest-valid resolution, and all six source-level handling constructs (checked arithmetic over `Word`, `Byte`, `Float`, and `Fixed<N>`, array indexing, newtype construction, discriminant-to-enum conversion, and the native call). Phase P8 specifies the native code-generation contract in [`docs/spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md); the guard-insertion lowering it describes is deferred to V0.4.0, the native-code-generation milestone, because no native backend exists yet. Phase P9 closes the documentation. The narrative lives in [`docs/architecture/LANGUAGE_DESIGN.md`](../architecture/LANGUAGE_DESIGN.md), the per-construct grammar in [`docs/spec/GRAMMAR.md`](../spec/GRAMMAR.md), and the fault contract in [`docs/spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md). The two carried-forward items are the native lowering (V0.4.0) and trap-localization detail (a consumer of the B29 debug-information records). The original design and per-phase implementation notes follow.

Every operation in the language that is mathematically partial, namely undefined on some inputs, currently traps at runtime when given an undefined input. The partial operations are integer division and modulo on a zero divisor, explicit array indexing out of bounds, refinement-newtype construction whose predicate fails, the planned discriminant-to-enum conversion on an invalid or payload-bearing discriminant, and fallible native function calls. The verifier proves termination and the worst-case execution time and worst-case memory usage bounds, but it does not prove totality, so these traps are the partiality the language admits. B35 gives every partial operation a defined contract and an opt-in handling mechanism, so that a program can be made total at the source level and so that native code generation has a defined, non-crashing contract on every target.

This is a redesign of the existing checked-arithmetic construct, not only an addition. The current construct requires a covering arm for each of the `ok`, `overflow`, and `underflow` classes and traps on a zero divisor even when handled. Under B35 the arms become optional with defined defaults, a zero-divisor outcome is added, and the construct family extends to indexing, newtype construction, discriminant-to-enum conversion, and native calls.

**The two-backend contract.**

- The virtual machine traps on any unhandled partial operation. A trap is a recoverable error returned to the host, not a process abort, consistent with the existing treatment of arena exhaustion.
- Native code produces a defined, non-crashing value for any unhandled partial operation. It uses the hardware result where the hardware does not fault, and an inserted guard that yields a defined value where the hardware would fault. The unhandled native value is therefore platform-specific, so handling the arm is what makes a program portable in value.

The virtual machine is the safe reference interpreter. Native code is the as-fast-as-hardware target with its own contract. The two intentionally diverge on unhandled partial operations, and that divergence is part of the contract rather than an accident, so verification on the virtual machine does not by itself establish the values a native build produces.

**Hardware basis.**

- x86 and x86-64 raise a divide-error fault on integer division or modulo by zero. Native code inserts a guard to avoid the fault.
- ARM returns zero for integer division by zero and does not fault.
- RISC-V returns all-ones for the quotient and the dividend for the remainder, and does not fault.
- The 6502 has no divide instruction at all. Division is a software routine, so the routine defines the zero-divisor result, and there is no hardware fault. The 6502 also has no arithmetic fault mechanism, no memory protection, and on the NES no operating system, so a trap there can only be a compiler-emitted software check, and the nominally-safe defined value is the appropriate contract.
- Institute of Electrical and Electronics Engineers 754 floating point already defines non-trapping results, namely signed infinity, not-a-number, and a defined zero-divisor result. So for floating point the arms intercept those special results rather than avert a trap.

**Native default values.**

| Operation | Native default where the hardware does not fault | Native default where the hardware faults |
|-----------|---------------------------------------------------|-------------------------------------------|
| Division by zero | hardware result | zero |
| Modulo by zero | hardware result | the numerator |
| Out-of-bounds index | not applicable | the element type's zero-or-lowest-valid value |
| Newtype predicate failure | not applicable | see the lowest-valid precedence |
| Discriminant-to-enum, invalid | not applicable | the zero-discriminant variant, or the lowest valid variant |
| Native error | not applicable | trap, since there is no safe default |

Modulo defaults to the numerator rather than zero, which matches the RISC-V remainder convention and the value ARM derives, so it is closer to portable than zero would be.

**Lowest-valid precedence.** Several native defaults need the lowest valid value of a refined type. It is resolved in this order. First, the value declared by the newtype's `with saturate_min` clause, which already exists in the grammar and is predicate-checked. Second, the lowest valid value computed by the interval and lattice analysis, when the valid set is analyzable. Third, where neither exists, the virtual machine traps and native code uses a hard zero even if it violates the predicate, because a bare-metal target has no recovery context and no better option.

**The canonical zero value.** A single zero value is defined once for every type and used by the out-of-bounds, newtype, and conversion native defaults. It is zero for `Word`, `0` for `Byte`, `0.0` for `Float`, `false` for `Bool`, the empty string for `Text`, each field's zero value for a tuple or struct, and the zero-discriminant variant, or lowest-discriminant variant when zero is not present, for an enum. For a refined newtype it is the lowest-valid value above.

**The construct family.** Each construct is a standard match block over a fallible operation, distinguished only by a fixed vocabulary of specialized outcome-arm keywords. The arms are optional unless noted, and omitting an admissible arm invokes the default behavior above. An arm keyword is admissible only when its condition can arise, and writing an inadmissible arm is a compile error. The type checker enforces both the vocabulary and the per-construct exhaustiveness rule.

| Construct | Admissible outcome arms | Mandatory coverage |
|-----------|------------------------|--------------------|
| Division, modulo | `ok`, `overflow` where it can arise, `zero_divisor` | success only |
| Checked add, subtract, multiply, negate | `ok`, `overflow`, `underflow` where each can arise | success only |
| Array indexing | `ok`, `invalid_index` | success only |
| Newtype construction | `ok`, `invalid_newtype` | success only |
| Discriminant-to-enum | `ok`, `payload_discriminant`, `invalid_discriminant` | success and every payload-bearing variant |
| Fallible native call | `ok`, `error` | success only |

The zero-divisor arm is named `zero_divisor` for both division and modulo. The arithmetic arms bind the relevant datum, for example `zero_divisor(numerator)` and `invalid_index(index)`, and the wildcard form is permitted in every position.

**Per-operation admissibility.** The admissibility of `overflow` and `underflow` depends on the operator and the operand type. The table below is for the signed `Word` type.

| Operation | overflow | underflow | zero_divisor |
|-----------|----------|-----------|--------------|
| `+`, `-`, `*` | yes | yes | no |
| unary `-` | yes, negating the minimum | no | no |
| `/` | yes, minimum over negative one | no | yes |
| `%` | no | no | yes |

For the unsigned `Byte` type the table differs. Addition and multiplication can overflow but not underflow, subtraction can underflow below zero but not overflow, division never overflows, and modulo neither overflows nor underflows. For `Float` the Institute of Electrical and Electronics Engineers 754 special results replace overflow and underflow with the infinity, not-a-number, and zero-divisor outcomes. The implementation computes admissibility from the operand type, not the operator alone.

**Default for missing arithmetic arms.** Overflow and underflow default to two's-complement wrapping. So an `ok(v) => v` arm with no overflow or underflow arm is functionally identical to the bare wrapping operation, which is acceptable. The success arm is required.

**Discriminant-to-enum conversion.** The conversion `discriminant as EnumType { ... }` turns a `Word` into an enum value. Because only the `Word` is available, payload data cannot be reconstructed from it, so the three arm kinds split the variants by what the discriminant can determine.

- `ok(UnitValue)` names a unit, that is discriminant-only, variant. The conversion produces that variant automatically, and the arm is an optional override. A unit variant with no `ok` arm converts to itself. A generic `ok(v)` arm is permitted and binds the converted unit-variant value as a blanket post-processor, ordered after any specific `ok` arms.
- `payload_discriminant(IntValue)` names a payload-bearing variant. The discriminant cannot carry the payload, so the author supplies it in the arm body, and coverage of every payload-bearing variant is mandatory, including through a `payload_discriminant(_) => SomeUnitVariant` catch-all.
- `invalid_discriminant(_)` catches a `Word` that matches no variant. The virtual machine default is a trap. The native default is the zero-discriminant variant, or the lowest valid variant when zero is not a discriminant.

Arms match by variant name, not by raw discriminant integer, so the construct is robust to discriminant renumbering. The type checker rejects a payload variant in `ok` and a unit variant in `payload_discriminant`. Every arm body yields the target enum type. An example follows.

```
let enum_value = discriminant as EnumType {
  ok(UnitValue) => OtherUnitValue,
  ok(ThirdUnitValue) => StringValue("empty"),
  payload_discriminant(IntValue) => IntValue(0),
  payload_discriminant(OtherIntValue) => OtherIntValue(generate_inner()),
  invalid_discriminant(_) => safe_default(),
};
```

The forward enum-to-`Word` cast already exists. The reverse `Word`-to-enum cast is new machinery.

**Native function errors.** A fallible native yields a `Word` error code on failure. The `error(code)` arm binds it. The error arm is admissible only for fallible natives, and an `error` arm on an infallible native is a compile error. Native errors have no safe default, so an unhandled native error traps on both backends, which is consistent with the rule that an operation gets a defined non-trapping default only when a total result exists. The `Word` error code may be converted to a structured error enum with the discriminant-to-enum construct, where a failed conversion falls through to the next arm.

```
let result = native_function(parameters) {
  ok(v) => v,
  error(code) => recover(code as ErrorEnum {
    invalid_discriminant(raw) => default_error(raw),
  }),
};
```

A native that fails as part of normal control flow should instead return an option or result enum as an ordinary value, handled by a standard match. That keeps expected failures in the type system. The `error` arm is reserved for exceptional host failures.

**Specific trap errors.** The generic `VmError::Trap(String)` is replaced by specific error variants, for example a refinement-failure error, a no-matching-head error, a no-matching-arm error, a zero-divisor error, an out-of-bounds error, and an invalid-discriminant error. This lets the host's error-category mechanism map outcomes to policy without parsing a message string.

**Exhaustiveness.** Ordinary matches and multiheaded functions remain exhaustive. The specialized outcome arms are opt-in, and omitting an admissible arm invokes the default behavior, except for the mandatory coverage noted in the construct table.

**Construct syntax.** The failure-handling block is uniform across the construct family. The keyword choice, whether a leading `match`, a postfix brace, or another keyword, is open, but it must be the same across arithmetic, indexing, newtype construction, conversion, and native calls.

**Open items requiring verification before implementation.**

- Whether a zero-sized array `[T; 0]` can be constructed at all. If the parser and monomorphizer never produce one, the out-of-bounds default needs only a defensive note. If they can, the element type's zero value is the answer.
- The `Word`-to-enum cast does not exist yet and is net-new.
- The checked-arithmetic construct is being redesigned from mandatory arms to optional arms with a new zero-divisor outcome, so the change is not purely additive.

**Phased plan.**

| Phase | Scope |
|-------|-------|
| P1 | Specific trap error variants replacing `VmError::Trap`. Independent and low risk. |
| P2 | Canonical zero value for every type, and the lowest-valid precedence that consults `with saturate_min`. |
| P3 | Redesign the arithmetic construct, namely optional arms, the `zero_divisor` outcome, per-operand-type admissibility, and wrapping defaults. |
| P4 | Indexing construct and the out-of-bounds native default. |
| P5 | Newtype construction construct and its native default. |
| P6 | `Word`-to-enum cast and the discriminant-to-enum construct with its three arm kinds. |
| P7 | Native error `error(code)` arm and the option-or-result idiom. |
| P8 | Native code generation contract per target, namely the inserted guards and the platform-specific defaults. Gated on the native code generation work tracked elsewhere. |
| P9 | Documentation, namely the grammar, the language design narrative, and a runtime-faults reference, and B35 closure. |

**Implementation status.**

P1 is implemented on the `feat-partial-op-handling` branch. The compiler-emitted `Op::Trap` now carries a `crate::bytecode::TrapKind` code in its operand rather than a string-constant index, and the virtual machine surfaces each cause as a distinct top-level `VmError` variant, namely `RefinementFailed`, `NoMatchingHead`, `NoMatchingArm`, `CheckedArithNoArm`, and `EnumVariantUnmapped`, so a host categorizes the fault without parsing a message. `TrapKind` is the bytecode-level operand encoding, and the virtual machine maps it to the matching `VmError` variant. The change touches only the compiler and the virtual machine. The wire format is unaffected because the operand is an opaque `u16` that the encoder round-trips and the verifier ignores, so there is no opcode change, no instruction-count change, and no `BYTECODE_VERSION` change.

Distinct top-level variants were chosen over a single `Trap(TrapKind)` variant for consistency, since `VmError` already represents `DivisionByZero`, `IndexOutOfBounds`, and `FieldNotFound` as top-level fault variants, and the coarse grouping a host might want is already provided by `category()`, where all of these map to `SoftScript`. The vestigial `VmError::NoMatch(String)` variant, which was never raised, was removed in the same change so it does not sit confusingly beside `NoMatchingArm`.

One behavioral change is recorded as open. The prior runtime trap message embedded dynamic detail such as the failing predicate and newtype names, and that detail is dropped from the runtime error in favor of the structured kind, consistent with the language's move away from runtime strings. Localization, if wanted, should return through a compact source location on the trap rather than through a message string, which is B29 debug-information territory.

P2 is implemented on the same branch as the `crate::zero_value` module, gated behind the `compile` feature because it operates on abstract-syntax-tree types. It provides `zero_value`, which returns the canonical zero value of any type as a `ConstValue`, and `lowest_valid`, which resolves a refined newtype's lowest valid value by the precedence above, namely a declared `with saturate_min`, then the minimum of the predicate's true set computed by reusing the compiler's `predicate_true_set` over the interval and lattice analysis, then none. The module is pure and operates on a borrowed `TypeRegistry` of declarations, so it is unit-tested in isolation. It has no runtime consumer yet, since the virtual machine traps rather than substituting a zero value, and native code generation is the intended consumer, so this is parallel infrastructure in the same sense as the B28 P0 and P1 scaffolding.

P3 is large enough to be split into sub-phases, since it modifies a working construct with many pinning tests and, for the zero-divisor outcome, the virtual-machine `(low, high, flag)` protocol. The sub-phases are P3a optional `overflow` and `underflow` arms with wrapping defaults, P3b the `zero_divisor` outcome for division and modulo, which reifies the divisor check the virtual machine currently traps on and thus changes the checked-op protocol, P3c per-operand-type admissibility so an inadmissible arm such as `underflow` on division is a compile error, and P3d the extension of the construct to `Byte` and `Float` operands with their own admissibility rules.

P3a is implemented. The type checker now requires only an `ok` catch-all arm; the `overflow` and `underflow` classes are optional. When a class has no covering arm, the compiler emits a wrapping default: it pushes the `low` slot, which holds the in-range result for `ok` and the two's-complement wrapped result for `overflow` and `underflow`, so an unhandled outcome wraps rather than trapping. The defensive `CheckedArithNoArm` trap the compiler previously emitted is replaced by this default and is no longer reachable from the checked construct.

P3b is implemented. A `CheckedArmKind::ZeroDivisor(numerator)` arm is added across the abstract syntax tree, parser, type checker, and compiler. The virtual machine's `CheckedDiv` and `CheckedMod` no longer trap on a zero divisor; they reify it as flag 3 with the numerator in the low slot. The compiled dispatch routes flag 3 to a `zero_divisor` arm when present, binding the numerator, and otherwise traps. The unhandled case uses a new `TrapKind::ZeroDivisor` that the virtual machine maps to `VmError::DivisionByZero`, so an unhandled zero divisor in a checked construct produces the same error as a plain division by zero. The change reuses the existing single-pattern binding, so the numerator binds exactly as an `ok` value does.

P3c is implemented. The type checker now rejects an arm whose outcome cannot arise for the operator: `+`, `-`, and `*` admit `overflow` and `underflow`, unary `-` admits `overflow` only, `/` admits `overflow` and `zero_divisor`, and `%` admits `zero_divisor` only; `ok` is admissible for every operator. To make the runtime consistent with this table, `CheckedMod` no longer reports `i64::MIN % -1` as overflow; a remainder is always in range, so modulo produces only the `ok` and `zero_divisor` outcomes and the corner surfaces as an in-range `0`. The admissibility table is for the signed `Word` type; the per-operand-type generalization arrives with the Byte and Float extension.

P3d is the extension to the remaining numeric operand types, split into P3d-i Byte, P3d-ii Float, and P3d-iii Fixed. The design decisions are settled: a Byte `overflow`/`underflow` arm binds the single wrapped Byte result, written `overflow(w)`; a Float construct uses `ok`, `overflow` for positive infinity, `underflow` for negative infinity, and `nan`; a Fixed construct mirrors the signed Word admissibility but binds a single result; Byte lands before Float, and Float before Fixed.

The P3d preparation refactor is implemented. The `CheckedArmKind::Overflow` and `Underflow` second pattern is now `Option<Pattern>`, so the two-pattern `Word` form is `Some` and the single-pattern Byte form is `None`; the parser accepts one or two patterns; the compiler binds the single-pattern form against the low slot and the two-pattern form against the high and low slots. This is behavior-neutral, since the type checker still requires the two-pattern form (only `Word` operands are admitted so far) and rejects the single-pattern form.

P3d-i is implemented. The type checker admits `Byte` operands with the unsigned admissibility table (`+` and `*` overflow, `-` underflow, `/` and `%` zero divisor, no unary negation), enforces the single-pattern arity for the Byte `overflow`/`underflow` arms, and binds the patterns at `Byte` type. The compiler binds at the operand type, determined from the operand. The virtual machine computes Byte-checked arithmetic in the `CheckedAdd`, `CheckedSub`, `CheckedMul`, `CheckedDiv`, and `CheckedMod` handlers, placing the wrapped Byte in the low slot, with overflow above 255, underflow below 0, no overflow on division or modulo, and a zero divisor reified as flag 3 carrying the numerator. A lexer fix was needed along the way: `0Byte` had been mislexed as a `0B` binary prefix, and now lexes as the byte literal zero.

P3d-ii is implemented. A Float construct admits `+`, `-`, `*`, `/` (not `%` or unary `-`, which are not Float operations) and uses `ok` for a finite result, `overflow` for positive infinity, `underflow` for negative infinity, and `nan` for a not-a-number result, each binding a single result through the existing single-pattern path. A new `CheckedArmKind::Nan` arm and `nan(result)` surface syntax were added. The virtual machine's `CheckedAdd`, `CheckedSub`, `CheckedMul`, and `CheckedDiv` gained floats-gated `(Float, Float)` arms that classify the Institute of Electrical and Electronics Engineers 754 result into the flag (0 finite, 1 positive infinity, 2 negative infinity, 4 NaN); there is no zero-divisor flag for floats, since a float division by zero produces an infinity or NaN rather than trapping. The type checker rejects `nan` on integer operands and `zero_divisor` on float operands.

P3d-iii is implemented, completing P3. A Fixed construct admits `+`, `-`, `*`, `/`, `%`, and unary `-`. Fixed is signed, so its outcome admissibility mirrors Word (`+`, `-`, `*` overflow and underflow, unary `-` overflow, `/` overflow and zero divisor, `%` zero divisor), but its `overflow`/`underflow` arms bind a single result like Byte and Float rather than the high and low halves, and `nan` is inadmissible. The checked Fixed multiply and divide wrap an out-of-range result, matching the wrapping default of the other checked families, whereas the plain `FixedMul`/`FixedDiv` saturate. A zero divisor on `/` or `%` reifies as flag 3 carrying the numerator, and an unhandled zero divisor traps as a division by zero.

The encoding is a unified one that adds no opcodes and keeps the count at 69. Only Q-format multiply and divide need the fraction-bit count, because only they carry a shift, namely a right shift after the multiply and a left shift of the dividend before the divide, whereas Q-format add, subtract, modulo, and negate preserve the scale and need no shift. The fraction-bit count is therefore a `u8` operand on the existing `CheckedMul` and `CheckedDiv` opcodes, where `0` denotes integer arithmetic and a positive count denotes Fixed, so zero fraction bits is exactly integer multiply or divide. Add, subtract, modulo, and negate stay zero-operand and dispatch on the operand type alone. The `u8` operand occupies one of the three inline operand bytes, so the parameterized opcodes remain ordinary inline instructions and never become pool-indexed. This encoding was chosen over distinct Fixed opcodes because it is the static, type-independent format selector that a radiation-hardened silicon decoder wants, and it keeps the integer and fixed-point datapaths unified around a single shift parameter. The initial implementation used two dedicated opcodes; they were refactored away into the parameterized form once the silicon-encoding tradeoff was settled.

The work landed across four commits: the dispatch-only ISA layer and the frontend for the dedicated-opcode form, then the parameterizing refactor and its documentation.

With P3a through P3d complete, the entire checked-arithmetic redesign is done: optional arms with wrapping defaults, the `zero_divisor` outcome, per-operand-type admissibility, and the Byte, Float, and Fixed extensions across all four numeric operand types.

P4 is implemented. The indexing construct `array[i] { ok(v) => ..., invalid_index(idx) => ... }` is the third member of the construct family and reuses the `Expr::Checked` node: when the guarded operation is an array index, the type checker routes to a dedicated path admitting only `ok` (binding the element type) and `invalid_index` (binding the offending index `Word`), with `ok` a mandatory catch-all and `invalid_index` optional. The lowering adds no opcode. The compiler synthesizes the bounds check from `Op::Len`, integer comparisons, and `Op::If`, computing an outcome flag and stashing the element when in range, then dispatches arms through the same virtual-loop machinery as the arithmetic construct. An unhandled out-of-bounds index re-issues the plain `Op::GetIndex`, which traps with the precise `VmError::IndexOutOfBounds(index, len)`, so the unhandled path keeps full diagnostic payload. This phase deliberately stays opcode-free, in contrast to P3d-iii, because a bounds check decomposes cleanly into existing instructions whereas the `Fixed` Q-format shift did not. The native out-of-bounds default (the element type's zero-or-lowest-valid value) is part of P8, which is gated on native code generation.

P5 is implemented. The newtype-construction construct `Name(value) { ok(v) => ..., invalid_newtype(x) => ... }` is the fourth member of the family and reuses the `Expr::Checked` node: when the guarded operation constructs a newtype, the type checker routes to a dedicated path admitting only `ok` (binding the constructed newtype) and `invalid_newtype` (binding the underlying value the refinement rejected). The `invalid_newtype` arm is admissible only for a refined newtype, since a non-refined newtype's construction is total; the type checker tracks the refined names in a new `Ctx::refined_newtypes` set populated during refinement validation. The lowering adds no opcode. The compiler computes the underlying value, runs the refinement predicate when one exists, and branches on the result into an outcome flag (0 ok, 1 invalid newtype), then dispatches arms through the same virtual-loop machinery as the other constructs. An unhandled failure traps with `TrapKind::RefinementFailed`, the same fault a bare construction produces. The native default (the lowest-valid value, per the lowest-valid precedence) is part of P8, which is gated on native code generation.

P6 is implemented. The discriminant-to-enum construct `discriminant as Enum { ok(Variant) => ..., payload_discriminant(Variant) => ..., invalid_discriminant(raw) => ... }` is the fifth member of the family and the reverse of the existing enum-to-`Word` cast. It reuses the `Expr::Checked` node: when the guarded operation is a `Word as Enum` cast, the type checker routes to a dedicated path. The three arm kinds split the variants by what a bare discriminant determines. An `ok(Variant)` overrides a unit variant; a generic `ok(v)`/`ok(_)` post-processes any unit variant, binding the converted value; a unit variant with no `ok` arm converts to itself. A `payload_discriminant(Variant)` supplies a payload variant's payload, with coverage of every payload-bearing variant mandatory (specifically or through a `payload_discriminant(_)` catch-all), since the discriminant cannot reconstruct a payload. An `invalid_discriminant(raw)` binds the raw `Word` of an unmapped discriminant and, unhandled, traps with `TrapKind::EnumVariantUnmapped`. Arms match by variant name, so the construct survives discriminant renumbering. Upper-case identifiers in arm patterns name variants and lower-case identifiers (or `_`) are binders and catch-alls, distinguished by leading-character case. The bare `Word as Enum` cast without arms stays inadmissible. The lowering adds no opcode: the compiler evaluates the discriminant once, then emits a per-variant `if discriminant == d { <action> }` chain mirroring the enum-to-`Word` cast, and `infer_expr_type` learns the construct's enum result type so a let-bound result casts back to `Word` correctly. The native default (the zero-discriminant variant, or lowest valid when zero is absent) is part of P8.

P7 is implemented. The native-error construct `native(args) { ok(v) => ..., error(code) => ... }` is the sixth and final member of the family and the only one that crosses the native boundary, so it is not pure opcode-free synthesis. Three facts shaped the implementation, none of which matched the original design assumptions: native errors were `String` messages with no `Word` code, fallibility was untracked at compile time (and indistinguishable even at runtime, both registration paths producing the same boxed function), and native errors propagate by Rust `?` rather than as stack values. The resolved design, confirmed with the pilot: the native reports the `Word` code itself, optionally via a `keleusma-macros` `KeleusmaError` derive that maps a fieldless enum's variants to their discriminants and generates `From<E> for VmError` producing the new `VmError::NativeErrorCode { code, message }`; the VM reifies a soft host failure into the construct through an error-reify flag in the high bit of the call opcodes' argument-count byte, set only when an `error` arm is present, so no new opcode is added and a call without an `error` arm propagates errors unchanged; a message-only `NativeError` reifies to the sentinel code `-1`; and `error` is admissible on any native call since fallibility is untracked, the arm simply never taken on an infallible native. The `ok` arm is a mandatory catch-all binding the success value; `error` binds the `Word` code. The native default is a trap on both backends, since a host failure has no safe default, consistent with the rule that an operation gets a defined non-trapping default only when a total result exists.

With P1 through P7 complete, all six constructs of the partial-operation family are implemented at the virtual-machine layer: checked arithmetic across the four numeric types, indexing, newtype construction, discriminant-to-enum conversion, and native calls.

P8 is specified, and its implementation is deferred to V0.4.0 per the roadmap. P8 is the native code generation contract per target, namely the inserted guards and the platform-specific default values. There is no native code generation backend in the project; the runtime is a bytecode virtual machine, and native code generation via the Low Level Virtual Machine is the V0.4.0 milestone. The guards and defaults P8 describes are therefore code that a backend that does not yet exist would emit, so the lowering cannot be implemented now. What is implementable, and is delivered, is the full normative contract the future backend must honor. The new [`docs/spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md) specifies the two-backend contract, the per-operation virtual-machine trap variants, the native default values, the per-target hardware basis, the canonical zero value, and the lowest-valid precedence, so the V0.4.0 backend has a complete and reviewable target. [`V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md) records the partial-operation native lowering as V0.4.0 scope and references the contract. The canonical zero value and the lowest-valid resolution the guards consult are already implemented in `src/zero_value.rs` (B35 P2).

P9 is complete, closing B35. The documentation is in place across three layers: the per-construct grammar in [`docs/spec/GRAMMAR.md`](../spec/GRAMMAR.md) (added incrementally in P3 through P7), the fault contract and native-code-generation contract in [`docs/spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md) (P8), and the language-design narrative in [`docs/architecture/LANGUAGE_DESIGN.md`](../architecture/LANGUAGE_DESIGN.md), whose former "Numeric Overflow Construct" subsection is rewritten as "Partial Operation Handling" to describe the construct family, the optional-arm defaults, and the two-backend contract in place of the superseded mandatory-arm Word-only construct. The decision header is struck through and marked resolved for V0.2.x.

Two items are carried forward beyond B35, each tracked in its own home rather than as open B35 work: the native guard-insertion lowering (V0.4.0, recorded in [`V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md)) and the restoration of dynamic trap-localization detail (a consumer of the B29 debug-information records). With those filed forward, B35 is closed.

**Cross-references.**

- The existing checked-arithmetic construct in `src/compiler.rs` and `src/vm.rs` is the redesign site for P3.
- The `with saturate_min` and `saturate_max` machinery in `src/parser.rs` and the grammar is reused by P2.
- The forward enum-to-`Word` cast `compile_enum_to_word` in `src/compiler.rs` is the companion to the new reverse cast in P6.
- The native code generation target work, when it exists, consumes the P8 contract.
- The conservative-verification stance in `docs/architecture/LANGUAGE_DESIGN.md` is the framing this entry refines toward genuine totality.
- B29 (strippable debug metadata) is where trap localization returns, through its `SourceSpan` or `CallSite` record or a dedicated trap-context record, after P1 dropped the dynamic detail from runtime traps.

## ~~B36. Narrow-word composite width: `from_value` versus `from_value_ctx` contract~~ (Resolved)

Surfaced during B28 item 2 Increment 3 (2026-06-15). On a narrow-word build (for example `--features narrow-word-8`, or `--all-features`, which also enables `narrow-float-32`), a flat composite's canonical byte layout uses the **module**-declared scalar widths: the compiler bakes module-width field offsets, and the VM packs every composite (constructed, const, native-result) so the script reads it at module widths. The host marshalling boundary, however, has two decoders: `KeleusmaType::from_value`, which reads at the **host runtime** widths (`W::BITS_LOG2`/`F::BITS_LOG2`), and `KeleusmaType::from_value_ctx`, which reads at the module widths carried in the `RefContext`. When the module word or float is narrower than the runtime `i64`/`f64`, these disagree, so a single composite body cannot satisfy both a module-width reader and a runtime-width reader.

The concrete symptom: a native that returns a composite the **script field-accesses** wants the body at module widths; a native whose composite result the **host decodes through `from_value`** wants it at runtime widths (the audio `pan_law` tests rely on this). Increment 3 kept native results at runtime widths (byte-identical to the prior `into_value` behavior, so `from_value` and the existing tests stay correct) and gated the two new Word-struct field-access tests off the narrow-word builds, because resolving this is a design decision rather than a mechanical fix. The bundled runtime is unaffected, since module and runtime widths coincide there.

**Resolved (2026-06-15), operator decision: option (a) with a cast rule.** A composite has one canonical layout, the module-declared widths, the same widths the compiler bakes into field access and the VM uses everywhere else. A value crossing into a composite body is cast from the host runtime width to the module width. The load-time width check guarantees the module width is at most the runtime width, so on production this cast is either identity (the bundled runtime, where the widths coincide) or a narrowing. A narrowing integer is the same wrapping overflow the VM already applies to in-script narrow-word arithmetic (`truncate_int_to_declared_width`), realised for free because `write_scalar_le` writes the low module-width bytes. A narrowing float is the ordinary `f64`-to-`f32` rounding cast. There is no undefined behaviour at any point: the cast is a value operation, the byte write stays within the freshly allocated body, and every read is a bounds-checked slice access. Decoding is the mirror: `from_value_ctx` and `Vm::decode` read at the module widths carried in the `RefContext` and widen to the runtime type, so they are correct on every build. The bare `from_value` reads at the runtime widths and is therefore a bundled-runtime convenience, correct only when the module and runtime widths coincide; on a narrow build a host decodes a composite through `from_value_ctx`/`Vm::decode`.

Implemented in B28 item 2 (the producing `into_value_ctx` family packs native composite results at the context's module widths, recursing for nested fields). The audio `pan_law` test helper was moved from `from_value` to `Vm::decode`, and the two Word-struct return tests in `tests/marshall.rs`, briefly gated off the narrow-word builds while this was open, now run unconditionally and pass under `--all-features`. The bundled runtime is unaffected throughout.

## ~~B37. Unsignatured-native text-bearing composite returns disagree on flat-versus-boxed body~~ (Resolved for V0.2.1: native-result flatten plus the signatured-native direction; flat-text snapshot materialisation dispositioned as B38)

Surfaced 2026-06-25 while repairing the `shell` natives so the docs-links check script (`scripts/check-md-links.kel`) runs again. Two B28 regressions sat in series on that path. The first, the argument side, is fixed (commit `fix(shell): resolve dynamic KStr string arguments via native context`): the shell natives read `Text` arguments through the context-free `GenericValue::as_str`, which resolves only `StaticStr`, so a script-computed `KStr` argument trapped with `expected Text, got KStr`. The fix moved the text-taking natives to `register_native_with_ctx` and resolves arguments through `as_str_with_arena`. Fixing it exposed the second regression, recorded here, which still blocks the docs-links gate with `InvalidBytecode("GetTupleField operand form does not match tuple body")`.

### The inconsistency

A text-bearing composite (for example the tuple `(Word, Text)` that `shell::run` returns) is classified two different ways depending on which path builds it.

- The **type-driven path** treats it as flat. The compiler classifies a `Text` field as `FlatFieldForm::Scalar(Text)` when the module word slot is at least the host pointer width (`classify_flat_field`, compiler.rs around 4099), so it bakes a flat `NewComposite` for in-script construction (compiler.rs around 6783) and a flat `GetTupleField` for access. The VM `NewComposite` handler packs the flat body, converting a `StaticStr` element into an arena `KStr` `(ptr, len)`.
- The **value-driven path** treats it as boxed. The value-side flat-eligibility predicate excludes `Text` for tuple and array elements by design (bytecode.rs around 602 to 611), so that a tuple's `KStr` stays visible to the `materialise_kstrings`/`contains_dynstr` lifecycle and a static-text tuple can still be yielded. `GenericValue::tuple` with no arena therefore builds the boxed body, and `into_arena_canonical` (the native-result canonicalisation at vm.rs around 6310 to 6366) leaves a reference-bearing composite boxed.

In-script values take the type-driven path, so they are flat and field access matches. A native return takes the value-driven path, so it is boxed, but its consumer field-accesses it with the type-driven flat form. The form mismatch reaches the `_` arm at vm.rs around 5070 and raises `InvalidBytecode`. The bundled test suite does not catch it because no current test destructures an unsignatured native's text-bearing composite return; the docs-links script is the only consumer that does, and it was masked until now by the argument-side trap above.

### Scope of affected returns

Any unsignatured native (one registered through `register_native`, `register_native_with_ctx`, or `register_native_closure`, so the VM has no declared return type to canonicalise against) that returns a composite containing a `Text` field. In the shell bundle this is `shell::run`, `shell::run_full`, and `shell::run_timeout` (each returns a tuple with a `Text` element) and `shell::getenv` and `shell::arg` (each returns `Option<Text>`). A bare `Text` return (`shell::run_checked`, `shell::read_file`, `shell::hostname`, `shell::pwd`) is a scalar `StaticStr` and is not field-accessed, so it is unaffected. There is no native-side workaround using the existing helpers, because `tuple_in_arena` and `pack_flat_in_arena` consult the same value-driven predicate that excludes `Text`, so they also produce the boxed body.

### Reconciliation directions

Three directions close the gap. Each is a core B28 representation change and warrants its own review.

- **Signatured native returns through the marshalling family, the nominal host-side solution.** The mismatch exists only for an *unsignatured* native, where the VM has no declared return type and falls back to the value-driven canonicalisation. A native registered through the typed `KeleusmaType` marshalling family carries its return type, so the VM canonicalises the result through the type-driven flat path (the same `into_value_ctx` packing B36 settled), and the body matches the baked field access by construction. The shell natives are unsignatured today only because, per the comment in their `register`, the marshalling family does not yet support tuple or composite return types. The host-facing fix is therefore to extend the marshalling family and the `keleusma-macros` derives so a native may declare a tuple, struct, enum, or `Option<Text>` return and be registered signatured, after which `shell::run` and friends carry their `(Word, Text)` and `Option<Text>` types and pack flat. This is the strategic direction even though the supporting marshalling and macro surface is not yet built out; it connects to B34 (the keleusma-macros extension) and B33. It does not by itself decide the value-driven question below for hosts that stay on the raw `register_native` path.
- **Flatten text composites on the native-result path.** Make `into_arena_canonical`, and the value-side flat-eligibility it consults, flatten a text-bearing composite into the same arena-flat body the compiler bakes, converting each `StaticStr` or `KStr` field into an arena `(ptr, len)`. This makes every path agree on flat for raw unsignatured natives too, matching the in-script `NewComposite` behaviour and the rest of B28 text-flattening. The risk is the `materialise_kstrings`/`contains_dynstr` lifecycle and the static-text-tuple yield path that the value side intentionally protects; the design must confirm a flattened native-result tuple still yields and materialises correctly, or restrict the change to the native-result canonicalisation entry point rather than all value-driven construction.
- **Box text composites everywhere.** Align the compiler to keep a text-bearing composite boxed in both construction and access, matching the value side. This reverts part of the B28 P3/P4 text-flattening and has broad fallout in the layout, WCMU, and wire-format expectations that already assume flat text composites, so it is the less attractive direction.

### Phased implementation plan (value-driven flatten, the minimal unblock)

The signatured-native direction above is the strategic host-side solution, but it depends on extending the marshalling family and the macros to carry composite return types (B34), which is the larger build-out. The plan below is the minimal unblock for hosts on the raw `register_native` path, the flatten direction; the operator selects the direction before implementation begins.

1. **Reproduce in a unit test.** Register a throwaway native that returns `GenericValue::tuple(vec![Int, StaticStr])`, declare its type to the compiler, field-access the result in a script, and assert the current `InvalidBytecode`. This pins the regression independent of the `shell` feature and the docs-links script.
2. **Localise the flatten.** Extend `into_arena_canonical` so a reference-bearing composite whose fields are all flat-eligible-including-text is re-packed flat, allocating each `StaticStr`/`KStr` field as an arena `KStr` and writing its `(ptr, len)` at the packed offset, recursing for nested composites. Prefer changing the native-result canonicalisation entry rather than the shared `is_flat_eligible`, so value-driven construction elsewhere keeps its documented boxed behaviour unless the lifecycle review clears a wider change.
3. **Verify the lifecycle.** Confirm a flattened native-result tuple still yields to the host and that `materialise_kstrings`/`contains_dynstr` see its text. Add a yield-then-inspect test and a RESET-staleness test over the flattened body.
4. **Re-green the docs-links gate.** Run `scripts/check-md-links.kel` through the CLI and confirm it completes. Add an integration test that drives `shell::run` and destructures the tuple.
5. **Full gate.** default, signatures, all-features (which also exercises the narrow-word path, where `Text` stays boxed and the flatten must not fire), clippy, fmt, and `cargo doc` under the CI `RUSTDOCFLAGS`.

### Prerequisite already landed

The argument-side fix (commit `140f3bc` on the `chore-fast-iteration` branch) is the prerequisite that exposed this item; it is independently correct and carries its own regression test.

### Resolved (2026-06-25), direction (2), the native-result flatten

Implemented the value-driven flatten as the minimal unblock. `GenericValue::into_arena_canonical` now routes each composite field through a new `into_arena_canonical_field`, which canonicalises nested composites and then promotes an owned `StaticStr` field to an arena `KStr`. The promotion is what lets the value-driven packer (`pack_flat_in_arena` via `flat_field_size`, which already accepts a `KStr` as a flat `Text` field but not a `StaticStr`) pack the composite flat, so a native-returned `(Word, Text)` tuple becomes byte-for-byte the body the in-script `NewComposite` path builds for the same type, and the compiler's baked flat access matches. The promotion is gated on `word_bytes >= host pointer width`, the same condition `classify_flat_field` uses to admit a flat `Text` field, so a narrow-word build keeps the field a `StaticStr`, the composite boxed, and the access boxed, all consistent. A top-level bare `StaticStr` return is not promoted, since it is read directly rather than through baked flat access. The shared eligibility predicates and the value-driven `NewTuple`/`NewArray` construction paths are untouched.

Two further repairs followed from testing the other composite kinds. First, `into_arena_canonical` had kept `Option` boxed on a stale B28 P2 comment, even though `Option::Some` has flattened like any enum since B28 P3 item 5 C4; a native-returned `Option<Text>`, including `shell::getenv` and `shell::arg`, was therefore boxed against the compiler's flat-baked access. The `Option` arm now promotes the payload and flattens through `enum_in_arena` with the fixed `Some == 1` discriminant, falling back to boxed for a non-flat payload, which matches the compiler on both paths. Second, struct and array native returns were confirmed to flatten correctly by the same field promotion. The covering tests are in `tests/native_composite_return.rs`, exercising struct, array, enum, and `Option<Text>` returns.

A residual remains for non-`Option` enums. The simple `EnumBody::boxed` constructor records discriminant `0` and no largest-variant padding hint, so an unsignatured native that returns a non-first, non-largest variant produces a body that disagrees with the compiler on both the discriminant and the size, and the variant silently misreads. A native can avoid it by building the result with `EnumBody::boxed_with_layout(disc, min_payload)`, and the signatured-native direction recovers the enum type and supplies both automatically. This is pinned by an ignored test, `native_enum_smaller_later_variant_known_limitation`.

Verification: the `scripts/check-md-links.kel` gate, which destructures `shell::run`'s `(Word, Text)` tuple, now runs to completion; a new end-to-end test `native_returning_word_text_tuple_flattens_and_destructures` in `tests/flat_ref_tuple.rs` pins the regression; full default suite (1443) and the narrow-word-8 suite (1225) pass, as do the signatures suite, `cargo clippy --workspace --all-targets`, `cargo fmt --check`, `cargo doc` under the CI flags, and `cargo miri test` over the new test under tree borrows.

### Signatured-native direction resolved (2026-06-27)

The first formerly-deferred part is closed, and a stale claim in the analysis above is corrected. The signatured-native direction does **not** depend on B34 (which was the shared-data `to_flat_bytes` work). `register_fn` already marshals a composite result through the declared type's `KeleusmaType::into_value_ctx` (B28 P3), which packs the arena-flat body with the correct discriminant and largest-variant padding taken from the type. So a native registered through `register_fn` returning a tuple, struct, enum, or `Option` is canonicalised correctly by construction, with no further marshalling or macro work; the "the marshalling family does not yet support tuple or composite return types" premise (and the matching `shell.rs` comment, now corrected) was already stale.

This is the strategic fix for the enum residual above: a non-first, non-largest enum variant returned by a *signatured* native reads back correctly, where the unsignatured one misreads. Demonstrated by `tests/native_composite_return.rs::signatured_native_returns_smaller_enum_variant_correctly` (registers `code` via `register_fn` returning `Msg::Code`, the smaller variant; the script destructures it to the right value). The unsignatured limitation stays pinned by the ignored test alongside it, since the value-driven flatten genuinely cannot supply the type's discriminant and padding.

The shell natives stay on `register_native`/`register_native_with_ctx`: the value-driven flatten already handles their `(Word, Text)` tuple and `Option<Text>` returns, so a migration to signatured `register_fn` is cleanup, not a correctness need.

### Deferred

One part remains, and it is tracked elsewhere: a flat text composite, whether native-returned or in-script, carries its `Text` as opaque `(ptr, len)` bytes with no layout, so the layout-blind `materialise_kstrings` and `contains_dynstr` treat a flat composite as scalar-only and do not materialise its text on a cross-arena snapshot (the REPL / hot-swap transport path). This is tracked and dispositioned as B38, resolved there as not reachable in V0.2.1: the transport boundary and the layout mechanism do not exist yet, so the layout-aware walk is part of the unbuilt snapshot/Phase D feature rather than standalone work.

## ~~B38. Layout-aware materialisation of flat composite reference fields across a snapshot boundary~~ (Resolved 2026-06-26: no V0.2.1 bug; the walk is subsumed into the snapshot/Phase D feature)

Surfaced by B37 (2026-06-25). A flat composite body is opaque bytes with no attached layout. The value-driven materialisation walks that run when a value is transported across a VM boundary, `GenericValue::materialise_kstrings` for `Text` today and the analogous `OpaqueRef`-to-`Arc` yield walk that B33 will add, are layout-blind. They recurse through boxed composites by walking `Value` fields, but they treat a flat composite (`TupleBody::Flat`, `ArrayBody::Flat`, `StructBody::Flat`, `EnumBody::Flat`) as transitively scalar and clone it unchanged. A flat body that holds a `Text` field as a `(ptr, len)` arena reference, or under B33 an `OpaqueRef` index, is therefore not materialised.

**Symptom.** On a cross-arena snapshot, the REPL or hot-swap transport path that snapshots a value from one VM and restores it into a VM backed by a different arena, a flat composite's text reads stale after the swap. The arena epoch tag makes a stale read resolve to an empty string rather than dereference freed memory, so this is a correctness gap, not memory unsafety. Normal in-arena host reads and in-script field access are unaffected, because the former hold the live arena and the latter use compiler-baked flat access that already knows the field offsets.

**Scope and pre-existing status.** The compiler has produced flat text composites for in-script construction since B28 P3, so this gap predates B37. B37's native-result flatten makes native-returned composites identical to in-script ones, so they now share the gap rather than introduce a new one. The gap is reachable only through the snapshot or hot-swap transport path; a program that only reads fields or yields into a live-arena host never hits it.

**Design directions.**

1. **Layout-aware walk.** Thread the value's type or layout into the materialisation walk so it can locate `Text` and `OpaqueRef` fields at their packed offsets inside a flat body and materialise them, mirroring the offset logic the compiler already bakes for access. The layout descriptor is the same `crate::value_layout::LayoutDescriptor` B34 emits.

2. **Layout-carrying flat composites.** Attach a compact layout reference to each flat composite body so the walk is self-describing and needs no external type, at the cost of widening the flat value or holding a side table.

**Cross-references.**

- B33 adds the `OpaqueRef`-to-`Arc` yield walk, which faces the identical layout-blindness over flat bodies; the two materialisation walks should be unified rather than written twice.
- B34 supplies the `LayoutDescriptor` machinery that direction one reuses, so B38 is smaller once B33 and B34 land.
- B37 is the origin and the immediate motivation.

### Resolved (2026-06-26): no V0.2.1 bug; subsumed into snapshot/Phase D

A premise-check found the symptom unreachable in the current codebase, so there is no V0.2.1 bug to fix and no standalone work; the layout-aware walk lands with the snapshot feature.

- `materialise_kstrings` is `pub` but has no callers anywhere in `src/`, `examples/`, or `keleusma-cli/`, only its own recursion and its tests. `contains_dynstr` is the same. Neither is on a live VM path.
- There is no snapshot or cross-arena restore mechanism (no `snapshot`/`restore`/relocatable code; Phase D is unimplemented). B38's symptom is a flat composite's text reading stale *across a snapshot*, and that boundary does not exist.
- The one live, layout-blind guard, `value_has_ephemeral_str` (the yield boundary), is intentionally so. A flat `Text` field is epoch-tagged, so a flat-text composite crossing the yield boundary is governed by read-before-resume: the host decodes it before the next RESET, and a later read resolves cleanly Stale (an empty string), never undefined behaviour. Making the guard reject it would break intended behaviour, not fix a bug.

B38 also cannot be implemented in isolation: `materialise_kstrings` takes only `&arena`, and a flat composite body is opaque bytes with no attached layout, so walking its `Text`/`OpaqueRef` fields requires either attaching layout to flat bodies or threading the type in at the call site, and there is no call site to thread it from. The layout-availability mechanism is exactly what the snapshot (Phase D) feature would establish. Building the walk now would be dead, untestable, speculative code with no consumer.

Resolution: defer B38 to the snapshot/Phase D effort, where the value-transport boundary and the layout mechanism are introduced together; the layout-aware walk (unified for `Text` and `OpaqueRef`) is then part of that feature rather than standalone infrastructure ahead of it. This is the same forward-looking-ahead-of-its-consumer situation as B33's deliberately-omitted persistent registry.

## ~~B39. Correct the false arena-reservation rationale for the nextest concurrency cap~~ (Resolved 2026-06-26)

Filed 2026-06-26. An earlier diagnosis, repeated in the `.config/nextest.toml` and `CONTRIBUTING.md` comments added in `d1bc4e1`, claimed that `keleusma-arena` reserves a very large virtual region, on the order of hundreds of gigabytes per process, and that concurrent arena-mapping test processes therefore exhaust memory. That claim is false and must be corrected wherever it appears.

**The facts.** `Arena::with_capacity` (`keleusma-arena/src/lib.rs`) calls `alloc_zeroed(Layout::from_size_align(capacity, 16))`. It allocates exactly `capacity` heap bytes, with no `mmap` and no oversized reservation. `DEFAULT_ARENA_CAPACITY` is 64 KiB, and the tests use small capacities (16 bytes, 64 KiB, or small auto-sized values from `auto_arena_capacity_for`). The hundreds-of-gigabytes figure was read from `ps` `VSZ`, which on macOS is virtual-address noise from system libraries and malloc zones and is present in every process. A trivial `/bin/bash` shows roughly 415 GB `VSZ` against 5 MB `RSS`; the arena contributes effectively nothing to it.

**What actually happened.** The wedge observed while validating B37 was ordinary host memory exhaustion driven by running several heavy `cargo` invocations concurrently over a long session, parallel `rustc` compilation plus nextest's process-per-test plus the desktop, on a 32 GiB machine. Once RAM, the compressor, and swap filled, processes blocked on memory at zero percent central-processing-unit. This is not arena-specific and not a Keleusma defect.

**Actions.**

1. Correct the rationale comments in `.config/nextest.toml` and `CONTRIBUTING.md`: the `test-threads = 4` cap is a defensible mitigation because it bounds peak memory from concurrent test processes, not because of any arena reservation. Keep the cap; fix the reason.
2. Operationally, do not run multiple full-gate builds concurrently, and prefer serial test execution when host memory is constrained.

There is no arena change to make. This entry exists to retract the incorrect claim and points at the comment corrections.

**Resolved (2026-06-26).** Both actions are complete in commit `0c6e299`: the `.config/nextest.toml` and `CONTRIBUTING.md` comments now state the memory rationale and explicitly retract the arena-reservation claim, and the `test-threads = 4` cap is kept. The single-gate nextest runs earlier in the session succeeded repeatedly; the wedges occurred only when several heavy builds overlapped, so the cap plus serial discipline is an adequate mitigation. Whether to harden the gate further (a memory-aware runner, or reverting to serial `cargo test`) is a separate operational question, not an arena defect, and is left open as a note rather than tracked work.

## B40. General const generics

**Status: implemented (V0.2.1).** Implemented on the `feat-const-generics-bignum` branch after an operator decision to proceed at maximal scope. Const parameters are declared on functions, structs, and enums, are usable as `Word` values in code bodies, and support total const arithmetic over `+`, `-`, and `*`. The implementation summary and the erasure invariant that preserves the worst-case-execution-time and worst-case-memory-usage guarantees are recorded at the end of this entry. The original rationale below is retained as filed.

**Original status (deferred, tracked).** A deliberate operator decision (2026-07-03): scope the feature and track it here rather than implement it now, because it is a large type-system feature disproportionate to the B19 operator residuals it was raised alongside.

**Current situation.** `Multiword<N, F>` is the only type that carries compile-time-constant parameters, and the compiler recognises `N` (word count) and `F` (fraction bits) **specially**: they are fields of a dedicated `Type::Multiword(N, F)` and `TypeExpr::Multiword`, parsed by a bespoke path, and consumed by hand-written per-`N` lowerings. There is no general notion of a `const` type parameter that a user type or function can declare and use. A program cannot write, for example, `struct Buffer<const N> { data: [Word; N] }`, `fn zeros<const N>() -> [Word; N]`, or a bound like `Multiword<N>` where `N` is itself a generic parameter threaded from an enclosing definition.

**What the feature is.** General const generics would add const-valued type parameters across the surface language and the pipeline:

1. **Grammar and AST.** A `const` generic parameter in `<...>` lists (`<const N>`, and mixed with type parameters), const arguments at use sites (`Buffer<8>`, `Multiword<N>` with `N` a parameter), and const parameters usable in array sizes and other const positions.
2. **Type system.** A const parameter is a distinct kind from a type parameter; unification and the occurs check extend to const equality; const arguments must be checked for kind and for satisfying any declared bound (for example an array length must be a non-negative `Word` constant). Const expressions in argument position need a small, total const-evaluation sublanguage or a literals-only restriction.
3. **Monomorphization.** The monomorphizer, which already specialises over type arguments, must specialise over const **values**, keying instances by the concrete const and substituting it into array sizes, `Multiword` parameters, and loop bounds. `Multiword<N, F>`'s per-`N` cascades would become the first consumer, replacing the special-case path.
4. **Verifier (the load-bearing constraint).** Keleusma's guarantee is definitive WCET and WCMU. A const parameter that feeds a loop bound or an allocation size must remain **statically known at each monomorphized instance**, so the bound stays provable. This is compatible with monomorphization (each instance has a concrete const) but forbids any route by which a const becomes runtime-dependent. The design must state and enforce that const parameters are resolved before verification, per instance, exactly like the current `Multiword<N>` unrolling depends on a concrete `N`.

**Why deferred.** The feature is comparable in size to the existing generics-and-monomorphization system and touches every stage of the pipeline; it is not a residual-sized change. The one type that needs const parameters today, `Multiword<N, F>`, already works through its special-case path, so there is no functional gap for current programs, only a generality gap. Implementing it now would not unblock any committed milestone; V0.3.0 is committed to self-hosting the lexer, parser, and compiler, which does not depend on const generics (the same reasoning that deferred `Multiword` itself, recorded in B19).

**When to revisit.** When a concrete use case needs a user-defined type or function parameterised by a compile-time constant (for example a fixed-capacity ring buffer or a statically-sized matrix in an embedding), or when the `Multiword` special-case path becomes a maintenance burden worth replacing with the general mechanism. At that point this entry should become a phased plan mirroring the generics rollout: grammar and AST, type-system kind and unification, monomorphization over const values, then verifier integration with the static-known-per-instance invariant made explicit.

**Relation to other items.** Supersedes the "general const generics remain a separate feature" note in B19 and Standard 5.1.2, which now points here.

### Implementation summary (V0.2.1)

The feature was delivered in five phases, each committed only after passing the full gate under default, default plus signatures, and all features, with `clippy --tests --all-features -D warnings` and `cargo fmt --check`.

**Surface syntax.** A const parameter is a lowercase name declared `const n` or `const n: Word`, where `Word` is the only admissible const-parameter type, mixed freely with type parameters as in `<T, const n: Word>`. Const arguments are always explicit because they cannot be inferred from value arguments. A call writes a turbofish, `f::<8>(...)`. A struct or enum construction writes a turbofish before the body or variant, `Buf::<8> { ... }` and `Opt::<8>::Some(...)`. A type reference writes the const in the argument list, `Buf<8>`, `Multiword<n>`, `[Word; n]`. In a `<...>` list an uppercase-leading argument is a type argument and a const argument follows all type arguments, an ordering the parser enforces. A const argument may be an arithmetic expression over `+`, `-`, and `*` with the usual precedence, for example `Buf<n + 1>` and `Multiword<2 * n>`; division and modulo are excluded so evaluation is total.

**The erasure invariant.** The pipeline order is typecheck, then monomorphize, then a mandatory re-typecheck, then compile, then verify. Monomorphization substitutes every const parameter to a concrete integer literal, so the verifier never observes a symbolic const and the static-bound analyses are preserved unchanged. A symbolic const dimension reaching the layout pass is an internal-compiler-error tripwire, which turns the erasure property into a checked assertion rather than a convention. The post-monomorphization re-typecheck is the soundness gate; a const dimension that is symbolically compatible in a generic body but concretely mismatched at an instantiation is rejected there.

**Type system.** Resolved array and Multiword dimensions became a `ConstDim` that is either `Known` of a folded literal or `Sym` of a canonically rendered symbolic form. Two `Known` dimensions unify when equal, two `Sym` dimensions unify when their canonical renderings are equal, and a `Known` against a `Sym` is accepted in the first pass and deferred to the re-typecheck. The canonical rendering folds fully-literal subexpressions and orders the operands of the commutative operators, so `n + 1` and `1 + n` unify. Associativity across nested operations is not normalized and defers to the re-typecheck.

**Monomorphization.** The specializers key each instance on the concrete const values in addition to the type arguments, mangle the specialized name with the const values, substitute const dimensions into field, payload, parameter, and return types, and rewrite const-parameter value references in bodies through a scoped walk that honors local shadowing. A shared signature-resolution pass rewrites const-generic type references in function parameters and returns to the specialized names so a `Buf<8>` parameter and a `Buf::<8>` construction agree on the same specialization.

**Known limitations.** Associativity is not normalized in the first-pass `Sym` comparison, so `(n + 1) + m` and `n + (1 + m)` differ in the first pass and defer to the re-typecheck; this is not a soundness gap because the re-typecheck sees only concrete literals. Arity of a struct or enum const-argument turbofish is validated at the post-monomorphization re-typecheck rather than in the first pass, so a wrong count on a construction surfaces there rather than at the construction span; the function turbofish is arity-checked in the first pass. A construction turbofish carries const arguments only, `Pair::<3> { ... }`, so a type argument on a mixed `<T, const n>` type must be inferable from the field values; there is no `Pair::<Word, 3> { ... }` form, and a mixed type whose type parameter is not inferable from a field cannot be constructed. Mixing a type parameter and a const parameter otherwise works and is covered by `mixed_type_and_const_parameters`.

**Resolved: trait-method dispatch on generic structs and enums.** Formerly an open gap. A trait implemented for a generic type now resolves for concrete, type-generic, and const-generic receivers alike, on both structs and enums. The gap had three parts, each addressed. The first-pass type checker now seeds the impl block's type and const parameters into every method signature and body, so an impl's generic receiver `Cell<T>` is instantiated with a fresh variable and unifies with a concrete `Cell<Word>` at a call site. Monomorphization now specializes each generic impl once per recorded concrete instantiation of its target type through a new `specialize_impls` pass, substituting the impl's type and const parameters through the method signatures and bodies, rewriting the specialized signature types (`Cell<Word>` to `Cell__Word`, `Buf<4>` to `Buf__c4`) and, for an enum receiver, the method body's match-arm patterns, and dropping the generic original. The specialized method chunks the compiler folds under `Trait::SpecName::method` therefore reconcile with the specialized receiver head at dispatch, and the mandatory re-typecheck sees only concrete impls. Coverage is `tests/generic_methods.rs`: a concrete regression sentinel, a type-generic struct method, a const-generic struct method, two distinct const specializations each dispatching to their own impl, a const parameter used as a value in a method body, and a const-generic enum method. One limitation remains and is a property of the type specializer rather than of methods: a type-generic struct with a phantom type parameter (one not used in any field) cannot have its construction inferred, so no specialization is minted and a method on it does not resolve; a type parameter that appears in a field works.

Indexing a struct field that is itself an array (`b.items[i]`), noted in earlier drafts as a limitation, is resolved: the compiler previously assumed any `identifier.field[index]` was a `data`-segment indexed access and rejected a struct receiver with an "unknown data block" error, and the fix takes the data-segment route only when the base identifier is actually a data block so a struct field falls through to the general array-index lowering. Enum payload arrays were never affected because a payload is bound by `match` rather than accessed as `field[index]`.

## B41. Rex-review borrowable lessons

Filed 2026-07-09 from the durable comparative review of Peter Kelly's Rex (`rex` head `f25eb00`, 2026-06-29), retained project-locally at `tmp/rex-review/rex-review-internal-2026-07-10.md`. Rex is the closest peer to Keleusma in design and the furthest apart in guarantee model, and the review distilled seven borrowable lessons plus two lower-level items. This entry tracks them so they stop living only in the git-ignored review. The tooling-parity work the review motivated (the LSP, the browser playground, the VS Code extension, the hosted book, and the Instruction Set reference) is already delivered and is not tracked here.

None of these are blocking, and several are partially addressed. Status and effort are the file-time estimates.

| # | Lesson | Status | Effort | Cross-reference |
|---|--------|--------|--------|-----------------|
| 1 | Write the prelude and standard library in Keleusma itself, as an incremental self-hosting on-ramp before codegen-in-Keleusma. | open | large | `docs/roadmap/V0_3_0_SELF_HOSTING.md` |
| 2 | Design verifier and compile rejections for the machine-repair loop, not only the human reader. A stable error code, the specific unprovable obligation, and the offending span let an LLM or tool repair a rejected program. No stable error-code taxonomy exists today. | open | medium | `book/src/WHY_REJECTED.md`, `LLM_USAGE.md`, and the B29 `VerifierWitness`/obligation machinery |
| 3 | Bound the compiler front end against pathological input, a denial-of-service surface distinct from runtime WCET. The parser caps recursion at `MAX_PARSE_DEPTH = 24`, the monomorphizer caps specializations at 1024, and the type checker now caps inferred tuple type size at `MAX_INFERRED_COMPOSITE_TYPE_NODES = 100_000`. | addressed | medium | `src/parser.rs`, `src/typecheck.rs`, `src/monomorphize.rs` |
| 4 | Generate reference material from source and fail the build on drift. The Instruction Set book chapter is now generated from `docs/spec/INSTRUCTION_SET.md` by `scripts/gen-book-instruction-set.py` with a CI drift gate. Remaining stronger move: generate the opcode and cost tables from `src/bytecode.rs`, and the standard-library signatures from source, closing silent doc rot at the primary source. | partial | small-to-medium | `scripts/gen-book-instruction-set.py`, `.github/workflows/ci.yml` |
| 5 | State purity-as-optionality in the design rationale. Referential transparency is spent once; Keleusma spends it on deterministic worst-case bounds and Rex on implicit parallelism. Saying so frames single-threaded execution as a choice that buys tight WCET, and a future parallel non-WCET mode as a different product rather than a gap. | open | small | `docs/architecture/LANGUAGE_DESIGN.md` |
| 6 | Give any standard library or primitive set one canonical registry from day one. The keyword set is now single-sourced at `keleusma::token::KEYWORDS`; the native and standard-library registry is the remaining candidate. | partial | medium | `src/token.rs`, native registration in `src/marshall.rs` and the natives modules |
| 7 | A version-cadence caution rather than a lesson to copy. Rex's high major-version churn is embedder friction; keep Keleusma's conservative 0.2.x cadence and codified release gate. | posture | none | `docs/process/RELEASE_PROCESS.md` |

**Lesson 3 addressed (2026-07-10).** A confirmed reproduction closed the inference path. A roughly twenty-five-line program of type-doubling tuple bindings, `let t0 = (0, 0); let t1 = (t0, t0); ...`, drove the type checker into exponential time, since `Type::Tuple` stores each element's type inline as an owned tree and each binding clones and doubles the previous type. Measured `check()` alone took 29 milliseconds at sixteen bindings, 345 at twenty, and 1.37 seconds at twenty-two, and full compilation did not finish within twelve seconds at twenty-two. Arrays store their element type once and structs, enums, and options carry only distinct type arguments, so the tuple literal is the sole inline-unfolding construction. The fix caps the inferred tuple type's node count in `type_of_expr` with an early-aborting bounded walk, so the check itself costs at most the cap and the first over-cap tuple is rejected with a clear `TypeError`. Pathological inputs now reject in about twenty-five milliseconds and ordinary tuples are unaffected. Coverage is `tests/frontend_resource_bounds.rs`.

Two lower-level items from Part IV of the review:

- **`CallSite`-style native-call context threading.** Rex threads a parent-pointer token through native calls for rooting and context-aware callbacks. Already tracked and largely implemented as the B29 `CallSite` debug record, which cites Rex as the prior art.
- **Public-versus-internal `Value` separation.** Drawn from Rex's own documented debt (`issues/05`). Keeping the host-facing value type distinct from the internal representation before the surface grows avoids a later painful split. Open, medium effort.

The review's bottom line is that the provenance-for-research thesis did not change Rex's trajectory, that Rex leads on editor tooling and onboarding docs and longevity, and that Keleusma leads on its own guarantee model, `no_std` fit, and test and comment density.
