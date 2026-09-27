#!/usr/bin/env python3
"""Verify that the Keleusma code printed in the book still describes the
implementation, by executing it against the current CLI.

WHAT IS ASSERTED, AND WHY EACH CLASS EXISTS

  1. A block whose prose states an output ("output is `X`", "prints `X`") is run
     and its output compared. This is the original guarantee.
  2. A REPL block has each expression piped through `keleusma repl` and each
     printed result compared against the line beneath it.
  3. EVERY block carrying a `main` entry point is executed, with an expectation
     recorded beside it as an HTML comment:

         <!-- verify: accept -->            must run, exit zero
         <!-- verify: compile -->           must compile; cannot run standalone
         <!-- verify: reject -->            must be refused
         <!-- verify: reject: <substring> --> must be refused, naming that text
         <!-- verify: skip: <reason> -->    not checkable, with the reason

     Any of the above may be prefixed with `prelude`, meaning "prepend the
     preceding complete program in this chapter, with its own entry point
     removed". Several chapters advance by saying "change the program to ..." and
     printing only the replacement entry point. Run standalone such a block fails
     on the definitions it no longer has, which is the HARNESS misreading a
     fragment as a program rather than a defect in the book. With `prelude` the
     block is assembled the way the prose tells the reader to assemble it, and the
     chapter's claim becomes checkable instead of excused.

     A block with an output claim needs no marker; the claim is the stronger
     assertion. An UNMARKED, UNCLAIMED complete program is a FAILURE, because a
     program nobody checks is how a guide comes to print code that does not work.

  The comment renders as nothing and is not translated material: `book/po/ja.po`
  carries no HTML comment and no fence line.

WHY THE `reject` SUBSTRING MATTERS. A program refused for a reason other than the
one its chapter is about demonstrates nothing, and is indistinguishable from a
passing assertion. Where a chapter quotes a diagnostic, the marker should pin it.

WHY THERE ARE FLOORS. `checked` used to be printed and never compared, so a
rewording that defeated the extraction, or a wrong source path, reported
`checked 0 claimed examples; 0 failure(s)` and exited zero. A run that asserted
nothing is not a pass. The floors below fail such a run.

WHY AN ACCEPTANCE COUNT IS REQUIRED. Whole chapters assert that the
implementation REFUSES a program. A harness that refuses everything -- a missing
binary, a bad flag -- satisfies every one of those. Requiring a positive number of
ACCEPTED programs in the same run is what makes the refusals mean something.

Bare fences are Keleusma; Rust, shell, and text snippets are language-tagged.

Usage: verify_examples.py <path-to-keleusma-binary> [book/src]
Exit 0 on success, 1 on any failure.
"""
import re
import os
import sys
import glob
import subprocess
import tempfile

CLI = sys.argv[1] if len(sys.argv) > 1 else "keleusma"
SRC = sys.argv[2] if len(sys.argv) > 2 else "book/src"

# Floors. Derived from the tree on 2026-09-26 and set below the measurement with
# room for ordinary movement, so that a COLLAPSE fails and an edit does not.
# Measured then: 166 bare blocks, 60 complete programs, 51 claim/REPL assertions.
MIN_BLOCKS = 140
MIN_PROGRAMS = 50
MIN_CLAIM_ASSERTIONS = 40
MIN_ACCEPTED = 20   # the positive control on the refusals
MAX_SKIPPED = 6     # an excused program is a debt, not a category

ENTRY = re.compile(r"^\s*(?:signed\s+|impure\s+)*(?:fn|yield|loop)\s+main\b", re.M)
MARKER = re.compile(r"<!--\s*verify:\s*(.*?)\s*-->")


def strip_entry_point(src):
    """Remove the `main` definition from a program, leaving its other definitions.

    Used by `prelude`. Brace depth is counted directly; the programs in the guide
    contain no brace inside a string or a comment, and a failure to find the
    closing brace is reported rather than guessed at.
    """
    lines = src.split("\n")
    start = None
    for idx, ln in enumerate(lines):
        if ENTRY.match(ln):
            start = idx
            break
    if start is None:
        return (None, "the preceding program has no entry point to replace")
    depth = 0
    seen = False
    for idx in range(start, len(lines)):
        depth += lines[idx].count("{") - lines[idx].count("}")
        if "{" in lines[idx]:
            seen = True
        if seen and depth <= 0:
            return ("\n".join(lines[:start] + lines[idx + 1:]), None)
    return (None, "the preceding program's entry point has no closing brace")


def write_temp(src):
    with tempfile.NamedTemporaryFile("w", suffix=".kel", delete=False) as fh:
        fh.write(src)
        return fh.name


def invoke(args, timeout=60):
    """Return (returncode, combined-output). A timeout is a distinct outcome."""
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return (None, "timed out")
    out = (r.stdout or "") + (r.stderr or "")
    return (r.returncode, out.strip())


