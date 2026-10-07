#!/usr/bin/env python3
"""Acceptance of the actual native executable's private JSON boundary."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BINARY = Path(sys.argv.pop(1)).resolve(strict=True) if len(sys.argv) > 1 else Path("/usr/local/bin/ion-harness-worker")
MARKER = "fixture-private-input-must-never-be-echoed"


class WorkerContract(unittest.TestCase):
    def invoke(self, data, args=(), expected=1):
        with tempfile.TemporaryDirectory(prefix="ion-worker-contract-") as root:
            env = {k: v for k, v in os.environ.items() if k.upper() in {"PATH", "SYSTEMROOT", "WINDIR", "LANG"}}
            env.update(HOME=root, USERPROFILE=root, APPDATA=root, LOCALAPPDATA=root,
                       TMPDIR=root, TEMP=root, TMP=root)
            result = subprocess.run([str(BINARY), *args], input=data, capture_output=True,
                                    cwd=root, env=env, timeout=15, check=False)
        self.assertEqual(result.returncode, expected)
        self.assertEqual(result.stdout, b"")
        self.assertTrue(result.stderr)
        self.assertLess(len(result.stderr), 1024)
        self.assertNotIn(MARKER.encode(), result.stderr)
        return result

    @staticmethod
    def request():
        return {"version": 1, "request_id": "ed4a52ab-e0cf-4f88-b32f-9bd84a0c8c0f",
                "capability": {"provider": "amp", "model": "fixture-model", "input_cap": 1000, "output_cap": 100},
                "system": MARKER, "messages": [], "tools": []}

    def test_arguments_are_not_echoed(self):
        self.invoke(b"", [MARKER], expected=2)

    def test_malformed_input_has_private_diagnostic(self):
        self.invoke(("{invalid " + MARKER).encode())

    def test_oversized_input_fails_with_no_output(self):
        self.invoke(b" " * (128 * 1024 + 1))

    def test_unimplemented_harness_does_not_substitute(self):
        self.invoke(json.dumps(self.request()).encode())

    def test_invalid_version(self):
        value = self.request()
        value["version"] = 2
        self.invoke(json.dumps(value).encode())

    def test_missing_request_identity(self):
        value = self.request()
        value["request_id"] = "00000000-0000-0000-0000-000000000000"
        self.invoke(json.dumps(value).encode())

    def test_invalid_caps(self):
        for key, amount in [("input_cap", 0), ("output_cap", 0), ("input_cap", 32000001), ("output_cap", 32000001)]:
            with self.subTest(key=key, amount=amount):
                value = self.request()
                value["capability"][key] = amount
                self.invoke(json.dumps(value).encode())

    def test_explicit_identity_fails_before_native_authentication(self):
        for identity in [{"uid": 0, "gid": 1000, "home": "/home/ion"},
                         {"uid": 1000, "gid": 1000, "home": "/home/ion"},
                         {"uid": 1000, "gid": 1000, "home": "/home/ion", "private": MARKER}]:
            with self.subTest(identity=identity):
                value = self.request()
                value["capability"]["provider"] = "codex"
                value["execution"] = identity
                # Even the valid registered identity must reject this isolated
                # temporary HOME/cwd before discovering a native account.
                self.invoke(json.dumps(value).encode())

    def test_unrecognized_input_cannot_return_private_payload(self):
        value = self.request()
        value["private_payload"] = MARKER
        self.invoke(json.dumps(value).encode())


if __name__ == "__main__":
    unittest.main()
