# BRIEF — a C host linking a Keleusma object file, solving a real problem

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The gap this closes, which is packaging rather than capability

**The capability is already proved.** `aot_linkage.rs` compiles Keleusma source, lowers it, emits a
genuine object file, links it against a C `main` with the system linker, runs the result as a
separate process, and compares the answer to the virtual machine. All three of its tests pass on the
machine of record rather than skipping. It covers a plain module, a module with natives, and a module
with a data segment.

**What does not exist is an artefact a person could copy.** The tests build and link in a temporary
directory. `V0_3_X_ROADMAP.md` success criterion 2 says native artefacts *"link as static libraries
against a host"*, and the criterion is met while nothing demonstrates it outside a test harness.

## The problem, and why this one

**A motor drive's protection policy: thermal derating and overcurrent trip.**

Every drive has one. It runs each control cycle, it is tuned per deployment and per customer, and
the people who most want to change it are furthest from the firmware team. **That is precisely the
shape where a host wants user-supplied logic and cannot take the risk**, because an unbounded loop
or an allocation inside a control loop is a safety incident rather than a slow response.

**It exercises what Keleusma actually sells rather than what it merely supports.** The policy is
total, so it terminates by construction. Its memory is statically bounded, so it cannot exhaust the
controller. The verifier rejects a policy whose bound cannot be proved, which is the guarantee the
firmware team needs before accepting a field-updatable rule at all.

**And it demonstrates the operator's own framing of the `Fixed` ruling.** The shared data segment is
the input and output contract, with the host knowing the interpretation of the bits from a header,
exactly as a C program's headers lay out an interface to a separately compiled procedure. Engineering
units are `Fixed`, which is the type that ruling was about.

## What it must contain to be worth shipping

A `.kel` policy using a shared data segment as its contract, fixed-point engineering units, a struct
per thermal zone, and a bounded loop over a fixed array of zones. A C host that declares the contract
in a header, fills the inputs, calls the entry through the platform ABI, and reads the outputs. A
native callback so the policy can report a fault code to the host. A build script and a README
stating the problem, the contract, and what is proven rather than merely tested.

**The proven bounds should be printed.** The point of the example is not that it runs; it is that
its worst-case time and memory were known before it ran.

## Where it lives, and why that is not a free choice

**Under `native_codegen/`, not the repository's `examples/`.** The root `examples/` are built by the
workspace and ship in the crate tarball, and this example requires LLVM. Putting it there would make
LLVM a dependency of the whole repository, which is the exact property the package's detachment
exists to preserve.

## Prior failures to avoid repeating

- **Do not write the `.kel` from memory.** Two probes this session were refused by the reference
  compiler because of my syntax rather than the language: `Bool` is spelled `bool`, `for` iterates an
  array, there is no early return inside a loop and no reassignment. **Compile the policy before
  writing prose about it.**
- **Do not let the example skip quietly.** If no C compiler is present it must say so loudly, for the
  same reason `aot_linkage.rs` does: a step that quietly does nothing reads as a step that passed.
- **The private region must be word-aligned.** A `char *` base violates the alignment the ABI
  assumes, and the failure presents as a bus fault rather than a wrong answer.
- **Streams do not lower.** `Stream` and `Reset` are refused, so the policy must be a plain function
  called once per cycle, not a coroutine. Designing around a stream would produce an example that
  cannot be built.
- **Do not claim a bound the tree does not produce.** The arena figure the backend demands can exceed
  the runtime's verified heap figure in 11 of 71 corpus modules. If that gap applies here, the README
  states it rather than quoting the smaller number.

## The wrong turn most likely here

**Writing a demonstration that is really a test.** The audience is someone deciding whether to embed
Keleusma in a controller. It should read as a worked answer to "can I let a customer change this
rule", not as a proof that the linker works.