def run_program(src):
    path = write_temp(src)
    try:
        return invoke([CLI, "run", path])
    finally:
        os.unlink(path)


def check_without_running(src):
    """Compile and verify a program, then exit, without executing it.

    `run --print-memory` is the right instrument and `compile` is NOT. Measured
    2026-09-26: `keleusma compile` accepts a program that `keleusma run` refuses,
    because it writes bytecode without the structural pass; the no-yield `loop` of
    `WHY_REJECTED.md` is caught by this path and passed by `compile`. Using the
    weaker one would have let a documented rejection read as satisfied.

    It is still weaker than a full `run`: the recursive program of
    `19_why_rejected.md` is refused by `run` and reported a bound by this path.
    That is why a rejection expectation uses `run` and this is reserved for
    programs that cannot be run to completion.
    """
    path = write_temp(src)
    try:
        return invoke([CLI, "run", path, "--print-memory"])
    finally:
        os.unlink(path)


def run_repl(exprs):
    inp = "\n".join(exprs) + "\n"
    r = subprocess.run([CLI, "repl"], input=inp, capture_output=True, text=True, timeout=60)
    outs = []
    for ln in r.stdout.split("\n"):
        if ln.startswith("> "):
            v = ln[2:].strip()
            if v:
                outs.append(v)
    return outs


def fenced_blocks(lines):
    """Yield (code, open_index, close_index) for BARE fenced blocks only.

    The guide fences Keleusma with a bare fence; Rust, shell, and text snippets
    are language-tagged. Tagged blocks are consumed and skipped rather than run.
    """
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("```"):
            info = s[3:].strip()
            j = i + 1
            buf = []
            while j < len(lines) and lines[j].strip() != "```":
                buf.append(lines[j])
                j += 1
            if info == "":
                yield ("\n".join(buf), i, j)
            i = j + 1
        else:
            i += 1


def marker_for(lines, open_index):
    """The `verify:` expectation written just above the opening fence, if any.

    Searched over the three preceding lines so a blank line may separate the
    comment from the fence. Anchored ABOVE the fence deliberately: a marker
    inside the block would be printed to the reader.
    """
    for k in range(open_index - 1, max(-1, open_index - 4), -1):
        m = MARKER.search(lines[k])
        if m:
            return m.group(1).strip()
    return None


def output_claim(lines, close_index):
    window = "\n".join(lines[close_index + 1:close_index + 16])
    m = (re.search(r"[Oo]utput is[:\s]+`([^`]+)`", window)
         or re.search(r"[Oo]utput is[:\s]*\n+```\n([^\n]+)\n```", window)
         or re.search(r"prints\s+`([^`]+)`", window))
    return m.group(1).strip() if m else None


