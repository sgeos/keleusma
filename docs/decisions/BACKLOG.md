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

Compilation and runtime. Keleusma's runtime-tagged `Value` enum dispatches polymorphically. Generic chunks work for any concrete type. Impl methods are emitted as regular chunks under mangled names. Receiver-style method dispatch (`x.method(args)` resolving to the impl for `x`'s type) requires either monomorphization-rewriting at compile time or runtime lookup; the parser does not yet have a method-call syntax distinct from struct field access.

The remaining future work tracked under this entry.

- Method call surface syntax (`x.method(args)`). Parser change plus resolution. Pairs naturally with monomorphization which makes the receiver type concrete.

## ~~B2.4 Compile-time monomorphization~~ (MVP landed)

Monomorphization specializes generic functions per concrete type instantiation. The MVP is implemented in `src/monomorphize.rs` and runs between type checking and compilation in `compile()`.

What lands.

- Call-graph traversal from non-generic functions. The pass walks every call site of a generic function and infers the concrete type arguments from literal arguments and locals with declared types.
- Specialization generation. Each `(function, type_args)` pair clones the generic function and substitutes the abstract type-parameter names with the concrete `TypeExpr` throughout the parameter list, the return type, and the function body (let bindings, casts, struct constructions, and so on).
- Trait method resolution within specializations. After substitution, the receiver of a method call has a concrete type. The compiler's existing `MethodCall` resolution path looks up the impl's mangled name `Trait::TypeHead::method` in the function map and emits a direct call.
- Output. The compiler emits the monomorphic specializations and drops the original generic functions whose specialization was generated. Calls in the program are rewritten to point to the specializations through the mangled names.
- Re-typecheck after monomorphization validates the specialized bodies under their concrete types, which is what allows generic-receiver method calls to resolve.

End-to-end example. `examples/monomorphize_generic_method.rs` compiles and executes `fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }` where the body's method call resolves only after monomorphization specializes `use_doubler` for `T = i64`.

Remaining work.

- Generic structs and enums. The current pass handles only generic functions. Generic struct construction and field access continue to use runtime tag dispatch. Specialization of structs requires emitting per-instantiation copies of struct templates and rewriting field offsets accordingly. Estimated 3 to 5 hours.
- Inference reach. The MVP infers concrete type arguments from literal arguments and locally-declared identifiers. Cases where the type argument flows through a chain of function calls or through generic-receiver method results are not yet handled and the call site is left generic. Estimated 2 to 4 hours.
- Polymorphic recursion guard. If a generic function calls itself with type arguments derived from its own type parameters, the pass would loop indefinitely. The MVP relies on the call graph being finite and the test suite not exercising polymorphic recursion. A guard against unbounded specialization should be added before the feature is considered production-ready.
- Pruning unused generic functions. The MVP retains generic functions that have at least one specialization; non-specialized generics remain in the program as dead code that the compiler emits but the runtime never enters. A pruning pass would remove these.

## B3. Closures and anonymous functions (parser and type-check landed; runtime deferred)

Surface syntax `|args| body` and `|args| -> ret { body }` is parsed and the AST carries `Expr::Closure { params, return_type, body, span }`. The lexer distinguishes the bare `|` token (`Bar`) from the pipeline operator `|>` (`Pipe`). The type checker walks the closure body in a fresh scope where parameters are bound to fresh type variables or their declared types and returns a fresh type variable for the closure expression. Monomorphization recurses through closure bodies during substitution and call-site rewriting.

The runtime side is deferred. The compiler currently rejects closure expressions at compile time with a clear "closure runtime is not yet implemented" error so the syntax surface is stable while the implementation evolves.

Remaining work.

- Runtime representation. Add a `Value::Func(u16)` variant carrying the chunk index of the compiled closure body, and a closure type.
- Compiler hoisting. Each closure expression compiles to a top-level chunk with a synthetic mangled name. The closure expression evaluates to `Value::Func(chunk_idx)`.
- Indirect call. Add `Op::CallIndirect(u8)` that pops `arg_count` args plus a `Value::Func` from the operand stack and invokes the carried chunk.
- Environment capture. The body resolves identifiers against the closure's parameter scope plus a copied environment captured from the enclosing scope at closure creation. The captured environment becomes part of the closure value.
- Surface syntax for invocation. Calls of the form `f(args)` where `f` is a closure-typed local resolve to `CallIndirect` rather than the existing name-based `Call`. The parser may need a separate `IndirectCall` AST variant or the compiler resolves at compile time based on the receiver's known type.

Estimated implementation effort. Six to ten hours for the runtime including capture.

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
