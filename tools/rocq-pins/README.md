# Rocq Pin Generator

> **Navigation**: [Tools](../README.md) | [Rocq proofs](../../docs/proofs/rocq/)

`genpins.py` regenerates a Rocq directory's `Statements.v` as fully qualified pins. It reads the
current `Statements.v`, written with bare names under `Import` and already compiled, prints each
item's type with `Set Printing Fully Qualified`, renames the file to `Claims.v`, writes a new
`Statements.v` that imports no module of the project and pins each item by its printed type, and
updates the directory's `check.sh` to compile and scan `Claims.v` and to expect the new pin
count. The docstring gives the arguments.

An audit of 2026-11-18 showed by execution that pins written with bare names under `Import` can
be bypassed by redefining a used name in an imported module. Fully qualified pins in a file that
imports no project module were not bypassed by any attack the delta audit tried. The printed
text comes from the tree as it is when the script runs, so running it on a tampered tree records
the tampering. Review the diff it produces.
