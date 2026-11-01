#!/usr/bin/env python3
"""Correct the Workstream C mislabelling. Six sites, four of them shipped strings.

PREPARED WHILE ANOTHER SESSION'S GATE HELD THE MACHINE. Not yet applied.

WHY THIS IS NOT COSMETIC. `V0_3_X_ROADMAP.md` defines Workstream C as
"Arena-resident coroutine frames and the native arena model". Composites are not
a workstream at all -- they fall under A, "Bytecode-to-LLVM-IR lowering", whose
full pass lowers every opcode of the full-language ISA.

CORRECTED 2026-08-10: this docstring named `V0_4_0_NATIVE_CODEGEN.md` as the
defining document. That is WRONG -- the architecture document uses the string
"Workstream" zero times. The lettering is defined in `V0_3_X_ROADMAP.md` as
markdown headings, `### A. Bytecode-to-LLVM-IR lowering` through `### F.`. The
inventory had the right document all along; only this file was wrong, which is
how an artefact prepared away from its source drifts from it.

BEWARE A COLLISION THAT IS NOT MINE TO FIX. `V0_4_X_ROADMAP.md` defines its OWN
`### A.` through `### F.` for a different taxonomy entirely -- its A is
"Sub-coroutines (callable ephemeral `loop`)", its B is "Three-mode purity
discipline". So a bare letter is ambiguous ACROSS roadmaps even when it is right
within one. That is why the replacement text below spells out the workstream
rather than using the letter alone.

Four of these sites are strings inside a `LowerError`, so a CONSUMER who hits a
refusal about composite bodies is currently told to consult a workstream about
coroutine frames. The inventory also used the label correctly, twice, for arena
residency -- so one identifier meant two incompatible things in one codebase,
which is worse than using the wrong one consistently.

The replacement says "Workstream A (full pass)" rather than bare "A", because a
lone letter is exactly what made the original error easy to make and hard to see.

Run from the worktree root:
    python3 <this file>
"""

import sys

EDITS = [
    (
        "native_codegen/src/lib.rs",
        'why: String::from("shared composite body; Workstream C"),',
        'why: String::from("shared composite body; Workstream A (full pass)"),',
        "shared composite body refusal",
    ),
    (
        "native_codegen/src/lib.rs",
        '6 => String::from("Text slot; string representation is Workstream C"),',
        '6 => String::from("Text slot; string representation is Workstream A (full pass)"),',
        "Text slot refusal",
    ),
    (
        "native_codegen/src/lib.rs",
        '"shared array of composite bodies; Workstream C",',
        '"shared array of composite bodies; Workstream A (full pass)",',
        "shared composite array refusal",
    ),
    (
        "native_codegen/src/lib.rs",
        "// Composite and string constants are Workstream C and are refused.",
        "// Composite and string constants are Workstream A (full pass) and are\n"
        "            // refused. NOT Workstream C, which is arena-resident coroutine\n"
        "            // frames; composites are not a workstream of their own.",
        "composite constant comment",
    ),
    (
        "native_codegen/tests/spike_corpus_coverage.rs",
        '| Op::IsStruct(..) => "C (composites)",',
        '| Op::IsStruct(..) => "A full pass (composites)",',
        "coverage spike bucket label",
    ),
    (
        "native_codegen/tests/differential.rs",
        "// Workstream C, the flat byte composite representation.",
        "// Workstream A (full pass), the flat byte composite representation.\n"
        "    // Not Workstream C, which is arena-resident coroutine frames.",
        "differential comment",
    ),
]


def main():
    by_file = {}
    for path, old, new, label in EDITS:
        by_file.setdefault(path, []).append((old, new, label))

    for path, edits in by_file.items():
        s = open(path).read()
        for old, new, label in edits:
            assert old in s, f"ANCHOR MISSING in {path}: {label}"
            assert s.count(old) == 1, (
                f"ANCHOR AMBIGUOUS in {path}: {label} ({s.count(old)} matches)"
            )
            s = s.replace(old, new, 1)
        open(path, "w").write(s)
        print(f"{path}: {len(edits)} replacement(s)")

    print()
    print("NEXT: cargo fmt && cargo clippy --tests -- -D warnings && cargo test")
    print("Expect NO behavioural change; these are diagnostic strings and comments.")
    print("The coverage spike's bucket label changes, which alters only its report.")


if __name__ == "__main__":
    sys.exit(main())
