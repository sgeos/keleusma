<!-- Saved verbatim from a research subagent of the kaizen line on 2026-10-24. Its labels are the subagent's own. The kaizen line re-fetched the Monitor and background limits, asyncRewake and the Codex hook input fields. -->

## Waiting for long external processes across coding-agent harnesses (checked 2026-10-24)

Labels used below. FACT means a cited document states it. INFERENCE means I reasoned it from cited facts. UNKNOWN means I found no official statement. I wrote no files in the repository.

### Comparison table

| Capability | Claude Code | Codex CLI | Grok Build (`grok`) | Gemini CLI / Cursor / OpenCode |
|---|---|---|---|---|
| Background shell command | FACT `run_in_background` [T] | FACT background terminals through `unified_exec`, listed with `/ps` [CMD] | FACT [BG] | UNKNOWN |
| Re-invoked automatically when it exits | FACT under `-p`. "Claude then takes one more turn with the outcome" [H]. Interactive mode is INFERENCE (see note 1) | INFERENCE that the model must poll. The config describes "empty `write_stdin` polls (background terminal polling)" [CFG] | UNKNOWN. Docs say only that the agent "collects their output when needed" [BG] | UNKNOWN |
| Monitor or watch primitive | FACT `Monitor`. Each output line goes to Claude. Deadline is 5 min by default and 30 min at most [T] | UNKNOWN in the CLI | FACT monitors. "each line the script prints becomes a notification" [BG] | OpenCode has a FACT `session.idle` plugin event [OC] |
| Scheduled wake-up or loop | FACT `/loop`, `CronCreate`, `ScheduleWakeup` (1 min to 1 h), 7-day expiry, fires only when the session is idle [S] | FACT the CLI cannot manage schedules. In-chat scheduled tasks exist only in the app and web [AUTO] | FACT `/loop`, minimum 60 s, 7-day expiry, 50 tasks [BG] | UNKNOWN |
| External push into a live session | FACT Channels, a research preview built on an MCP server, with a webhook example [CH]. FACT `asyncRewake` hook [HK] | FACT `Stop` hook `decision:"block"` continues the turn [CHK]. No push found | UNKNOWN | Gemini `AfterAgent` can "force retry" (FACT) [GH] |
| Headless fresh session | FACT `claude -p`, `--bare` [H] | FACT `codex exec` [NI] | FACT `grok -p` [GHS] | Gemini `-p` (FACT) [GHL]. Cursor `--print` (third-party) |
| Headless resume | FACT `-p --resume <id>` / `--continue` [H] | FACT `codex exec resume <id>` / `--last` [NI] | FACT `-r`, `-c`, `-s` [GHS] | Gemini UNKNOWN officially |
| Hooks that inject context | FACT `UserPromptSubmit`, `SessionStart`, `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStart/Stop`, and others through `additionalContext` [HK] | FACT `SessionStart`, `UserPromptSubmit`, `SubagentStart` (stdout or `additionalContext`), `PreToolUse`, `PostToolUse` (JSON only) [CHK] | FACT none documented. "For passive events, stdout is ignored" [GHK] | Gemini `SessionStart`, `BeforeAgent`, `AfterTool` show "Context" (FACT) [GH] |
| Pre-compact hook | FACT `PreCompact` can block and cannot inject context [HK] | FACT `PreCompact` and `PostCompact`. No injection [CHK] | FACT `PreCompact` and `PostCompact` [GHK] | Gemini `PreCompress` (FACT) [GH]. OpenCode `experimental.session.compacting` can add context (FACT) [OC] |
| Auto-compact threshold | FACT `autoCompactWindow`, `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, `/autocompact`, `autoCompactEnabled`, `idleCompaction` [MC][SR] | FACT `model_auto_compact_token_limit`, `_scope`, `compact_prompt` [CFG] | UNKNOWN officially. `auto_compact_threshold_percent` appears only in a third-party mirror | UNKNOWN |
| Per-project threshold | FACT all four Claude keys have scope "Any file", which includes `.claude/settings.json` [SR] | INFERENCE that it is allowed, because it is absent from the visible ignored-keys list (`notify` is on that list) [CFG] | FACT project config is "limited to MCP servers, plugins, and permission rules" [GSET], so INFERENCE no | UNKNOWN |
| Manual compact / clear | FACT `/compact [focus]`, `/clear`, `/rewind` summarise [CW] | FACT `/compact`, `/new`, `/clear` [CMD] | FACT `/compact [context]`, `/clear`=`/new` [GMC] | n/a |
| Cache TTL relevant to a wait | FACT 5 min, or 1 h on a subscription main conversation. Configurable with `promptCacheTtl` [PC] | Model-dependent. GPT-5.6+ minimum is 30 min, older in_memory is about 5–10 min [OAPC]. Codex's own setting is UNKNOWN | UNKNOWN | UNKNOWN |

### Per-harness notes

**Claude Code**

1. Background completion. FACT [H]. In `claude -p`, a run waits for a background command started by the main conversation, and "Claude then takes one more turn with the outcome." FACT [T]. A background command in a local terminal session "has no time limit". In unattended sessions it gets 30 minutes, or up to 2 hours with `timeout`. Under `-p` with a text prompt the default is 10 minutes. FACT [IM]. Output goes to a file that Claude reads. For interactive mode, the public pages I read do not literally say "re-invoked on exit". The Bash tool schema in this very session says it "re-invokes you when it exits". So that point is FACT for this harness build but has no public URL, and it is INFERENCE for public documentation.
2. Monitor and loops. FACT [T]. Monitor feeds each output line to Claude, and "Claude interjects when an event arrives". Its deadline is 5 min by default and 30 min at most, so a 90-minute CI run needs at least three re-arms. FACT [S]. Scheduled prompts fire "between your turns". A self-paced `/loop` picks 1 min to 1 h. The docs say Monitor "avoids polling altogether and is often more token-efficient". Wake-up cost is FACT [PC]. Each request "re-sends the full context: the system prompt, your project context, every prior message and tool result". Inside the TTL the prefix is billed at the cache-read rate. Past the TTL the whole prefix is reprocessed and written again. So every wake-up, Monitor line, or loop iteration costs one full-context request. It is cheap only while warm.
3. Ending a turn and resuming. FACT [HK]. `"asyncRewake": true` "runs in the background and wakes Claude on exit code 2". The hook's stderr is shown as a system reminder. FACT [CH]. Channels push a webhook or CI event into an open session as a `<channel source=…>` event. They are a research preview, need claude.ai or Console authentication, and need the `--channels` flag. FACT [H]. `claude -p --resume "$id"` resumes from a script. `--bare` skips hooks, CLAUDE.md, MCP, skills and auto memory. It also disables background tasks and is "recommended for scripted calls". FACT [PC]. Resuming re-sends the whole conversation, which is uncached when the TTL has passed.
4. Hooks. FACT [HK]. There are 33 events. Those that can inject context include `SessionStart`, `UserPromptSubmit` (stdout or `additionalContext`), `PreToolUse`, `PostToolUse`, `PostToolBatch`, `Stop`, `SubagentStart`, `SubagentStop`, and `PostModelSwitch`. `PreCompact` can only block. `FileChanged` exists but injects nothing. FACT [CW]. Context added earlier by hooks is "summarized with the rest of the conversation". `SessionStart` hooks matching `compact` re-run after compaction. UNKNOWN: whether `UserPromptSubmit` fires on a scheduled-task fire, on a background-completion turn, or on a channel event. If it does, the roughly 750-character banner would be added again on every wake-up.
5. Compaction. FACT [MC]. `autoCompactWindow` accepts 100K to 1M tokens. The env var takes precedence over the setting. FACT [SR]. `autoCompactWindow`, `autoCompactEnabled`, `idleCompaction` and `promptCacheTtl` all have scope "Any file", which includes project `.claude/settings.json`. FACT [PC]. On the one-hour TTL, an idle session can be compacted "shortly before the cache expires". The setting `idleCompaction:false` turns that off.
6. Caching. FACT [PC]. A main conversation on a subscription within plan usage gets 1 h. API-key and usage-credit sessions get 5 min. Subagents get 5 min unless `subagentPromptCacheTtl` is set. FACT [APC]. The multipliers are 1.25x base input for a 5-minute write, 2x for a 1-hour write, and 0.1x for a read (0.05x on Opus 5.5 and Sonnet 5.5). A hit refreshes the TTL for free. INFERENCE. A 30-minute wait survives only on the 1 h TTL. A 90-minute wait always pays a full re-write of the prefix, at 2x on the 1 h tier.

**OpenAI Codex CLI**

- FACT [NI]. `codex exec` prints the final message to stdout. `--json` emits JSONL with `thread.started` and `turn.completed`. `-o` writes the last message. `--ephemeral` skips persistence. `codex exec resume <SESSION_ID>` or `--last` resumes. `--ignore-user-config` skips `config.toml`.
- FACT [CMD]. `/ps` shows background terminals and their recent output, and `/stop` (alias `/clean`) stops them. FACT [CFG]. `background_terminal_max_timeout` is the "maximum poll window … for empty `write_stdin` polls". INFERENCE from that wording. The model polls a running terminal. I found no documented exit notification.
- FACT [CHK]. Hooks are "enabled by default", and non-managed hooks must be trusted. `SessionStart` and `UserPromptSubmit` add stdout as developer context. `Stop` with `decision:"block"` "automatically creates a new continuation prompt". Async hooks exist, with up to 8 per session, but "can't block … or otherwise control" anything.
- FACT [CFG]. `notify` runs a program with a JSON payload. It is ignored in project config. The payload type `agent-turn-complete` is reported only by third-party sources.
- FACT [AUTO]. "Codex CLI doesn't provide the Scheduled management interface." The in-chat scheduled tasks that return to an existing chat are app and web only.

**xAI Grok Build (CLI `grok`, beta)**

- FACT [GHS]. `grok -p`, `--output-format json|streaming-json`, `-r <ID>`, `-c`, `-s <ID>`, and `--always-approve`.
- FACT [BG]. Background tasks, a `/loop` with each firing "a new agent turn", and line-based monitors ("every output line interrupts the conversation"). Completion notification is UNKNOWN.
- FACT [GHK]. The events are SessionStart/End, UserPromptSubmit, Pre/PostToolUse, Stop, PreCompact/PostCompact and others. Only `PreToolUse` blocks. No context injection is documented. Grok reads Claude Code `.claude/settings.json` and Cursor hook files. INFERENCE from this. A Claude `UserPromptSubmit` banner hook would execute under Grok, but its output would be discarded.

**Others, briefly.** Gemini CLI has 11 hook events. `SessionStart`, `BeforeAgent` and `AfterTool` can add context, and `AfterAgent` can force a retry [GH]. It has a headless `-p` [GHL]. OpenCode has `session.idle` and a compaction hook that adds context [OC]. The Cursor CLI headless hook coverage is reported as incomplete on a forum, which is not official documentation. For Aider I found nothing official.

### Recommended portable waiting pattern

Keep the waiting out of the model entirely.

1. **Blocking waiter script.** Write a script such as `wait-for.sh` that does the polling itself, with `gh run watch`, a `sleep` loop on a file, or `wait` on a PID. It should be idempotent and print one short summary line. Give it an exit status of 0 for pass, 1 for fail, and 2 for timeout. It writes a small result file, for example `tmp/wait/<id>.json`, holding the status, the URL and a tail of 20 lines.
2. **Inside a harness that notifies on exit** (Claude Code). Launch the waiter with `run_in_background`, end the turn, and get one wake-up at completion. Do not use Monitor for waits over 30 minutes, and do not use `/loop` polling, because each fire is a full-context request. The `asyncRewake` hook is an alternative event source.
3. **Inside a harness without exit notification** (Codex, Grok, Gemini), or when the wait exceeds the cache TTL. End the session after writing a short handoff file with the goal, the next action, and the paths to read. Then use an external supervisor, such as launchd, cron, a shell `&`, or a CI `workflow_run` job. It runs the waiter, and on exit starts a **fresh small headless session** whose prompt is "read handoff + result file", for example `claude --bare -p …`, `codex exec --ephemeral …`, or `grok -p …`. INFERENCE from [PC]. After a 90-minute wait, a fresh session costs the handoff size, whereas `--resume` costs the whole transcript at the uncached rate.
4. Use `--bare` or `--ignore-user-config` so the per-prompt banner hooks do not fire on the wake-up session. Supply any essential instructions explicitly.

### Claims I could not verify

- Public documentation that interactive Claude Code re-invokes the model when a background Bash command exits. The only evidence is this session's own tool schema.
- Whether Claude Code `UserPromptSubmit` hooks fire on cron, loop, background-completion or channel turns.
- Codex background-terminal exit notifications, whether `unified_exec` is on by default in the CLI, the `notify` payload schema, Codex's prompt-cache retention setting, and whether `model_auto_compact_token_limit` is honoured in project config. The ignored-keys list was truncated.
- Grok Build completion notifications, its auto-compact key, cache TTL, and whether its hooks run in headless mode.
- Gemini CLI compression settings and official `--resume` documentation, Cursor CLI official hooks and resume documentation, and Aider.

### Sources

- [T] https://code.claude.com/docs/en/tools-reference
- [IM] https://code.claude.com/docs/en/interactive-mode
- [S] https://code.claude.com/docs/en/scheduled-tasks
- [H] https://code.claude.com/docs/en/headless
- [HK] https://code.claude.com/docs/en/hooks
- [CH] https://code.claude.com/docs/en/channels
- [CW] https://code.claude.com/docs/en/context-window
- [MC] https://code.claude.com/docs/en/model-config
- [SR] https://code.claude.com/docs/en/settings-reference
- [PC] https://code.claude.com/docs/en/prompt-caching
- [APC] https://platform.claude.com/docs/en/build-with-claude/prompt-caching
- [CFG] https://learn.chatgpt.com/docs/config-file/config-reference
- [CHK] https://learn.chatgpt.com/docs/hooks
- [NI] https://learn.chatgpt.com/docs/non-interactive-mode.md
- [CMD] https://learn.chatgpt.com/docs/developer-commands?surface=cli
- [AUTO] https://learn.chatgpt.com/docs/automations
- [OAPC] https://developers.openai.com/api/docs/guides/prompt-caching
- [GHS] https://docs.x.ai/build/cli/headless-scripting
- [BG] https://docs.x.ai/build/features/background-tasks
- [GHK] https://docs.x.ai/build/features/hooks
- [GMC] https://docs.x.ai/build/modes-and-commands
- [GSET] https://docs.x.ai/build/settings
- [GH] https://geminicli.com/docs/hooks/
- [GHL] https://geminicli.com/docs/cli/headless.md
- [OC] https://opencode.ai/docs/plugins/

The developers.openai.com/codex pages now redirect (HTTP 308) to learn.chatgpt.com, which is why the Codex URLs above use that host.