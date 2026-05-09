# Backlog Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Deferred decisions for future consideration. These are explicitly out of scope for the current development phase.

## B1. Hindley-Milner type inference

Full type inference would reduce annotation burden by allowing the compiler to deduce types from usage rather than requiring explicit declarations. Deferred due to implementation complexity and the current lack of generic types in the language.

## B2. Traits or generic type parameters

Traits or generic type parameters would enable polymorphic functions and reusable abstractions across different data types. Deferred to keep the VM and compiler simple during early development.

## B3. Closures or anonymous functions

Closures or anonymous functions would enable higher-order programming patterns such as callbacks and inline transformations. Deferred to keep the VM simple. Multiheaded function dispatch serves as a partial alternative for pattern-based dispatch.

## ~~B4. Hot code swap implementation~~ (Resolved as R29)

Hot code swap is implemented through `Vm::replace_module`. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. See R29 in [RESOLVED.md](./RESOLVED.md).

## ~~B5. Structural verification implementation~~ (Resolved as R22, R23)

Structural verification is implemented. See R22 and R23 in [RESOLVED.md](./RESOLVED.md).

## B5. Static string discipline

String values currently use `Value::Str(String)`, which is heap-allocated and variable-length. The V0.0-M5 milestone splits this into `Value::StaticStr` and `Value::DynStr` with the static-strings-anywhere and dynamic-strings-arena-only discipline. Anything beyond the minimum coherent design, namely surface-language string concatenation, formatting, slicing, or other variable-cost operations, is deferred. Keleusma is not a value-add for string work, so further string features are recorded here for future consideration only.

## B6. String interpolation

String interpolation is not needed for a control language. Keleusma scripts primarily produce structured enum values and numeric outputs, not formatted strings. If formatting is needed, the host can provide native functions for string construction.

## B7. Error propagation through yield

Allowing yield to return `Result<T, E>` so the host can signal errors to the script would enable bidirectional error handling. Deferred due to type system complexity and the need to define error recovery semantics at the language level.

## B8. VM allocation model

Should the VM allocate per-script or share an arena across all active scripts? Currently each VM instance is independent with its own heap allocations. A shared arena could reduce allocation overhead for hosts running many concurrent scripts, but would add complexity to lifetime management.

## B9. Hot update of yielded static strings

A static string in the dialogue type B is a pointer or index into rodata. The host may hold a yielded value containing a static string across a hot update boundary. After the hot update, the rodata changes, and the held pointer or index may become invalid. Two resolution paths exist. Host responsibility, namely the host is told to consume or copy yielded static strings before allowing a hot update. Eager resolution, namely the yield path materializes static strings into host-owned memory before returning. The user has noted this concern and asked it be tracked alongside future work on precompiled code and Keleusma type interop.

The precompiled-code work resolved in R39 implements path A, where the parsed Module owns heap-allocated string data. Yielded static strings under path A are owned `String` values, so the cross-update lifetime concern does not apply. The concern returns under path B (P10), where yielded static strings are byte-offset references into a specific bytecode buffer.

## B10. Portability and target abstraction

Keleusma should eventually be portable across architectures from the 6502 to ARM64. This requires several substantial design extensions. The type system gains `word`, `byte`, `bit`, and `address` primitives whose sizes and alignments are target-defined. The compiler accepts a target descriptor as input. The runtime representation of `Value` becomes target-aware, with the current 64-bit-tagged-union form unsuitable for 8-bit and 16-bit targets. The block-structured ISA itself is target-portable in principle, with code generation backends producing target-specific assembly or machine code. The synchronous-language tradition uses a comparable approach in Lustre and SCADE, where target-independent intermediate representations feed into target-specific backends. Recorded for future conversation. This entry interacts with B5 (static strings), B9 (hot update of yielded static strings), and the precompiled-code question. The triple shares a common theme of cross-environment portability of Keleusma artifacts.

The precompiled-code question is partially addressed by R39 and the wire format established there. The bytecode loading API now accepts any addressable byte slice including `.rodata`. Full zero-copy execution from `.rodata` and the broader portability work remain open under P10 and this entry.
