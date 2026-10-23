# Order 3 proven execution

## Implemented mode

ExecutionChecks::Checked is the default. ExecutionChecks::Proven runs only after
obtaining a private certificate for the loaded image, which proves that no
operand-stack underflow check can fail. The checks themselves are retained. See
the operator ruling below. It requires TrapPolicy::RequireTrapFree. The host must disable proven
execution before relaxing trap policy. Runtime type, representation, lifetime,
allocation, storage and other unproved checks remain active.

VmOptions selects the mode during construction. set_execution_checks and
with_execution_checks can enable it on a fresh or explicitly reset instance.
Borrowed and trust-skip loaders must obtain the same proofs. Verification-disabled
builds cannot substitute a host attestation. The private certificate is never
accepted as caller input or serialized into bytecode.

The command-line runner exposes --proven-execution, which also selects strict
trap policy. --require-trap-free selects policy with ordinary execution checks.
Both source and bytecode inputs use these settings. Offline validate supports
--require-trap-free with either validator backend, preserving unavailable analysis
as a separate exit status. A trap-free verdict alone does not pass the stronger
execution proof. Proven execution cannot be combined with memory-only reporting.

## Certificate construction and identity

Certification decodes the immutable installed image, compares its instructions
with the execution cache, requires completed trap-free classification and performs
structural, typed and resource validation. A separate bounded control-flow walk
then proves a lower bound on operands at every reachable instruction. Parameter
counts must fit local storage.

The image and instruction cache are private to the virtual machine. Breakpoints
are separate state and do not modify code. Successful replacement computes a new
certificate for the candidate encoded image and cache before committing it.
Rejected validation preserves the old certificate and suspended state. The old
certificate is revoked before any replacement mutation or host-owned value drop.

## Operand proof

The abstract depth excludes frame locals. Every chunk has an initial depth of
zero. The walk propagates the minimum depth over incoming edges, using the
existing true operand effects for normal instructions and requiring one operand
for Return. The ordinary verifier's terminal Return effect is insufficient for
this proof.

If and BreakIf have both runtime successors. Else, EndLoop and Break have their
actual target successors. Return and Trap terminate paths. Yield's net-zero model
includes the one value supplied by its matching resume. Reset contributes an edge
to the instruction after Stream with depth one, matching truncation to locals and
the subsequent resume input. This catches programs that depend on operands pushed
before Stream only during their initial iteration.

A finite work queue propagates only strictly decreasing nonnegative bounds. With
n instructions and maximum positive effect g, a first arrival has depth at most
n times g plus one. Positive cycles cannot improve a minimum, and Reset seeds one.
Each point can therefore be queued at most n times g plus two times. The queue
has a checked work bound derived from those quantities. An incomplete walk cannot
produce a certificate. Targets and fall-through paths are checked independently.
The instruction profile retains its 2048-opcode limit.

## Physical stack invariant

At a certified instruction, the physical stack has at least the current frame's
base, its runtime local count and the proved operand lower bound. Frame creation
initializes its locals before dispatch. Successful instructions preserve the
modeled effects. Internal calls establish a new local prefix and restore one
result on return. Allocation and contract failures leave the dispatch loop and
invalidate execution-state proof before another proven operation can run.

A completed Return requires an operand above the local prefix and truncates to
the frame base. A Reset truncates to locals and requires exactly one resume input.
A Yield requires exactly one reply before continuation. A breakpoint requires no
reply because it suspends before executing its instruction. The runtime entry
protocol enforces those distinctions.

## Proved checks, all retained

| Execution site | Check proved unable to fail | Proof and remaining guards |
|---|---|---|
| pop helper | Per-pop empty-stack test | The instruction requires the consumed operand count. |
| peek helper | Empty-stack test for BoundsCheck, Dup, IsEnum and IsStruct | Each requires one operand and preserves it. |
| PopN | Underflow test before truncation | The complete immediate count is admitted by the lower-bound proof. Normal value destruction remains. |
| NewComposite | Operand-count test before draining | The baked count is part of the validated instruction effect. Standard container, allocation, representation and layout checks remain. |
| Call argument extraction | Underflow test for the argument prefix | The call's argument count is part of the instruction effect. Callee-index and local-storage checks remain. |

### Operator ruling, 2026-10-20

The first implementation removed these checks. The single-value and peek paths
used raw pointer access without an emptiness branch and the counted paths used a
wrapping subtraction. Soundness then rested entirely on every entry of
verify::op_depth_effect matching the runtime effect of its opcode, and no test
compared the two. A single wrong entry would have produced undefined behaviour
where the checked path produces StackUnderflow.

The operator ruled that the project rejects undefined behaviour while admitting
platform-specific behaviour, and directed that a real length check be kept on the
fast path and that the agreement be tested. The checks are therefore retained in
both modes. The proof still establishes that they cannot fail, which is the
property the timing analysis depends on. The unit test module in
src/vm_operand_effect_tests.rs steps every executed instruction of a source
corpus and of hand-built subjects for the opcodes no compiler emits. It
compares each observed stack change with the table, and requires every opcode
to be checked or explicitly excluded with a reason. An understated requirement
on a peek-only opcode remains invisible to that method, as its header records.

Internal instrumentation still distinguishes the proven control from the checked
control by counting which mode performed each pop. No hardware speedup claim is
made from that control.

## Execution-state protocol

A configured proven instance also tracks whether its state is ready, prepared,
yielded, reset, paused at a breakpoint, finished or poisoned. Configuration needs
empty stacks and no started frame. A fresh call is admitted only from ready or
finished state. Yield and reset resume use the input-bearing path. Breakpoint
resume uses the input-free path. A wrong entry method is rejected before changing
a valid suspension.

Preparation poisons the state before argument conversion, allocation or user-value
destruction. Successful preparation permits the private dispatch loop. Dispatch
again leaves the state poisoned until a successful result establishes its exact
suspension kind. A returned error or caught panic cannot reuse a partial frame.
Explicit reset clears borrowed-buffer state and re-establishes a fresh stack only
when reset succeeds. Configuration and direct function entry clear stale shared
buffer bindings. Internal calls with a current buffer use a separate helper.

Replacement clears borrowed-buffer bindings and revokes the old proof at its
commit boundary. Old private slots become initialized Unit values before their
owners are dropped, so a host destructor panic cannot leave dangling owners for
a second drop. Successful installation, reset and constant-pool rebuilding precede
assignment of the new certificate. A post-commit failure leaves proven execution
unavailable rather than retaining the old image's proof.

## Evidence and completion boundary

Controls cover checked/proven equivalence, arithmetic and handled bounds, ordinary
loops and calls, return-value requirements, lower branch and break bounds,
consuming back edges, reset reentry, wrong resume methods, runtime type failures,
cache/image disagreement, failed and successful replacement, authenticated and
borrowed loading, explicit mode changes and verification-disabled refusal.

Memory controls, recorded before the 2026-10-20 ruling, exercised the then
unchecked pops, error recovery, caught dispatch
panics, private-value destructor panics and borrowed-buffer cleanup under Tree
Borrows. Command-line controls exercise source execution, both compiler artifacts,
both validator backends, strict rejection, unavailable analysis and the distinction
between a trap verdict and an execution certificate. Exact receipts and the scope
of reruns are recorded in REVERSE_PROMPT.md.

The full Order 3 feature, shipping, compatibility and memory-safety audit remains
a separate completion requirement. No bytecode version, fingerprint, opcode count
or operator-selected capacity changes are part of this mode.
