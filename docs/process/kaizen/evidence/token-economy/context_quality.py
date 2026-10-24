"""Relate tool-call failure rates to context size without copying content.

A weak, content-free proxy for whether work degrades as context grows. For
each Claude Code tool call it records the context size of the assistant call
that issued it and whether the harness marked its result as an error. It also
compares calls shortly before and shortly after each compaction, and the
first calls of sessions that began fresh rather than from a compaction.

Usage: python3 -I context_quality.py > context-quality.json

Limits, stated so the output is not over-read:
- A tool error is not a quality measure. A failing test run, a denied
  permission and a non-zero grep exit are all marked as errors. Permission
  denials are separated by a fixed phrase match and excluded from the rate.
- Edit and Write errors are reported separately. A failed exact-string edit
  is the closest available proxy for a model misremembering file contents.
- Context size is confounded with session age, task type and model. Large
  contexts occur late in long sessions, which do different work.
- Only main-conversation transcripts are counted. Subagents are excluded.
"""
import collections
import json
import pathlib
import sys

ROOT = pathlib.Path.home() / ".claude" / "projects"
CONTEXT_KEYS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")
BUCKETS = ((50_000, "<50k"), (100_000, "50k-100k"), (200_000, "100k-200k"), (400_000, "200k-400k"),
           (700_000, "400k-700k"))
DENIAL = ("doesn't want to proceed", "was rejected", "permission", "denied by", "user declined")
EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}
WINDOW = 50


def bucket(tokens):
    for limit, name in BUCKETS:
        if tokens < limit:
            return name
    return ">=700k"


def result_text(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(p.get("text", "") or "" for p in content if isinstance(p, dict))
    return ""


def records(path):
    for line in path.open(encoding="utf-8", errors="replace"):
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            continue


def scan(path):
    """Return the ordered tool events and compaction positions of one transcript."""
    context = {}
    issued = {}
    events = []
    compaction_at = []
    for rec in records(path):
        if rec.get("isCompactSummary"):
            compaction_at.append(len(events))
        msg = rec.get("message")
        if not isinstance(msg, dict):
            continue
        if rec.get("type") == "assistant":
            mid = msg.get("id")
            usage = msg.get("usage")
            if mid and usage:
                context[mid] = sum(int(usage.get(k) or 0) for k in CONTEXT_KEYS)
            for part in msg.get("content") or []:
                if isinstance(part, dict) and part.get("type") == "tool_use":
                    issued[part.get("id")] = (part.get("name"), mid, msg.get("model"))
        elif rec.get("type") == "user" and isinstance(msg.get("content"), list):
            for part in msg["content"]:
                if not (isinstance(part, dict) and part.get("type") == "tool_result"):
                    continue
                name, mid, model = issued.get(part.get("tool_use_id"), (None, None, None))
                if name is None:
                    continue
                error = part.get("is_error") is True
                denied = error and any(d in result_text(part.get("content")).lower() for d in DENIAL)
                events.append({"tool": name, "mid": mid, "model": model, "error": error, "denied": denied})
    for event in events:
        event["context"] = context.get(event["mid"], 0)
    return events, compaction_at


def rate(rows):
    counted = [r for r in rows if not r["denied"]]
    errors = sum(r["error"] for r in counted)
    return {"calls": len(counted), "errors": errors, "rate": round(errors / len(counted), 4) if counted else None}


def main():
    by_bucket = collections.defaultdict(list)
    by_model_bucket = collections.defaultdict(list)
    edits_by_bucket = collections.defaultdict(list)
    before = []
    after = []
    fresh = []
    paired = collections.Counter()
    after_errors_by_tool = collections.Counter()
    denied = 0
    for path in sorted(ROOT.glob("*keleusma*/*.jsonl")):
        events, compactions = scan(path)
        for event in events:
            denied += event["denied"]
            b = bucket(event["context"])
            by_bucket[b].append(event)
            by_model_bucket[(event["model"] or "?", b)].append(event)
            if event["tool"] in EDIT_TOOLS:
                edits_by_bucket[b].append(event)
        if not compactions or compactions[0] > 0:
            fresh.extend(events[:WINDOW])
        for at in compactions:
            pre = events[max(0, at - WINDOW):at]
            post = events[at:at + WINDOW]
            before.extend(pre)
            after.extend(post)
            after_errors_by_tool.update(r["tool"] for r in post if r["error"] and not r["denied"])
            pre_errors = sum(r["error"] and not r["denied"] for r in pre)
            post_errors = sum(r["error"] and not r["denied"] for r in post)
            paired["after_more" if post_errors > pre_errors else "before_more" if pre_errors > post_errors else "tied"] += 1
    order = [name for _, name in BUCKETS] + [">=700k"]
    out = {
        "permission_denials_excluded": denied,
        "all_tools_by_context": {b: rate(by_bucket[b]) for b in order if by_bucket[b]},
        "edit_tools_by_context": {b: rate(edits_by_bucket[b]) for b in order if edits_by_bucket[b]},
        "by_model_and_context": {f"{m} {b}": rate(rows) for (m, b), rows in sorted(by_model_bucket.items())
                                 if len(rows) >= 200},
        "compaction_window_calls": WINDOW,
        "before_compaction": rate(before),
        "after_compaction": rate(after),
        "fresh_session_start": rate(fresh),
        "compactions_paired": dict(paired),
        "after_compaction_errors_by_tool": dict(after_errors_by_tool.most_common()),
        "edit_before_compaction": rate([r for r in before if r["tool"] in EDIT_TOOLS]),
        "edit_after_compaction": rate([r for r in after if r["tool"] in EDIT_TOOLS]),
    }
    json.dump(out, sys.stdout, indent=1, sort_keys=True)
    sys.stdout.write("\n")


main()
