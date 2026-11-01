"""Relate tool-call failure rates to context size without copying content.

A weak, content-free proxy for whether work degrades as context grows. For
each Claude Code tool call it records the context size of the assistant call
that issued it and whether the harness marked its result as an error. It also
compares calls shortly before and shortly after each compaction, and the
first calls of sessions that began fresh rather than from a compaction. It
reports how the error rate changes with calls since a compaction, the class
of each failed edit by the harness's fixed error phrases, and how many result
characters the first calls after a fresh start or a compaction read, with the
part whose arguments name a process or instruction document. For the
`idleCompaction: false` trial it also reports every resumption after an idle
gap, compacted or not, with the error rate of the calls that follow and the
cache tokens the first assistant call after it wrote and read, by day.

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
import datetime
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
# A compaction after this much idle time is the harness's idle compaction, which runs shortly
# before the one-hour cache expires. A threshold compaction follows activity within minutes.
IDLE_MINUTES = 30
# The prompt cache's time to live. An uncompacted resumption past it rewrites the whole prefix.
CACHE_TTL_MINUTES = 60
RECOVERY = 30
SINCE = ((10, "0-9"), (25, "10-24"), (50, "25-49"), (100, "50-99"), (200, "100-199"))
# Fixed phrases of the harness's own edit errors. Only the class is emitted.
EDIT_CLASSES = (("not read or modified since read", ("has not been read", "modified since")),
                ("string not found", ("not found",)),
                ("several matches", ("matches of the string",)))
STARTUP_DOCS = ("docs/process/", "CLAUDE.md", "AGENTS.md")


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


def since_bucket(n):
    for limit, name in SINCE:
        if n < limit:
            return name
    return None


def edit_class(text):
    lower = text.lower()
    for name, phrases in EDIT_CLASSES:
        if any(phrase in lower for phrase in phrases):
            return name
    return "other"


def mentions_startup_doc(args):
    """Whether a tool call's own arguments name a process or instruction document."""
    target = " ".join(str(args.get(k, "")) for k in ("file_path", "command", "path", "pattern"))
    return any(doc in target for doc in STARTUP_DOCS)


def records(path):
    for line in path.open(encoding="utf-8", errors="replace"):
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            continue


def scan(path):
    """Return the ordered tool events, compaction positions and idle-gap resumptions of one
    transcript."""
    context = {}
    issued = {}
    events = []
    compaction_at = []
    gaps = []
    pending = None
    last_activity = None
    for rec in records(path):
        stamp = rec.get("timestamp")
        moment = datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00")) if stamp else None
        summary = bool(rec.get("isCompactSummary"))
        idle = bool(moment and last_activity and (moment - last_activity).total_seconds() / 60 >= IDLE_MINUTES)
        # A resumption opens at the first activity or compaction after an idle gap and closes
        # at the first assistant call with usage, whose cache tokens it records.
        if idle and pending is None and (summary or rec.get("type") in ("assistant", "user")):
            pending = {"at": len(events), "day": moment.date().isoformat(), "compacted": False,
                       "minutes": (moment - last_activity).total_seconds() / 60}
        if summary:
            compaction_at.append((len(events), "idle" if idle else "threshold"))
            if pending is not None:
                pending["compacted"] = True
        msg = rec.get("message")
        if not isinstance(msg, dict):
            continue
        if moment and rec.get("type") in ("assistant", "user") and not summary:
            last_activity = moment
        if rec.get("type") == "assistant":
            mid = msg.get("id")
            usage = msg.get("usage")
            if mid and usage:
                context[mid] = sum(int(usage.get(k) or 0) for k in CONTEXT_KEYS)
                if pending is not None:
                    pending["cache_write"] = int(usage.get("cache_creation_input_tokens") or 0)
                    pending["cache_read"] = int(usage.get("cache_read_input_tokens") or 0)
                    gaps.append(pending)
                    pending = None
            for part in msg.get("content") or []:
                if isinstance(part, dict) and part.get("type") == "tool_use":
                    args = part.get("input") if isinstance(part.get("input"), dict) else {}
                    issued[part.get("id")] = (part.get("name"), mid, msg.get("model"), mentions_startup_doc(args))
        elif rec.get("type") == "user" and isinstance(msg.get("content"), list):
            for part in msg["content"]:
                if not (isinstance(part, dict) and part.get("type") == "tool_result"):
                    continue
                name, mid, model, doc = issued.get(part.get("tool_use_id"), (None, None, None, False))
                if name is None:
                    continue
                text = result_text(part.get("content"))
                error = part.get("is_error") is True
                denied = error and any(d in text.lower() for d in DENIAL)
                events.append({"tool": name, "mid": mid, "model": model, "error": error, "denied": denied,
                               "chars": len(text), "doc": doc,
                               "edit_class": edit_class(text) if error and name in EDIT_TOOLS else None})
    for event in events:
        event["context"] = context.get(event["mid"], 0)
    return events, compaction_at, gaps


def rate(rows, guard=True):
    # guard=False also excludes the harness's read-before-edit guard, which compaction trips.
    counted = [r for r in rows if not r["denied"] and (guard or r["edit_class"] != "not read or modified since read")]
    errors = sum(r["error"] for r in counted)
    return {"calls": len(counted), "errors": errors, "rate": round(errors / len(counted), 4) if counted else None}


