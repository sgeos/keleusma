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

7. **Demonstrator `Vm<i16, u16, f32>` plus cookbook recipe.** Worked example at `examples/narrow_runtime.rs` exercises `GenericVm<i16, u16, f32>` against bytecode compiled with `Target::embedded_16()`. Three scenarios: plain arithmetic (1 + 2 = 3 as i16), wrapping at the word boundary (30_000 + 10_000 = -25_536 in i16), and host-side `register_fn` with a natural Rust `i64` closure that the marshall layer truncates to `i16`. Integration test at `tests/narrow_vm.rs` pins all three. Cookbook recipe added at `docs/guide/COOKBOOK.md` under *Narrow-runtime type alias*, documenting the `type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>` pattern, the host-function marshall-widening behaviour, the standard-library-bundle bound to the default shape, and the word-width arithmetic discipline. *Resolved on `v0.2.0`.*

8. **Soundness-closure follow-up pass.** After steps 1-7 closed the public API, three residual gaps remained from the gap-audit pass and now land together. *Resolved on `v0.2.0`.*

   - **Load-time width validation.** `Vm::new`, `Vm::new_unchecked`, and `Vm::view_bytes_zero_copy` now validate that the bytecode's declared `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` are each at most the runtime's `<W as Word>::BITS_LOG2`, `<A as Address>::BITS_LOG2`, and `<F as Float>::BITS_LOG2`. The narrow Vm previously admitted wider bytecode and silently truncated constants through `Word::from_i64_wrap`; the new check rejects the mismatch as `VmError::VerifyError`. The `Address` parameter `A` now carries runtime semantics through this check. `replace_module_inner` also runs the check so hot-swap respects the same soundness property.
   - **Standard-library bundle lift.** `stddsl::Math` and `stddsl::Audio` lift to `impl<W: Word, A: Address> Library<W, A, f64>`. The inner `math::register` and `register_audio_natives` quantify over `W` and `A` and pin `F = f64` because their closures use `f64` arguments. Hosts that combine a narrow `Word` (i16, i32) with `f64` floats can now register these bundles directly.
   - **`Word::to_usize_checked`** helper added with a default impl that delegates to `to_i64` and `usize::try_from`. Mirrors `Address::to_usize_checked` and gives custom `Word` impls a uniform conversion path. Two new unit tests pin the conversion for the positive and negative branches across `i8`, `i16`, and `i64`.

   Integration coverage in `tests/narrow_vm.rs`: width-mismatch rejection (`narrow_runtime_rejects_wider_word_bytecode`, `wider_float_bytecode_rejected_by_f32_runtime`), lifted Math bundle on `GenericVm<i16, u16, f64>` (`narrow_runtime_can_register_math_library_via_lifted_impl`), a runtime whose Float type is f32 running matching bytecode through `register_fn` (`narrow_float_runtime_runs_f32_bytecode`), checked-arithmetic on the narrow i16 runtime exercising `Word::widen`/`WideWord` (`narrow_runtime_checked_arithmetic_exercises_word_widen`), and hot-swap width-mismatch rejection (`narrow_runtime_rejects_hot_swap_to_wider_bytecode`).

9. **Text and Shell library lift.** `stddsl::Text` and `stddsl::Shell` move from `Library<i64, u64, f64>` to `impl<W: Word, A: Address, F: Float> Library<W, A, F>`. `register_utility_natives` and `stddsl::shell::register` quantify over `(W, A, F)`. Every utility native (`native_to_string_with_ctx`, `native_length_with_ctx`, `native_concat_with_ctx`, `native_slice_with_ctx`, `native_println`, plus the helpers `render_value_to_string`, `read_string_arg`, `check_arity`, `read_i64_arg`, `finalize_string_result`) quantifies the same way. Pattern arms switch from `Value::` to `GenericValue::`; integer-payload formatting bridges through `Word::to_i64` so any narrow word type produces the same numeric rendering; length values returned by `length` wrap through `Word::from_i64_wrap` so they fit the runtime's word width. The same lift applies to all five shell natives (`getenv`, `has_env`, `run`, `run_checked`, `exit`); the exit-code argument bridges through `Word::to_i64` and the `(exit_code, stdout)` tuple's word component is wrapped through `Word::from_i64_wrap`. A narrow runtime test (`narrow_runtime_can_register_text_library_via_lifted_impl`, gated on the `text` feature) pins the lift on `GenericVm<i16, u16, f64>` calling `length("hello")` to obtain `5_i16`. *Resolved on `v0.2.0`.*

