"""Run the packaged model/player closure with null audio and isolated state."""
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest


BIN = Path(sys.argv.pop(1))


class PackagedDaemonTests(unittest.TestCase):
    def test_synthesis_controls_and_clean_shutdown(self):
        with tempfile.TemporaryDirectory(prefix="vd-", dir="/tmp") as directory:
            root = Path(directory)
            env = {**os.environ, "HOME": directory,
                   "XDG_CONFIG_HOME": str(root / "config"),
                   "TTS_MPV_ARGS": "--ao=null"}
            for name in ("XDG_RUNTIME_DIR", "TTS_API_BASE", "TTS_API_KEY_FILE"):
                env.pop(name, None)
            runtime = root / "Library/Caches/vroca"
            if sys.platform != "darwin":
                runtime = root / "run"
                runtime.mkdir(mode=0o700)
                env["XDG_RUNTIME_DIR"] = str(runtime)

            def command(*args):
                result = subprocess.run(
                    [str(BIN / "tts"), *args], env=env,
                    capture_output=True, text=True, timeout=20,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                return result.stdout.strip()

            log_path = root / "daemon.log"
            with log_path.open("w") as log:
                process = subprocess.Popen(
                    [str(BIN / "tts-daemon")], env=env,
                    stdout=log, stderr=log, start_new_session=True,
                )
                try:
                    deadline = time.monotonic() + 90
                    while not (runtime / "tts.sock").exists():
                        if process.poll() is not None or time.monotonic() > deadline:
                            self.fail("daemon did not start:\n" + log_path.read_text())
                        time.sleep(0.1)

                    snapshot = json.loads(command("status"))
                    self.assertEqual(snapshot["engine"], "kokoro")
                    self.assertTrue(snapshot["player_alive"])
                    self.assertEqual(stat.S_IMODE(runtime.stat().st_mode), 0o700)
                    catalogue = json.loads(command("catalogue"))
                    self.assertEqual(len(catalogue), 54)
                    self.assertEqual(catalogue[53]["id"], "em_santa")
                    command("voice", "em_santa")
                    self.assertEqual(json.loads(command("status"))["voice"], 53)
                    command("voice", "af_kore")
                    command("speed", "1.2")
                    command("aligner", "energy")
                    snapshot = json.loads(command("status"))
                    self.assertEqual(snapshot["voice"], 5)
                    self.assertAlmostEqual(snapshot["speed"], 1.2)
                    self.assertEqual(command("say", "Local package smoke test."), "reading 1")

                    deadline = time.monotonic() + 45
                    while True:
                        snapshot = json.loads(command("status"))
                        render_ms = snapshot["last_render_ms"]
                        if isinstance(render_ms, (int, float)) and render_ms > 0:
                            break
                        if time.monotonic() > deadline:
                            self.fail("synthesis did not finish:\n" + log_path.read_text())
                        time.sleep(0.1)
                    self.assertTrue(list(runtime.glob("tts-*/*.wav")))
                    self.assertEqual(command("quit"), "bye")
                    self.assertEqual(process.wait(timeout=10), 0)
                    self.assertFalse((runtime / "tts.sock").exists())
                    self.assertFalse((runtime / "tts-state.json").exists())
                    self.assertFalse(list(runtime.glob("tts-*")))
                finally:
                    # Own a separate process group so a failed assertion cannot
                    # leak this test's daemon/player into the developer session.
                    try:
                        os.killpg(process.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    process.wait(timeout=10)


if __name__ == "__main__":
    unittest.main()
