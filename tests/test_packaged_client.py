"""Exercise the installed CLI against a real, isolated legacy socket."""
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest


CLIENT = sys.argv.pop(1)


class PackagedClientTests(unittest.TestCase):
    def setUp(self):
        # Darwin Unix sockets have a short path limit. Do not inherit the Nix
        # builder's long TMPDIR or the logged-in user's runtime directory.
        self.temp = tempfile.TemporaryDirectory(prefix="vc-", dir="/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {**os.environ, "HOME": str(self.root)}
        self.env.pop("XDG_RUNTIME_DIR", None)

    def invoke(self, *args):
        return subprocess.run(
            [CLIENT, *args], env=self.env, capture_output=True, text=True,
            timeout=10,
        )

    def exchange(self, arguments, expected, response=b"ok", runtime=None, delay=0):
        runtime = runtime or self.root / "run"
        runtime.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.env["XDG_RUNTIME_DIR"] = str(runtime)
        errors = []
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(runtime / "tts.sock"))
            listener.listen(1)
            listener.settimeout(10)

            def serve():
                try:
                    conn, _ = listener.accept()
                    with conn:
                        conn.settimeout(10)
                        data = bytearray()
                        while part := conn.recv(65536):
                            data.extend(part)
                        self.assertEqual(bytes(data).decode(), expected)
                        time.sleep(delay)
                        conn.sendall(response)
                except Exception as exc:
                    errors.append(exc)

            worker = threading.Thread(target=serve, daemon=True)
            worker.start()
            result = self.invoke(*arguments)
            worker.join(timeout=12)
            self.assertFalse(worker.is_alive(), "fake daemon did not finish")
            if errors:
                raise errors[0]
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, response.decode() + "\n")

    def test_argument_forwarding(self):
        for arguments in [
            ["speed", "1.2"], ["voice", "kokoro:af_kore"],
            ["volume", "80"], ["trim", "-15"], ["mode_set", "scroll_rsvp"],
            ["queue", "--voice", "sid3", "Hello café 世界"],
        ]:
            with self.subTest(arguments=arguments):
                self.exchange(arguments, " ".join(arguments))
                (self.root / "run/tts.sock").unlink()

    def test_no_arguments_reads_selection(self):
        self.exchange([], "read")

    def test_large_response_is_complete(self):
        self.exchange(["catalogue"], "catalogue", response=b"x" * 150000)

    def test_waits_for_a_slow_synthesis_reply(self):
        self.exchange(["say", "hello"], "say hello", response=b"reading 1", delay=0.8)

    def test_missing_daemon_fails_without_creating_runtime(self):
        runtime = self.root / "missing"
        self.env["XDG_RUNTIME_DIR"] = str(runtime)
        result = self.invoke("status")
        self.assertEqual(result.returncode, 1)
        self.assertIn("daemon not running", result.stderr)
        self.assertFalse(runtime.exists())

    @unittest.skipUnless(sys.platform == "darwin", "Mac runtime policy")
    def test_mac_runtime_fallback(self):
        runtime = self.root / "Library/Caches/vroca"
        runtime.mkdir(parents=True, mode=0o700)
        # exchange normally sets an explicit path; remove it at invocation to
        # prove the packaged client finds the approved default instead.
        original = self.invoke

        def invoke_default(*args):
            self.env.pop("XDG_RUNTIME_DIR", None)
            return original(*args)

        self.invoke = invoke_default
        self.exchange(["status"], "status", runtime=runtime)

    @unittest.skipUnless(sys.platform == "darwin", "Mac runtime policy")
    def test_mac_runtime_rejects_symlinks_and_regular_files(self):
        target = self.root / "target"
        target.mkdir()
        link = self.root / "link"
        link.symlink_to(target)
        regular = self.root / "file"
        regular.write_text("preserve me")
        for path in (link, regular):
            with self.subTest(path=path):
                self.env["XDG_RUNTIME_DIR"] = str(path)
                result = self.invoke("status")
                self.assertEqual(result.returncode, 1)
                self.assertIn("real directory owned by", result.stderr)
        self.assertEqual(regular.read_text(), "preserve me")


if __name__ == "__main__":
    unittest.main()
