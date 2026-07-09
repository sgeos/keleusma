#!/usr/bin/env python3
"""Verify that the runnable code examples in the book still produce their
documented output against the current Keleusma CLI.

This is the freshness guarantee that motivated keeping the book in the main
repo: every example with a stated output is executed and compared on each CI
run, so the book cannot silently drift from the implementation.

Rule: only examples that make an explicit output claim are asserted.
  - A fenced ```` fn main ```` block followed within a few lines by prose of the
    form "output is `X`" (or "prints `X`") is written to a file, run through
    `keleusma run`, and its output must equal X.
  - A REPL block (lines beginning "> ") has each expression piped through
    `keleusma repl`; each printed result must equal the line beneath it.
Blocks without an output claim (illustrative fragments, deliberately rejected
programs, Rust host snippets) are skipped, not asserted.

Usage: verify_examples.py <path-to-keleusma-binary> [book/src]
Exit code 0 on success, 1 if any claimed example fails.
"""
import re
import os
import sys
import glob
import subprocess
import tempfile

CLI = sys.argv[1] if len(sys.argv) > 1 else "keleusma"
SRC = sys.argv[2] if len(sys.argv) > 2 else "book/src"


def run_program(src):
    with tempfile.NamedTemporaryFile("w", suffix=".kel", delete=False) as fh:
        fh.write(src)
        path = fh.name
    try:
        r = subprocess.run([CLI, "run", path], capture_output=True, text=True, timeout=60)
    finally:
        os.unlink(path)
    return r.returncode, (r.stdout.strip() if r.returncode == 0 else (r.stderr.strip() or r.stdout.strip()))


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
    """Yield (code, close_index) only for BARE ``` fenced blocks.

    The guide fences Keleusma code with a bare ```` ``` ````; Rust and shell host
    snippets are language-tagged (```` ```rust ````, ```` ```sh ````). Only bare
    blocks are Keleusma programs, so language-tagged blocks are consumed and
    skipped rather than run.
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
                yield ("\n".join(buf), j)
            i = j + 1
        else:
            i += 1


def main():
    failures = []
    checked = 0
    for f in sorted(glob.glob(os.path.join(SRC, "*.md"))):
        name = os.path.basename(f)
        lines = open(f).read().split("\n")
        for code, close in fenced_blocks(lines):
            if any(ln.startswith("> ") for ln in code.split("\n")):
                # REPL block
                exprs, expected = [], []
                cl = code.split("\n")
                for k, ln in enumerate(cl):
                    if ln.startswith("> "):
                        e = ln[2:].strip()
                        r = cl[k + 1].strip() if (k + 1 < len(cl) and not cl[k + 1].startswith("> ") and cl[k + 1].strip()) else None
                        exprs.append(e)
                        expected.append(r)
                if not any(expected):
                    continue
                got = run_repl(exprs)
                for idx, (e, exp) in enumerate(zip(exprs, expected)):
                    if exp is None:
                        continue
                    actual = got[idx] if idx < len(got) else "<none>"
                    checked += 1
                    if actual != exp:
                        failures.append(f"{name} REPL `{e}`: claim {exp!r}, actual {actual!r}")
                continue
            if "fn main" not in code:
                continue
            window = "\n".join(lines[close + 1:close + 16])
            m = (re.search(r"[Oo]utput is[:\s]+`([^`]+)`", window)
                 or re.search(r"[Oo]utput is[:\s]*\n+```\n([^\n]+)\n```", window)
                 or re.search(r"prints\s+`([^`]+)`", window))
            if not m:
                continue
            claim = m.group(1).strip()
            checked += 1
            rc, out = run_program(code)
            if rc != 0:
                failures.append(f"{name}: claimed `{claim}` but program failed: {out[:160]}")
            elif out != claim:
                failures.append(f"{name}: claim {claim!r}, actual {out!r}")
    print(f"checked {checked} claimed examples; {len(failures)} failure(s)")
    for x in failures:
        print("  FAIL:", x)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
