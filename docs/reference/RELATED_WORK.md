# Related Work

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

This document positions Keleusma within the established landscape of synchronous reactive languages, stream processing theory, verified bytecode formats, high-assurance verification, and language-based information-flow security. Each section explains the relationship to Keleusma, identifies what the project adopts, adapts, or defers from prior work, and provides citations to the relevant literature. A formal bibliography appears at the end.

## 1. Synchronous Reactive Languages

Keleusma belongs to the family of synchronous reactive languages, a class of programming languages designed for deterministic, real-time reactive systems. The foundational insight of synchronous languages is the synchronous hypothesis: outputs are produced simultaneously with inputs, and all computation within a logical tick completes before the next tick begins [L1, SY1]. This hypothesis makes programs amenable to static timing analysis because each tick has a bounded and predictable execution cost.

The three principal synchronous languages are Lustre [L1, L2], a declarative dataflow language for reactive systems; Esterel [E1], an imperative synchronous language with concurrent composition; and Signal [S1], a relational synchronous language that defines systems as constraints on signal clocks. Halbwachs provided a book-length treatment synthesizing the synchronous approach to reactive system design [L3]. Benveniste et al. published a retrospective survey covering twelve years of development and industrial adoption [SY1].

SCADE is a widely used industrial realization of the synchronous approach. SCADE 6 combines Lustre-style dataflow with control structures from Esterel [SC1].

**Relationship to Keleusma.** Keleusma shares the synchronous hypothesis with Lustre, Esterel, and SCADE. The yield domain (control clock) corresponds to the synchronous tick: all computation between two YIELD points completes within a bounded number of instructions. The RESET domain (phase clock) provides a coarser temporal boundary analogous to mode changes in SCADE.

Keleusma differs from the established synchronous languages in several ways. It is a bytecode VM language rather than compiling to automata or native C code. It does not support multi-clock domains or concurrent composition. It targets embedded scripting (audio engines, game logic) rather than high-assurance control systems as its primary application domain. Its claims of suitability for high-assurance applications are design aspirations informed by synchronous language principles, not verification status. See Section 7 for a discussion of the gap between current implementation and high-assurance verification.

## 2. Coalgebra and Stream Processing

Keleusma's stream processing model draws on coalgebraic foundations. The formulation of productive divergent functions as coalgebras of the form `f : Stream<A> -> Stream<B>` derives from Rutten's theory of universal coalgebra [C1] and coinductive stream calculus [C2]. Rutten established coalgebra as the mathematical dual of algebra, providing a categorical framework for modeling state-based systems. His coinductive calculus of streams based on stream derivatives enables coinductive proofs and definitions to be formulated as behavioral differential equations [C2].

The productivity invariant (every control path from STREAM to RESET must encounter at least one YIELD) is a concrete instance of productivity for corecursive definitions. Endrullis et al. studied the decidability of productivity for stream definitions, demonstrating that static productivity checking is possible for suitably restricted stream definitions [C4]. Keleusma's restriction to block-structured control flow places it within a decidable subclass where productivity can be verified by a single-pass analysis.

Abel and Pientka unified termination checking (for inductive data) and productivity checking (for coinductive codata) through sized types and copatterns [C3]. Their data/codata distinction corresponds directly to Keleusma's function categories: `fn` functions operate on finite data and must terminate (inductive), while `loop` functions produce infinite streams and must be productive (coinductive). The `yield` category occupies an intermediate position as non-atomic total functions that may interact with the host but must eventually return.

**Relationship to Keleusma.** Keleusma adopts the coalgebraic stream model as its theoretical foundation for loop functions. The productivity verification pass (`analyze_yield_coverage` in `src/verify.rs`) is a pragmatic implementation of productivity checking for the restricted block-structured control flow of the bytecode ISA. The implementation does not use sized types or copatterns, but achieves a similar guarantee through structural analysis of the finite control flow graph.

## 3. Block-Structured Bytecode Validation

Keleusma's block-structured ISA (R17) uses the same design principle as WebAssembly: structured control flow enables single-pass validation without constructing a full control flow graph [W1]. Haas et al. demonstrated that restricting a bytecode format to block-structured control flow avoids the fixpoint computations required by languages like Java bytecode, enabling efficient validation and compilation to SSA form [W1]. WebAssembly received the PLDI 2017 Distinguished Paper Award in part for this design insight.

Watt provided a mechanized Isabelle specification for WebAssembly with a verified executable interpreter, type checker, and fully mechanized proof of type system soundness [W2]. This work exposed several issues in the official WebAssembly specification and demonstrates that mechanized verification of bytecode language specifications is both feasible and valuable.

