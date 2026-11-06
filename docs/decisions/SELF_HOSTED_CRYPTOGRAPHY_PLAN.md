# Self-hosted cryptography, modules, and deferred language features

**Status.** Plan with open questions for the operator, recorded 2026-11-06. Nothing here is
implemented. It records the operator's rulings of 2026-11-06 and the measurements behind them,
and it sets out six decisions still to be made. [`ORDER5_CRYPTO_FRAMING_PLAN.md`](./ORDER5_CRYPTO_FRAMING_PLAN.md)
is the framing that today delegates every primitive to the Rust host.

## The rulings

On 2026-11-06 the operator accepted a recommendation to record two language features as design
candidates and to defer their implementation, and then ruled the following, verbatim.

> We can add this feature and rewrite the self hosted stages after the next public release.
>
> We will need our own crypographic routine to retire the Rust host in the future. Therefore, we
> probably want an implementation written sooner than later. The self hosted cryptographic
> implementation will need to get good enough to retire the Rust versions, and this likely
> implies figuring out something like modules and dependencies so that the cryptographic
> implementations can live in dedicate repositories.

Later the same day the operator added, verbatim.

> In theory, high quality cryptographic routines can be written with IFC and techniques that
> defeat timing side channel vulnerabilities. In practice, the language might need to be
> enhanced to the point where acceptable quality implementations can be written.

The work therefore has two parts. The language is enhanced until an acceptable implementation is
enforceable rather than merely possible, and the implementation is written against that.

The operator then added, verbatim.

> Another couple comments. My understanding is that Keleusma is nominally a good language for
> cryptographic work. Rust host retirement requires cryptographic work. Therefore, steps will need
> to be taken to lean into the theoretical strengths to make sure sure the language is good for
> cryptography in actual practice.

So cryptography is a design goal of the language, and not only an application written in it. Part
3 lists the enhancements this implies.

## Part 1. Deferred until after the next public release

### Block bodies in match arms, then named states

A `match` arm takes an expression and cannot take a block of statements. A state machine whose
every branch both assigns the next state and produces a value therefore cannot be written as a
`match`, and the self-hosted stages use nested `if` chains over integer codes instead. Measured
on `f372463d`, counting comparisons of a state-like field against an integer literal.

| Stage | Lines | State compared to an integer literal | `enum` declarations | `match` uses |
|---|---|---|---|---|
| `src/selfhost/kel/parse.kel` | 10,703 | 166 | 3 | 44 |
| `src/selfhost/kel/lexer.kel` | 1,053 | 39 | 0 | 3 |
| `src/selfhost/kel/codegen.kel` | 3,914 | 16 | 0 | 16 |

A four-state machine written today as an `enum` held in `private data` and read by two `match`
expressions inside `loop main` compiles and verifies, and leaving out one variant is refused as a
non-exhaustive match. Named states can therefore be pure sugar that lowers to the same bytecode.

The order is block bodies in match arms first, then named-state sugar on top, then a rewrite of
the stages one at a time, each re-pinned. The self-hosted compiler must support a feature before
any stage uses it, or the stages stop compiling themselves. One design point is open. The lexer
dispatches on `dq.state >= 6`, a range of states, so named states need sub-machines or an arm that
lists several states.

### The secret-label policy

Information-flow labels already taint a result with the labels of the condition it was computed
under, in `src/typecheck.rs`, but they do not forbid a branch on, or an index by, a labelled value.
A `secret` label under which the compiler refuses a data-dependent branch and a data-dependent
index is the language feature that a constant-time guarantee rests on. Whether this one moves
ahead of the release is question C1 below, because Part 2 depends on it.

## Part 2. Self-hosted cryptography, sooner

### What the Rust host provides today

Measured by reading `Cargo.toml` and `keleusma-cli/Cargo.toml` on `f372463d`.

| Primitive | Role | Rust crate |
|---|---|---|
| Ed25519 | Signing and signature verification | `ed25519-dalek` |
| X25519 | Key agreement | `x25519-dalek` |
| HKDF over SHA-256 | Key derivation | `sha2` |
| AES-GCM | Authenticated encryption | `aes-gcm` |
| SHA-256 | Hashing | `sha2` |
| SHA-512 | Inside Ed25519 | `ed25519-dalek` |

### What the language already offers

Read from `docs/spec/INSTRUCTION_SET.md` and `docs/spec/GRAMMAR.md`, and run for the selection.

- `CheckedMul` pushes the low and high halves of the product, which the specification calls the
  value big-number multiplication rests on, and `CheckedAdd` carries in the same way. The product
  is computed as signed `i128`, while cryptographic limbs are unsigned, which suggests 32-bit limbs
  or a correction of the high half. That is inferred, not measured.
- `band`, `bor`, `bxor`, `bnot` and the shifts are available on `Word`, `Byte` and
  `Multiword<N>`, so rotations and masks can be written.
- A branchless selection, `(a band m) bor (b band bnot m)` with the mask taken from the sign bit
  of a difference by `asr`, compiles and gives correct results. A `bool` does not cast to `Word`,
  so the mask comes from arithmetic.
- Every loop is bounded, every result is defined, and the time and memory a program can take are
  statically bounded.

### What is missing for a security-grade implementation

1. **A constant-time guarantee.** A WCET figure is an upper bound and not an equal time. The
   virtual machine's time per operation is not specified, and the native route passes through
   LLVM, which may turn a masked selection back into a branch. Both need a stated contract, and the
   secret-label policy has to enforce it at compile time.
