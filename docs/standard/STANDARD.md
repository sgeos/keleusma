# The Keleusma Standard

**Status.** Draft. This document is normative for the Keleusma language, its
bytecode instruction set, its wire format, its verifier guarantee, its stable
host application binary interface, and its native code generation contract. It is
authored against the structure fixed by
[`META_SPECIFICATION.md`](META_SPECIFICATION.md). The informative rationale lives
in [`JUSTIFICATION.md`](JUSTIFICATION.md), which this document never cites and
which never resolves a conformance question.

**How to read this document.** Every clause is normative unless it is enclosed in
a Note or an Example block, which is informative. The requirement keywords MUST,
MUST NOT, SHOULD, SHOULD NOT, and MAY are used in the sense of RFC 2119 as
clarified by RFC 8174, and only in upper case where a requirement is stated. A
reference to another clause of this document is written as Standard followed by
the section number. A reader is able to read this document alone.

**Editor's notes and known non-conformances.** The reference implementation is
known to diverge from the intended semantics at a small number of points recorded
in Annex A. Where this document states a rule that the current reference
implementation does not yet satisfy, the divergence is a known non-conformance,
not an error in this document, because this document is normative over the
implementation per Standard 1.3.

---

## 1. Status, Authority, and Version

### 1.1 Authority

This document defines what it means to conform to Keleusma. It is normative over
every implementation, being every compiler, verifier, virtual machine, host
boundary, and native code generator that claims conformance. When an
implementation and this document disagree, the implementation is non-conforming
until either it is corrected or this document is revised through issuance.

### 1.2 Issuance and governance

Issuance is dictatorial. There is no external conformance body. A revision takes
effect only when the issuing authority issues it with a change-record entry in
Annex A. Between issuances the document may evolve through peer review, but the
act of issuance remains with the issuing authority.

### 1.3 Direction of authority

This document states the intended semantics. It does not transcribe the current
behavior of the reference implementation. Where the two differ, the reference
implementation is wrong by definition until this document is revised. This
direction is what gives a self-hosted compiler a stable target.

### 1.4 Version

This document carries a standard version identifier separate from the runtime
`BYTECODE_VERSION`, which it governs rather than equals. Conformance is always
conformance to a named standard version. The current runtime bytecode version is
1. This is the first draft and carries no issued standard version yet.

---

## 2. Scope and Conformance

### 2.1 Scope

This document specifies the Keleusma surface language, its type system and
totality discipline, the canonical value and memory model, the bytecode
instruction set architecture, the verifier guarantee, the wire format including
its cryptographic framing, the stable host application binary interface, the host
contract and trust boundary, and the native code generation contract. It does not
specify a particular hardware target, a particular deployment, or the functional
correctness of any program.

### 2.2 Conformance classes

This document names four conformance classes.

- A conforming module is a wire artifact that satisfies the wire format of
  Standard 9 and the verifier of Standard 8.
- A conforming producer, being the compiler, the host boundary, or a native code
  generator, emits only conforming modules and materializes only the canonical
  layouts of Standard 5.
- A conforming program is well-formed under the grammar of Standard 4, well-typed
  under the type system of Standard 4, and admissible under the verifier of
  Standard 8.
- A conforming implementation obeys the accept and reject obligations of Standard
  2.3.

### 2.3 The three verdicts

A conforming implementation partitions every well-formed, well-typed program into
exactly one of three verdicts.

- MUST-ACCEPT. This is an implementation-independent class. It is the floor fixed
  by the positive conformance suite of Standard 13. Every program in the class has
  no structural or memory-safety violation and has a proven worst-case execution
  time and worst-case memory usage bound. Every conforming implementation accepts
  every program in the class. The existence of a bound is a portable property. The
  numeric value of the bound is a target-defined characteristic recorded in Annex
  B.
- MUST-REJECT. This is an implementation-independent class. Every program in the
  class has a proven structural or memory-safety violation, or a proven absence of
  an upper bound. The set of provable structural and memory-safety violations is
  decidable, and every conforming implementation decides it completely, so a
  program in this class is never in the MAY-REJECT band. Every conforming
  implementation rejects it.
- MAY-REJECT. This is the only implementation-relative band. Every program in the
  band is one the implementation has neither proven admissible nor proven
  violating. Rejecting here is always safe, because the only route to acceptance
  is upgrading to a proof, so no unproven and unsafe program is ever admitted. A
  stronger analysis that proves such a program admissible and accepts it remains
  conforming.

### 2.4 Acceptance requires proof

Acceptance always rests on a positive proof. An implementation accepts a program
only when it has proven both the absence of a structural or memory-safety
violation and the presence of the worst-case bounds. A program whose bound
merely exists, without a proof the implementation holds, is MAY-REJECT for that
implementation, not MUST-ACCEPT.

### 2.5 Conformance profiles

Conformance is claimed against one or more profiles over a common Core. An
implementation conforms to the Core profile by satisfying every section of this
document except the native code generation contract of Standard 12, the
cryptographic framing of Standard 9.3, and the float scalar. A Core
implementation carries no cryptography and no floating point and is deterministic
by default. Four additive profiles extend Core.

- The Native profile adds the native code generation contract of Standard 12.
- The Signing profile adds signature verification of a signed module, being the
  runtime obligation, with signature generation a producer capability under the
  same profile.
- The Encryption profile adds decryption of an encrypted module and its
  counterpart production.
- The Float profile adds the float scalar with the exception, rounding, and NaN
  semantics of Standard 5.1.5.

The Signing and Encryption profiles are independent. A conformance claim names the
profiles met. A minimal interpreter on a trusted target MAY claim Core alone.

### 2.6 The Sealed profile

The Sealed profile is a restriction over Core rather than a capability over it. A
Sealed implementation resolves every MAY-REJECT program to a single documented
deterministic decision, so its accept and reject sets carry no implementation
latitude. It admits no At-risk or Reserved feature in the accepted surface. No
module executes without verification, and no admitted path reaches execution
without it. It requires the verifier. A Sealed build is float-free by
construction, because Core is float-free, and Sealed does not compose with the
Float profile, whose exception and rounding behavior is target-sensitive. Sealed
composes with the Native, Signing, and Encryption profiles.

### 2.7 Scope boundary on functional correctness

The verifier guarantee of Standard 8 covers safety, structural validity,
totality, and the worst-case bounds. It does not cover functional correctness,
being whether a program computes its intended result. Functional correctness is
the program author's responsibility and is established by the author's own
testing. Structural coverage of an implementation and requirement-to-code
traceability within it are that implementation's own concern, outside this
document.

---

## 3. Terms, Notation, Normative References, and Target Parameters

### 3.1 Defined terms

The terms below are defined for use throughout this document. A complete glossary
and an index of definitions accompany the issued document.

- Word. The signed two's-complement integer of the target word width.
- Scalar. A value whose flat layout is a fixed number of bytes determined by its
  kind and the target widths.
- Composite. A tuple, struct, array, or enumeration.
- Flat body. The little-endian byte image of a value under the canonical layout of
  Standard 5.2.
- Arena. The bounded region of memory from which the runtime allocates, described
  in Standard 5.5.
