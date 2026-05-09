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

Inference reach extension. `infer_arg_type` now resolves the type of function calls (through a function-return-type map), tuple and array literals, cast expressions, enum variants, the first-arm of if/match expressions, field access expressions, tuple-index expressions, and array-index expressions. Generic call sites whose arguments use these shapes specialize correctly. Field-access inference threads a struct table through the rewrite chain and resolves `o.field` against the struct's declared field type, applying per-instance type-argument substitution when the receiver carries concrete type arguments. Abstract field types (those whose declared type is exactly one of the struct's type parameters and the receiver has no type arguments) are guarded against erroneous propagation. Tuple-index inference reads the indexed element type from the inferred tuple type. Array-index inference returns the array's element type regardless of the index value.

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
`examples/closure_recursive.rs` computes `fact(5) == 120`. Bytecode
version is bumped to `7`.

Capture by reference disposition. Capture by reference is not meaningful in Keleusma's pure-functional surface. The language's `let` bindings are immutable by design. There is no surface assignment operator that mutates a previously bound local, which means a captured local cannot diverge from the captured snapshot regardless of whether the capture is by value or by reference. The only mutable mechanism is the data segment, which is accessed through `data.field` and `data.field = expr` syntax that is independent of closure capture. A closure that wants to mutate shared state therefore reads and writes data segment slots directly. Capture by reference would only matter in a language with mutable locals, which Keleusma intentionally does not have. The item is therefore closed as not applicable rather than deferred.

## ~~B4. Hot code swap implementation~~ (Resolved as R29)

Hot code swap is implemented through `Vm::replace_module`. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. See R29 in [RESOLVED.md](./RESOLVED.md).

## ~~B5. Structural verification implementation~~ (Resolved as R22, R23)

Structural verification is implemented. See R22 and R23 in [RESOLVED.md](./RESOLVED.md).

## B5b. Static string discipline extensions

String values use the two-string-type discipline of `Value::StaticStr` and `Value::DynStr` with the host-owned arena boundary type `Value::KStr` for stale-pointer detection. The minimum coherent design is in place. Anything beyond, namely surface-language string concatenation, formatting, slicing, or other variable-cost operations, is deferred. Keleusma is not a value-add for string work, so further string features are recorded here for future consideration only.

## B6. String interpolation

String interpolation is not needed for a control language. Keleusma scripts primarily produce structured enum values and numeric outputs, not formatted strings. If formatting is needed, the host can provide native functions for string construction.

## B7. Error propagation through yield

Allowing yield to return `Result<T, E>` so the host can signal errors to the script would enable bidirectional error handling. Deferred due to type system complexity and the need to define error recovery semantics at the language level.

## B8. VM allocation model

Should the VM allocate per-script or share an arena across all active scripts? Currently each VM instance is independent with its own heap allocations. A shared arena could reduce allocation overhead for hosts running many concurrent scripts, but would add complexity to lifetime management.

## ~~B9. Hot update of yielded static strings~~ (Resolved structurally)

The lifetime concern is structurally avoided in the current implementation. `Value::from_const_archived` materializes archived `StaticStr` constants into owned `String` values at the moment they are pushed onto the operand stack. Yielded values that contain a `Value::StaticStr` therefore hold owned heap data that is independent of the bytecode buffer. A hot update that swaps the buffer through `Vm::replace_module` does not affect the host's retained yield value because the string bytes were already copied out at the lift boundary.

Eager resolution at the lift boundary is the resolution path B from the original design. The trade-off is a heap allocation per `StaticStr` push, which is acceptable for the dialogue surface where yielded values cross out of the VM. Future zero-copy yield paths that retain `&ArchivedString` references in `Value` would re-introduce the concern; if they are pursued, the host-responsibility model from path A is the alternative.

## B11. Per-op decode optimization for zero-copy execution

The zero-copy execution path reads each instruction through `op_from_archived(&chunk.ops[ip])`, which performs a discriminant match per fetch. The cost is one match arm and a small payload copy on hot loops. For very hot bytecode this could become measurable.

Two candidate optimizations.

A. Cache a decoded `Vec<Op>` per chunk at VM construction. Hot path becomes a direct slice index. Cost: heap allocation proportional to chunk size at construction; defeats zero-copy for the operation slice but preserves zero-copy for constants and string data.

B. Specialize the dispatch loop on a small set of hot opcodes through a separate dispatch table generated from the archived form. Cost: more complex codegen; benefit depends on opcode distribution in real workloads.

Deferred until profiling identifies the dispatch as a hot path on real workloads. The current implementation is correct and the cost is bounded by the structural verifier's per-op accounting, so this is a performance enhancement rather than a correctness concern.

## B10. Portability and target abstraction

Keleusma should eventually be portable across architectures from the 6502 to ARM64. This requires several substantial design extensions. The type system gains `word`, `byte`, `bit`, and `address` primitives whose sizes and alignments are target-defined. The compiler accepts a target descriptor as input. The runtime representation of `Value` becomes target-aware, with the current 64-bit-tagged-union form unsuitable for 8-bit and 16-bit targets. The block-structured ISA itself is target-portable in principle, with code generation backends producing target-specific assembly or machine code. The synchronous-language tradition uses a comparable approach in Lustre and SCADE, where target-independent intermediate representations feed into target-specific backends. Recorded for future conversation. This entry interacts with B5 (static strings), B9 (hot update of yielded static strings), and the precompiled-code question. The triple shares a common theme of cross-environment portability of Keleusma artifacts.

The precompiled-code question is partially addressed by R39 and the wire format established there. The bytecode loading API now accepts any addressable byte slice including `.rodata`. Full zero-copy execution from `.rodata` and the broader portability work remain open under P10 and this entry.