2. **Key erasure.** A Reset reclaims arena memory without zeroing it, measured in
   [`NESTED_LOOP_ARENAS.md`](./NESTED_LOOP_ARENAS.md). Secret material needs an explicit wipe that
   is guaranteed to survive lowering.
3. **Entropy.** Randomness has to come from the host, so part of the trusted base stays outside
   the language.
4. **Assurance.** No audit, proof or side-channel measurement exists yet.

### The tension with the engineering rules

The operator's engineering rules say never to implement custom cryptographic primitives. This plan
implements standard, specified algorithms, never new ones, and it keeps the Rust implementations
as the reference until a stated bar is met. That narrows the risk the rule guards against, but it
does not remove it. The retirement bar in question C4 is where that risk is accepted or not.

### Modules and dependencies

Today `use` imports host natives, as in `use host::transmit(payload: Word@Open) -> bool`. No
construct imports Keleusma source from another package. A cryptography library that lives in its
own repository therefore needs a module and dependency system, which question C3 sets out.

### A proposed order

1. The secret-label policy and a constant-time contract for the virtual machine and the native
   lowering, so that the first implementation is checked as it is written.
2. A module and dependency system, at least enough for one library package to be imported.
3. The primitives, hashes first because the others build on them: SHA-256 and SHA-512, then HKDF,
   then X25519, then Ed25519 verification, then Ed25519 signing, then the authenticated cipher.
4. For each primitive, the published test vectors and a differential test against the Rust crate
   over generated inputs, so the two must agree before anything is retired.

## Part 3. Leaning into the theoretical strengths

Each item turns a property the language has in principle into one an implementation can rely on.
The opcode count is a design limit, so each is planned as a compile-time rule or a surface form
that lowers to existing opcodes, as the `Text<N>` operations do. An item that cannot be done that
way is to be raised separately rather than add an opcode.

1. **A `secret` label that is enforced.** The compiler refuses a branch condition, an array index,
   a loop bound and a division or modulo operand that carries the label, and refuses passing it to
   a host native that is not declared to accept it. Declassification is an explicit, auditable
   form. This builds on the information-flow labels that already exist.
2. **A constant-time contract.** The virtual machine states which opcodes take time independent
   of their operand values, and a measurement harness tests that statement statistically, so the
   contract is evidence and not a claim. The native lowering states the same for its code, with a
   check that a masked selection reaches the machine code without a branch.
3. **Branchless selection as a surface form.** A `select` built from masks, and a way to turn a
   comparison into a mask, since a `bool` does not cast to `Word` today. Both lower to the existing
   bitwise opcodes.
4. **Unsigned wide arithmetic.** Unsigned multiply-high and add-with-carry forms over `Word` and
   `Multiword<N>`, lowered to the existing checked opcodes with the sign corrections the signed
   `i128` product needs.
5. **Guaranteed erasure.** A wipe of secret storage that survives every lowering, and a rule that
   secret values do not outlive their scope in reclaimed arena memory.
6. **Byte-oriented helpers.** Fixed-size byte arrays with the conversions to and from words in
   both byte orders, and constant-time equality, which hashes and ciphers use everywhere.
7. **A known-answer and differential test harness.** Published test vectors, and a differential
   check against the Rust crate over generated inputs, available to any library.
8. **Modules and dependencies,** question C3, so the cryptography lives in its own repository.

Items 1 to 4 are the core. Without them an implementation can be correct but cannot be shown to
be constant-time.

## Questions for the operator, recommendation first

**C1. When the secret-label policy is built.**
- A. Before the next public release, since the cryptography depends on it and it is small.
- B. After the release, with the other deferred features.

**C2. The authenticated cipher.** AES in software is hard to make constant-time, because its usual
form reads tables at secret-dependent addresses, and the safe forms are slow.
- A. Keep AES-GCM for compatibility, written in a constant-time bitsliced form.
- B. Add ChaCha20-Poly1305, which is simpler to make constant-time, as the self-hosted cipher, and
  keep AES-GCM in Rust until a format change can retire it. This changes the framing format, which
  needs its own ruling.
- C. Decide when the cipher is reached in the order above.

**C3. The shape of modules.**
- A. Source-level modules with a dependency manifest that names versioned packages, compiled into
  one module. Static linking keeps the whole-program time and memory analysis intact and needs no
  wire change.
- B. Separately compiled bytecode modules linked at load. This needs wire, linking and
  cross-module WCET and WCMU work.
- C. Start with a single source tree and add packaging later.

**C4. The bar for retiring a Rust primitive.**
- A. All published vectors, differential agreement with the Rust crate over generated inputs, the
  secret-label policy enforced with no exception, and an independent review.
- B. The same without the independent review.
- C. Decide per primitive.

**C5. Where the first implementation lives.**
- A. In this repository, in a directory that is ready to move, until the module system exists, and
  then in its own repository.
- B. In its own repository from the start.

**C6. Where the constant-time guarantee is enforced.** The trust boundary is bytecode. The
verifier, the time and memory analyses, `Vm::new`, hot reload, signing and the native backend all
read bytecode, not source. Information-flow labels exist only in the surface language. Measured on
`f372463d`, `src/bytecode.rs` has no mention of a label, and the verifier does not consult one. A
`secret`-label rule checked only by the compiler therefore protects programs compiled from trusted
source, and not bytecode that arrives another way. No option is recommended yet.
- A. A bytecode-level form the verifier can check, such as secret-carrying values marked in the
  module with branch and index rules checked on bytecode. This needs a wire change and its own
  authorization.
- B. A signed attestation from a trusted compiler that the rule passed, so the artifact is trusted
  by its signature rather than by what can be proved of its bytecode.
- C. Both, B first and A later.
