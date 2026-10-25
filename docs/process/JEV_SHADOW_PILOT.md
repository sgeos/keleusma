# Jev shadow pilot

The operator authorized setup on 2026-10-04 for V0.2.X work and possible V0.3.X
work. The registered routes are `v0.2.3` and `v0.3.0`. The pilot is independent of
the agent harness and operates alongside ordinary development. It cannot pass
a gate, approve a change, select tests to omit, or change the line's work plan.

## Starting a line

After checking out a branch containing these tools, install once for the worktree.
Use Python 3.10 or later. Live evaluation also requires curl and the existing
private `TYPESAFE_API_KEY` environment. Installation makes no network requests.

```sh
python3 -B scripts/jev_shadow.py --line v0.2.3 install
```

Use `v0.3.0` for a participating V0.3.X worktree. A temporary feature branch does
not change the line identifier. The tools can also be installed from a separate
checkout containing this implementation, without changing the target branch.

```sh
python3 -B scripts/jev_shadow.py --repo /path/to/target/worktree --line v0.2.3 install
```

The shared installation lives in `secret/jev-shadow/` below the primary checkout.
Both lines use the same scripts and transactional database. Define the helper
below in each shell, from any linked worktree. No harness-specific command is
required. For an unenrolled new worktree, pass `--line v0.2.3` or install it first.

```sh
jev_shadow_store="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/secret/jev-shadow"
jevshadow() {
    python3 -B "$jev_shadow_store/jev_shadow.py" --repo "$PWD" "$@"
}
jevshadow status
```

The shared interline reminder reports the guide's availability when these tools
are installed. It does not send messages or launch evaluations. Existing sessions
can use the helper immediately. Hook trust and sandbox permissions remain those
of the host client. A sibling worktree may need access to the narrow shared store
through its existing permission mechanism. A missing reminder does not prevent
manual use. No additional hook configuration is required for this pilot.

## Sampling during development

At each development increment, preserve the first diagnostic requiring
interpretation after a failed or unexpectedly empty verification run. Also sample
up to three atomic completion claims against their evidence before the handoff.
Do not manufacture a diagnostic when all checks succeed. Record omitted eligible
samples and reasons in the line's ordinary handoff so the collection bias remains
visible. This is a sampling convention, not a new completion gate.

Record the line's initial classification before any model result is inspected.
Continue development using the original sources, full logs and existing gates.
The pilot deliberately cannot measure avoided System 2 work while its judgments
remain hidden. It prepares a labelled corpus and paired judgment comparison for
the later controlled evaluation described in the research spike.

Use the shapes in `docs/process/jev-shadow/examples/triage.json` and `claims.json`
to create a packet under the current worktree's ignored `tmp/`. The supplied
examples are explicitly synthetic. Real records must use `sample_kind` equal to
`development`, a fresh case identifier, a stable task identifier and the actual
initial judgment. The actor field identifies the session or reviewer, not merely
the model brand. Both lines should use globally distinct case identifiers.

The installer also copies these templates to `$jev_shadow_store/examples/`, so a
line whose branch does not contain the tools can start immediately. Copy a template
to `tmp/jev-case.json`, then replace its identifier, synthetic marker, input,
baseline and source references with the actual reviewed sample.

Each packet contains one diagnostic or one claim-evidence pair, source references,
the baseline judgment, and optional command metadata. Source paths are relative
to the recording worktree. The evidence must occur verbatim in at least one
referenced file. Sources are locally hashed and bounded to one megabyte each.
For a large log, prepare a reviewed excerpt file and retain the original log.
The pilot does not execute the reported command or independently verify its exit
code, feature set or test count. These fields remain caller-reported provenance.

```sh
jevshadow record --packet tmp/jev-case.json
jevshadow preview --id v023-your-case-id
```

Preview prints the exact outbound request and its digest. Inspect that small
payload for relevance and private material, then pass its digest to evaluation.
This review is performed by the working line within the authorized pilot. It
does not require asking the operator to approve each ordinary sample.

```sh
jevshadow evaluate --id v023-your-case-id --reviewed-sha256 DIGEST_FROM_PREVIEW
```

Only `input` and the fixed question reach TypeSafe. Baseline judgments, source
paths, branch identifiers, labels and usage measurements stay local. No automatic
repository scan, log upload or model call runs from a hook. Appendix B, credentials,
private correspondence and unrelated source material are excluded. Known
credential markers and the loaded key are rejected, but that check is not a
general secret detector. A new capture whose input names the home directory is
rejected as well. Write that path as `~` in a reviewed excerpt file. Records
captured before 2026-10-25 keep their verbatim paths. Review remains necessary. Provider retention terms for
private project data have not been assessed, so use public or appropriately
sanitized excerpts only.

## Blind review and task outcomes

Evaluation prints service status with answers hidden. It does not recommend an
action to the working line. An unavailable result leaves development unchanged.
For review, export the blind view before looking at results.

```sh
jevshadow blind --id v023-your-case-id > tmp/jev-blind.json
```

