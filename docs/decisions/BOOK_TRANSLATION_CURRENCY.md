# Book Translation Currency

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: measured 2026-09-26, session 67. The catalogues are made honest here. **The
translation of the remaining messages is the operator's call and is not done.** No language,
instruction-set, or wire-format change; no authored Japanese prose.

---

## The claim this is about

The guide ships in English and Japanese, the Japanese text living in `book/po/ja.po` keyed by
the English it translates. `book/po/messages.pot` is the extracted catalogue of translatable
messages. Before this work, `ja.po` reported **3047 entries, zero untranslated, zero fuzzy**.

That report is true of the catalogue and false of the book. A message is untranslated only
if the catalogue knows it exists, and the catalogue had not been regenerated.

## Measured, with the project's own extractor

`mdbook-xgettext` regenerates the template; `msgmerge --no-fuzzy-matching` compares the
committed translation against it. Both are the tools the book workflow already installs.

```sh
# regenerate the template from the current English source
mdbook build book -d /tmp/pot-check      # with [output.xgettext] configured
# compare the committed translation against it
msgmerge --no-fuzzy-matching book/po/ja.po /tmp/pot-check/messages.pot -o /tmp/merged.po
```

| quantity | `v0.2.3` before this work | including session 67's own English edits |
|---|---|---|
| live translatable messages in the source | 3071 | 3072 |
| messages in the committed `messages.pot` | **2782**, 290 behind | 2782 |
| that template's `POT-Creation-Date` | **2026-07-08**, nearly three months stale | same |
| untranslated | **30** | **38** |
| orphaned translations | **6** | **13** |

**Eight of the untranslated and seven of the orphans are session 67's own**, caused by the
English prose that increment 1 added when it corrected the productivity-rule entry and
removed two stale byte counts. That split is stated rather than folded into one total,
because a combined figure is how a regression gets absorbed into a baseline.

### The two committed files did not describe each other either

The template held 2781 live messages and the translation 3047, and **56 template messages
had no entry in the translation at all**. They were generated at different times and neither
described the other. The consequence for a reader is that roughly one per cent of the
Japanese book renders in English, silently, because an untranslated message falls back and an
orphaned translation simply stops being used.

### After this work

The template is regenerated and holds **3073** messages. `ja.po` holds **3072** live
entries, of which **3034 are translated (98 per cent)**, **38 are untranslated**, and **13
translations are marked obsolete**. Regenerating the template through the real `book/book.toml`
reproduces the committed file byte for byte apart from its creation date, which is the
property the drift check asserts.

## What is done here, and what is deliberately not

**Done.** The template is regenerated from the current English. `ja.po` is merged against it
with `msgmerge`, so every live message has an entry, untranslated ones are visibly empty, and
orphaned ones are marked obsolete. This is mechanical: it inserts no content.

**Verified rendering-neutral.** Two complete Japanese book builds, before and after the
merge, are byte-identical. The comparison was then shown able to detect a single deliberately
mutated translation, because a neutrality result from an instrument that cannot see any
change is not a result.

**Not done: the translation itself.** Filling the untranslated messages is an editorial act
on a published, human-curated artifact. Machine-authored Japanese prose in a shipped guide is
the operator's decision, not a mechanical consequence of this measurement. **The debt is
recorded here and the decision is yours.**

## How the guard is split, and why neither half suffices

`tests/book_translation_currency.rs` compares the **two committed catalogues to each other**:
they must agree in size, the translation must cover every extracted message, and the
untranslated and orphaned counts must stay at or below the ceilings recorded in that file. It
needs no gettext tooling and no book build, so it runs on every branch in every feature
configuration.

It **cannot** tell whether the template still matches the markdown. Both files could be
stale together. That question needs the real extractor, and the check that regenerates the
template and fails on drift lives where those tools are installed — the same pattern
`book/src/INSTRUCTION_SET.md` already uses against `docs/spec/INSTRUCTION_SET.md`.

**Stating that division is the point.** Either half alone reads as coverage it does not have.

## Prior failure this work is shaped around

`docs/process/DESIGN_JOURNAL.md` records an earlier attempt to size this class by matching
each message against its source line. It reported **2,329 stale of 2,926**, which the same
entry calls *"not a finding, it is my wrong model of `mdbook-i18n-helpers`, which extracts
INLINE content and strips markdown syntax"*, and the instrument was **deleted rather than
repaired** — recorded there as the seventh instance of a check built from the same model as
the thing it checks.

A second hand-rolled attempt on 2026-09-26 reproduced the class at smaller scale, reporting
34 with visible false positives on escaped punctuation and on the chapter that documents
escape sequences. It was discarded on reading the journal entry.

**The correction is not a better approximation.** It is that the two gettext files are
written by the same tool, so comparing them to each other needs no model of the extractor at
all, while comparing either to the markdown needs the extractor itself. The guard does the
first and the workflow does the second.
