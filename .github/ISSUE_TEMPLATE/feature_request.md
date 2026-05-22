---
name: Feature request
about: Suggest a new language feature, runtime capability, or tooling improvement
title: ''
labels: enhancement
assignees: ''
---

## Summary

<!--
One sentence: what the feature is. Avoid framing as a solution
to a problem the maintainer has not encountered; describe the
feature itself.
-->

## Motivating use case

<!--
Describe the concrete situation that prompted the request. A
running program, a host application, a verification scenario,
a profile that exposed a missing capability, etc. Keleusma is
intentionally opinionated; the use case is the load-bearing
argument for any addition.
-->

## Proposed surface

<!--
If the request adds language surface (keyword, operator,
construct), sketch the syntax with a worked example.
-->

```keleusma
// example surface for the proposed feature
```

## Static-guarantee impact

Which of the five static guarantees does this feature need to preserve?

- [ ] Totality (programs admitted by the verifier always terminate)
- [ ] Productivity (every `loop` iteration yields)
- [ ] Bounded-step (declarable WCET)
- [ ] Bounded-memory (declarable WCMU)
- [ ] Safe-swap (hot-swap preserves schema)

If any of these would be relaxed by the feature, explain how the relaxation is bounded.

## Alternatives considered

<!--
Patterns or host-side workarounds you tried before deciding the
feature is the right solution. The host-registered native is
often the V0.2.0 alternative to a language-level addition; if
you considered that and rejected it, say why.
-->

## Scope and willingness to contribute

- [ ] I am willing to implement this with guidance
- [ ] I can review a maintainer's implementation
- [ ] I am requesting only; implementation by the maintainer

## Cross-references

<!--
Link to related issues, prior discussions, or documents under
docs/decisions/BACKLOG.md, docs/roadmap/, or similar.
-->