**Relationship to Keleusma.** Keleusma's block-structured control flow (If/Else/EndIf, Loop/EndLoop, Break/BreakIf) follows the same structural principle as WebAssembly. Both formats prohibit flat jumps, ensuring that all forward and backward control flow transfers are mediated by matching block delimiters. This enables the structural verifier (`verify()` in `src/verify.rs`) to validate programs in a single linear pass.

Keleusma differs from WebAssembly in purpose and scope. WebAssembly is a portable execution format with a structural stack-based type system. Keleusma is a coroutine-based scripting language with nominal types, bidirectional yield, and streaming semantics. WebAssembly does not have yield, stream, or reset primitives.

Soundness of Keleusma's verification passes has not been formally proven. Watt's mechanized verification of WebAssembly [W2] provides a model for what such a proof would require: a formal specification of the bytecode semantics, a formal statement of the verification rules, and a machine-checked proof that well-verified programs satisfy the stated safety properties.

## 4. Worst-Case Execution Time Analysis

Worst-Case Execution Time (WCET) analysis is a well-studied problem in real-time systems. Wilhelm et al. published a comprehensive survey of static and dynamic methods for WCET analysis, covering abstract interpretation-based approaches, measurement-based approaches, and available tools [WC1]. Modern WCET analysis must account for pipeline effects, cache behavior, branch prediction, and interrupt latency on the target hardware [WC1].

Industrial WCET analysis tools include aiT (AbsInt), which uses abstract interpretation with formal cache and pipeline models to compute sound upper bounds on worst-case execution time directly from binary executables [WC2]. aiT was originally developed in collaboration with Airbus France and has been used for A380 flight control software validation. OTAWA provides an open-source framework for WCET analysis supporting multiple architectures [WC3]. Chronos performs detailed micro-architectural modeling including superscalar pipelines and instruction caches [WC4].

For bytecode-level WCET analysis, Schoeberl et al. demonstrated that WCET analysis is feasible at the bytecode level when the execution platform has predictable timing [WC5]. Their work combined a time-predictable Java processor (JOP) with WCET analysis at the bytecode level using integer linear programming.

**Relationship to Keleusma.** Keleusma's `Op::cost()` method and `wcet_stream_iteration()` function implement abstract opcode counting: each instruction is assigned a relative integer cost, and the worst-case total cost of one Stream-to-Reset iteration is computed by taking the maximum cost branch at each control flow join. This is a form of high-level WCET analysis that provides a sound bound on abstract execution cost.

However, abstract opcode cost does not directly correspond to wall-clock execution time. The relationship between abstract cost and real time depends on the host interpreter's execution characteristics, including the cost of dispatching each opcode, memory allocation patterns, and the host platform's cache and pipeline behavior. For high-assurance verification, a sound bound on real-time WCET requires either a time-predictable execution platform (as in [WC5]) or a validated mapping from abstract cost to physical time on the target hardware.

Keleusma's current WCET analysis is sufficient for soft real-time applications (audio engines, game scripting) where approximate cost bounds inform scheduling decisions. It is not sufficient for hard real-time verification without additional analysis of the execution platform. The cost weights are preliminary and subject to refinement.

## 5. Abstract Interpretation

Abstract interpretation, introduced by Cousot and Cousot [AI1], provides a general framework for static program analysis where program properties are computed as fixpoints over abstract lattice domains. The framework guarantees that if the abstract domain is chosen correctly, the analysis produces sound results: the abstract computation over-approximates all concrete executions.

**Relationship to Keleusma.** Two analyses in Keleusma are instances of abstract interpretation over finite lattices.

The productivity analysis (`analyze_yield_coverage` in `src/verify.rs`) operates over a two-element boolean lattice `{false, true}` representing whether all control flow paths have passed through at least one YIELD. At If/Else joins, the analysis takes the meet (AND) of both branches. At loop exits, it takes the meet of all break states. This is a forward abstract interpretation where the abstract state tracks a single boolean property.

The WCET analysis (`wcet_region` in `src/verify.rs`) operates over the natural numbers with maximum as the join operator. At If/Else joins, the analysis takes the maximum (worst case) of both branches. Along sequential paths, it sums costs. This computes the longest path cost through the block-structured control flow.

Both analyses terminate in a single pass because the block-structured ISA has no backward edges within the analyzed region (Stream to Reset). The absence of cycles within the analysis region means no fixpoint computation is required, which is a direct benefit of the block-structured ISA design.

## 6. Totality and Productivity Checking

Turner argued that functional programming should be total rather than partial, requiring a type-level distinction between data (finite, defined by constructors) and codata (potentially infinite, defined by observations) [T1]. This distinction enables static verification of termination for data-consuming functions and productivity for codata-producing functions.

