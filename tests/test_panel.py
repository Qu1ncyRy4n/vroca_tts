"""Native GTK smoke test; run in a GUI session via nix develop.

Uses an isolated socket, never a real daemon, clipboard, or audio device.
"""
import json
from pathlib import Path
import socket
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python_impl"))
import panel
from gi.repository import GLib, Gtk


class PanelSmoke(unittest.TestCase):
    def test_controls_and_responsiveness(self):
        errors = []
        commands = []
        state = dict(total=0, index=0, paused=False, rendered=0, sentence="",
                     speed=1.25, volume=80, trim=0, font_size=24,
                     words_visible=3, loaded=True, engine="kokoro",
                     engines=["kokoro"], aligner="asr")
        with tempfile.TemporaryDirectory(prefix="vroca-panel-", dir="/tmp") as tmp:
            server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            server.bind(tmp + "/vroca.sock")
            server.listen()
            server.settimeout(0.1)
            stop = threading.Event()

            def serve():
                while not stop.is_set():
                    try:
                        conn, _ = server.accept()
                    except socket.timeout:
                        continue
                    with conn:
                        cmd = conn.recv(4096).decode()
                        commands.append(cmd)
                        if cmd == "status":
                            reply = json.dumps(state)
                        elif cmd == "catalogue":
                            reply = json.dumps([dict(sid=0, name="Test voice",
                                                     lang="en", gender="F")])
                        else:
                            if cmd.startswith("preview"):
                                time.sleep(0.8)
                            reply = "ok"
                        conn.sendall(reply.encode())

            thread = threading.Thread(target=serve)
            thread.start()
            app = panel.Panel()
            old_hook = sys.excepthook
            sys.excepthook = lambda *error: errors.append(error)

            def pump_until(predicate, timeout=5):
                deadline = time.monotonic() + timeout
                context = GLib.MainContext.default()
                while time.monotonic() < deadline:
                    while context.pending():
                        context.iteration(False)
                    if errors:
                        raise errors[0][1]
                    if predicate():
                        return
                    time.sleep(0.01)
                self.fail("GTK condition timed out; commands: " + repr(commands))

            try:
                with patch.object(panel, "SOCK", tmp + "/vroca.sock"), \
                     patch.object(panel, "MODE_FILE", tmp + "/vroca-mode"):
                    self.assertTrue(app.register(None))
                    app.activate()
                    pump_until(lambda: app.store.get_n_items() == 1)
                    self.assertEqual(app.speed.get_value(), 1.25)
                    self.assertEqual(app.volume.get_value(), 80)
                    self.assertTrue(all(c in ("status", "catalogue") for c in commands),
                                    "status refresh must not write preferences")
                    app.search.set_text("absent")
                    app._refilter()
                    self.assertEqual(app.filtered.get_n_items(), 0)
                    app.search.set_text("")
                    app._refilter()
                    app.vlist.get_model().set_selected(0)
                    app.speed.set_value(1.5)
                    app.volume.set_value(70)
                    app._use_voice(None)
                    app._preview(None)
                    pump_until(lambda: "preview 0" in commands)
                    heartbeat = []
                    GLib.timeout_add(50, lambda: heartbeat.append(True) and False)
                    pump_until(lambda: bool(heartbeat), timeout=0.4)
                    for cmd in ("read", "toggle", "back", "next", "stop"):
                        app._run(None, cmd)
                    pump_until(lambda: "stop" in commands)
                    for cmd in ("speed 1.50", "volume 70", "voice 0"):
                        self.assertIn(cmd, commands)
                    state.update(engine="other", engines=["kokoro", "other"])
                    pump_until(lambda: commands.count("catalogue") >= 2)
                    pump_until(lambda: app.store.get_n_items() == 1 and
                               app.engine.get_selected() == 1)
                    if panel.IS_MAC:
                        with patch.object(GLib, "spawn_async") as spawn:
                            app._update_status(None)
                            self.assertIn("Daemon unavailable", app.stat.get_text())
                            spawn.assert_not_called()
                        self.assertFalse(app.mode.get_parent().get_visible())
                        self.assertFalse(app.aligner.get_parent().get_visible())
                    print("GTK controls, catalogue, read-only refresh, slow-preview responsiveness, and missing daemon: passed")
            finally:
                app._closed = True
                if app._timer is not None:
                    GLib.source_remove(app._timer)
                    app._timer = None
                app._worker.shutdown(wait=True, cancel_futures=True)
                if app.get_active_window():
                    app.get_active_window().close()
                sys.excepthook = old_hook
                stop.set()
                thread.join(timeout=2)
                server.close()


if __name__ == "__main__":
    unittest.main(warnings="default")
