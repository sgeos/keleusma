# Tools

> **Navigation**: [Repository Root](../README.md)

Project-side development tooling. The contents of this directory support contributors and project maintainers; they are not part of the Keleusma runtime, not part of the published crates.io tarball, and not required by downstream embedders.

## Contents

| File | Purpose |
|------|---------|
| [`cloc.lang`](./cloc.lang) | Language definition for [cloc](https://github.com/AlDanial/cloc) so the line-count tool recognises `.kel` files as Keleusma source. |

## Using `cloc.lang`

Count Keleusma source across the project:

```sh
cloc --read-lang-def=tools/cloc.lang \
     src/ keleusma-arena/src/ keleusma-macros/src/ \
     keleusma-bench/src/ keleusma-cli/src/ \
     examples/ tests/
```

The definition recognises:

- Line comments beginning with `//`
- Block comments delimited by `/*` and `*/`
- The `.kel` extension on standalone Keleusma scripts
- A third-generation language scale of 1.50, reflecting that Keleusma is denser per line than C through its pattern matching, multi-headed functions, and pipeline operator, but not as dense as a fully dynamic scripting language

If your cloc invocation should treat Keleusma as a built-in language without `--read-lang-def`, install the definition into your cloc config (`~/.config/cloc/lang_def`) or submit it upstream to the cloc project.