Agda [T2] is a dependently typed programming language where all computations must terminate, enforced through structural recursion checking. Idris [T3] integrates totality checking into its type system, allowing programmers to mark functions as `total` with compiler-verified termination for recursive definitions and productivity for corecursive definitions.

Rocq (formerly Coq) [T4] is the most widely used proof assistant of the dependently-typed-total tradition. Rocq's `Fixpoint` mechanism admits structural recursion: a recursive call whose argument is syntactically smaller than the original parameter is accepted as a sound terminator. Rocq's `Function` and `Program Fixpoint` mechanisms generalise this to well-founded recursion with an explicit measure that strictly decreases. The `CoInductive` types and `cofix` corecursion handle the productive divergent case dually, with a `Guarded` discipline that enforces productivity. Rocq's combination of structural recursion plus coinductive productivity is the most rigorous treatment of the totality-versus-productivity distinction available in a production tool. Friedman and Eastlund's *The Little Prover* [T5] introduces the technique pedagogically in the ACL2 tradition that Rocq inherits.

The class of primitive recursive functions (Skolem, 1923; Kleene, 1952) constitutes the canonical example of total computable functions. Every primitive recursive function is total by construction because the recursion scheme guarantees termination. Not all total computable functions are primitive recursive, but the restriction to primitive recursion provides a decidable and well-understood subclass.

**Relationship to Keleusma.** Keleusma's three function categories map to Turner's data/codata distinction [T1]. The `fn` category corresponds to total (terminating) functions that operate on finite data. The `loop` category corresponds to productive corecursive definitions that produce infinite streams. The `yield` category bridges the two as non-atomic total functions that interact with the host but must eventually return.

Keleusma enforces totality through simpler mechanisms than Agda, Idris, or Rocq. Rather than dependent types or structural recursion checking, Keleusma prohibits all recursion (R4) and restricts loops to bounded ranges (`for i in 0..n`). This reduces totality checking to a syntactic property: any well-typed `fn` function without recursion or unbounded loops must terminate, assuming all called native functions return. The trade-off is reduced expressiveness: algorithms that require recursion must be supplied by the host as native functions.

The blanket recursion prohibition is broader than strictly necessary for totality. A structural-recursion check in the Rocq tradition would admit recursion over statically-sized data without breaking Keleusma's worst-case execution time and worst-case memory usage analyses, because the depth bound is static. The prohibition is conservative because Keleusma's bounded-resource analysis is conservative, not because totality requires it. A future-direction relaxation in the Rocq style is recorded as B22 in [`BACKLOG.md`](../decisions/BACKLOG.md); the dual coinductive-productivity refinement is B23.

The totality guarantee depends on an explicit trust boundary: host-registered native functions are assumed to be total (R9). If a native function diverges, the totality guarantee for any Keleusma function that calls it is invalidated. The documentation does not currently specify mitigation strategies for this trust boundary beyond declaring it.

## 7. Verification Maturity

Keleusma's design choices, being no recursion, a block-structured instruction set, statically bounded loops, and single-pass verification, reduce the verification burden and favor static analysis over runtime checking. The design is deliberately amenable to independent checking by a small verifier. Several properties remain design aspirations rather than achieved guarantees in the current implementation, and are recorded here as honest gaps.

- **Compiler correctness.** The compiler has no formal correctness proof. Establishing that the compiler preserves source semantics in the emitted bytecode would require formal verification or exhaustive testing with documented coverage.
- **Verifier soundness.** The structural verifier (`verify()`) is tested but not formally proven sound. A soundness proof would require a formal specification of bytecode semantics and a machine-checked proof that verified programs satisfy the stated safety properties.
- **Worst-case timing validity.** The abstract opcode cost model does not fully account for execution-platform characteristics. A demonstrably conservative worst-case timing bound would require either a time-predictable execution platform or a validated mapping from abstract cost to physical time.
- **Native function trust boundary.** Totality and the worst-case bounds depend on host-declared native function behavior. Closing this would require a contract mechanism for native functions with verifiable pre- and post-conditions.
- **Requirements traceability.** The current documentation provides design rationale but not full bidirectional traceability between requirements, design, implementation, and verification artifacts.
- **Structural coverage.** The test suite provides functional coverage but does not yet demonstrate structural coverage of the implementation.

These are the areas where additional formal-methods and evidence-generation work would raise verification confidence. The design's austerity is intended to make that work tractable.

## 8. Hot Code Update with Persistent State