10. **Math and Audio lift to generic Float; documentation pass.** `stddsl::Math` and `stddsl::Audio` move from `Library<W, A, f64>` to `impl<W: Word, A: Address, F: Float> Library<W, A, F>`. The inner `math::register` and `register_audio_natives` quantify the same way. The closures still use `f64` arguments and returns; on a runtime whose `F` is `f32`, every closure argument and return passes through `Float::from_f64` / `Float::to_f64` at the marshall boundary, narrowing constants and intermediates. The narrowing is mathematically defined and silent; programs that require full `f64` precision should select an `f64`-Float runtime rather than relying on the narrowing. A new test (`f32_narrow_runtime_can_register_math_library_via_lifted_impl`) pins `math::sqrt(9.0) = 3.0_f32` on a `GenericVm<i64, u64, f32>`. Documentation pass on the architecture and design knowledge graph: the *Narrow-runtime type alias* recipe in `docs/guide/COOKBOOK.md` is rewritten to reflect that all four `stddsl` bundles work on narrow runtimes; the stale "current 64-bit Keleusma runtime" prose in `docs/architecture/LANGUAGE_DESIGN.md` is replaced with parametric-aware text; the `i128` literal in the checked-arithmetic section becomes `W::Wide` with a concrete mapping table; the bytecode-load section in `docs/architecture/EXECUTION_MODEL.md` distinguishes the binary's framing-level upper bound from the per-Vm bound and explains how the two compose; the primitive-type tables in `docs/design/TYPE_SYSTEM.md` and `docs/design/GRAMMAR.md` annotate the `Word` and `Float` sizes as defaults that vary under the parametric shape. *Resolved on `v0.2.0`.*

11. **Verifier precision and ancillary test coverage.** Four post-audit follow-ups land together.

    - **Verifier `value_slot_bytes` threading.** The WCMU analysis previously used the hard-coded `VALUE_SLOT_SIZE_BYTES = 32` constant regardless of the runtime's chosen `(W, F)`. The `verify_resource_bounds_with_cost_model` entry point accepted a `CostModel` but ignored its `value_slot_bytes` field. The internal `wcmu_region`, `wcmu_subregion`, and `compute_chunk_wcmu` functions now thread `value_slot_bytes: u32` through; new public variants `module_wcmu_with_value_slot_bytes`, `wcmu_stream_iteration_with_value_slot_bytes`, and `verify_resource_bounds_with_natives_and_value_slot_bytes` expose the parameter. The cost-model entry point now honors `cost_model.value_slot_bytes` through this plumbing. `Vm::new_with_options` and `replace_module_inner` pass `core::mem::size_of::<GenericValue<W, F>>() as u32` so the WCMU bound matches the runtime's actual slot footprint. On a `GenericVm<i16, u16, f32>` whose `Value` enum is materially smaller than 32 bytes, the verifier now admits programs that would previously have been rejected as exceeding the conservative bound. The public-API functions `module_wcmu`, `wcmu_stream_iteration`, `verify_resource_bounds_with_natives`, and `verify_resource_bounds` retain their signatures and delegate with the 32-byte default.
    - **Audio bundle narrow-runtime test.** New `narrow_runtime_can_register_audio_library_via_lifted_impl` pins `audio::midi_to_freq(69) = 440.0_f64` on `GenericVm<i16, u16, f64>`. Belt-and-suspenders coverage of the lift code path (was previously verified by symmetry with the Math bundle test).
    - **Zero-copy regression tests.** Two new tests pin the load-time width check on the `view_bytes_zero_copy` path. `narrow_runtime_view_bytes_zero_copy_runs_embedded_16_bytecode` runs a narrow runtime against narrow precompiled bytes through the zero-copy entry point. `narrow_runtime_view_bytes_zero_copy_rejects_wider_bytecode` confirms that wider bytecode is rejected on the same path (matching the `Vm::new` rejection behavior).
    - **`Vm<i8>` end-to-end smoke tests.** Two new tests exercise an 8-bit signed-Word runtime end-to-end against `Target::embedded_8()` bytecode. `i8_narrow_runtime_runs_embedded_8_bytecode` confirms `100 + 27 = 127_i8` (fits `i8::MAX`); `i8_narrow_runtime_wraps_at_i8_boundary` confirms `100 + 28 = -128_i8` (wraps via `Word::wrapping_add`). *Resolved on `v0.2.0`.*

