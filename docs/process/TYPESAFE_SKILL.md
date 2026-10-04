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

No TypeSafe application programming interface request was made. Authentication
and actual Jev workflow execution remain unverified. The project documentation
link, citation and claimed-count checks passed fourteen cases. No runtime gate,
push or integration merge was performed for this setup increment.