Long-running systems frequently require updates to executable code without interrupting service. Hot code update is the general term for replacing program code while the program continues running. The literature distinguishes update of the code text alone, which is comparatively well understood, from update of code together with persistent mutable state, which raises additional questions about state migration, schema compatibility, and the temporal point at which the update takes effect.

Erlang and the Open Telecom Platform (OTP) provide the most extensive industrial precedent for hot code update. Armstrong's thesis describes the language and runtime design principles, including the multi-version code coexistence model in which two versions of any module may be loaded simultaneously [H1]. The OTP design principles formalize this through behaviors such as `gen_server`, where a `code_change` callback receives the previous state value and produces the new state value at the moment of the upgrade. Cesarini and Thompson document the engineering practice of hot code upgrade in production Erlang systems [H2]. The defining property of the OTP model is that the upgrade transition is mediated by a callback under the application's control, which permits arbitrary schema migration but introduces a trust boundary between the runtime and the application.

In the synchronous reactive language tradition, the closest analogue to hot code update is the mode change construct. SCADE 6 supports state machines with mode transitions in which the state of the source mode is either preserved or discarded according to whether the transition is weak or strong [SC1]. Maraninchi and Rémond's mode-automata extend Lustre with explicit mode constructs that compose with the synchronous data flow semantics, and define the formal semantics of mode transition with respect to the underlying state vector [H3]. The mode change boundary in SCADE corresponds closely to the RESET boundary in Keleusma. The state vector in SCADE corresponds closely to the data segment.

A distinct line of work concerns dynamic update of running C programs and operating system kernels. Arnold and Kaashoek's Ksplice provides automatic rebootless kernel updates by analyzing source patches and inserting redirection trampolines at quiescent points in the kernel call graph [H4]. Hayden, Smith, Denchev, Hicks, and Foster's Kitsune extends dynamic software update to general-purpose C programs by inserting update points into long-running loops and providing a state transformation language for migrating heap data across versions [H5]. The literature on kernel and C-program live update emphasizes safe points and stack quiescence as preconditions for an update to be applied. The RESET boundary in Keleusma is by construction such a safe point.

The conventional executable memory layout, with sections for code, read-only data, preinitialized read-write data, and zero-initialized read-write data, provides the engineering vocabulary for the Keleusma memory model. The four sections are commonly written as `.text`, `.rodata`, `.data`, and `.bss`. This layout originates in the Unix linker and assembler tradition and is codified in the System V Application Binary Interface and in the Executable and Linkable Format. Keleusma adopts this analogy as its organizing frame for runtime memory.

**Relationship to Keleusma.** Keleusma adopts the multi-version code coexistence model from Erlang and OTP, with the host responsible for installing and selecting code versions. The RESET boundary serves as the point at which an update takes effect, analogous to a strong mode transition in SCADE. The data segment is conceptually the state vector of a SCADE mode automaton or the persistent state of an OTP `gen_server`. The four memory regions of the Keleusma runtime correspond directly to the four conventional executable sections.

| Keleusma region | Conventional analogue | Properties |
|---|---|---|
| Bytecode chunks | `.text` | Immutable, double-buffered, swappable at RESET. |
| Constant pool and templates | `.rodata` | Immutable, swappable at RESET alongside text. |
| Data segment | `.data` | Mutable, persistent across yield and reset, host-owned, schema may change at hot update. |
| Arena and operand stack | `.bss` | Mutable, ephemeral within a stream phase, cleared at RESET. |

Keleusma differs from Erlang and OTP in two specific ways. The host owns the data segment storage rather than the runtime. There is therefore no `code_change` callback within the script. Instead, the host is responsible for supplying whatever data segment instance is appropriate at each RESET, including possibly a migrated instance, a freshly initialized instance, or the unchanged previous instance. This is referred to as Replace semantics in the architecture documents. Schema may change arbitrarily across hot updates because the script never observes any cross-update invariant on the data segment beyond what the host elects to provide.

Keleusma differs from SCADE mode automata in that the schema of the state vector is permitted to change across the mode transition when that transition coincides with a hot code update. SCADE's mode automaton model fixes the state vector at code generation time. Keleusma's model places this responsibility on the host, which permits schema flexibility at the cost of moving the verification responsibility to the host as well. This division of concerns is consistent with the broader Keleusma philosophy in which the script is austere and auditable while the host is rich and responsible for orchestration.

Keleusma differs from Ksplice and Kitsune in that update points are explicit and structurally enforced rather than inferred. RESET is the only update point. Stack quiescence is trivial because the operand stack is empty at RESET by construction.

