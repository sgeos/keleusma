# Tools

> **Navigation**: [Repository Root](../README.md)

Project-side development tooling. The contents of this directory support contributors and project maintainers; they are not part of the Keleusma runtime, not part of the published crates.io tarball, and not required by downstream embedders.

## Contents

| File | Purpose |
|------|---------|
| [`cloc.lang`](./cloc.lang) | Language definition for [cloc](https://github.com/AlDanial/cloc) so the line-count tool recognises `.kel` files as Keleusma source. |
| [`proof-difftest/`](./proof-difftest/README.md) | The differential test of the shipping Rust verifier against the extracted Rocq core model of increment A, a detached package with its own workspace. |

## Using `cloc.lang`

[`scripts/loc.sh`](../scripts/loc.sh) counts the project with this definition, one block
per category, such as the runtime library, the workspace crates, the examples and their
documentation, the project documentation and the process records, followed by the total.
It counts only tracked files and leaves out data such as lockfiles and the fuzz corpus.

```sh
scripts/loc.sh                 # every category, then the total
scripts/loc.sh runtime docs    # only the named categories
scripts/loc.sh -l              # list the categories
```

For a count of your own choosing, pass the definition to cloc directly:

```sh
cloc --read-lang-def=tools/cloc.lang src/ tests/
```

The definition recognises:

- Line comments beginning with `//`
- Block comments delimited by `/*` and `*/`
- The `.kel` extension on standalone Keleusma scripts
- A third-generation language scale of 1.50, reflecting that Keleusma is denser per line than C through its pattern matching, multi-headed functions, and pipeline operator, but not as dense as a fully dynamic scripting language

If your cloc invocation should treat Keleusma as a built-in language without `--read-lang-def`, install the definition into your cloc config (`~/.config/cloc/lang_def`) or submit it upstream to the cloc project.
