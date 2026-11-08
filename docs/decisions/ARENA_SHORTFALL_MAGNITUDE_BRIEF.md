# BRIEF — "eleven modules exceed" is a count, and the operator needs a size

## The gap as it stands

`bound_transfer.rs` establishes a real finding, and establishes it correctly: **the verified heap
figure does not bound the backend's composite-region demand.** The argument is negative and needs no
semantics — if it bounded the demand, no module could exceed it; modules do; therefore it does not.
Eleven of sixty-seven exceed.

It is also explicit that this is **not a safety defect**: the host is told what to supply and nothing
reads memory it was not given. What is missing is the GUARANTEE.

## What is missing, and why it blocks a decision that is not mine

One of the operator's open decisions is *"whether the runtime should absorb the arena term the
backend now publishes"* — adding a term changes what every host is told to provision.

**That decision needs a magnitude and does not have one.** The report prints a per-module line and a
single largest-demand figure, but nothing states the SHORTFALL: `demand - verified`, across the
eleven. An operator reading it cannot tell whether the missing term is tens of bytes or tens of
megabytes, and those argue for opposite answers:

- If the shortfall is small and roughly constant, it looks like a fixed overhead a host margin
  already covers, and the case for changing every host's provisioning is weak.
- If it scales with the module, no fixed margin is safe and the term has to be published.

**Nothing in the tree distinguishes those today.** That is a cheap measurement standing between a
recorded finding and an actionable one.

## Prior failures and the specific wrong turns to avoid

- **DO NOT WEAKEN THE EXISTING NEGATIVE CONCLUSION.** It is correct and it is the load-bearing part.
  This adds a magnitude beside it; it does not restate or soften it.
- **DO NOT RECOMMEND THE ANSWER.** Whether the runtime absorbs the term is the operator's, and it is
  recorded as theirs. Produce the number; do not argue the decision from it.
- **DO NOT PIN THE SHORTFALL AS A CONSTANT.** It moves with the corpus and with the backend's region
  planning. Report it; assert only what must hold for the report to mean anything.
- **A RATIO NEEDS ITS DENOMINATOR CHECKED.** Several verified figures may be zero, and
  `demand / verified` is then meaningless rather than infinite. Report the absolute shortfall, and
  any ratio only where the denominator is non-zero — stating how many were excluded for that reason.
- **THE EXISTING `!exceed.is_empty()` GUARD MUST SURVIVE UNCHANGED**, including its message. It says
  that no module exceeding would be NEWS and possibly GOOD news, and that the test should become the
  positive assertion rather than be deleted. That instruction is worth more than the count.
- **DO NOT CONFLATE THE TWO POPULATIONS.** Sixty-seven are compared; eleven exceed; a statistic over
  the eleven is not a statistic over the sixty-seven. Name which set each figure is taken over — this
  line has already shipped one defect of exactly that kind this session.

## What a good outcome looks like

The report states the shortfall across the exceeding modules — at minimum the smallest, the largest,
and their total — each naming the population it is taken over. A reader can tell in one line whether
this is a fixed overhead or a scaling one. **The negative conclusion is untouched, the decision is
still recorded as the operator's, and no recommendation is made.**