The atomicity of the swap in Keleusma is logical only. The new code text must be resident in memory before it is eligible for installation. The host writes the candidate slot and the VM reads it at the next RESET. Crash atomicity, namely recovery from a fault that interrupts the swap, is the responsibility of the host platform. The Ksplice and Kitsune literature treats this question in detail and provides a model for what would be required of the host if Keleusma were deployed in a context where crash atomicity is required.

## 9. Embedded Scripting Languages and Static Marshalling

Embedded scripting in Rust applications is a well-populated design space. Rhai [E2] is the closest comparable for general-purpose embedded scripting in Rust, with substantial ergonomic affordances for host type registration through `Engine::register_type`, `Engine::register_fn`, and the `#[export_module]` proc macro. The Rhai approach centers on a `Dynamic` runtime value that carries `Box<dyn Any + Send + Sync>` plus trait-driven marshalling that converts arbitrary Rust function signatures into the engine's uniform call convention. The dynamic approach maximizes flexibility at the cost of unsafe-adjacent pointer manipulation and runtime type-erasure overhead.

Lua bindings for Rust, including mlua and rlua, provide similar ergonomics through the `UserData` trait pattern, in which arbitrary Rust types are wrapped and exposed to Lua scripts with method bindings. Like Rhai, the design relies on `Any` plus runtime type checks.

WebAssembly host bindings, in contrast, marshal values across the boundary through a fixed type system at the boundary surface. The wasm-bindgen crate generates static marshalling code at compile time, mediating between Rust types and JavaScript values without dynamic dispatch on the host side.

**Relationship to Keleusma.** Keleusma adopts the static marshalling approach. The discipline of fixed-size, fixed-layout interop types, established for the data segment and extended to native function arguments and return values, makes the dynamic `Box<dyn Any>` mechanism unnecessary. The `KeleusmaType` trait provides the marshalling contract. The `#[derive(KeleusmaType)]` macro generates implementations for host structs and enums whose fields and variants compose admissible types. The `IntoNativeFn` trait family produces registration glue from ordinary Rust function signatures.

Keleusma differs from Rhai in three specific ways. The interop value space is closed at compile time rather than open at runtime, which trades flexibility for static analyzability. There is no boxing of host types because every interop value has a statically known shape. The marshalling layer is amenable to qualification under safety standards because no `Box<dyn Any>` cast site requires trust at runtime.

Keleusma differs from wasm-bindgen in scope. Keleusma is a complete embedded scripting runtime with its own bytecode and verifier. The marshalling layer is comparable in approach but operates within the closed `Value` representation rather than across a portable binary interface.

The static marshalling approach has precedents in the typed embedded scripting tradition, including the Lua bindings used in Tarantool and the typed effects in Koka, but Keleusma's combination of synchronous reactive semantics with a typed marshalling layer is, to the author's knowledge, novel.

Rex [E3] is the closest contemporary parallel to Keleusma's embedded-scripting framing. Both languages are pure functional, target embedding in Rust applications through host-injected native functions, use Hindley-Milner inference, and treat the native-function boundary as the single point where effects enter the system. The two designs diverge in three substantive ways. Rex targets scientific workflows and high-performance computing orchestration with implicit parallel execution via tokio; Keleusma targets embedded real-time and high-assurance use cases on `no_std + alloc` hosts with single-threaded execution. Rex has no worst-case execution time or worst-case memory usage bounds; Keleusma's bounded-resource discipline is its defining property. Rex treats LLM code-generation as an explicit first-class design goal with a published `LLMS.md` guidance document; Keleusma's [`LLM_USAGE.md`](../../book/src/LLM_USAGE.md) is the parallel artefact, added after operator-level conversations with the Rex maintainer surfaced the convention.

Rex's `CallSite` mechanism, where every native invocation in `rex-engine/src/native_fn.rs::apply_at_site` carries an opaque token through the runtime, is a useful precedent for a future Keleusma per-call-site identifier (B-numbered backlog item; see `tmp/call_site_identifier.md` for the design spec). Both projects independently converged on the static-marshalling approach (Rex's `#[derive(Rex)]` mirrors Keleusma's `#[derive(KeleusmaType)]`) and on host-injected native functions as the unique effect boundary, suggesting the architectural pattern is broadly correct for pure-functional embedded scripting.

## 10. Language-Based Information-Flow Security

Keleusma's information-flow labels, added in V0.2.0, place it within the tradition of language-based information-flow security. This field studies how to constrain, by static or dynamic means, the way data of differing confidentiality may propagate through a program.