def main():
    failures = []
    claim_assertions = 0
    program_assertions = 0
    blocks = 0
    programs = 0
    accepted = 0
    skipped = []
    verdicts = {"accept": 0, "compile": 0, "reject": 0, "skip": 0, "claim": 0}

    for f in sorted(glob.glob(os.path.join(SRC, "*.md"))):
        name = os.path.basename(f)
        lines = open(f).read().split("\n")
        # The `prelude` form reaches back within a CHAPTER only. Reaching across files
        # would assemble a program from material the reader never saw together.
        previous_program = None
        for code, opened, close in fenced_blocks(lines):
            blocks += 1
            where = f"{name}:{opened + 1}"

            # ---- REPL blocks -------------------------------------------------
            if any(ln.startswith("> ") for ln in code.split("\n")):
                cl = code.split("\n")
                exprs, expected = [], []
                for k, ln in enumerate(cl):
                    if ln.startswith("> "):
                        e = ln[2:].strip()
                        nxt = cl[k + 1] if k + 1 < len(cl) else ""
                        r = nxt.strip() if (nxt and not nxt.startswith("> ") and nxt.strip()) else None
                        exprs.append(e)
                        expected.append(r)
                if not any(expected):
                    continue
                got = run_repl(exprs)
                for idx, (e, exp) in enumerate(zip(exprs, expected)):
                    if exp is None:
                        continue
                    actual = got[idx] if idx < len(got) else "<none>"
                    claim_assertions += 1
                    verdicts["claim"] += 1
                    if actual != exp:
                        failures.append(f"{where} REPL `{e}`: claim {exp!r}, actual {actual!r}")
                continue

            claim = output_claim(lines, close)
            marker = marker_for(lines, opened)
            is_program = bool(ENTRY.search(code))
            if is_program:
                programs += 1

            # `prelude` assembles the block the way the prose tells the reader to.
            source = code
            if marker and marker.startswith("prelude"):
                marker = marker[len("prelude"):].strip().lstrip(",").strip()
                if previous_program is None:
                    failures.append(
                        f"{where}: `prelude` requested with no preceding complete program in "
                        f"this chapter, so there is nothing to prepend.")
                    continue
                assembled, why = strip_entry_point(previous_program)
                if assembled is None:
                    failures.append(f"{where}: `prelude` could not be assembled: {why}")
                    continue
                source = assembled.rstrip() + "\n\n" + code
            if is_program:
                # The ASSEMBLED source, so a chapter that changes the entry point twice in
                # a row still reaches back to a whole program rather than to a fragment.
                previous_program = source

            # ---- an output claim is the strongest assertion available ---------
            if claim is not None and is_program:
                claim_assertions += 1
                verdicts["claim"] += 1
                rc, out = run_program(source)
                if rc != 0:
                    failures.append(f"{where}: claimed `{claim}` but the program failed: {out[:200]}")
                elif out != claim:
                    failures.append(f"{where}: claim {claim!r}, actual {out!r}")
                else:
                    accepted += 1
                continue

            if not is_program:
                # A fragment. Not compilable standalone without inventing a
                # context the book does not state. Counted, never asserted.
                if marker:
                    failures.append(
                        f"{where}: carries a verify marker but has no `main` entry point, so "
                        f"nothing can be executed. Remove the marker or complete the program.")
                continue

            # ---- a complete program with no output claim ----------------------
            if marker is None:
                failures.append(
                    f"{where}: a complete program with neither an output claim nor a "
                    f"`<!-- verify: ... -->` expectation. An unchecked program is how a guide "
                    f"comes to print code that does not work.")
                continue

            program_assertions += 1
            if marker == "accept":
                verdicts["accept"] += 1
                rc, out = run_program(source)
                if rc != 0:
                    failures.append(f"{where}: expected to run, was refused: {out[:200]}")
                else:
                    accepted += 1
            elif marker == "compile":
                verdicts["compile"] += 1
                rc, out = check_without_running(source)
                if rc != 0:
                    failures.append(f"{where}: expected to compile, was refused: {out[:200]}")
                else:
                    accepted += 1
            elif marker == "reject" or marker.startswith("reject:"):
                verdicts["reject"] += 1
                want = marker[len("reject:"):].strip() if marker.startswith("reject:") else None
                # The full pipeline. A refusal at ANY stage satisfies the chapter's claim,
                # and `compile` alone misses the resource-bound verifier entirely.
                rc, out = run_program(source)
                if rc == 0:
                    failures.append(
                        f"{where}: expected to be REFUSED and it was accepted. The chapter's "
                        f"claim about the implementation is no longer true.")
                elif rc is None:
                    failures.append(f"{where}: expected a refusal and the compiler timed out.")
                elif want and want not in out:
                    failures.append(
                        f"{where}: refused for the wrong reason. Expected text {want!r}, got "
                        f"{out[:200]!r}. A refusal that never reaches the documented check "
                        f"demonstrates nothing.")
            elif marker.startswith("skip:"):
                reason = marker[len("skip:"):].strip()
                verdicts["skip"] += 1
                program_assertions -= 1
                if not reason:
                    failures.append(f"{where}: `skip` requires a stated reason.")
                skipped.append(f"{where} ({reason})")
            else:
                failures.append(
                    f"{where}: unrecognised expectation {marker!r}. Use accept, compile, "
                    f"reject, reject: <text>, or skip: <reason>.")

    # ---- non-vacuity. A run that asserted nothing is not a pass. -------------
    floors = []
    if blocks < MIN_BLOCKS:
        floors.append(f"discovered {blocks} bare blocks, floor {MIN_BLOCKS}: the block "
                      f"extraction has broken, or SRC is wrong")
    if programs < MIN_PROGRAMS:
        floors.append(f"found {programs} complete programs, floor {MIN_PROGRAMS}: the entry-point "
                      f"detection has broken")
    if claim_assertions < MIN_CLAIM_ASSERTIONS:
        floors.append(f"made {claim_assertions} output-claim assertions, floor "
                      f"{MIN_CLAIM_ASSERTIONS}: the claim extraction has stopped matching")
    if accepted < MIN_ACCEPTED:
        floors.append(f"only {accepted} programs were ACCEPTED, floor {MIN_ACCEPTED}: a harness "
                      f"that refuses everything satisfies every rejection assertion, so this "
                      f"count is the control on them")
    if len(skipped) > MAX_SKIPPED:
        floors.append(f"{len(skipped)} programs are excused, ceiling {MAX_SKIPPED}")

    print(f"blocks {blocks}  complete programs {programs}  "
          f"claim/REPL assertions {claim_assertions}  program assertions {program_assertions}")
    print(f"  verdicts: {verdicts}   accepted {accepted}")
    for s in skipped:
        print("  skipped:", s)
    for x in floors:
        print("  FLOOR:", x)
    for x in failures:
        print("  FAIL:", x)
    print(f"{len(failures)} failure(s), {len(floors)} floor violation(s)")
    return 1 if (failures or floors) else 0


if __name__ == "__main__":
    sys.exit(main())