- Stream iteration. One traversal of the productive divergent loop body from the
  stream point to the reset point, described in Standard 4.6 and Standard 5.5.
- Native. A host-provided function registered with the runtime, described in
  Standard 10 and bounded by the host contract of Standard 11.
- Producer. A compiler, host boundary, or native code generator that emits or
  materializes conforming artifacts.

### 3.2 Notation

The grammar of Standard 4 uses one stated metasyntax. The dynamic semantics of
instructions and values use a small-step operational style over the
abstract-machine configuration of Standard 6, after the manner of the WebAssembly
specification. The canonical layout of Standard 5.2 is stated as total functions
over a type and the target widths.

### 3.3 Normative references

The always-core references are RFC 2119 and RFC 8174 for requirement keywords, and
Unicode with its Unicode Transformation Format 8 for text. The Float profile adds
IEEE 754 for floating-point. The Signing profile adds RFC 8032 for the
Edwards-curve Digital Signature Algorithm over Curve25519. The Encryption profile
adds RFC 7748 for the X25519 key agreement, RFC 5869 with FIPS 180-4 for the
Hash-based Key Derivation Function over SHA-256, and FIPS 197 with NIST SP 800-38D
for the Advanced Encryption Standard in Galois/Counter Mode at a 256-bit key.

### 3.4 Target parameters

Everything layout-related or width-dependent is parameterized by three target
widths, being the word width, the float width, and the address width. No fixed
byte figure appears in this document without its width parameterization. The
reference runtime uses a 64-bit word, a 64-bit float, and a 64-bit address, so a
word is 8 bytes and a float is 8 bytes on that runtime. A conforming
implementation MAY use a narrower word, being 8, 16, or 32 bits, in which case
every size in Standard 5 scales accordingly. The runtime records the three widths
as base-2 logarithms in the module header of Standard 9.1.

---

## 4. Surface Language

### 4.1 Lexical structure

Source text is Unicode encoded as Unicode Transformation Format 8. The lexer
produces a token stream. The token categories are keywords, identifiers, literals,
operators, and delimiters.

**Keywords.** The reserved keyword set is `fn`, `yield`, `loop`, `break`, `let`,
`for`, `in`, `if`, `else`, `match`, `use`, `external`, `struct`, `enum`,
`newtype`, `where`, `overflow`, `underflow`, `saturate_max`, `saturate_min`,
`signed`, `true`, `false`, `as`, `when`, `not`, `and`, `or`, `xor`, `andalso`,
`orelse`, `lsl`, `asl`, `lsr`, `asr`, `band`, `bor`, `bxor`, `bnot`, `pure`,
`data`, `shared`, `private`, `const`, `ephemeral`, `trait`, and `impl`. The words
`classify`, `declassify`, and `assert` are context-sensitive and are recognized as
operators or statements only when not immediately followed by a call.

**Identifiers.** A lower identifier matches `[a-z_][a-z0-9_]*` and names a
variable, a function, or a field. An upper identifier matches `[A-Z][A-Za-z0-9]*`
and names a type, a struct, or an enumeration. A bare underscore is a wildcard.

**Literals.** An integer literal is decimal, hexadecimal with a `0x` prefix, or
binary with a `0b` prefix, and MUST fit the `i64` range at lex time. A numeric
literal MAY carry a type suffix drawn from `Word`, `Byte`, `Float`, and
`Fixed<N>`. A `Byte` suffix range-checks to zero through 255. A `Fixed<N>` suffix
requires `N` in zero through 62 and encodes the Q-format value. A float literal
matches digits, a point, and digits, with an optional `Float` or `Fixed<N>`
suffix, and is admitted only under the Float profile. A text literal is delimited
by double quotes and admits the escapes newline, tab, carriage return, backslash,
double quote, and nul. The boolean literals are `true` and `false`.

**Comments.** A line comment runs from `//` to end of line. A block comment runs
from `/*` to `*/` and does not nest. A leading shebang line is ignored.

**Operators.** The arithmetic operators are `+ - * / %`. The shift operators are
the keywords `lsl`, `asl`, `lsr`, and `asr`, named after the assembly mnemonics.
The bitwise operators are the keywords `band`, `bor`, `bxor`, and the prefix
`bnot`. The comparison operators are `== != < > <= >=` and are non-associative, so
a chained comparison does not parse. The eager logical operators are the keywords
`and`, `or`, `xor`, and the prefix `not`; the short-circuit logical operators are
the keywords `andalso` and `orelse`. A logical or bitwise operation is selected by
the operator name and never by the operand type. The pipeline operator is `|>`.
Field access is `.`, path resolution is `::`, the
range operator is `..`, the return arrow is `->`, the match arrow is `=>`, the
information-flow-control marker is `@`, and a trait bound joins with `+`.

**Note.** The grammar document in `docs/spec/GRAMMAR.md` is descriptive. The
parser is authoritative where the two differ.

### 4.2 Grammar

A program is a sequence of `use` declarations followed by any mix of type
declarations, data declarations, trait declarations, implementation blocks, and
function declarations. The metasyntax below is illustrative of the declaration
forms. The authoritative grammar accompanies the issued document.

- A `use` declaration imports a native, optionally with the `external` modifier
  and an inline signature of the form `use host::name(T1, T2) -> R`.
- A `struct` declaration is `struct Name<T, ...> { field: Type, ... }`.
- An `enum` declaration lists variants that are unit, tuple, or struct-shaped,
  with an optional explicit discriminant that is an integer literal with an
  optional leading minus and no expression. A duplicate discriminant is rejected
  at parse time.
- A `newtype` declaration is `newtype Name = Underlying` with an optional `where`
  refinement predicate and an optional `with saturate_max` and `saturate_min`
  contract.
- A data declaration is `[shared|private|const] data name { field: Type [= init],
  ... }`. A bare `data` is shared. A `const` field requires a literal initializer.
  A shared or private field rejects an initializer.
- A `trait` declaration lists method signatures without bodies. An `impl` block is
  `impl [<T, ...>] Trait for Type { ... }`.
- A function declaration is `[ephemeral|signed]* [pure] (fn|yield|loop)
  name<T, ...>(params) -> R [when guard] { block }`.

Statements are the let binding `let pattern [: Type] = expr;`, the bounded
iteration `for var in iterable { block }`, the `break` statement that is valid
only inside a `for`, the debug `assert cond [, "message"];`, the data-field
assignment `name.field = expr;` and its indexed form, and the expression
statement.

Expression precedence from weakest to tightest is the pipeline; the logical
operators in the order `orelse`, `andalso`, `or`, `xor`, `and`; the
non-associative comparison; the bitwise operators in the order `bor`, `bxor`,
`band`; the shifts `lsl`, `asl`, `lsr`, `asr`; the additive; the multiplicative;
the prefix `not`, `bnot`, and unary minus; the postfix field access and method
call and index and the cast `as Type`; and the primary forms. The primary forms include literals, the
`classify` and `declassify` operators, saturation literals, calls and qualified
calls, enum-variant construction, struct initialization, newtype construction,
`yield`, `if` with `else`, `match`, the `loop` expression, the unit value,
grouping, tuple construction, and array literals.