def median(values):
    ordered = sorted(values)
    if not ordered:
        return None
    mid = len(ordered) // 2
    return ordered[mid] if len(ordered) % 2 else (ordered[mid - 1] + ordered[mid]) // 2


def gap_summary(rows):
    return {"resumptions": len(rows), "after": rate([r for g in rows for r in g["after"]], guard=False),
            "first_call_cache_write_median": median([g["cache_write"] for g in rows]),
            "first_call_cache_read_median": median([g["cache_read"] for g in rows])}


def main():
    by_bucket = collections.defaultdict(list)
    by_model_bucket = collections.defaultdict(list)
    edits_by_bucket = collections.defaultdict(list)
    before = []
    after = []
    fresh = []
    paired = collections.Counter()
    by_kind = collections.defaultdict(lambda: {"before": [], "after": [], "paired": collections.Counter()})
    after_errors_by_tool = collections.Counter()
    since = collections.defaultdict(list)
    edit_classes = collections.Counter()
    recovery = {"fresh": collections.Counter(), "compaction": collections.Counter()}
    denied = 0
    resumptions = []
    for path in sorted(ROOT.glob("*keleusma*/*.jsonl")):
        events, compactions, gaps = scan(path)
        for gap in gaps:
            gap["after"] = events[gap["at"]:gap["at"] + WINDOW]
            resumptions.append(gap)
        for event in events:
            denied += event["denied"]
            b = bucket(event["context"])
            by_bucket[b].append(event)
            by_model_bucket[(event["model"] or "?", b)].append(event)
            if event["tool"] in EDIT_TOOLS:
                edits_by_bucket[b].append(event)
        for event in events:
            if event["edit_class"]:
                edit_classes[event["edit_class"]] += 1
        starts = []
        if not compactions or compactions[0][0] > 0:
            fresh.extend(events[:WINDOW])
            starts.append(("fresh", 0))
        starts.extend(("compaction", at) for at, _ in compactions)
        for kind, at in starts:
            window = events[at:at + RECOVERY]
            recovery[kind]["windows"] += 1
            recovery[kind]["calls"] += len(window)
            recovery[kind]["result_chars"] += sum(r["chars"] for r in window)
            recovery[kind]["startup_doc_calls"] += sum(r["doc"] for r in window)
            recovery[kind]["startup_doc_chars"] += sum(r["chars"] for r in window if r["doc"])
            if window:
                recovery[kind]["context_at_window_end_total"] += window[-1]["context"]
                recovery[kind]["context_at_window_start_total"] += window[0]["context"]
        for i, (at, _) in enumerate(compactions):
            end = compactions[i + 1][0] if i + 1 < len(compactions) else len(events)
            for n, event in enumerate(events[at:end]):
                b = since_bucket(n)
                if b:
                    since[b].append(event)
        for at, kind in compactions:
            pre = events[max(0, at - WINDOW):at]
            post = events[at:at + WINDOW]
            before.extend(pre)
            after.extend(post)
            by_kind[kind]["before"].extend(pre)
            by_kind[kind]["after"].extend(post)
            after_errors_by_tool.update(r["tool"] for r in post if r["error"] and not r["denied"])
            pre_errors = sum(r["error"] and not r["denied"] for r in pre)
            post_errors = sum(r["error"] and not r["denied"] for r in post)
            verdict = "after_more" if post_errors > pre_errors else "before_more" if pre_errors > post_errors else "tied"
            paired[verdict] += 1
            by_kind[kind]["paired"][verdict] += 1
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
        "by_compaction_kind": {kind: {"before": rate(v["before"], guard=False), "after": rate(v["after"], guard=False),
                                      "paired": dict(v["paired"])} for kind, v in sorted(by_kind.items())},
        "before_compaction_without_edit_guard": rate(before, guard=False),
        "after_compaction_without_edit_guard": rate(after, guard=False),
        "fresh_session_start_without_edit_guard": rate(fresh, guard=False),
        "by_calls_since_compaction": {name: rate(since[name]) for _, name in SINCE if since[name]},
        "edit_error_classes": dict(edit_classes.most_common()),
        "recovery_window_calls": RECOVERY,
        "recovery_reads": {k: dict(v) for k, v in recovery.items()},
        "compactions_paired": dict(paired),
        "after_compaction_errors_by_tool": dict(after_errors_by_tool.most_common()),
        "edit_before_compaction": rate([r for r in before if r["tool"] in EDIT_TOOLS]),
        "edit_after_compaction": rate([r for r in after if r["tool"] in EDIT_TOOLS]),
        "idle_gap_minutes": IDLE_MINUTES,
        "idle_gap_resumptions": {
            "compacted": gap_summary([g for g in resumptions if g["compacted"]]),
            "uncompacted_within_cache_ttl": gap_summary(
                [g for g in resumptions if not g["compacted"] and g["minutes"] < CACHE_TTL_MINUTES]),
            "uncompacted_past_cache_ttl": gap_summary(
                [g for g in resumptions if not g["compacted"] and g["minutes"] >= CACHE_TTL_MINUTES])},
        "idle_gap_resumptions_by_day": {
            day: {kind: sum(1 for g in resumptions if g["day"] == day and g["compacted"] == flag)
                  for kind, flag in (("compacted", True), ("uncompacted", False))}
            for day in sorted({g["day"] for g in resumptions})},
    }
    json.dump(out, sys.stdout, indent=1, sort_keys=True)
    sys.stdout.write("\n")


main()
