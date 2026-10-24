"""Aggregate agent token usage for the Keleusma project without copying content.

Reads Claude Code transcripts under ~/.claude/projects/*keleusma*/ and Codex
session logs under ~/.codex/sessions/ whose working directory names keleusma.
Emits only counts, token totals, tool names, byte sizes and timestamps. No
message text, command text or tool output leaves the transcript.

Usage: python3 -I measure.py > measurements.json

Limits, stated so the output is not over-read:
- Transcripts are pruned by the harness retention period, so older sessions
  are absent. The totals cover only what survived on the measuring machine.
- Assistant records are deduplicated by message id. A streamed message that
  is logged several times with one usage block is counted once.
- Context per call is input plus cache-write plus cache-read tokens. It is the
  size of the prompt the model was sent, not a cost.
- Polling is classified by a pattern over Bash command text. It counts
  commands containing `sleep <n>` or a GitHub run or check status query. It
  misses other polling forms and may include a non-polling command.
"""
import collections
import json
import pathlib
import re
import sys

HOME = pathlib.Path.home()
CLAUDE_ROOT = HOME / ".claude" / "projects"
CODEX_ROOT = HOME / ".codex" / "sessions"
CONTEXT_KEYS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")
TOKEN_KEYS = CONTEXT_KEYS + ("output_tokens",)
SLEEP = re.compile(r"\bsleep [0-9]")
CI_STATUS = re.compile(r"gh run (view|watch|list)|gh pr checks")
BUCKETS = ((100_000, "<100k"), (200_000, "100k-200k"), (400_000, "200k-400k"), (700_000, "400k-700k"))


def bucket(tokens):
    for limit, name in BUCKETS:
        if tokens < limit:
            return name
    return ">=700k"


def checkout_label(directory):
    """Reduce a transcript directory name to the checkout suffix after the project name."""
    suffix = directory.split("keleusma", 1)[-1].strip("-")
    return suffix or "primary"


def text_len(content):
    if isinstance(content, str):
        return len(content)
    if isinstance(content, list):
        return sum(len(p.get("text", "") or "") + text_len(p.get("content")) for p in content if isinstance(p, dict))
    return 0


def records(path):
    for line in path.open(encoding="utf-8", errors="replace"):
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            continue


def scan_claude(path):
    usage = {}
    tool_name = {}
    tool_calls = collections.Counter()
    tool_chars = collections.Counter()
    polls = []
    wakeups = []
    noop_wakeups = 0
    compactions = 0
    first = last = None
    for rec in records(path):
        ts = rec.get("timestamp")
        if ts:
            first = first or ts
            last = ts
        if rec.get("isCompactSummary"):
            compactions += 1
        msg = rec.get("message")
        if not isinstance(msg, dict):
            continue
        if rec.get("type") == "assistant":
            mid = msg.get("id")
            if mid and msg.get("usage"):
                usage[mid] = msg["usage"]
            for part in msg.get("content") or []:
                if not (isinstance(part, dict) and part.get("type") == "tool_use"):
                    continue
                name = part.get("name")
                tool_name[part.get("id")] = name
                tool_calls[name] += 1
                args = part.get("input") or {}
                if name == "ScheduleWakeup":
                    wakeups.append(mid)
                    noop_wakeups += args.get("noop") is True
                if name == "Bash":
                    command = args.get("command", "")
                    if SLEEP.search(command):
                        polls.append(("sleep", mid))
                    elif CI_STATUS.search(command):
                        polls.append(("ci_status", mid))
        elif rec.get("type") == "user" and isinstance(msg.get("content"), list):
            for part in msg["content"]:
                if isinstance(part, dict) and part.get("type") == "tool_result":
                    tool_chars[tool_name.get(part.get("tool_use_id"), "?")] += text_len(part.get("content"))
    context = {mid: sum(int(u.get(k) or 0) for k in CONTEXT_KEYS) for mid, u in usage.items()}
    totals = collections.Counter()
    ttl = collections.Counter()
    for u in usage.values():
        for key in TOKEN_KEYS:
            totals[key] += int(u.get(key) or 0)
        for key, value in (u.get("cache_creation") or {}).items():
            ttl[key] += int(value or 0)
    sizes = sorted(context.values())
    n = len(sizes)
    buckets = collections.Counter(bucket(s) for s in sizes)
    poll_calls = collections.Counter(kind for kind, _ in polls)
    poll_context = collections.Counter()
    for kind, mid in polls:
        poll_context[kind] += context.get(mid, 0)
    return {
        "transcript": checkout_label(path.parent.name if "subagents" not in path.parts else path.parents[2].name),
        "session": (path.parents[1].name if "subagents" in path.parts else path.stem)[:8],
        "subagent": "subagents" in path.parts,
        "first": first,
        "last": last,
        "api_calls": n,
        "tokens": dict(totals),
        "cache_write_by_ttl": dict(ttl),
        "context_mean": round(sum(sizes) / n) if n else 0,
        "context_p50": sizes[n // 2] if n else 0,
        "context_p90": sizes[int(n * 0.9)] if n else 0,
        "context_max": sizes[-1] if n else 0,
        "context_buckets": dict(buckets),
        "context_share_in_calls_over_200k": round(sum(s for s in sizes if s >= 200_000) / max(sum(sizes), 1), 3),
        "compactions": compactions,
        "tool_calls": dict(tool_calls.most_common()),
        "tool_result_chars": dict(tool_chars.most_common()),
        "wakeups": len(wakeups),
        "noop_wakeups": noop_wakeups,
        "context_at_wakeups": sum(context.get(m, 0) for m in wakeups),
        "poll_calls": dict(poll_calls),
        "context_at_polls": dict(poll_context),
    }


def model_counts(path):
    counts = collections.Counter()
    seen = set()
    for rec in records(path):
        msg = rec.get("message")
        if rec.get("type") == "assistant" and isinstance(msg, dict) and msg.get("id") not in seen:
            seen.add(msg.get("id"))
            counts[msg.get("model")] += 1
    return dict(counts)


def scan_codex():
    sessions = 0
    totals = collections.Counter()
    window = 0
    for path in sorted(CODEX_ROOT.rglob("*.jsonl")):
        cwd = None
        last = None
        for rec in records(path):
            payload = rec.get("payload") or {}
            if cwd is None and isinstance(payload, dict) and payload.get("cwd"):
                cwd = payload["cwd"]
            if isinstance(payload, dict) and payload.get("type") == "token_count" and payload.get("info"):
                last = payload["info"]
        if cwd and "keleusma" in cwd and last:
            sessions += 1
            for key, value in last["total_token_usage"].items():
                totals[key] += int(value or 0)
            window = max(window, int(last.get("model_context_window") or 0))
    return {"sessions": sessions, "tokens": dict(totals), "max_context_window": window}


def main():
    claude = []
    for path in sorted(CLAUDE_ROOT.glob("*keleusma*/**/*.jsonl")):
        row = scan_claude(path)
        row["models"] = model_counts(path)
        claude.append(row)
    json.dump({"claude_code": claude, "codex": scan_codex()}, sys.stdout, indent=1, sort_keys=True)
    sys.stdout.write("\n")


main()