The foundational model is Denning's lattice model of secure information flow [IF1], which represents security classes as a lattice and states the condition under which information may flow from one class to another. Denning and Denning followed with a certification mechanism that checks a program's information flows at compile time against the lattice ordering [IF2]. Volpano, Smith, and Irvine recast information-flow checking as a type system and gave the first proof that a well-typed program satisfies noninterference, the property that publicly observable outputs do not depend on confidential inputs [IF3].

Two language realizations are the closest comparators. Jif extends Java with the decentralized label model of Myers and Liskov [IF4], in which labels name principals and the readers each principal permits, and the JFlow work showed that such checking can be performed mostly statically [IF5]. FlowCaml extends OCaml with information-flow types and full type inference over a lattice of security levels [IF6]. Sabelfeld and Myers surveyed the field and fixed its vocabulary, including the distinction between explicit flows through assignment and implicit flows through control structure, and the role of declassification as a controlled and audited escape from noninterference [IF7].

Information-flow control is not confined to static type systems. LIO performs dynamic information-flow control in Haskell through a labeled IO monad that tracks a floating label at run time [IF8]. Perl's taint mode is the most widely deployed dynamic mechanism, marking data derived from external input as tainted and refusing tainted data at sensitive operations until it is laundered [IF10]. SecVerilog carries information-flow labels into hardware description and checks timing-sensitive flows in a Verilog-derived language at synthesis time [IF9].

**Relationship to Keleusma.** Keleusma adopts the lattice model [IF1] and the type-system formulation [IF3]. Labels are user-defined names, the label set carried by a type is ordered by subset inclusion with set union as the join, and the empty set is the least restrictive class. The type checker enforces the subset rule at every position and rejects a program in which a source label set is not contained in the target label set. The `classify` and `declassify` operators correspond to the raising and the controlled declassification described in the survey literature [IF7]. Labels are erased after checking and impose no runtime representation, which places Keleusma with the static tradition of Jif and FlowCaml [IF5, IF6] rather than the dynamic tradition of LIO and taint mode [IF8, IF10].

Keleusma differs from Jif and FlowCaml in scope. Both are general-purpose languages and neither is total or resource-bounded. Keleusma places the label lattice inside a total functional stream-processing language with static worst-case execution-time and memory bounds, targeting `no_std` plus `alloc` embedded hosts. The combination of information-flow labels with the synchronous, bounded-step execution model is, to the author's knowledge, not present in the prior languages. Keleusma also admits negative labels at parameter and return positions, a boundary clause that forbids named labels rather than bounding them from above. This is motivated by multi-party module delivery to embedded targets and is a narrower mechanism than the principal hierarchy of the decentralized label model [IF4].

The guarantee Keleusma currently provides is narrower than noninterference. The analysis tracks explicit value flows. It propagates labels through arithmetic, unary operations, conditional and match expressions, and casts, and it applies the subset rule recursively at tuple, array, and option positions, so a value computed from a confidential value carries the confidential label. It does not currently track implicit flows that arise when a confidential condition selects between observable side effects, for example a branch that writes different values to the data segment or invokes different native functions. A program with no declassification therefore does not leak a confidential value through a tracked data flow, but whole-program noninterference is not claimed. Closing the implicit-flow gap would require a program-counter label in the sense of the survey [IF7] and is left to future work.

Information-flow control should not be conflated with two adjacent disciplines. Object-capability security, realized in the language E, constrains which code holds the authority to invoke an operation [IF11]. Reference capabilities, realized in Pony, constrain aliasing and mutability to guarantee data-race freedom [IF12]. Both concern who may act on a value. Information-flow labels concern where a value may travel regardless of who holds a reference to it. The disciplines compose, but none subsumes another.

## Cross-References

- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the design goals and five guarantees.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the two temporal domains and structural verification.
- [STRUCTURAL_ISA.md](../spec/STRUCTURAL_ISA.md) specifies the structural ISA and verification rules.
- [GRAMMAR.md](../spec/GRAMMAR.md) Section 13 compares Keleusma to related languages.
- [GLOSSARY.md](./GLOSSARY.md) defines key terminology.

## Bibliography

### Synchronous Languages

[L1] P. Caspi, D. Pilaud, N. Halbwachs, and J. Plaice. "LUSTRE: A Declarative Language for Programming Synchronous Systems." In Proceedings of the 14th ACM SIGACT-SIGPLAN Symposium on Principles of Programming Languages (POPL), pages 178-188. ACM, 1987.

[L2] N. Halbwachs, P. Caspi, P. Raymond, and D. Pilaud. "The Synchronous Data Flow Programming Language LUSTRE." Proceedings of the IEEE, 79(9):1305-1320, September 1991.

[L3] N. Halbwachs. Synchronous Programming of Reactive Systems. Kluwer International Series in Engineering and Computer Science, vol. 215. Kluwer Academic Publishers, 1993.

