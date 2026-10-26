#!/usr/bin/env python3
"""Generate the book's Instruction Set chapter from the authoritative spec.

Single source of truth: `docs/spec/INSTRUCTION_SET.md`. The book chapter
`book/src/INSTRUCTION_SET.md` is a mechanical transform of it, so the two can
never drift. This script applies that transform; CI runs it and fails the build
on any diff (the Rex-review "generate reference material from source and fail on
drift" lesson).

The transform is intentionally small and total:
  1. Replace the docs-graph navigation line with the book-context intro.
  2. Repoint the in-repo cross-links: the compilation narrative to the book's
     Chapter 25, and the spec/architecture/source docs (which are not book
     chapters) to their canonical GitHub URLs.
  3. Prepend a generated-file marker so the copy is not hand-edited.

Run: `python3 scripts/gen-book-instruction-set.py` from the repo root.
"""

import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SPEC = REPO / "docs" / "spec" / "INSTRUCTION_SET.md"
CHAPTER = REPO / "book" / "src" / "INSTRUCTION_SET.md"
GH = "https://github.com/sgeos/keleusma/blob/main"

MARKER = (
    "<!-- GENERATED FILE. Do not edit. Source: docs/spec/INSTRUCTION_SET.md. "
    "Regenerate with: python3 scripts/gen-book-instruction-set.py. CI fails on drift. -->\n"
)

NAV_LINE = "> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)"
INTRO = (
    "This chapter is the bytecode reference. It lists every instruction the "
    "Keleusma virtual machine executes, so the disassembly shown in the "
    "[playground](https://sgeos.github.io/keleusma/playground/) bytecode view "
    "has a place to be looked up. It reproduces the authoritative "
    "`docs/spec/INSTRUCTION_SET.md` from the repository."
)

# (source substring -> book substring). Order-independent; each must be present.
LINKS = {
    "[COMPILATION_PIPELINE.md](../architecture/COMPILATION_PIPELINE.md)":
        "[Chapter 25, From Source to Bytecode](25_from_source_to_bytecode.md)",
    "[EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md)":
        f"[EXECUTION_MODEL.md]({GH}/docs/architecture/EXECUTION_MODEL.md)",
    "[STRUCTURAL_ISA.md](./STRUCTURAL_ISA.md)":
        f"[STRUCTURAL_ISA.md]({GH}/docs/spec/STRUCTURAL_ISA.md)",
    "[`CompileWarning`](../../src/compiler.rs)":
        f"[`CompileWarning`]({GH}/src/compiler.rs)",
    "[WIRE_FORMAT.md](./WIRE_FORMAT.md)":
        f"[WIRE_FORMAT.md]({GH}/docs/spec/WIRE_FORMAT.md)",
}


def generate() -> str:
    text = SPEC.read_text(encoding="utf-8")
    if NAV_LINE not in text:
        sys.exit("gen-book-instruction-set: spec navigation line not found; "
                 "the transform is stale and must be updated.")
    text = text.replace(NAV_LINE, INTRO, 1)
    for src, dst in LINKS.items():
        if src not in text:
            sys.exit(f"gen-book-instruction-set: expected link not found in spec: {src}")
        text = text.replace(src, dst)
    return MARKER + text


def main() -> None:
    out = generate()
    CHAPTER.write_text(out, encoding="utf-8")
    print(f"wrote {CHAPTER.relative_to(REPO)} from {SPEC.relative_to(REPO)}")


if __name__ == "__main__":
    main()
