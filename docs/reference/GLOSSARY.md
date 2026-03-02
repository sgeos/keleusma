# Glossary

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

Key terminology used in the Keleusma documentation and source code.

**Atomic function** -- A function declared with the `fn` keyword that must terminate without yielding. May call other atomic functions and native functions.

**Bytecode** -- The compiled representation of a Keleusma program. A sequence of instructions executed by the virtual machine.

**Chunk** -- A compiled function in the bytecode representation. Contains an instruction sequence, constant pool, struct templates, and metadata.

**Coroutine** -- An execution context that can be suspended via yield and resumed by the host. Keleusma scripts are coroutines.

**Guard clause** -- A boolean condition attached to a function head using the `when` keyword. Evaluated after pattern matching succeeds. Limited to comparisons and logical operators.

**Host** -- The Rust application that embeds the Keleusma VM, registers native functions, and drives coroutine execution by calling `call()` and `resume()`.

**Keleusma** -- A Total Functional Stream Processor that compiles to bytecode. The name derives from the Greek word for a command or signal, specifically the rhythmic calls used by ancient Greek rowing masters to coordinate oar strokes.

**Loop function** -- A function declared with the `loop` keyword that never exits. Must yield on every iteration. Exactly one per script. Serves as the coroutine entry point.

**Module** -- The compiled output of the compiler. Contains chunks for compiled functions, an entry point index, and enum definitions.

**Multiheaded function** -- Multiple function definitions with the same name and arity that form a single logical function. Dispatch selects the first head whose pattern matches the arguments.

**Native function** -- A Rust function registered by the host and callable from Keleusma scripts. Registered via `vm.register_native()` or `vm.register_native_closure()`.

**Opaque type** -- A host-defined type that scripts can pass through function calls but cannot destructure or inspect. Recognized from the native function registry.

**Pipeline** -- The `|>` operator that passes the result of the left expression as an argument to the right function call. Supports placeholder `_` for argument position control.

**Productive divergent function** -- A function declared with the `loop` keyword. It diverges by never exiting, but is productive because it yields a value on every iteration.

**Span** -- A source location record containing byte offsets, line number, and column number. Attached to tokens and AST nodes for error reporting.

**Total function** -- A function guaranteed to terminate, assuming called functions return. Both `fn` and `yield` functions are total. Loop functions are not total but are productive.

**Value** -- The runtime representation of data in the VM. An enum with variants: Unit, Bool, Int, Float, Str, Tuple, Array, Struct, Enum, and None.

**Yield** -- The mechanism by which a coroutine suspends execution, sends a value to the host, and receives a value when resumed. Expressed as `let input = yield output;` in source code.

**Yield contract** -- The agreement between a loop or yield function and the host about the types exchanged. Defined by the parameter type representing input from the host and the return type representing output to the host. Propagating yield functions must share the same contract.