[E1] G. Berry and G. Gonthier. "The Esterel Synchronous Programming Language: Design, Semantics, Implementation." Science of Computer Programming, 19(2):87-152, 1992.

[S1] A. Benveniste, P. Le Guernic, and C. Jacquemot. "Synchronous Programming with Events and Relations: The SIGNAL Language and Its Semantics." Science of Computer Programming, 16(2):103-149, 1991.

[SC1] J.-L. Colaco, B. Pagano, and M. Pouzet. "SCADE 6: A Formal Language for Embedded Critical Software Development." In Proceedings of the 11th International Symposium on Theoretical Aspects of Software Engineering (TASE), 2017.

[SY1] A. Benveniste, P. Caspi, S. A. Edwards, N. Halbwachs, P. Le Guernic, and R. de Simone. "The Synchronous Languages 12 Years Later." Proceedings of the IEEE, 91(1):64-83, January 2003.

### Coalgebra and Stream Processing

[C1] J. J. M. M. Rutten. "Universal Coalgebra: A Theory of Systems." Theoretical Computer Science, 249(1):3-80, 2000.

[C2] J. J. M. M. Rutten. "A Coinductive Calculus of Streams." Mathematical Structures in Computer Science, 15(1):93-147, 2005.

[C3] A. Abel and B. Pientka. "Wellfounded Recursion with Copatterns: A Unified Approach to Termination and Productivity." In Proceedings of the 18th ACM SIGPLAN International Conference on Functional Programming (ICFP), pages 185-196. ACM, 2013.

[C4] J. Endrullis, C. Grabmayer, D. Hendriks, A. Isihara, and J. W. Klop. "Productivity of Stream Definitions." Theoretical Computer Science, 411(4-5):765-782, 2010.

### Block-Structured Validation

[W1] A. Haas, A. Rossberg, D. L. Schuff, B. L. Titzer, M. Holman, D. Gohman, L. Wagner, A. Zakai, and J. F. Bastien. "Bringing the Web up to Speed with WebAssembly." In Proceedings of the 38th ACM SIGPLAN Conference on Programming Language Design and Implementation (PLDI). ACM, 2017.

[W2] C. Watt. "Mechanising and Verifying the WebAssembly Specification." In Proceedings of the 7th ACM SIGPLAN International Conference on Certified Programs and Proofs (CPP), pages 53-65. ACM, 2018.

### WCET Analysis

[WC1] R. Wilhelm, J. Engblom, A. Ermedahl, N. Holsti, S. Thesing, D. Whalley, G. Bernat, C. Ferdinand, R. Heckmann, T. Mitra, F. Mueller, I. Puaut, P. Puschner, J. Staschulat, and P. Stenstrom. "The Worst-Case Execution-Time Problem -- Overview of Methods and Survey of Tools." ACM Transactions on Embedded Computing Systems (TECS), 7(3):36:1-36:53, 2008.

[WC2] C. Ferdinand and R. Heckmann. "aiT: Worst-Case Execution Time Prediction by Static Program Analysis." AbsInt GmbH. Commercial tool, 2002.

[WC3] C. Ballabriga, H. Casse, C. Rochange, and P. Sainrat. "OTAWA: An Open Toolbox for Adaptive WCET Analysis." In Proceedings of the 8th IFIP WG 10.2 International Workshop on Software Technologies for Embedded and Ubiquitous Systems (SEUS), LNCS 6399, pages 35-46. Springer, 2010.

[WC4] X. Li, Y. Liang, T. Mitra, and A. Roychoudhury. "Chronos: A Timing Analyzer for Embedded Software." Science of Computer Programming, 69(1-3):56-67, 2007.

[WC5] M. Schoeberl, W. Puffitsch, R. U. Pedersen, and B. Huber. "Worst-Case Execution Time Analysis for a Java Processor." Software: Practice and Experience, 40(6):507-542, 2010.

### Abstract Interpretation

[AI1] P. Cousot and R. Cousot. "Abstract Interpretation: A Unified Lattice Model for Static Analysis of Programs by Construction or Approximation of Fixpoints." In Conference Record of the Fourth ACM SIGPLAN-SIGACT Symposium on Principles of Programming Languages (POPL), pages 238-252. ACM, 1977.

### Totality and Productivity Checking

[T1] D. A. Turner. "Total Functional Programming." Journal of Universal Computer Science, 10(7):751-768, 2004.

[T2] U. Norell. "Towards a Practical Programming Language Based on Dependent Type Theory." PhD thesis, Chalmers University of Technology, 2007.