The checked-arithmetic construct attaches an arm block to a preceding expression
when the opening brace is followed by an arm keyword. The arm kinds are `ok`,
`overflow`, `underflow`, `zero_divisor`, `nan`, `invalid_index`, `invalid_newtype`,
`payload_discriminant`, `invalid_discriminant`, and `error`. Each arm kind is
admissible only for the operations that can raise it. Every checked construct MUST
carry an `ok` arm and MUST cover every outcome its operation admits, either by an
arm or by an arm with a `when` guard whose alternatives are exhaustive.

### 4.3 Type system

The base types are `Byte`, being an 8-bit unsigned integer with wrapping
arithmetic, `Word`, being the signed two's-complement integer of the target word
width, `Fixed`, being a signed Q-format fixed-point of the target word width whose
fraction-bit count is part of the type, `Float`, being the target float under the
Float profile, `Bool`, `Text`, being a Unicode string, and `Unit`. The compound
types are tuples, fixed-length arrays `[T; N]`, structs, enumerations, `Option<T>`
as a specific enumeration, refined newtypes, and opaque host references.

Type inference is Hindley-Milner with Robinson unification and the occurs check.
Inference is local to each function and does not generalize across functions.
There is no implicit numeric coercion. A numeric conversion is written with the
`as` operator. A conversion from an enumeration to `Word` yields the discriminant.
A conversion from `Word` to an enumeration is admitted only through the
discriminant construct of Standard 4.2.

Generics carry trait bounds of the form `<T: A + B>`. A trait is a set of method
signatures. An implementation block registers a type as implementing a trait, and
its methods MUST match the trait signatures. At a call site the resolved head type
of each type parameter MUST implement every required trait, or the program is
MUST-REJECT.

### 4.4 Monomorphization

Generic functions, structs, and enumerations are monomorphized at compile time. A
conforming producer generates one specialized definition per distinct
instantiation and rewrites call sites to the specialized names. Monomorphization
is bounded, so a producer MUST reject a program whose instantiation set is not
finite. This preserves the worst-case bounds, since every executed body is
concrete.

### 4.5 Information-flow-control labels

A type MAY carry a set of information-flow-control labels written `T@Label` or
`T@{L1, L2}`. Labels are Normative. The empty label set is the pure state.

A positive label constrains flow by the subset rule. At every position a source
value's label set MUST be a subset of the target position's label set, checked
recursively through tuple, array, and option positions. A violation is
MUST-REJECT. Labels union through arithmetic, comparison, and branch joins. The
`classify` operator adds labels and is always admitted. The `declassify` operator
removes labels, is always admitted, and is the audited disclosure point. Labels
carry no runtime representation and no runtime cost.

A negative label written `T@!Label` is admissible only at three boundary
categories, being function parameter and return types including native
signatures, shared data field types, and private data field types. A value
crossing a negative boundary MUST NOT carry any listed label in its positive set.
A negative label does not propagate into the body. Mixing positive and negative
labels in one set is rejected at parse time, and a negative label nested below a
top-level boundary is rejected.

### 4.6 Totality and the productive divergent loop

Keleusma is a total functional stream processor. Without host natives it admits
only pure total functions and the productive divergent loop. There are three
function categories.

- An `fn` is atomic and total. It contains no `yield` and no divergent loop, its
  `for` loops range over fixed arrays or bounded ranges, and it does not recur.
- A `yield` function is non-atomic and total. It MAY yield, contains no divergent
  loop, ranges its `for` loops over bounded domains, does not recur, and MUST exit
  on every path. Its callees share the same yield contract.
- A `loop` function is productively divergent. It never exits, and every execution
  path through its body MUST pass through at least one `yield`. There is exactly
  one `loop` function per program, and it is the coroutine entry point. The
  `break` statement is not admitted inside it.

Recursion is prohibited entirely. A conforming producer MUST reject any call-graph
cycle among Keleusma functions, whether direct or mutual. The only cyclic
execution is the reentry of the divergent loop, which is a coroutine step and not
a call. Productivity MUST be decided structurally, so that the host receives an
output on every stream iteration.

**Note.** The totality guarantee is conditional on the host contract of Standard
11, being that every native is total and that the host resumes after each yield.

---

## 5. Value and Memory Model

### 5.1 Value domain

#### 5.1.1 The closed scalar set

The scalar-kind set is closed and exhaustive. It is stated once here and
referenced by size elsewhere. Sizes are given in bytes as functions of the target
word width `w` and float width `f`.

| Scalar kind | Surface type | Byte size | Wire tag |
|---|---|---|---|
| Unit | `()` | 0 | 0 |
| Bool | `Bool` | 1 | 1 |
| Byte | `Byte` | 1 | 2 |
| Int | `Word` | `w` | 3 |
| Fixed | Q-format | `w` | 4 |
| Float | `Float` | `f` | 5 |
| Text | `Text` | `2 * w` | 6 |
| Opaque | host reference | `w` | 7 |

A boolean is stored as `0u8` for false and `1u8` for true. The `Fixed` fraction-bit
count is carried by the instructions that produce or consume the value, not by the
byte layout. A `Text` value is a two-word handle carrying either a rodata offset
and length for a static string or an arena handle and length for a dynamic string.
An `Opaque` value is a single-word handle to a host reference. The Float kind and
its wire tag are reserved even in a Core build so that tags never shift, and the
kind is materialized only under the Float profile.

#### 5.1.2 The multi-word fixed-point family

A fixed-width multi-word fixed-point number, written `Multiword<N>` or
`Multiword<N, F>`, is a parameterized numeric type distinct from the closed base
scalar set in the way an array is distinct from its element. It is N words wide, so
a value is exactly `N * w` bytes, stored little-endian in two's complement, with F
fractional bits. The fraction-bit count F defaults to zero, and `Multiword<N>` is
`Multiword<N, 0>`, the multi-word integer case. Distinct word counts and distinct
fraction-bit counts are distinct types, related only by an explicit cast. Addition
and subtraction are scale-independent and wrap at the word-count width.
Multiplication of two values of fraction-bit count F shifts the double-width product
right by F, and division shifts the dividend left by F, so the scale is preserved.
Division by zero is the same bounded fault as for the word-width integer.

**Note.** The multi-word family is partially implemented, and its completion is a
known non-conformance per Annex A. The type, its construction and indexing, its
scale-independent addition and subtraction with the correct unsigned carry and
borrow, the six comparison operators, and multiplication for every fraction-bit
count F are implemented. The comparison orders a value by its most significant
differing word, the top word read signed and the lower words read unsigned. The
integer multiply (F equal to zero) is the low-N-word two's-complement product
computed as an unsigned schoolbook product with a signed-to-unsigned high-word
correction per digit product; the fixed-point multiply (F greater than zero) forms
the full 2N-word signed product and shifts it right by F, taking the low N words.
The divide and modulo are also implemented at every scale, signed with truncation
toward zero as for the word-width integer, the quotient taking the sign of the
operand exclusive-or and the remainder the sign of the dividend, over a branchless
binary long division of the operand magnitudes; a zero divisor is the same bounded
fault as the word-width integer. The fixed-point divide pre-shifts the dividend left
by F, since the raw quotient representing the ratio of two same-scale values is the
shifted dividend divided by the divisor, while the fixed-point modulo needs no
shift, a same-scale remainder keeping the scale. The four shift operators are
implemented for a compile-time-constant amount and for a runtime-variable amount,
named after the assembly mnemonics. `lsl` and `lsr` are the logical shifts,
zero-filling the vacated bits, and `asl` and `asr` are the arithmetic shifts, the
arithmetic right shift filling the vacated top with the sign and the arithmetic
left shift carrying the value `x * 2^k` so it admits overflow capture on the
word-width type. A variable multi-word shift is unrolled over the compile-time
word count with runtime index arithmetic and branch-free bounds guards, so it
carries no runtime loop and preserves the definitive worst-case bounds. The
bitwise operators `band`, `bor`, `bxor`, and the prefix `bnot` are implemented,
applied to each limb independently with no cross-limb interaction. The multi-word
arithmetic family is therefore complete; general const generics for the word and
fraction-bit parameters remain a separate feature, tracked as B40 in the backlog.

