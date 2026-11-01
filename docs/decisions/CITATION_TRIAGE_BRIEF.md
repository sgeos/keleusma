# Brief — triage the citation debt register

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-25. `tests/comment_citations.rs` records **21 citations that
resolve to nothing**, listed rather than fixed because verifying each is
per-item work the increment that created the guard did not do. The register's
own instruction is *shrink this, never grow it*. This is the shrinking.

## What a near-miss scan already found, before any triage

Running each excused name against the tree's defined names by edit distance:
**thirteen have a close match and eight do not.** Two of the thirteen are the
interesting kind.

**`a_trailing_semicolon_after_for_is_rejected_where_after_if_it_is_accepted`**
matches `a_trailing_semicolon_after_for_is_accepted_as_it_is_after_if`. The
citation names a test asserting a REJECTION; the tree asserts the ACCEPTANCE. The
behaviour reversed and the citation kept the old claim.

**`the_two_self_hosted_compilers_disagree_on_a_string_literal`** matches
`the_two_self_hosted_compilers_agree_on_a_string_literal`. Same shape: a
divergence that was closed, with a comment still pointing at the divergence.

**A citation naming a test that asserts the opposite of what the tree does is
worse than one naming nothing.** A dangling name fails to inform; a reversed one
misinforms, and it does so with a plausible-looking pointer.

**`narrow_runtime_can_register_text_library_via_lifted_impl`** matches the `math`
and `audio` variants. There is no `text` one — the bundled text DSL was retired
in V0.1.x. That citation names a feature the language no longer has.

## The specific wrong turns

**Do not accept the near-miss as the answer.** Edit distance proposes; it does
not establish. **Go and touch what the name points at**: read the candidate test
and the citing sentence and confirm the candidate supports the claim being made.
A confident rename to a test that asserts something else is how a reversal is
created rather than repaired.

**Do not repoint a citation whose CLAIM is stale.** If the sentence asserts a
behaviour the tree no longer has, the sentence is what needs rewriting, not the
name inside it. Two of these are exactly that, and swapping `disagree` for
`agree` in the pointer while leaving the surrounding prose asserting a divergence
would leave the paragraph wrong and the guard green.

**Do not shrink the register by widening the excuse.** The register must fall
because citations were repaired, never because the scan stopped seeing them.
`no_allow_list_entry_is_stale` enforces both directions and is the check that
this holds.

**Do not claim to have triaged what was skipped.** Eight have no near match and
some will need archaeology. Leaving them with a reason recorded is a result;
quietly leaving them and reporting a smaller number is not.

## The failure this session has paid for repeatedly

Four instrument failures, all first outputs of freshly written tools; two
forwarded to another line unverified. **The escape route that worked is cheaper
than verification: go and touch the thing the output points at.** This whole
increment is that discipline applied to a list of names, which is the case where
re-reading tells you nothing — a plausible name is indistinguishable from a real
one by inspection.