[T3] E. Brady. "Idris, a General-Purpose Dependently Typed Programming Language: Design and Implementation." Journal of Functional Programming, 23(5):552-593, 2013.

[T4] The Coq Development Team. "The Coq Proof Assistant Reference Manual." Inria, 1989-present. The proof assistant was renamed Rocq in 2024. Reference manual available at https://coq.inria.fr/refman/ and https://rocq-prover.org/.

[T5] D. P. Friedman and C. Eastlund. *The Little Prover*. MIT Press, 2015, ISBN 978-0-262-52795-8. Pedagogical introduction to structural-recursion termination proofs in the ACL2 tradition that Rocq inherits.

### Hot Code Update

[H1] J. Armstrong. "Making Reliable Distributed Systems in the Presence of Software Errors." PhD thesis, The Royal Institute of Technology, Stockholm, 2003.

[H2] F. Cesarini and S. Thompson. Erlang Programming. O'Reilly Media, 2009.

[H3] F. Maraninchi and Y. Rémond. "Mode-Automata: A New Domain-Specific Construct for the Development of Safe Critical Systems." Science of Computer Programming, 46(3):219-254, 2003.

[H4] J. Arnold and M. F. Kaashoek. "Ksplice: Automatic Rebootless Kernel Updates." In Proceedings of the 4th ACM European Conference on Computer Systems (EuroSys), pages 187-198. ACM, 2009.

[H5] C. M. Hayden, E. K. Smith, M. Denchev, M. Hicks, and J. S. Foster. "Kitsune: Efficient, General-Purpose Dynamic Software Updating for C." In Proceedings of the ACM International Conference on Object Oriented Programming Systems Languages and Applications (OOPSLA), pages 249-264. ACM, 2012.

### Embedded Scripting

[E2] J. Lim and contributors. "Rhai: An Embedded Scripting Language for Rust." Open-source project. https://rhai.rs (accessed 2026).

[E3] P. Kelly and contributors. "Rex: A Strongly-Typed, Pure, Implicitly Parallel Functional Programming Language." Open-source project by QDX. https://github.com/peterkelly/rex (accessed 2026). Documentation at https://peterkelly.github.io/rex/.

### Information-Flow Security

[IF1] D. E. Denning. "A Lattice Model of Secure Information Flow." Communications of the ACM, 19(5):236-243, May 1976.

[IF2] D. E. Denning and P. J. Denning. "Certification of Programs for Secure Information Flow." Communications of the ACM, 20(7):504-513, July 1977.

[IF3] D. Volpano, C. Irvine, and G. Smith. "A Sound Type System for Secure Flow Analysis." Journal of Computer Security, 4(2-3):167-187, 1996.

[IF4] A. C. Myers and B. Liskov. "A Decentralized Model for Information Flow Control." In Proceedings of the 16th ACM Symposium on Operating Systems Principles (SOSP), pages 129-142. ACM, 1997.

[IF5] A. C. Myers. "JFlow: Practical Mostly-Static Information Flow Control." In Proceedings of the 26th ACM SIGPLAN-SIGACT Symposium on Principles of Programming Languages (POPL), pages 228-241. ACM, 1999.

[IF6] F. Pottier and V. Simonet. "Information Flow Inference for ML." ACM Transactions on Programming Languages and Systems (TOPLAS), 25(1):117-158, 2003.

[IF7] A. Sabelfeld and A. C. Myers. "Language-Based Information-Flow Security." IEEE Journal on Selected Areas in Communications, 21(1):5-19, January 2003.

[IF8] D. Stefan, A. Russo, J. C. Mitchell, and D. Mazieres. "Flexible Dynamic Information Flow Control in Haskell." In Proceedings of the 4th ACM SIGPLAN Symposium on Haskell, pages 95-106. ACM, 2011.

[IF9] D. Zhang, Y. Wang, G. E. Suh, and A. C. Myers. "A Hardware Design Language for Timing-Sensitive Information-Flow Security." In Proceedings of the 20th International Conference on Architectural Support for Programming Languages and Operating Systems (ASPLOS), pages 503-516. ACM, 2015.

[IF10] L. Wall and contributors. "perlsec: Perl Security." Perl documentation. https://perldoc.perl.org/perlsec (accessed 2026).

[IF11] M. S. Miller. "Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control." PhD dissertation, Johns Hopkins University, 2006.

[IF12] S. Clebsch, S. Drossopoulou, S. Blessing, and A. McNeil. "Deny Capabilities for Safe, Fast Actors." In Proceedings of the 5th International Workshop on Programming Based on Actors, Agents, and Decentralized Control (AGERE!), pages 1-12. ACM, 2015.