#### 5.1.3 Composite kinds

The composite kinds are tuple, array, struct, and enumeration. `Option<T>` is the
enumeration named `Option` with the variants `None` and `Some(T)`. There is no
distinct option layout.

#### 5.1.4 Runtime value forms

A runtime value is one of the scalar forms above, a static or dynamic text handle,
a composite in either a flat or a boxed form, the option-none form, or a host
opaque reference. A composite value slot is exactly 32 bytes on the reference
runtime, pinned by a compile-time assertion, and MUST NOT exceed the slot size the
target fixes.

#### 5.1.5 Float semantics under the Float profile

Under the Float profile the float scalar is IEEE 754 binary of the target float
width. The rounding mode, the treatment of not-a-number, the treatment of the
infinities, and the treatment of the inexact and invalid exceptions are fully
specified for defined behavior under Standard 6.3. A Core build does not admit the
float scalar.

### 5.2 Canonical flat layout

This is the crux of the value model. Layout is a total function of a type and the
target widths. It yields a byte size and, for a composite, a field offset for each
constituent.

#### 5.2.1 Scalars

A scalar occupies the byte size of Standard 5.1.1 and is stored little-endian. Every
stored scalar is little-endian, including each word of a multi-word value.

#### 5.2.2 Packing rule

Composites are packed with no alignment padding. A field offset is the sum of the
sizes of the preceding constituents in declaration order. A tuple size and a struct
size are the sum of their constituent sizes. An array size is the element size
multiplied by the count, computed with saturation so that a pathological type is
rejected rather than mis-sized. An array element offset is the element index
multiplied by the element size.

#### 5.2.3 Enumerations

An enumeration body is one discriminant word at offset zero followed by the payload,
where the discriminant is word-sized. The whole body is bounded by the size of one
discriminant word plus the largest variant payload, so every value of the type has
one fixed worst-case size for nesting. The discriminant of a variant is a property
of the type, derived from the type definition and computed by the consumer. It is
never supplied or trusted from a value producer or from a wire enumeration-layout
table. The empty-option case has one canonical representation, being the scalar
option-none form, and never takes an enumeration body. A `Some` payload flattens
like any uniformly flat enumeration when the payload type is flat.

#### 5.2.4 The single-representation rule

A value of a given type has exactly one flat body at a given set of target widths.
Representation is a pure function of the type and the widths. It is never a function
of a runtime value, a size threshold, or the producing agent. Every producer
materializes the same bytes, so a value written by the compiler, by the host
boundary, or by native code is byte-identical.

#### 5.2.5 The flatness predicate

A type is flat when every constituent is flat, and an enumeration is flat only when
every variant payload is flat. The flat-eligible scalars are unit, boolean, byte,
word, fixed, opaque, and, under the Float profile, float. Text is flat only when
the target word width is at least the host address width, so a narrow-word build
keeps text in a reference form to avoid truncating a stored pointer. An opaque value
is flat as a word-width registry index. A reference-bearing type that is not flat is
carried in a boxed form. The predicate is total over the type and the widths, so a
producer and a consumer always agree.

#### 5.2.6 Baked offsets

Composite and field access instructions carry offsets baked at compile time from
the accessed type. A conforming verifier reconstructs the operand-stack types and
confirms every baked offset equals the canonical layout of the accessed type, by the
typed pass of Standard 8.2, so layout is type-determined and verified rather than
trusted. Runtime access then uses the baked offset directly with no per-access bounds
check, because the offset was proven at load, which preserves the zero-copy goal.

**Note.** The typed pass that decides baked-offset agreement is a known
non-conformance per Annex A. In the reference implementation baked-offset
correctness is a compiler-integrity assumption, not a verifier-decided invariant.

### 5.3 Section model

The runtime memory image partitions into four regions after the manner of a
System V object.

- The text region holds the immutable bytecode of every chunk. It is immutable
  until a hot swap at a reset point.
- The rodata region holds the immutable constant pool, the struct templates, and
  the native-name table.
- The data region holds the data segment of Standard 5.4, being the shared data and
  the private data.
- The bss region holds the operand stack, the frame stack, and dynamic strings and
  ephemeral composite bodies. It is cleared at a reset point.

The arena of Standard 5.5 provides the bss region. The operand stack and the frame
stack occupy the arena bottom, growing upward. Dynamic strings and ephemeral
composite bodies occupy the arena top, growing downward.

### 5.4 Data segment

The data segment carries values that persist across a yield. It has two parts.

- Shared data is a host-owned buffer of flat bytes. The host lends the runtime a
  mutable byte slice for the dynamic extent of one call or resume and never longer.
  The slice length MUST equal the module's declared shared-data byte count. The host
  reads and writes scalar fields by byte offset between calls. A shared field MUST
  NOT be an arena reference type, being text or opaque, and a shared field MUST NOT
  be an array of composites, because a host buffer cannot hold an arena pointer that
  would dangle for the host after a reset.
- Private data lives in the arena persistent region and persists across every reset.
  A host resets private data only by replacing the module. Private data is
  zero-initialized at construction.

The accumulating state of a program, for example the symbol and type environment of
a self-hosted compiler, resides in the persistent or shared region and never in the
per-iteration arena. This is a general rule for every program.

### 5.5 Arena model and reset semantics

The arena is a fixed dual-ended bump allocator over a buffer sized once at
construction. It has three regions.

- The persistent region at the low end holds the private data. It is preserved
  across every reset form.
- The bottom region grows upward from the end of the persistent region and holds the
  operand stack, the frame stack, and the opaque registry.
- The top region grows downward from the high end and holds dynamic strings and
  ephemeral composite bodies.

Allocation fails, and the runtime raises a bounded arena-exhaustion fault, when the
bottom region and the top region would meet. There is no allocation after
initialization on the steady-state path.

A reset reclaims the ephemeral top region. It advances an epoch counter, so that any
handle into the reclaimed region reads as stale on its next access rather than
reading freed memory. A reset does not touch the persistent region and does not
reclaim the bottom-region operand stack, though the reset instruction separately
clears locals to unit and truncates the operand stack to the locals. A pointer into
the persistent region, or into memory outside the arena such as host data or rodata
reached through a flat composite body, is always live because a reset never reclaims
it. This region-aware liveness is what lets a flat composite body be a single
pointer that reads in place wherever it lives.

