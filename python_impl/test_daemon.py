import importlib.util
import sys
import types
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


# Selection handling is independent of synthesis, so provide only the engine
# names daemon.py imports rather than loading the model runtime in unit tests.
engines = types.ModuleType("engines")
engines.CLONE_DIR = "/tmp/tts-voices"
engines.CONFIG_DIR = "/tmp/tts-config"
engines.DEFAULT_ENGINE = "kokoro"
engines.ENGINES = {}
engines.RemoteEngine = object
engines.build_engine = lambda *args, **kwargs: None
engines.list_clone_refs = lambda: []
sys.modules["engines"] = engines

spec = importlib.util.spec_from_file_location("tts_daemon", Path(__file__).with_name("daemon.py"))
daemon = importlib.util.module_from_spec(spec)
spec.loader.exec_module(daemon)


class SelectionTests(unittest.TestCase):
    @staticmethod
    def result(text="", returncode=0):
        return SimpleNamespace(stdout=text, returncode=returncode)

    def test_primary_selection_wins_over_clipboard(self):
        with patch.object(daemon.sys, "platform", "linux"), \
             patch.object(daemon.subprocess, "run", return_value=self.result("primary")) as run:
            self.assertEqual(daemon.selection(), "primary")

        run.assert_called_once_with(
            ["wl-paste", "--primary", "--no-newline"],
            capture_output=True, text=True, timeout=2,
        )

    def test_clipboard_is_used_when_primary_is_empty(self):
        results = [self.result(), self.result(returncode=1), self.result("clipboard")]
        with patch.object(daemon.sys, "platform", "linux"), \
             patch.object(daemon.subprocess, "run", side_effect=results) as run:
            self.assertEqual(daemon.selection(), "clipboard")

        self.assertEqual(run.call_args_list[-1].args[0], ["wl-paste", "--no-newline"])

    def test_empty_when_no_selection_source_has_text(self):
        with patch.object(daemon.sys, "platform", "linux"), \
             patch.object(daemon.subprocess, "run", return_value=self.result(returncode=1)) as run:
            self.assertEqual(daemon.selection(), "")

        self.assertEqual(run.call_count, 4)


if __name__ == "__main__":
    unittest.main()
