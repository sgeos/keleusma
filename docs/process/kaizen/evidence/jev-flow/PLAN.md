# Jev workflow spike protocol

Fixed on 2026-10-04 before the first live experiment request. This is a bounded
research experiment, not a deployment or change to project gates.

## Questions and labels

Can Jev classify diagnostic routes, distinguish evidence support from unsupported
claims, and identify relevant process excerpts at low service cost? Does repeating
a request or reversing criteria order change the result?

The [fixtures](./fixtures.json) contain 60 synthetic cases, 20 per task. Their
SHA-256 digest is
`b91b3c859b4781ef47af9eeef7b72cb560fc8e3449495dc0a438ba66b46d06da`.
The researching Codex session authored and labelled them before live calls.
They are a convenience sample with deliberately difficult examples, not an
independent human-adjudicated benchmark. Labels and rationales never enter the
request. No private repository material is transmitted.

## Fixed procedure

Use `jev-1.13.0` through the documented TypeSafe endpoint. Make 18 sequential
requests, three tasks times two batches of ten cases times three variants.
The variants are baseline, an identical repeat, and reversed criteria order.
For retrieval, reverse the Noul true and false descriptions. This is weaker than
reversing a multiple-choice list and is reported separately.

Use the prompts and simple diagnostic baseline in
[`scripts/jev_flow_spike.py`](../../../../../scripts/jev_flow_spike.py).
Record their source digest before requests. Do not tune prompts, labels or
thresholds after seeing answers. Each request is limited to 20,000 encoded bytes,
which is a payload bound, not a measured token count. Stop on the first transport,
service or schema failure. Do not retry automatically. Stop if cumulative reported
input usage reaches 250,000 tokens. This is an after-response stop condition,
not a provider-enforced spending cap. Each curl call has a 30-second deadline.

## Metrics fixed in advance

- Report accuracy and confusion counts by task and variant. The independent case
  count is 60, not the 180 repeated judgments.
- For Choice, count coverage and errors at confidence at least 0.90. This is an
  experimental threshold, not a calibrated acceptance rule. Count erroneous
  `supports` answers separately because they provide false assurance.
- For retrieval, classify at probability at least 0.50. Also evaluate retaining
  every candidate above 0.10, with false exclusions and retained passage bytes.
  Mandatory process instructions are never eligible for this filter.
- Compare triage with a fixed, deliberately simple regular-expression baseline.
  It is not an optimized deterministic parser or a System 2 comparison.
- Count baseline-to-repeat and baseline-to-reversed decision changes. Agreement
  is not evidence of independent errors or a guarantee of correctness.
- Record all reported input and output tokens, per-call elapsed time, and estimated
  service cost at the advertised USD 0.042 per million input tokens. Output is
  advertised as free. This is not an invoice or a subscription usage measurement.
- Retained text bytes are only a context-size proxy. No System 2 subscription
  saving or overall error reduction can be measured without a paired workflow
  baseline that this spike does not contain.

## Boundaries

Typed answers must pass local validation before analysis. Missing or invalid
answers are failures, not negative judgments. Exact counts, command exit status,
receipt freshness and authority remain code or operator decisions. Nothing in
this experiment can waive verification or authorize publication. The service
receives only the reviewed synthetic inputs. Credentials remain in the existing
private environment and are excluded from command arguments and artifacts.

The offline analysis must reproduce the result from tracked receipts on a fresh
clone. Live repetition is explicit and requires a new output path. Future model
responses need not be identical.
