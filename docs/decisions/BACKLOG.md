# Backlog Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Deferred decisions for future consideration. These are explicitly out of scope for the current development phase.

## ~~B1. Hindley-Milner type inference~~ (Resolved)

Hindley-Milner is in place in `src/typecheck.rs`.

Foundation. The `Type` enum carries a `Var(u32)` variant. `Subst` maps type variables to types. `unify` implements Robinson's algorithm with the occurs check. `VarGen` allocates fresh type variables. The typing context carries the substitution and variable allocator across a function check.

Integration. `types_compatible` calls `unify` and records relationships in the substitution. Unannotated positions that previously returned `Type::Unknown` now allocate fresh type variables, so constraints propagate across let bindings, function calls, returns, and conditional branches. The substitution-application pass at end of `check_function` resolves locals to their inferred types and rolls back per-function variables so cross-function checking remains independent.

Generic functions (B2) reuse the same machinery: each generic call site instantiates the function's abstract type variables with fresh per-call variables before unifying with actual arguments.

The `Type::Unknown` sentinel is retained as a permissive transitional anchor for runtime-only dispatch positions (such as native function call results without declared signatures). Removing it would require declaring native signatures, which is recorded as future work in the typecheck module documentation.

## ~~B2. Generic type parameters and trait bounds~~ (Resolved for declaration and bound enforcement)

Generic functions, structs, enums, traits, and trait bounds are all parsed, represented in the AST, and validated by the type checker.

Surface syntax. `fn name<T, U>(args) -> ret { body }`, `struct Name<T, U> { fields }`, `enum Name<T, U> { variants }`, `trait Name { fn method(args) -> ret; }`, `impl Trait for Type { method definitions }`, and `fn name<T: Trait1 + Trait2>(...)` for bounds.

AST. `FunctionDef`, `StructDef`, and `EnumDef` carry `type_params: Vec<TypeParam>`. `TypeParam` carries `bounds: Vec<String>` for trait constraints. `TraitDef` and `ImplBlock` are top-level declarations. `TypeExpr::Named` carries `Vec<TypeExpr>` for generic instantiation references.

Type checking. Each generic declaration's signature records the abstract `Type::Var` allocated per type parameter and the declared bounds. At call sites, fresh per-instance variables are allocated and the substitution is applied to declared parameter/field/payload types before unifying with provided values. After unification, the per-call fresh variables resolve through the active substitution and each bounded type parameter checks against the trait `impls` registry. `Type::Struct(String, Vec<Type>)` and `Type::Enum(String, Vec<Type>)` carry per-instance type arguments. Field access on a generic struct constructs a per-instance substitution from the abstract variables to the captured arguments and applies it to the declared field type.

Compilation and runtime. Keleusma's `Value` enum is runtime-tagged. Bytecode operations dispatch on the tag, so generic chunks work for any concrete type without compile-time monomorphization. Operations constrained by trait bounds rely on runtime tag dispatch when the method itself is invoked; the bound enforcement at compile time prevents calls with types that lack the required impl.

The remaining future work tracked under this entry.

- Trait method dispatch. Currently impl methods are parsed and registered but the receiver-based method call (`x.method(args)` resolving to the impl for x's type) is not yet wired through the compiler and runtime. The bound enforcement at the call site prevents incorrect calls; the method body invocation is deferred.
- Validation of impl method signatures against the trait declaration.
- Compile-time monomorphization as a performance optimization. The current dispatch is correct but pays a runtime tag-check cost on every operation. Monomorphization would specialize each generic chunk per (function or type, type_args) pair and elide the dispatch.

These are future-session work and do not block the current declaration and bound enforcement functionality.

## B3. Closures or anonymous functions

Closures or anonymous functions would enable higher-order programming patterns such as callbacks and inline transformations. Deferred to keep the VM simple. Multiheaded function dispatch serves as a partial alternative for pattern-based dispatch.

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
