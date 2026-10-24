#!/usr/bin/env python3
"""Protocol contract tests with separate CLI processes and isolated stores."""

from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest

from install_interline import install

SCRIPT = Path(__file__).with_name("interline.py")


class InterlineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.store = self.root / "mail"
        self.body = self.root / "body.txt"
        self.body.write_text("Protocol test only. No work requested.\n", encoding="utf-8")
        self.call("init")
        for line in ("kaizen", "v0.2.3", "v0.3.0"):
            self.call("register", "--line", line)

    def process(self, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--store", str(self.store), *args],
            capture_output=True, text=True, timeout=15,
        )

    def call(self, *args):
        result = self.process(*args)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def reject(self, *args):
        result = self.process(*args)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(result.stdout)
        return result.stderr

    def send(self, message_id="test-1", sender="kaizen", recipient="v0.2.3", harness="codex", extra=()):
        return self.call(
            "send", "--from-line", sender, "--to-line", recipient, "--harness", harness,
            "--id", message_id, "--kind", "test", "--body-file", str(self.body), *extra,
        )

    def test_bidirectional_cross_harness_round_trip(self):
        for first, second in (("codex", "claude-code"), ("claude-code", "codex")):
            request = first + "-request"
            response = first + "-response"
            self.send(request, harness=first)
            inbox = self.call("inbox", "--line", "v0.2.3")
            self.assertIn(request, [item["id"] for item in inbox])
            # Read and receipt are distinct operations, across process restarts.
            read = self.call("show", "--id", request)
            self.assertIsNone(read["acknowledged"])
            ack = self.call("ack", "--id", request, "--line", "v0.2.3", "--harness", second)
            self.assertEqual(ack["acknowledged_by"], second)
            self.send(response, "v0.2.3", "kaizen", second, ("--reply-to", request))
            self.assertEqual(self.call("show", "--id", response)["reply_to"], request)
            self.call("ack", "--id", response, "--line", "kaizen", "--harness", first)
            self.assertEqual(self.call("inbox", "--line", "kaizen"), [])
        self.assertEqual(self.call("inbox", "--line", "v0.2.3"), [])

    def test_concurrent_retry_is_single_message(self):
        with ThreadPoolExecutor(max_workers=8) as pool:
            replies = list(pool.map(lambda _: self.send(), range(16)))
        self.assertTrue(all(reply == replies[0] for reply in replies))
        self.assertEqual(len(self.call("inbox", "--line", "v0.2.3")), 1)

    def test_pilot_reminder_only_advertises_installed_guide(self):
        before = self.process("hook", "--line", "v0.2.3")
        self.assertNotIn("Jev shadow pilot", before.stdout)
        pilot = self.store.parent / "jev-shadow"
        pilot.mkdir()
        (pilot / "GUIDE.md").write_text("Untrusted guide body must not enter hook output.")
        after = self.process("hook", "--line", "v0.2.3")
        self.assertEqual(after.returncode, 0)
        self.assertIn("Jev shadow pilot is available", after.stdout)
        self.assertNotIn("Untrusted guide body", after.stdout)
        self.assertEqual(self.call("inbox", "--line", "v0.2.3"), [])

    def test_harness_is_optional_and_never_part_of_route_or_retry_identity(self):
        args = ("send", "--from-line", "kaizen", "--to-line", "v0.2.3",
                "--id", "neutral", "--kind", "request", "--body-file", str(self.body))
        original = self.call(*args)
        self.assertEqual(original["harness"], "unspecified")
        self.assertEqual(self.call(*args, "--harness", "another-client"), original)
        receipt = self.call("ack", "--id", "neutral", "--line", "v0.2.3")
        self.assertIsNotNone(receipt["acknowledged"])
        reply = self.send("neutral-reply", "v0.2.3", "kaizen", "unknown-client", ("--reply-to", "neutral"))
        self.assertEqual(reply["recipient"], "kaizen")

    def test_checkpoint_restores_pending_mail_receipts_and_retries(self):
        self.send("read-message")
        self.call("ack", "--id", "read-message", "--line", "v0.2.3")
        self.send("pending-reply", "v0.2.3", "kaizen", "unspecified", ("--reply-to", "read-message"))
        before = [self.call("show", "--id", name) for name in ("read-message", "pending-reply")]
        snapshot = self.root / "checkpoint.json"
        self.call("checkpoint", "--output", str(snapshot))
        first_bytes = snapshot.read_bytes()
        self.call("checkpoint", "--output", str(snapshot))
        self.assertEqual(snapshot.read_bytes(), first_bytes)
        self.reject("restore", "--checkpoint", str(snapshot))
        (self.store / "mail.sqlite3").rename(self.store / "original.sqlite3")
        self.call("restore", "--checkpoint", str(snapshot))
        self.assertEqual([self.call("show", "--id", name) for name in ("read-message", "pending-reply")], before)
        self.assertEqual(self.call("inbox", "--line", "v0.2.3"), [])
        self.assertEqual(len(self.call("inbox", "--line", "kaizen")), 1)
        self.assertEqual(self.send("read-message", harness="new-harness"), before[0])

    def test_invalid_checkpoint_never_publishes_partial_database(self):
        self.send()
        snapshot = self.root / "checkpoint.json"
        self.call("checkpoint", "--output", str(snapshot))
        valid = json.loads(snapshot.read_text())
        (self.store / "mail.sqlite3").rename(self.store / "original.sqlite3")
        mutations = [
            lambda state: state.update(protocol=3),
            lambda state: state.update(lines=["kaizen", "kaizen"]),
            lambda state: state["messages"][0].update(recipient="missing"),
            lambda state: state["messages"][0].update(body="x" * 16385),
            lambda state: state["messages"][0].update(created="not-a-time"),
            lambda state: state["messages"][0].update(acknowledged_by="invented-receipt"),
            lambda state: state["messages"][0].update(reply_to="absent"),
            lambda state: state["messages"].append(state["messages"][0].copy()),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                state = json.loads(json.dumps(valid))
                mutate(state)
                snapshot.write_text(json.dumps(state))
                self.reject("restore", "--checkpoint", str(snapshot))
                self.assertFalse((self.store / "mail.sqlite3").exists())

    def test_fresh_clone_bootstraps_without_ignored_state_or_known_harness(self):
        self.send("clone-pending", harness="unspecified")
        self.send("clone-read", harness="unspecified")
        self.call("ack", "--id", "clone-read", "--line", "v0.2.3")
        origin = self.root / "origin"
        clone = self.root / "clone"
        source = SCRIPT.parent.parent
        for name in ("scripts/interline.py", "scripts/install_interline.py", "docs/process/INTERLINE_PROTOCOL.md"):
            target = origin / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / name, target)
        snapshot = origin / "docs/process/interline/checkpoint.json"
        self.call("checkpoint", "--output", str(snapshot))
        for args in (("init", "--quiet"), ("add", "."),
                     ("-c", "user.name=Protocol Test", "-c", "user.email=protocol@example.invalid",
                      "commit", "--quiet", "-m", "Fixture")):
            subprocess.run(["git", "-C", str(origin), *args], check=True, capture_output=True)
        subprocess.run(["git", "clone", "--quiet", str(origin), str(clone)], check=True, capture_output=True)
        self.assertFalse((clone / "secret").exists())
        self.assertFalse((clone / "tmp").exists())
        subprocess.run([sys.executable, "-B", "scripts/install_interline.py", "--line", "v0.2.3"],
                       cwd=clone, check=True, capture_output=True)
        base = [sys.executable, str(clone / "scripts/interline.py"), "--store", str(clone / "secret/interline")]
        inbox = json.loads(subprocess.check_output(base + ["inbox", "--line", "v0.2.3"], text=True))
        self.assertEqual([row["id"] for row in inbox], ["clone-pending"])
        subprocess.run(base + ["ack", "--line", "v0.2.3", "--id", "clone-pending"], check=True, capture_output=True)
        self.assertEqual(json.loads(subprocess.check_output(base + ["inbox", "--line", "v0.2.3"], text=True)), [])

    def test_concurrent_distinct_messages_are_preserved(self):
        with ThreadPoolExecutor(max_workers=8) as pool:
            list(pool.map(lambda i: self.send("message-" + str(i)), range(16)))
        self.assertEqual(len(self.call("inbox", "--line", "v0.2.3")), 16)

    def test_conflicting_retry_rejected(self):
        original = self.send()
        self.body.write_text("Changed content", encoding="utf-8")
        self.reject("send", "--from-line", "kaizen", "--to-line", "v0.2.3", "--harness", "codex",
                    "--id", "test-1", "--kind", "test", "--body-file", str(self.body))
        self.assertEqual(self.call("show", "--id", "test-1"), original)

    def test_ack_requires_recipient_and_is_idempotent(self):
        self.send()
        self.reject("ack", "--line", "v0.3.0", "--harness", "claude-code", "--id", "test-1")
        args = ("ack", "--line", "v0.2.3", "--harness", "claude-code", "--id", "test-1")
        self.assertEqual(self.call(*args), self.call(*args))
        self.reject("ack", "--line", "v0.2.3", "--harness", "claude-code", "--id", "absent")

    def test_reply_route_and_parent_are_validated(self):
        self.send()
        base = ("send", "--from-line", "kaizen", "--to-line", "v0.3.0", "--harness", "codex",
                "--id", "reply", "--kind", "response", "--body-file", str(self.body))
        self.reject(*base, "--reply-to", "test-1")
        self.reject(*base, "--reply-to", "absent")
        self.assertEqual(self.call("inbox", "--line", "v0.3.0"), [])

    def test_unknown_line_and_invalid_identifiers_rejected(self):
        self.reject("inbox", "--line", "absent")
        self.reject("register", "--line", "../escape")
        self.reject("send", "--from-line", "kaizen", "--to-line", "absent", "--harness", "codex",
                    "--id", "test-1", "--kind", "test", "--body-file", str(self.body))

    def test_body_bounds_encoding_and_literal_content(self):
        args = ("send", "--from-line", "kaizen", "--to-line", "v0.2.3", "--harness", "codex",
                "--id", "test-1", "--kind", "test", "--body-file", str(self.body))
        for value in (b"", b"x" * 16385, b"\xff"):
            self.body.write_bytes(value)
            self.reject(*args)
        self.body.write_text("$(touch never-run) `id` \u001b[31m\n" + "x" * 16000, encoding="utf-8")
        self.assertEqual(self.send()["body"], self.body.read_text(encoding="utf-8"))
        self.body.write_bytes(b"x" * 16384)
        self.assertEqual(len(self.send("exact-size-limit")["body"]), 16384)

    def test_init_never_resets_existing_store(self):
        self.send()
        self.reject("init")
        self.assertEqual(len(self.call("inbox", "--line", "v0.2.3")), 1)

    def test_missing_corrupt_and_wrong_schema_fail_visibly(self):
        original = self.store / "mail.sqlite3"
        original.rename(self.store / "saved.sqlite3")
        self.reject("inbox", "--line", "kaizen")
        self.assertFalse(original.exists())
        original.write_bytes(b"not a database")
        self.reject("inbox", "--line", "kaizen")
        original.unlink()
        with sqlite3.connect(original) as db:
            db.execute("PRAGMA user_version = 2")
        self.reject("inbox", "--line", "kaizen")

    def test_inbox_paging_and_hook_do_not_acknowledge(self):
        for i in range(21):
            self.send("message-" + str(i))
        inbox = self.call("inbox", "--line", "v0.2.3")
        self.assertEqual(len(inbox), 20)
        result = self.process("hook", "--line", "v0.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Pending messages 21", result.stdout)
        self.assertNotIn(self.body.read_text(encoding="utf-8"), result.stdout)
        self.assertEqual(self.call("inbox", "--line", "v0.2.3"), inbox)
        self.call("ack", "--id", "message-0", "--line", "v0.2.3", "--harness", "claude-code")
        self.assertEqual(self.call("inbox", "--line", "v0.2.3")[-1]["id"], "message-20")


