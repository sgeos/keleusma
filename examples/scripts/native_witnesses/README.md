# Native witness programs

> **Navigation** [Example scripts](../README.md) | [Native code generation](../../../native_codegen/README.md)

These programs exist to witness opcodes, not to demonstrate a language feature.
They belong to the V0.3.X line, which added them so the native lowering
censuses have a program that emits each instruction. Several are deliberately
inadmissible or deliberately refused, and that is their purpose rather than a
defect. Each carries a machine-checked `// WITNESSES:` line that
`witness_integrity.rs` verifies against what the file actually emits.

| File | Topic | Feature |
|------|-------|---------|
| [`opcode_witness.kel`](./opcode_witness.kel) | Opcode witness, lowered | Emits opcodes no application emits, all of which the native backend lowers |
| [`refused_witness.kel`](./refused_witness.kel) | Opcode witness, refused | Concentrates the opcodes the backend once refused, so their refusal could not exempt an otherwise lowering module. It cannot be given an arena while `len_witness` is present, deliberately |
| [`float_witness.kel`](./float_witness.kel) | Float opcodes | Split out because one float constant refused a whole module and took four unrelated witnesses with it |
| [`fixed_arithmetic.kel`](./fixed_arithmetic.kel) | Fixed-point arithmetic | `Fixed<N>` multiply, divide and the `Word` conversions |
| [`fixed_conversions.kel`](./fixed_conversions.kel) | Fixed-point conversions | `WordToFixed` and `FixedToWord` across widths |
| [`external_native_witness.kel`](./external_native_witness.kel) | External native call | `CallExternalNative`, which needs host registration rather than a verified native |

## Why they live in a subdirectory

Until absorption 95 they sat directly in `examples/scripts/`, a directory the
V0.2.X line asserts over. Three times a V0.2.X test that scanned that
directory flat was green on its own tree and red on the merged tree, because
only the merged tree holds these files. The last was the full-language table
in `tests/selfhost_full_language.rs`, which pins every source it finds.

Flat scans of `examples/scripts/` do not reach this subdirectory. Every
native corpus loader walks recursively and still reaches it. A scan of the
parent that does recurse would see these files, so a test that pins a count
over `examples/scripts/` should still scope it by name.