A stream iteration is one traversal of the divergent loop body from the stream
point to the reset point. The worst-case memory usage of Standard 8 is a per-stream-
iteration bound, reclaimed at each reset.

---

## 6. Semantic Objects and the Abstract Machine

### 6.1 The machine configuration

The abstract machine is defined once here, before the instruction semantics. A
configuration is the tuple of the following components. No later section introduces
a new kind of state.

- The operand stack, a sequence of value slots.
- The frame stack, a sequence of call frames, each carrying a chunk index, an
  instruction pointer, and an operand-stack base.
- The arena and its epoch, being the memory of Standard 5.5.
- The shared data region and the private data region of Standard 5.4.
- The program counter, being the current chunk index and instruction pointer.
- The coroutine and yield-resumption state, being whether the machine is running,
  yielded and awaiting a resume value, finished, or at a reset boundary.

### 6.2 Judgment forms and the transition relation

The dynamic semantics of Standard 7 is a small-step transition relation over the
configuration of Standard 6.1. Each instruction rule is a transition. A rule states
the operands it consumes from the operand stack, the values it produces, its effect
on the other components, and the closed set of faults it MAY raise. A program runs
by repeated transition until the machine finishes, yields, or reaches a reset
boundary.

### 6.3 The closed fault enumeration

Keleusma defines an outcome for every admissible program on every input. There is no
undefined behavior and no erroneous-execution category. Every fault is drawn from
the single closed and bounded enumeration below, which is the one authoritative list
referenced by each instruction rule's fault heading. A fault is a recoverable result
returned to the host, not a process abort.

| Fault | Category | Decided |
|---|---|---|
| StackUnderflow | halt | verify-time MUST-REJECT, runtime defensive |
| InvalidBytecode | halt | verify-time MUST-REJECT, runtime defensive |
| VerifyError | halt | verify-time MUST-REJECT |
| LoadError | halt | load-time |
| TypeError | soft, script | runtime-admissible |
| DivisionByZero | soft, script | runtime-admissible partial operation |
| IndexOutOfBounds | soft, script | runtime-admissible partial operation |
| FieldNotFound | soft, script | runtime-admissible |
| RefinementFailed | soft, script | runtime-admissible partial operation |
| NoMatchingHead | soft, script | runtime-admissible |
| NoMatchingArm | soft, script | runtime-admissible, unguarded matches proven exhaustive |
| CheckedArithNoArm | soft, script | runtime-admissible, guarded arms only |
| EnumVariantUnmapped | soft, script | runtime-admissible, host-constructed enum only |
| AssertionFailed | soft, script | debug build only |
| NativeError | soft, host | runtime-admissible host failure |
| NativeErrorCode | soft, host | runtime-admissible host failure |
| OutOfArena | halt | runtime-admissible arena exhaustion |
| NotSuspended | halt | host application misuse |

Each fault is classified as either eliminated at verify time, in which case it is
MUST-REJECT and a conforming verifier proves it cannot occur on a verified module,
or admissible at runtime, in which case the transition rule states it and it is a
bounded recoverable result. No fault is undefined. Overflow and underflow of
addition, subtraction, multiplication, and negation are not faults, since they wrap
in two's complement or are reified by the checked construct.

---

## 7. Bytecode Instruction Set Architecture

### 7.1 Encoding

A module body is partitioned into an opcode-record section and a separately
addressed operand pool. An opcode record is exactly 4 bytes. The first byte carries
an even-parity bit in its high bit and a 7-bit opcode identifier in its low seven
bits, so the identifier space is zero through 127. The remaining three bytes carry
either three inline operand bytes or a 24-bit little-endian index into the operand
pool. Every record is parity-checked before dispatch, and a parity mismatch is
MUST-REJECT at decode.

An operand-pool entry is exactly 8 bytes. Its first byte is a type tag, its second
byte is a parity byte over the entry, and its remaining six bytes are the payload.
The three tags carry a pair of 16-bit values, a pair of 16-bit values with a
trailing 8-bit value, and a triple of 16-bit values. The pool holds up to `2^24`
entries.

Dispatch is a single table lookup on the 7-bit identifier. This encoding is fixed so
that a dispatch table in software or a decode read-only memory in hardware realizes
the instruction fetch in constant time. The runtime bytecode version is 1, and the
module magic is the four bytes `KELE`.

### 7.2 Instruction set and operational semantics

The instruction set comprises 67 opcodes. The table below is the normative
inventory. For each opcode it gives the wire identifier, the operands, the operand-
stack effect as the values consumed and produced, and the behavior. A blank effect
denotes no net change. The identifiers 34 through 37 are reserved and MUST NOT be
emitted or accepted, being the retired separate composite constructors.

**Stack and locals.**

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 0 | Const | constant index | push 1 | push the chunk constant |
| 60 | PushImmediate | 1 byte | push 1 | push unit, a boolean, none, or a small integer; codes 20 through 255 reserved |
| 33 | Dup | | push 1 | duplicate the top |
| 61 | PopN | 1 byte | pop n | discard n from the top |
| 1 | GetLocal | slot | push 1 | push a local |
| 2 | SetLocal | slot | pop 1 | pop into a local |

**Data segment.**

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 3 | GetData | slot | push 1 | push a data slot, bounds-checked at load |
| 4 | SetData | slot | pop 1 | store into a data slot, persists across reset |
| 70 | SetDataComposite | slot, byte offset | pop 1 | copy a flat composite body into the persistent pool, surviving reset in place |
| 5 | GetDataIndexed | base, len | pop 1 push 1 | pop an index, push `data[base+index]` with a runtime bound |
| 6 | SetDataIndexed | base, len | pop 2 | pop an index and a value, store with a runtime bound |
| 7 | BoundsCheck | bound | | peek the top as an integer and fault if negative or at least the bound |

**Arithmetic, wrapping and floating.**

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 8 | Add | | pop 2 push 1 | byte, fixed, or float addition |
| 9 | Sub | | pop 2 push 1 | byte, fixed, or float subtraction |
| 10 | Mul | | pop 2 push 1 | byte or float multiplication |
| 11 | Div | | pop 2 push 1 | division, fault on zero divisor |
| 12 | Mod | | pop 2 push 1 | remainder, fault on zero divisor |
| 13 | Neg | | | negate byte, fixed, or float |

**Arithmetic, checked.** Each checked opcode pops its integer operands and pushes a
low word, a high word, and a flag, where the flag is zero for ok, one for overflow,
two for underflow, and three for a reified zero divisor carrying the numerator.

| Id | Opcode | Operands | Behavior |
|---|---|---|---|
| 54 | CheckedAdd | | integer addition with carry reification |
| 55 | CheckedSub | | integer subtraction with borrow reification |
| 56 | CheckedMul | fraction bits | integer or fixed multiplication, high half load-bearing for wide products |
| 58 | CheckedDiv | fraction bits | integer or fixed division with the overflow and zero cases reified |
| 57 | CheckedNeg | | negation with the single overflow case reified |
| 59 | CheckedMod | | remainder with the reified cases |

**Comparison, logic, and bitwise.**