class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        self.store = self.root / "secret/interline"
        self.codex = self.root / ".codex/hooks.json"
        self.claude = self.root / ".claude/settings.local.json"

    def test_install_preserves_hooks_and_is_repeatable(self):
        self.claude.parent.mkdir()
        prior = {"permissions": {"allow": []}, "hooks": {
            "SessionStart": [{"hooks": [{"type": "command", "command": "echo unrelated"}]}],
        }}
        self.claude.write_text(json.dumps(prior), encoding="utf-8")
        install(self.store, self.root, "kaizen")
        first = [p.read_bytes() for p in (self.codex, self.claude)]
        install(self.store, self.root, "kaizen")
        self.assertEqual(first, [p.read_bytes() for p in (self.codex, self.claude)])
        config = json.loads(self.claude.read_text())
        self.assertEqual(config["permissions"], prior["permissions"])
        self.assertEqual(config["hooks"]["SessionStart"][0], prior["hooks"]["SessionStart"][0])
        self.assertEqual(len(list((self.store / "backups").glob("*.json"))), 1)
        for path in (self.codex, self.claude):
            hooks = json.loads(path.read_text())["hooks"]
            for event in ("SessionStart", "UserPromptSubmit"):
                command = hooks[event][-1]["hooks"][0]["command"]
                # Execute the exact installed hook command with representative client input.
                result = subprocess.run(command, shell=True, input=json.dumps({
                    "hook_event_name": event, "cwd": str(self.root), "session_id": "test",
                }), capture_output=True, text=True, timeout=15)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Worktree default line is kaizen", result.stdout)
        ignored = subprocess.run(["git", "-C", str(self.root), "status", "--porcelain"],
                                 capture_output=True, text=True, check=True)
        self.assertEqual(ignored.stdout, "")

    def test_malformed_config_and_conflicting_route_do_not_replace_configs(self):
        self.claude.parent.mkdir()
        self.claude.write_text("{broken", encoding="utf-8")
        with self.assertRaises(ValueError):
            install(self.store, self.root, "kaizen")
        self.assertFalse(self.codex.exists())
        self.claude.unlink()
        install(self.store, self.root, "kaizen")
        before = [p.read_bytes() for p in (self.codex, self.claude)]
        with self.assertRaises(ValueError):
            install(self.store, self.root, "proofs")
        self.assertEqual(before, [p.read_bytes() for p in (self.codex, self.claude)])

    def test_refuses_wrong_store_and_symlink_configuration(self):
        with self.assertRaises(ValueError):
            install(self.root / "wrong-store", self.root, "kaizen")
        self.codex.parent.mkdir()
        target = self.root / "target.json"
        target.write_text("{}", encoding="utf-8")
        self.codex.symlink_to(target)
        with self.assertRaises(ValueError):
            install(self.store, self.root, "kaizen")
        self.assertEqual(target.read_text(), "{}")


if __name__ == "__main__":
    unittest.main()
