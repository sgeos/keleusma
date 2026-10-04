#!/usr/bin/env python3
"""Install local interline reminders without editing a line's tracked instructions."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile

from interline import identifier

SOURCE = Path(__file__).resolve().parent.parent


def atomic_write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=".interline-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as target:
            target.write(content)
            target.flush()
            os.fsync(target.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def configure(path, command, prefix):
    if path.is_symlink():
        raise ValueError("refusing symlink configuration " + str(path))
    original = path.read_bytes() if path.exists() else None
    config = json.loads(original) if original is not None else {}
    if not isinstance(config, dict) or not isinstance(config.get("hooks", {}), dict):
        raise ValueError("invalid hook configuration " + str(path))
    hooks = config.setdefault("hooks", {})
    for event in ("SessionStart", "UserPromptSubmit"):
        groups = hooks.setdefault(event, [])
        if not isinstance(groups, list):
            raise ValueError("invalid event configuration " + event)
        present = False
        for group in groups:
            if not isinstance(group, dict) or not isinstance(group.get("hooks", []), list):
                raise ValueError("invalid hook group " + event)
            for item in group.get("hooks", []):
                if not isinstance(item, dict) or not isinstance(item.get("command", ""), str):
                    raise ValueError("invalid hook handler " + event)
                existing = item.get("command", "")
                if prefix in existing:
                    if existing != command:
                        raise ValueError("existing interline hook has a different route")
                    present = True
        if not present:
            groups.append({"hooks": [{"type": "command", "command": command, "timeout": 10}]})
    return original, (json.dumps(config, indent=2) + "\n").encode()


def install(store, worktree, line):
    top = subprocess.check_output(
        ["git", "-C", str(worktree), "rev-parse", "--show-toplevel"], text=True,
    ).strip()
    if Path(top).resolve() != worktree:
        raise ValueError("worktree must be the repository root")
    common = Path(subprocess.check_output(
        ["git", "-C", str(worktree), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        text=True,
    ).strip())
    if store != common.parent / "secret" / "interline":
        raise ValueError("store must be secret/interline under the primary checkout")
    client = store / "interline.py"
    base = [sys.executable, str(client), "--store", str(store)]
    command = shlex.join(base + ["hook", "--line", line])
    paths = (worktree / ".codex/hooks.json", worktree / ".claude/settings.local.json")
    prepared = [(path, *configure(path, command, str(client))) for path in paths]
    # Preflight both configurations before writing either. Never replace unrelated hooks.
    store.mkdir(mode=0o700, parents=True, exist_ok=True)
    atomic_write(client, (SOURCE / "scripts/interline.py").read_bytes())
    atomic_write(store / "PROTOCOL.md", (SOURCE / "docs/process/INTERLINE_PROTOCOL.md").read_bytes())
    if not (store / "mail.sqlite3").exists():
        subprocess.run(base + ["init"], check=True, capture_output=True)
    subprocess.run(base + ["register", "--line", line], check=True, capture_output=True)
    # Keep local configuration out of all branch diffs without changing tracked ignore files.
    exclude = common / "info/exclude"
    current = exclude.read_text() if exclude.exists() else ""
    entries = ("/.codex/hooks.json", "/.claude/settings.local.json", "/secret/interline/")
    missing = [entry for entry in entries if entry not in current.splitlines()]
    if missing:
        atomic_write(exclude, (current + "\n# Local interline configuration\n" + "\n".join(missing) + "\n").encode())
    for path, original, updated in prepared:
        if original is not None and original != updated:
            # Preserve original configuration privately, outside any tracked worktree files.
            key = hashlib.sha256(str(path).encode()).hexdigest()
            backup = store / "backups" / (key + ".json")
            if backup.exists() and backup.read_bytes() != original:
                raise ValueError("backup already exists, inspect before changing " + str(path))
            if not backup.exists():
                atomic_write(backup, original)
        if original != updated:
            atomic_write(path, updated)
    return {"line": line, "worktree": str(worktree), "store": str(store), "hook_trust": "review in client"}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--store", type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--worktree", type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--line", type=identifier, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(install(args.store, args.worktree, args.line), indent=2))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print("interline install error: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