| Id | Opcode | Effect | Behavior |
|---|---|---|---|
| 14 through 19 | CmpEq CmpNe CmpLt CmpGt CmpLe CmpGe | pop 2 push 1 | comparison to a boolean |
| 20 | Not | | boolean negation |
| 62 63 64 | BitAnd BitOr BitXor | pop 2 push 1 | word bitwise operations |
| 65 66 | Shl Shr | pop 2 push 1 | logical left and arithmetic right shift, count masked to the word width |

**Control flow.** All control flow is block-structured. The operands are op-index
targets within the chunk.

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 21 | If | target | pop 1 | pop a boolean, jump to the matching else or end when false |
| 22 | Else | target | | jump to the matching end |
| 23 | EndIf | | | delimiter |
| 24 | Loop | target | | entry delimiter |
| 25 | EndLoop | target | | the only backward edge, jump to the instruction after Loop |
| 26 | Break | target | | jump past the enclosing end |
| 27 | BreakIf | target | pop 1 | pop a boolean, jump past the enclosing end when true |

**Coroutine and streaming.**

| Id | Opcode | Effect | Behavior |
|---|---|---|---|
| 28 | Stream | | the stream entry marker, only a reset may target it |
| 29 | Reset | | clear locals, truncate the operand stack to locals, reclaim the arena top, activate a scheduled hot swap, and return the reset state |
| 32 | Yield | | pop the output, reject an output carrying an ephemeral arena string, return the yielded state, and on resume push the host input |

**Calls and returns.**

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 30 | Call | chunk index, arg count | pop n push 1 | direct call, arity-checked at load |
| 31 | Return | | | pop the result and the frame, finish when the last frame |
| 67 | CallVerifiedNative | native index, arg count | pop n push 1 or 2 | call a verified native whose attested cost folds into the iteration budget |
| 68 | CallExternalNative | native index, arg count | pop n push 1 or 2 | call an external native bounded by a per-iteration invocation count |

The high bit of the argument count on the two native calls is the error-reify flag.
When set the call pushes a code and a flag instead of a single result. There is no
indirect call and no first-class function.

**Composite construction and access.**

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 69 | NewComposite | kind, count, size or meta | pop n push 1 | the single constructor for tuple, array, struct, and enumeration, flat or boxed |
| 38 | GetField | struct-field operand | pop 1 push 1 | read a struct field, flat by offset or boxed by name |
| 39 | GetIndex | array-element operand | pop 2 push 1 | read an array element by index |
| 40 | GetTupleField | tuple-field operand | pop 1 push 1 | read a tuple element |
| 41 | GetEnumField | enum-field operand | pop 1 push 1 | read an enumeration payload field past the discriminant word |
| 42 | Len | | pop 1 push 1 | push a composite length, not emitted for a flat array of constant length |

**Type testing.** These peek the value and push a boolean without popping.

| Id | Opcode | Operands | Effect | Behavior |
|---|---|---|---|---|
| 43 | IsEnum | enum, variant, discriminant | push 1 | test the enumeration variant |
| 44 | IsStruct | type | push 1 | test the struct type |

**Casting and fixed-point.**

| Id | Opcode | Operands | Behavior |
|---|---|---|---|
| 45 46 | IntToFloat FloatToInt | | word and float conversion, Float profile only |
| 47 48 | WordToByte ByteToWord | | truncate and zero-extend |
| 49 50 | WordToFixed FixedToWord | fraction bits | scale by shifting, saturating and truncating toward negative infinity |
| 51 | FixedMul | fraction bits | fixed multiplication through a wide intermediate, saturating |
| 52 | FixedDiv | fraction bits | fixed division through a wide intermediate, saturating, fault on zero divisor |

**Faults.**

| Id | Opcode | Operands | Behavior |
|---|---|---|---|
| 53 | Trap | kind code | halt with a trap kind, being refinement failed, no matching head, no matching arm, checked arithmetic no arm, enum variant unmapped, zero divisor, or assertion failed |

### 7.3 Structural constraints

A conforming module MUST satisfy the following structural constraints, each decided
completely by the verifier of Standard 8.

- Operand-stack discipline. A forward pass tracks the absolute operand depth from
  zero at entry. Any instruction that consumes more operands than are present is
  MUST-REJECT.
- Loop-body operand-stack neutrality. A loop body resumes at its entry depth across
  the back edge, and each break edge records its depth so the post-loop depth is
  well-defined.
- Branch-join convergence. The two arms of a conditional reach the join at the same
  operand depth, and a path that exits by break, trap, or return is absorbed.
- Operand-index bounds. Every memory-indexing operand is in range, being data-slot
  indices, indexed-access ranges, constant-pool indices, local-slot indices, and
  composite-metadata indices.
- Call arity. A call targets a valid chunk and passes no more arguments than the
  callee declares as locals.
- Block structure. Every conditional and loop is balanced and every branch target is
  in bounds and matches its delimiter, and the loop back edge targets the
  instruction after its loop entry.
- Block-type constraints. An atomic chunk contains no yield, stream, or reset. A
  reentrant chunk contains at least one yield and no stream or reset. A stream chunk
  contains exactly one stream, exactly one reset, and at least one yield, and every
  path from the stream to the reset passes through a yield.

### 7.4 Reserved opcodes and undefined encodings

The identifier space is zero through 127. The live identifiers are zero through 70
excluding the reserved 34 through 37. A record bearing a reserved or unassigned
identifier is a corrupted record and is MUST-REJECT at decode, not an
unimplemented-but-valid opcode. A reserved immediate operand value, an unknown
operand-pool tag, and an unknown trap or scalar-kind code are likewise MUST-REJECT.
The assignment and reservation of identifiers is governed by Annex C.

---

## 8. The Verifier

### 8.1 Structural and memory-safety invariants

A conforming verifier decides the structural and memory-safety invariants of
Standard 7.3 completely. A program with any such violation is MUST-REJECT. This set
is decidable, so it is never in the MAY-REJECT band. The invariants include
operand-stack discipline with loop-body neutrality and branch-join convergence, and
operand-index bounds for every memory-indexing operand.

### 8.2 The typed operand-stack pass

A conforming verifier reconstructs the type of every operand-stack entry by a
bytecode-level type-preservation abstract interpretation, after the manner of the
Java Virtual Machine and WebAssembly verifiers. The pass validates every baked
composite and field offset against the canonical layout of the accessed type per
Standard 5.2.6, so that a runtime access uses a baked offset that was proven at
load. The pass subsumes operand-stack depth discipline, operand-index bounds, and
baked-offset agreement in one obligation. This is a complete and decidable
MUST-REJECT obligation and never a MAY-REJECT one.

**Note.** This pass is a known non-conformance per Annex A. The reference
implementation tracks only operand depth and does not reconstruct operand types, so
baked-offset correctness is presently a compiler-integrity assumption.

### 8.3 Worst-case execution time and worst-case memory usage

A conforming verifier computes a conservative upper bound on the worst-case
execution time and the worst-case memory usage of a stream iteration. The existence
of a bound is a portable property, and the numeric value of the bound is a
target-defined characteristic recorded in Annex B. The analysis covers the operand
stack and the frame stack as well as the arena and the memory. A loop whose
iteration count is not statically extractable, a text operation whose length is not
statically boundable, and a recursive call graph are each MUST-REJECT. Worst-case
execution time is reported in pipelined cycles under a cost model the host
calibrates to a target. Worst-case memory usage is reported in bytes.