12. **Binary-build narrowing features for runtime maximums.** The framing-level constants `RUNTIME_WORD_BITS_LOG2`, `RUNTIME_ADDRESS_BITS_LOG2`, and `RUNTIME_FLOAT_BITS_LOG2` in `src/bytecode.rs` previously held the build-time-fixed value `6` (i64, u64, f64). The change introduces seven Cargo features that lower the constants on builds shipping only narrow runtimes. The feature set is `narrow-word-8`, `narrow-word-16`, `narrow-word-32`, `narrow-address-8`, `narrow-address-16`, `narrow-address-32`, and `narrow-float-32`. The narrowest enabled feature wins per dimension; absence of any narrowing feature retains the default of `6`. The narrowing affects the framing-level check inside `Module::access_bytes` and `Module::from_bytes`, the widths reported by `Target::host()`, and the binary's compile-time admissibility through `Target::validate_against_runtime`. It does not change opcode dispatch or the parametric `GenericVm<W, A, F>` shape; the per-Vm width check at `<W as Word>::BITS_LOG2` continues to apply on top of the framing-level rejection. Tests that exercise i64-boundary behavior (Q31.32 fixed-point, i64 checked-arithmetic overflow, golden bytecode bytes, saturate-keyword newtype contracts, embedded_16 admissibility tests) are gated on the absence of the relevant narrowing features so they remain in the default build's matrix but are skipped on narrowed builds. A new test `runtime_width_constants_track_narrowing_features` in `cost_model_tests` pins the constants per feature combination. *Resolved on `v0.2.0`. 737 lib tests pass in the default configuration; 725 on `narrow-word-16`; 720 on `narrow-word-8`. Clippy clean; STM32N6570-DK full pipeline check clean.*

### Status snapshot

All twelve steps complete. Steps 1-4 landed in commits `a820607`, `af6a307`, `25e4a39`, `dbd9594`. Step 5 landed on the `V0.2.0-parametric-vm` feature branch and merged to `v0.2.0` in merge commit `fa68a3f`; six WIP checkpoints from the feature branch travel into trunk as one merge. Step 6 landed on `v0.2.0` in commit `4f7be84`. Step 7 landed on `v0.2.0` in commit `d33fc9d`. Step 8 landed on `v0.2.0` in commit `a89582d`, with a follow-up hot-swap fix and checked-arithmetic test in `9c40b35`. Step 9 landed on `v0.2.0` in commit `e9166fb`. Step 10 landed on `v0.2.0` in commit `89892e4`. Step 11 landed on `v0.2.0` in commit `71095e2`. Step 12 lands alongside this BACKLOG update.

The bundled `Vm<'a, 'arena>` aliases `GenericVm<'a, 'arena, i64, u64, f64>`, so every pre-existing call site compiles unchanged. Hosts targeting narrower runtimes instantiate `GenericVm<i16, u16, f32>` (or any other admissible combination) directly. The worked demonstrator at `examples/narrow_runtime.rs` and the cookbook recipe at `docs/guide/COOKBOOK.md` document the host-side ergonomics.

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

The example is exercised by the integration test [`tests/big_number_arithmetic.rs`](../../tests/big_number_arithmetic.rs) (`big_number_example_returns_1`) and documented in the guide at [`docs/guide/BIG_NUMBERS.md`](../guide/BIG_NUMBERS.md) with a discussion of the signed/unsigned caveats, the chained two-digit addition pattern, and the cross-references to the grammar and language-design sections.