Have a reviewer classify that evidence without seeing the baseline or Jev result.
Use a label file with `choice`, `reviewer`, `independent` and `rationale`. Set
`independent` to true only when the reviewer is separate from the baseline author
and remained blind. Names alone cannot establish that condition. A self-review is
allowed with false and is excluded from independent accuracy comparisons.
Labels are immutable once recorded. Disputed labels need separate adjudication
outside the current corpus rather than silently changing the reference answer.

```json
{
  "choice": "unsupported",
  "reviewer": "reviewer-session-id",
  "independent": true,
  "rationale": "The excerpt does not establish the claimed property."
}
```

```sh
jevshadow label --id v023-your-case-id --file tmp/jev-label.json
jevshadow finish --task-id your-increment-id --file tmp/jev-outcome.json
jevshadow reveal --id v023-your-case-id
jevshadow report
```

The outcome shape is in `docs/process/jev-shadow/examples/outcome.json`. Record
one outcome per task, even when it contains multiple samples. Use null when
System 2 usage or elapsed time is unavailable. Never substitute zero or estimates
for missing provider usage. The resolved field describes the actual development
task. Synthetic installation checks are excluded from the default report.

Reveal requires both a completed task record and a label. Reports compare baseline
and Jev agreement only for successful service calls with a finished task and an
independently declared label. They count corrections, regressions and false
support separately. These are paired classification results, not proof that the
development outcome improved. Local database access can bypass blinding, so this
is a workflow convention rather than an adversarial access-control boundary.

## Failures, limits and concurrency

The fixed configuration pins `jev-1.13.0` and rubric `development-shadow-1`.
The claims rubric resolves the original spike's ambiguity about nonexecution, so
do not pool these measurements with the old rubric. Confidence is recorded but
does not authorize actions. The service contract and schema were checked against
the [TypeSafe application programming interface reference](https://docs.typesafe.ai/api).

The shared pilot allows at most 500 attempted calls and one million charged or
reserved input tokens across all lines, including synthetic checks. Each request
is bounded to 12,000 encoded bytes. Before a request, a database transaction
reserves 64,000 tokens, the documented total request context ceiling. A valid
response replaces that reservation with reported usage. Failed or interrupted
requests retain the reservation because billing is unknown. This accounting is
conservative and depends on the provider honoring its documented limit. It is
not a provider-enforced account spending cap. The configured rate estimates known
usage at USD 0.042 per million input tokens with free output.
[TypeSafe model limits and pricing](https://docs.typesafe.ai/models)

A duplicate evaluation reuses its stored status without another call. Concurrent
workers cannot evaluate the same sample or oversubscribe the shared reservation.
There are no automatic retries. Curl has a 30-second deadline and the subprocess
has a 35-second deadline. Service errors, malformed answers, model mismatches and
missing credentials remain explicit unavailable results. They never become
negative judgments or passes. A later investigation can use a new case identifier
without erasing the failed attempt.

If a process dies after reserving a call, confirm that it has stopped before
marking the case interrupted. Its token reservation remains charged to the pilot.

```sh
jevshadow recover --id v023-your-interrupted-case
```

The database uses local SQLite transactions with bounded lock waiting. Installers
must run serially. Configuration changes require a distinct pilot store and a
new measurement cohort. The first installation restores the tracked checkpoint
only when the store is empty. Existing evidence is never replaced on reinstall.

## Handoff and continuation on another machine

At handoff, export to a new ignored file, review the samples, and then copy the
reviewed export into the tracked checkpoint on the branch carrying the pilot.
The export includes reviewed excerpt text and labels, so review it before staging.
Do not commit the live database or any credential. Coordinate checkpoint ownership
through the kaizen line rather than editing the same checkpoint concurrently.

```sh
jevshadow export --output tmp/jev-shadow-checkpoint.json
```

The tracked destination is `docs/process/jev-shadow/checkpoint.json`. On a fresh
clone containing the pilot tools and checkpoint, run the install command again.
It restores case provenance, labels, task outcomes, results and budget accounting.
It does not restore old absolute worktree paths. Enrol the new local worktree and
provision credentials privately for subsequent live calls. Restored in-flight
calls become unavailable with unknown billing and are not silently retried.
Unexported local records do not survive a clone. A clean initialization from the
tracked empty checkpoint works without an old `tmp/` or `secret/` tree.

The implementation can run from an isolated kaizen checkout while V0.2.X and
V0.3.X branches continue their own work. To make it available on another machine,
the clone must contain the pilot commit. Local installation does not publish it
or merge it into either version branch.

## Verification

```sh
python3 -B -m unittest discover -s scripts -p test_jev_shadow.py -v
python3 -B -m unittest discover -s scripts -p test_interline.py -v
```

The tests use synthetic data and fake network responses. A live smoke receipt,
when present, is explicitly synthetic and provides no development accuracy claim.
The [research spike](./kaizen/2026-10-04-jev-development-flow-spike.md) explains the
original measurements and the subsequent comparison needed before changing the
workflow in response to model judgments.