### 8.4 The soundness obligation

A conforming verifier satisfies the soundness obligation, being that verifier
acceptance implies the semantic safety property holds. This is stated as a normative
property. A mechanized argument connecting the verifier to the semantics of Standard
6 is a planned activity.

### 8.5 The scope of the guarantee

The verifier guarantee is safety, structural validity, totality, and the worst-case
bounds. It is explicitly not functional correctness, being whether the program
computes its intended result, which the program author establishes by their own
testing.

### 8.6 The accept-reject decision

A conforming implementation applies Standard 2.3. A program that is well-formed,
well-typed, structurally valid, and proven bounded is MUST-ACCEPT. A program with a
proven structural or memory-safety violation or a proven absence of a bound is
MUST-REJECT. A program whose bound is neither proven present nor proven absent is
MAY-REJECT, and rejecting it is always safe. A conforming implementation MAY expose
a policy that admits a module with an unprovable declared bound, but such admission
is outside the MUST-ACCEPT guarantee.

---

## 9. Wire Format

### 9.1 Module framing and header

A module is a framed byte image. It begins with a header, followed by an
8-byte-aligned opcode-record section, an 8-byte-aligned operand-pool section, an
8-byte-aligned auxiliary body, and a 4-byte cyclic-redundancy-check trailer over the
whole image. The base header is 64 bytes with the fixed field offsets below. All
multi-byte fields are little-endian.

| Offset | Width | Field |
|---|---|---|
| 0 | 4 | magic `KELE` |
| 4 | 2 | version, being 1 |
| 6 | 2 | header length, being 64, 136 signed, or 224 signed and encrypted |
| 8 | 4 | total length |
| 12 | 1 | word width as a base-2 logarithm |
| 13 | 1 | address width as a base-2 logarithm |
| 14 | 1 | float width as a base-2 logarithm |
| 15 | 1 | flags |
| 16 | 4 | worst-case execution time in cycles |
| 20 | 4 | worst-case memory usage in bytes |
| 24 | 4 | shared data byte count |
| 28 | 4 | private data byte count |
| 32 | 4 | opcode-stream offset |
| 36 | 4 | opcode-stream length, a multiple of 4 |
| 40 | 4 | operand-pool offset |
| 44 | 4 | operand-pool length, a multiple of 8 |
| 48 | 4 | auxiliary body offset |
| 52 | 4 | auxiliary body length |
| 56 | 4 | auxiliary arena byte count |
| 60 | 4 | persistent composite byte count |

The flag bits are the ephemeral bit at value 1, the requires-signature bit at value
2, and the encrypted bit at value 4. Other flag bits are reserved and MUST be zero.
The schema hash is not a header field. It lives in the auxiliary body and is a
cyclic-redundancy check over the canonical serialization of the data-segment slot
names and visibilities.

### 9.2 Tables

The auxiliary body is a zero-copy archived structure carrying the tables. It holds
the chunk table, each chunk carrying its name, its per-chunk constant pool, its
struct templates, its local and parameter counts, its block type, its parameter
types, the offset and count that locate its records in the opcode stream, and an
optional strippable debug pool. It holds the native-name table, the entry point, the
data-layout table, the per-enumeration layout table, and the mirrored header
scalars including the schema hash. The header-mirrored fields are cross-checked
against the auxiliary body at load, and a disagreement is MUST-REJECT.

Every layout or offset table carried in the auxiliary body, being the data-layout
table, the per-enumeration layout table, and the shared-field byte offsets, is a
producer-supplied hint and is never trusted. A conforming consumer recomputes the
layout from the type per Standard 5.2. It either ignores the carried table or
MUST-REJECT a module whose carried table disagrees with the computed layout. No
shared-field byte offset, enumeration discriminant, or variant padding carried in the
wire is authoritative.

### 9.3 Cryptographic framing and loader policy

The cryptographic framing is the Signing and Encryption profiles. It is not part of
Core.

A signed module extends the header to 136 bytes with an 8-byte signature-metadata
block at offset 64 and a 64-byte Edwards-curve Digital Signature Algorithm signature
at offset 72. The signed message is the whole image with the signature bytes and the
trailer bytes zeroed. Verification uses the strict form that rejects malleable and
small-order inputs.

An encrypted module further extends the header to 224 bytes with an 88-byte
encryption-metadata block at offset 136. The scheme is an X25519 key agreement, a
Hash-based Key Derivation Function over SHA-256, and the Advanced Encryption
Standard in Galois/Counter Mode at a 256-bit key. The metadata carries the scheme
identifier, the ephemeral public key, the recipient key identifier as a SHA-256
fingerprint, and the authenticated-encryption nonce. Encryption requires signing,
and the signature authenticates the ciphertext and the metadata.

The loader validates a module in the following order. It strips a shebang, checks
the magic, checks the version against the runtime bytecode version, checks the
header and total lengths, verifies the trailer, validates the cryptographic
extension for consistency with the flags, bounds every section, admits the target
widths only when each is no wider than the runtime supports, decodes the auxiliary
body, and cross-checks the header against it. A signed module is verified against the
host trust matrix. Under host policy an unsigned module is rejected when the trust
matrix is non-empty, so signature enforcement is host policy and not the module's
self-asserted flag.

---

## 10. Stable Application Binary Interface

### 10.1 The host and native-function marshalling boundary

Domain functionality is provided by native functions the host registers. The
marshalling boundary carries host values across to Keleusma values and back. The
boundary is defined by a marshalling trait parameterized over the target word and
float widths, with a required decode and encode and provided methods that build a
composite body directly in the arena. A signatured native is registered with its
Rust signature, and the runtime marshals its arguments and result automatically. An
unsignatured native is registered as a function over runtime values and does its
own argument checking.

Values marshal at the module's declared widths, not the host runtime widths. On a
narrow-word target the marshalling casts each element from the host width to the
module width with the same wrapping the virtual machine applies to in-script
arithmetic, and on the reference runtime the two widths coincide. The faults of the
marshalling boundary belong to the closed set of Standard 6.3 and are never a panic
path.

### 10.2 Native calling convention and memory model

A native receives its arguments as runtime values and a context carrying the arena,
the opaque registry, and the target widths, and returns a runtime value or a fault.
A native allocates dynamic strings and composite bodies from the arena through the
context. A native result is canonicalized into an arena-resident flat body so that
no global-heap body crosses a reset. A yielded or finished composite stays
arena-resident, and the host MUST decode it before the next resume, which resets the
arena, or before dropping the machine. A read after the reset is a bounded fault and
never undefined behavior.

The shared buffer is lent to a native call as a mutable byte slice for the dynamic
extent of the call only. The slice length MUST equal the module's declared shared-
data byte count, and the two slices of the buffer MUST NOT be held at once, so the
mutable view is never aliased. The runtime retains no pointer into the buffer across
a yield, and between resumes the host owns the buffer and MAY swap, mutate, or drop
it.

### 10.3 Built-in functions and the standard-library boundary