Follow-on items that interact but remain out of scope:

- A standard-library `BigInt` type with arbitrary precision and the full arithmetic surface. The worked example demonstrates the underlying pattern; a fully-featured `BigInt` is its own subsystem.

The `Op::CheckedDiv` and `Op::CheckedMod` follow-on landed separately: the checked construct's `/` and `%` paths now route through dedicated opcodes that surface the `i64::MIN / -1` and `i64::MIN % -1` corners through the standard pattern-arm dispatch.

## B19. `Multiword<N>` parametric bignum type

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

// Bit shift operators with a shift amount of Word type.
let l = a << 8;
let r2 = a >> 16;

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

### Why no new opcodes are needed

- The checked-arithmetic opcodes (`Op::CheckedAdd` / Sub / Mul / Div / Mod) already produce the `(high, low, flag)` triple that the cascade consumes. The pattern-arm `ok` / `overflow` / `underflow` dispatch surfaces the flag through bytecode without an extra opcode.
- `Op::ArrayIndex` and `Op::NewArray` handle the internal array storage.
- Local slots and `Op::GetLocal` / `Op::SetLocal` carry the intermediate carries, borrows, and partial products between digit steps.
- `Op::If` and `Op::Loop` are sufficient for the comparison short-circuit, the Knuth D adjustment step, and the variable shift loop.

### Phased implementation plan (for the eventual implementation)

| Phase | Scope | Approximate Rust-side effort |
|-------|-------|----------------------------|
| 1 | Lexer + parser + AST + type checker for `Multiword<N>`, tuple constructor, `(...) as Multiword<N>` cast, indexing | ~600 lines |
| 2 | `+`, `-`, all six comparison operators | ~500 lines |
| 3 | `*` (schoolbook with carry plumbing) | ~300 lines |
| 4 | `/`, `%` (Knuth Algorithm D unrolled) | ~400 lines |
| 5 | `<<`, `>>` (constant amount first; variable amount as a stretch) | ~300 lines |

Each phase ends with end-to-end integration tests at N = 2 (128-bit on the default i64 runtime), N = 3 (192-bit), and N = 4 (256-bit). Earlier phases unblock testing of later phases.

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
| 7a | Wire format specification (`docs/architecture/WIRE_FORMAT.md`) plus `wire_format` module: framing header layout, four-byte opcode records with parity, eight-byte operand pool entries with type tag and parity, opcode-id table, encoder, decoder, round-trip tests. Execution path remains on rkyv. | Done |
| 7b | `wire_format::module_to_wire_bytes` and `module_from_wire_bytes` round-trip an entire `Module` through the section-partitioned body (opcode stream + operand pool + rkyv-archived auxiliary body). New `WireChunk` and `WireAuxBody` types. Round-trip tests cover empty/minimal/branchy/pool-using/Stream programs plus BadMagic, BadChecksum, Truncated, and shebang paths. `Module::to_bytes` and `Module::from_bytes` continue to route through rkyv pending the Phase 7c cutover. | Done |
| 7c | Default `Module::to_bytes` / `Module::from_bytes` / `Module::access_bytes` cut over to the wire-format codec; `access_bytes` returns `&ArchivedWireAuxBody`; VM zero-copy and ops-decode walk the section-partitioned body. `op_from_archived`, the legacy 32-byte framing header, and the legacy CRC residue constants retired. Golden bytes test refreshed to the V0.2.0 byte sequence; `zero_copy_demo.kel.bin` fixture regenerated. | Done |
| 8 | Documentation alignment with the V0.2.0 ISA: FAQ and cookbook text section rewritten; closure entries in FAQ and WHY_REJECTED point at the type-checker rejection rather than the load-time verifier; bundled-natives surface updated to reflect `register_utility_natives` shrinking to `println`. `BYTECODE_VERSION` re-affirmed at 1. `Archive`, `Serialize`, `Deserialize` derives dropped from `Module`, `Chunk`, and `Op` now that the wire format owns serialization. Stale piano_roll `.kel.bin` fixtures removed from the repo. | Done |
