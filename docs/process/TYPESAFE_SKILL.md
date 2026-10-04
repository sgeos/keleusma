# TypeSafe Skill Setup

The operator requested TypeSafe Jev integration into project workflows, starting
with installation of the `typesafe-ai` skill. Use that skill when working on this
project. Follow its live documentation guidance for the integration at hand.

## Installation and recovery

Use the quickstart's non-Claude installation method with Codex selected.
User scope makes the skill available across the project's worktrees.

```sh
npx --yes skills add typesafe-ai/skills --skill typesafe-ai --agent codex --global --yes
```

The installed skill is `~/.agents/skills/typesafe-ai/SKILL.md`. User-level
installation is not cloned with the repository. Run the command on a new machine
if the skill is absent. The tracked instructions here preserve that dependency.
The command follows the upstream default revision, so later installations may
have different contents.

The [upstream skill](https://github.com/typesafe-ai/skills/blob/main/skills/typesafe-ai/SKILL.md)
and [documentation index](https://docs.typesafe.ai/llms.txt) are the entry points.
The skill directs integrations to consult current documentation and evaluate
model judgments against the target task. Installing the skill does not establish
that a workflow using Jev has been implemented or verified.

## Verification on 2026-10-04

The installation command succeeded with `--json` added for its receipt. It selected
one skill and reported Codex as the destination agent. The installed skill and its
license were inspected. The installer subsequently listed `typesafe-ai` at user
scope with source `typesafe-ai/skills` and Codex among its agents.

```sh
npx --yes skills list --global --agent codex --json
shasum -a 256 ~/.agents/skills/typesafe-ai/SKILL.md
```

The Secure Hash Algorithm 256 digest of the installed skill file was
`71ea90d7906c6554c4f4c460ef7361b2d26f59116ccdae986dc6d997b9389f52`.

An initial offline listing failed because npm could not resolve the installer
from its offline cache. The subsequent normal listing succeeded. That failure
did not undo or invalidate the installation.

No TypeSafe application programming interface request was made during installation.
The project documentation
link, citation and claimed-count checks passed fourteen cases. No runtime gate,
push or integration merge was performed for this setup increment.

## Availability check on 2026-10-04

The operator requested an actual usability check and expected failure.
`TYPESAFE_API_KEY` was absent from the session environment and a fresh login
shell. The inspected project locations exposed no candidate environment or
TypeSafe credential files. This does not establish that no key exists elsewhere.

A minimal request to `https://api.typesafe.ai/v1/systemone` selected `jev-latest`
with synthetic state about a red apple and one Noul question. No authorization
header, project content or credential was sent. The service returned HTTP 403
with the following body.

```json
{"detail":{"error_type":"authentication_error","message":"Must supply an API key! Check your request and try again."}}
```

The [reference](https://docs.typesafe.ai/api) documents HTTP 401 for missing or
invalid keys. The observed status was 403. Both the status and authentication
message above are from the executed request. Network reachability is established.
At that point, an authenticated Jev evaluation was not established.

## Successful authenticated recheck on 2026-10-04

After the operator provisioned the key and requested another check, a fresh login
shell exposed a nonempty `TYPESAFE_API_KEY`. The same synthetic request succeeded
with HTTP 200. The response identified `jev-1.13.0` and returned a Noul value of
`0.99` for whether the apple was red. Reported usage was 279 input tokens and
20 output tokens. The response type and probability range were checked.

The credential was passed to curl through standard input, without placing its
value in command arguments, tracked files or output. The assistant did not edit
credential configuration. The login shell uses the existing private profile setup.
An already-running process need not inherit a later environment change, so this
result specifically establishes use through a fresh login shell.

Authentication and one minimal Jev evaluation are now verified. Broader project
workflow integration and task-specific model quality remain unverified.