Keleusma has no bundled standard library in Core. The only function bundled by
default is a print native, which is a no operation in a hosted-free build. Bundled
but host-registerable are a math bundle, an audio bundle, and, under a hosted
feature, a shell bundle. Every other function, including all text composition and
all domain logic, MUST be host-registered. A native with a domain precondition
reports a precondition failure as a host fault, which is distinct from a virtual-
machine trap.

---

## 11. The Host Contract and the Trust Boundary

### 11.1 Native obligations

The guarantees of this document hold for verified bytecode under the assumption that
every host-registered native honors its declared contract. The contract of a native
is its declared worst-case cost, its termination, and its memory behavior within the
model of Standard 10.2. A verified native attests a worst-case execution time and a
worst-case memory usage that fold into the iteration budget, and a native that
allocates from the arena MUST attest a non-zero memory figure for the analysis to
remain sound. An external native is bounded instead by a declared per-iteration
invocation count.

### 11.2 Host runtime obligations

The host MUST supply memory as the model requires. The shared buffer MUST be a
correctly sized and non-aliased mutable byte slice across the call and resume
boundary, per Standard 10.2. The arena MUST be sized to at least the worst-case
memory usage the module declares. The host MUST resume the machine after each yield
for the totality guarantee of Standard 4.6 to hold.

### 11.3 The conditional guarantee

A native is host-provided code the verifier does not analyze. A native that exceeds
its declared cost breaks the worst-case execution time bound. A native that violates
the memory model breaks safety. A violation of the host contract, whether by a
native or by the host runtime, places the execution outside the guaranteed set. This
is the trust boundary. Within the boundary the guarantees of Standard 8 hold. Across
it they do not.

---

## 12. Native Code Generation Contract

**This section is the Native profile. A Core implementation does not implement it.**

### 12.1 Semantics-preservation requirement

A native code generator translates verified bytecode to target machine code. It MUST
preserve the observable semantics of Standard 6 and Standard 7. The generated code
MUST produce the same value, the same yields in the same order, and the same faults
from the closed set of Standard 6.3 as the reference virtual machine, for every
admissible program on every input.

### 12.2 Target memory model

The generated code reserves a bss-based arena sized by the worst-case memory usage
of Standard 8. It performs no heap allocation on the steady-state path. The section
model of Standard 5.3 maps onto the target's text, rodata, data, and bss regions.

### 12.3 Section realization per target class

A mainstream target realizes the sections through the ordinary translation of
bytecode to native code. An exotic target with a small word, for example an 8-bit
target, MAY realize the instruction set through a bespoke assembly translation, in
which case the width-parameterized layout of Standard 5.2 fixes the byte images at
the target widths.

### 12.4 Module replacement under native code generation

Module replacement under native code generation is a design point of the Native
profile. Hot swap in a natively generated image needs a defined mechanism for
retargeting the text region and preserving the persistent region, which the issued
Native profile specifies.

---

## 13. Conformance Suite requirements

The conformance suite is the objective verification evidence for this document. The
suite is normative. This document is the authority, and a suite case that conflicts
with this document is a suite defect and never blesses a non-conforming
implementation.

The suite has positive cases that every implementation MUST accept and run to the
stated result, and negative cases that every implementation MUST reject. A negative
case asserts only the MUST-REJECT band and never the MAY-REJECT band, so the suite
never forbids a better analysis. Each MUST-ACCEPT and MUST-REJECT rule of this
document cites at least one suite case, and each suite case cites the rule it
exercises. A rule with no case and a case with no rule are both defects tracked
against issuance. The suite is partitioned by profile, so a Core-only implementation
is exercised only by Core cases.

The negative corpus is seeded from the recorded hostile-input classes that regressed
during development, so the suite exercises by construction the classes the model
closes, being an out-of-bounds nested-composite operand, a mismatched flat text
offset, a shared-layout mismatch, a stack underflow, a loop that is not neutral
across its back edge, and a signature-flag bypass.

---

## Annex A. Change record and known non-conformances

### A.1 Change record

This is the first draft. No standard version has been issued. The first issuance
records the initial version here.

### A.2 Known non-conformances

The reference implementation is known to diverge from the intended semantics at the
points below. Each is a known non-conformance under Standard 1.3, not an error in
this document.

1. The typed operand-stack pass of Standard 8.2 and the baked-offset validation of
   Standard 5.2.6 are not implemented. The reference verifier tracks only operand
   depth, and baked-offset correctness is a compiler-integrity assumption. This is
   the single largest verifier work item.
2. The multi-word fixed-point family of Standard 5.1.2 is implemented for its whole
   arithmetic surface. The type `Multiword<N>` and `Multiword<N, F>`, its
   construction and indexing, its scale-independent addition and subtraction, the
   six comparison operators, multiplication at every fraction-bit count (integer and
   fixed-point), the divide and modulo at every fraction-bit count, the four
   shift operators with a constant or runtime-variable amount, and the bitwise
   operators `band`, `bor`, `bxor`, and `bnot` are implemented; `Byte` is admitted
   by the scalar shift and bitwise operators. One item remains: the type is
   recognised specially rather than through general const generics, which stay a
   separate feature tracked as B40. The overflow-capturing form of `asl` inside the
   checked-arithmetic construct still requires a compile-time-constant amount,
   because it lowers to a multiply by the constant `2^k`.
3. The instruction count in prior project documents was recorded as 66. The instruction
   set is 67, because the `SetDataComposite` opcode at identifier 70 is implemented,
   dispatched, and verified but was undocumented. The issuing authority reconciles the
   opcode budget at issuance.

### A.3 Documentation drift corrected against the code

The following prior specification statements were stale and are superseded by this
document, which uses the verified code as ground truth. The enumeration discriminant
is word-sized, not one byte. The `IsEnum` operand-pool entry is a triple of 16-bit
values, not a pair. The wire header fields at offsets 56 and 60 carry the auxiliary
arena byte count and the persistent composite byte count, not reserved zeros. The
composite value slot is 32 bytes, not 40.

---

## Annex B. Implementation-defined and target-defined characteristics

A conforming implementation documents each characteristic below. These are the
points a downstream integrator reviews.

- The three target widths, being the word width, the float width, and the address
  width.
- The numeric value of each worst-case execution time bound, being target-defined
  through the calibrated cost model, while the existence of the bound is portable.
- The numeric value of each worst-case memory usage bound at the target widths.
- The arena capacity.
- The strength of the analysis in the MAY-REJECT band, being which unproven programs
  the implementation rejects.
- The loader policy for signing, being whether an unsigned module is admitted.
- Every point of target-defined behavior, which remains total and within the
  worst-case bounds and is never a license for undefined behavior.

---

## Annex C. Encoding registry

The 7-bit opcode identifier space is zero through 127. The assigned identifiers are
zero through 70, excluding the reserved 34 through 37, which are the retired separate
composite constructors and MUST NOT be reused without an issuance. An identifier in
71 through 127 is unassigned. A record bearing a reserved or unassigned identifier is
MUST-REJECT at decode per Standard 7.4. The operand-pool tags are the pair of 16-bit
values, the pair with a trailing 8-bit value, and the triple of 16-bit values. The
wire section tags and their assignment are governed here. A new identifier or tag is
assigned only by issuance.
