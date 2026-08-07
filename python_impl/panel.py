#!/usr/bin/env python3
"""TTS config panel -- Super+Shift+Z.

A GTK4 control panel to configure speech engine, voice, aligner, overlay mode,
font size, context window, screen positioning, and queue actions.
"""
import json
import os
import socket
import sys
import tempfile

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402

RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
SOCK = os.path.join(RUNTIME, "tts.sock")
MODE_FILE = os.path.join(RUNTIME, "tts-mode")
MODES = ["subtitle", "rsvp", "scroll_rsvp", "off"]
ALIGNERS = ["asr", "energy"]
POSITIONS = ["bottom", "top", "center"]


def _label_factory():
    f = Gtk.SignalListItemFactory()
    def _setup(_f, li):
        lbl = Gtk.Label(xalign=0, margin_start=6, margin_top=2, margin_bottom=2)
        lbl.add_css_class("vlabel")
        li.set_child(lbl)
    f.connect("setup", _setup)
    f.connect("bind", lambda _f, li: li.get_child().set_text(
        li.get_item().get_string()))
    return f


CSS = b"""
window, .background { background-color: #16161d; color: #c8ccd4; }
#title { font-size: 15pt; font-weight: 700; color: #f2f2f7; }
#stat  { font-family: monospace; font-size: 10pt; color: #c8ccd4; }
#dim   { color: #6f7683; font-size: 10pt; }
separator { background-color: #262631; min-height: 1px; }
button {
  background-image: none; background-color: #232330;
  color: #ffffff; border: 1px solid #2f2f3d; border-radius: 8px;
  padding: 7px 10px;
}
button:hover { background-color: #2c2c3b; color: #ffffff; }
button:active { background-color: #343446; }
button.danger { color: #e05561; border-color: #4a2b31; }
button.danger:hover { background-color: #3a2329; }
dropdown {
  background-color: #000000; color: #ffffff;
  border: 1px solid #2f2f3d; border-radius: 8px;
}
dropdown > button { background-color: #000000; color: #ffffff; border: none; }
dropdown > button label { color: #ffffff; }
popover, popover contents, popover > arrow {
  background-color: #000000; color: #ffffff;
  border: 1px solid #2f2f3d;
}
popover listview, popover listview row {
  background-color: #000000; color: #ffffff;
}
popover listview row:selected, popover listview row:hover {
  background-color: #262638; color: #ffffff;
}
searchentry, searchentry text {
  background-color: #000000; color: #ffffff;
  border: 1px solid #2f2f3d; border-radius: 6px;
}
scrolledwindow, scrolledwindow viewport, listview {
  background-color: #121218; color: #ffffff;
  border: 1px solid #2f2f3d; border-radius: 8px;
}
listview row, listview row label, listitem label, .vlabel {
  background-color: #121218; color: #ffffff;
}
listview row:hover, listview row:selected {
  background-color: #262638; color: #ffffff;
}
listview row:hover label, listview row:selected label {
  background-color: #262638; color: #ffffff;
}
scale trough { background-color: #262631; min-height: 5px; border-radius: 3px; }
scale highlight { background-color: #7aa2f7; border-radius: 3px; }
scale slider {
  background-color: #ffffff; border-radius: 8px;
  min-width: 14px; min-height: 14px; margin: -5px;
}
scale value { color: #9aa0a6; font-size: 9pt; }
"""


def send(cmd, timeout=3.0):
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(timeout)
        s.connect(SOCK)
        s.send(cmd.encode())
        out = b""
        while True:
            chunk = s.recv(4096)
            if not chunk:
                break
            out += chunk
        s.close()
        return out.decode()
    except OSError:
        return None


class Panel(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="dev.qix.ttspanel")

    def do_activate(self):
        win = Gtk.ApplicationWindow(application=self, title="Vroca Text-to-Speech")
        win.set_default_size(460, 780)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12,
                       margin_top=16, margin_bottom=16,
                       margin_start=16, margin_end=16)

        title = Gtk.Label(label="Vroca TTS Control Panel", xalign=0)
        title.set_name("title")
        root.append(title)

        self.stat = Gtk.Label(xalign=0, wrap=True)
        self.stat.set_name("stat")
        root.append(self.stat)

        root.append(Gtk.Separator())

        srow = Gtk.Box(spacing=10)
        srow.append(Gtk.Label(label="Speed"))
        self.speed = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0.5, 3.0, 0.05)
        self.speed.set_hexpand(True)
        self.speed.set_draw_value(True)
        self.speed.set_value(1.0)
        self.speed.connect("value-changed",
                           lambda w: send(f"speed {w.get_value():.2f}"))
        srow.append(self.speed)
        root.append(srow)

        vrow = Gtk.Box(spacing=10)
        vrow.append(Gtk.Label(label="Volume"))
        self.volume = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        self.volume.set_hexpand(True)
        self.volume.set_draw_value(True)
        self.volume.set_value(100)
        self.volume.connect("value-changed",
                            lambda w: self._send_if_user(f"volume {int(w.get_value())}"))
        vrow.append(self.volume)
        root.append(vrow)

        trow = Gtk.Box(spacing=10)
        trow.append(Gtk.Label(label="Voice Trim"))
        self.trim = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, -50, 50, 1)
        self.trim.set_hexpand(True)
        self.trim.set_draw_value(True)
        self.trim.set_value(0)
        self.trim.connect("value-changed",
                          lambda w: self._send_if_user(f"trim {int(w.get_value())}"))
        trow.append(self.trim)
        root.append(trow)

        erow = Gtk.Box(spacing=10)
        erow.append(Gtk.Label(label="Engine"))
        self.engine = Gtk.DropDown.new_from_strings(["kokoro"])
        self.engine.set_hexpand(True)
        self.engine.connect("notify::selected", self._set_engine)
        self.engine_names = ["kokoro"]
        erow.append(self.engine)
        root.append(erow)

        self.search = Gtk.SearchEntry(placeholder_text="Search voices (name, lang, gender)")
        self.search.connect("search-changed", lambda *_: self._refilter())
        root.append(self.search)

        self.store = Gtk.StringList.new([])
        self.filtered = Gtk.FilterListModel.new(self.store, None)
        self.vfilter = Gtk.CustomFilter.new(self._match)
        self.filtered.set_filter(self.vfilter)
        self.vlist = Gtk.ListView.new(
            Gtk.SingleSelection.new(self.filtered), _label_factory())
        self.vlist.connect("activate", self._activate_voice)
        scroll = Gtk.ScrolledWindow(vexpand=True, min_content_height=180)
        scroll.set_child(self.vlist)
        root.append(scroll)

        vbtn = Gtk.Box(spacing=8, homogeneous=True)
        for label, fn in (("Preview", self._preview), ("Use voice", self._use_voice)):
            b = Gtk.Button(label=label)
            b.connect("clicked", fn)
            vbtn.append(b)
        root.append(vbtn)

        self.catalogue = []
        self.n_voices = 1

        arow = Gtk.Box(spacing=10)
        arow.append(Gtk.Label(label="Aligner"))
        self.aligner = Gtk.DropDown.new_from_strings(ALIGNERS)
        self.aligner.set_hexpand(True)
        self.aligner.connect("notify::selected", self._set_aligner)
        arow.append(self.aligner)
        root.append(arow)

        mrow = Gtk.Box(spacing=10)
        mrow.append(Gtk.Label(label="Overlay Mode"))
        self.mode = Gtk.DropDown.new_from_strings(MODES)
        self.mode.set_hexpand(True)
        self.mode.connect("notify::selected", self._set_mode)
        mrow.append(self.mode)
        root.append(mrow)

        prow = Gtk.Box(spacing=10)
        prow.append(Gtk.Label(label="Position"))
        self.position = Gtk.DropDown.new_from_strings(POSITIONS)
        self.position.set_hexpand(True)
        self.position.connect("notify::selected", self._set_position)
        prow.append(self.position)
        root.append(prow)

        fsrow = Gtk.Box(spacing=10)
        fsrow.append(Gtk.Label(label="RSVP Font Size"))
        self.font_size = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 14, 48, 1)
        self.font_size.set_hexpand(True)
        self.font_size.set_draw_value(True)
        self.font_size.set_value(24)
        self.font_size.connect("value-changed",
                               lambda w: send(f"font_size {int(w.get_value())}"))
        fsrow.append(self.font_size)
        root.append(fsrow)

        wvrow = Gtk.Box(spacing=10)
        wvrow.append(Gtk.Label(label="Scroll Context Words"))
        self.words_vis = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 1, 9, 1)
        self.words_vis.set_hexpand(True)
        self.words_vis.set_draw_value(True)
        self.words_vis.set_value(3)
        self.words_vis.connect("value-changed",
                               lambda w: send(f"words_visible {int(w.get_value())}"))
        wvrow.append(self.words_vis)
        root.append(wvrow)

        root.append(Gtk.Separator())

        qrow = Gtk.Box(spacing=8, homogeneous=True)
        for label, cmd in (("Skip Item", "skip"), ("Clear Queue", "clear"), ("Reset Defaults", "reset")):
            b = Gtk.Button(label=label)
            b.connect("clicked", self._run, cmd)
            qrow.append(b)
        root.append(qrow)

        brow = Gtk.Box(spacing=8, homogeneous=True)
        for label, cmd, danger in (("Stop", "stop", False),
                                   ("Reload model", "reload", False),
                                   ("Unload model", "unload", False),
                                   ("Quit daemon", "quit", True)):
            b = Gtk.Button(label=label)
            if danger:
                b.add_css_class("danger")
            b.connect("clicked", self._run, cmd)
            brow.append(b)
        root.append(brow)

        win.set_child(root)

        css = Gtk.CssProvider()
        css.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_display(
            win.get_display(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        esc = Gtk.EventControllerKey()
        esc.connect("key-pressed", lambda c, k, code, st: (
            win.close() if k == 0xff1b else None))
        win.add_controller(esc)

        self.win = win
        self._syncing = False
        self._starting = False
        self._refresh()
        GLib.timeout_add(400, self._refresh)
        win.present()

    def _send_if_user(self, cmd):
        """Only forward slider moves the user made. _refresh writes these
        scales to match daemon state, and without this guard each refresh would
        echo the value straight back and fight anything set elsewhere."""
        if getattr(self, "_syncing", True):
            return
        send(cmd)

    def _set_mode(self, *_):
        if self._syncing:
            return
        with open(MODE_FILE, "w") as f:
            f.write(MODES[self.mode.get_selected()])

    def _match(self, item):
        q = self.search.get_text().strip().lower()
        return not q or q in item.get_string().lower()

    def _refilter(self):
        self.vfilter.changed(Gtk.FilterChange.DIFFERENT)

    def _selected_sid(self):
        sel = self.vlist.get_model().get_selected_item()
        if sel is None:
            return None
        label = sel.get_string()
        try:
            return int(label.split(None, 1)[0])
        except ValueError:
            return None

    def _activate_voice(self, *_):
        self._use_voice(None)

    def _use_voice(self, _btn):
        sid = self._selected_sid()
        if sid is not None:
            send(f"voice {sid}", timeout=30)

    def _preview(self, _btn):
        sid = self._selected_sid()
        if sid is not None:
            self.stat.set_text(f"previewing voice {sid}…")
            send(f"preview {sid}", timeout=30)

    def _set_engine(self, *_):
        if self._syncing:
            return
        name = self.engine_names[self.engine.get_selected()]
        self.stat.set_text(f"switching to {name}…")
        r = send(f"engine {name}", timeout=60)
        self.catalogue = []
        if r:
            self.stat.set_text(r)

    def _set_aligner(self, *_):
        if self._syncing:
            return
        send(f"aligner {ALIGNERS[self.aligner.get_selected()]}", timeout=10)

    def _set_position(self, *_):
        if self._syncing:
            return
        send(f"position {POSITIONS[self.position.get_selected()]}", timeout=10)

    def _run(self, _btn, cmd):
        r = send(cmd, timeout=30 if cmd == "reload" else 3)
        if cmd == "quit":
            self.win.close()
        elif r:
            self.stat.set_text(r)

    def _refresh(self):
        raw = send("status", timeout=0.5)
        if raw is None:
            if not self._starting:
                self._starting = True
                self.stat.set_text("daemon not running — starting…")
                GLib.spawn_async(["systemctl", "--user", "start", "tts"],
                                 flags=GLib.SpawnFlags.SEARCH_PATH)
            return True
        self._starting = False
        try:
            st = json.loads(raw)
        except ValueError:
            return True

        model = "loaded" if st.get("loaded") else "unloaded"
        qlen = st.get("queue_len", 0)
        qstr = f"   queue: {qlen}" if qlen else ""
        if st["total"]:
            head = (f"{st['index'] + 1}/{st['total']}{qstr}"
                    f"{'  (paused)' if st['paused'] else ''}\n"
                    f"{st['rendered']}/{st['total']} rendered")
            if st.get("last_render_ms") is not None:
                head += f"   last {st['last_render_ms']}ms"
            if st.get("avg_render_ms"):
                head += f"   avg {st['avg_render_ms']}ms"
            head += f"\nmodel {model}\n\n{st['sentence']}"
        else:
            head = f"idle{qstr}\nmodel {model}"
        ev = st.get("effective_volume")
        if ev is not None and ev != 100:
            head += f"\nvolume {ev}"
        self.stat.set_text(head)

        self._syncing = True
        if abs(self.speed.get_value() - st["speed"]) > 0.01:
            self.speed.set_value(st["speed"])

        if st.get("volume") is not None and abs(self.volume.get_value() - st["volume"]) > 0.5:
            self.volume.set_value(st["volume"])

        if st.get("trim") is not None and abs(self.trim.get_value() - st["trim"]) > 0.5:
            self.trim.set_value(st["trim"])

        if st.get("font_size") and abs(self.font_size.get_value() - st["font_size"]) > 0.5:
            self.font_size.set_value(st["font_size"])

        if st.get("words_visible") and abs(self.words_vis.get_value() - st["words_visible"]) > 0.5:
            self.words_vis.set_value(st["words_visible"])

        pos = st.get("position")
        if pos in POSITIONS and self.position.get_selected() != POSITIONS.index(pos):
            self.position.set_selected(POSITIONS.index(pos))

        engines = st.get("engines") or ["kokoro"]
        if engines != self.engine_names:
            self.engine_names = engines
            self.engine.set_model(Gtk.StringList.new(engines))
        if st.get("engine") in engines:
            i = engines.index(st["engine"])
            if self.engine.get_selected() != i:
                self.engine.set_selected(i)

        al = st.get("aligner")
        if al in ALIGNERS and self.aligner.get_selected() != ALIGNERS.index(al):
            self.aligner.set_selected(ALIGNERS.index(al))

        if st.get("voices") and not self.catalogue:
            raw = send("catalogue", timeout=10)
            try:
                self.catalogue = json.loads(raw) if raw else []
            except ValueError:
                self.catalogue = []
            if self.catalogue:
                self.n_voices = len(self.catalogue)
                self.store.splice(0, self.store.get_n_items(), [
                    f"{v['sid']}  {v['name']}  ·  {v['lang']}  {v['gender']}"
                    for v in self.catalogue])
                self._refilter()
        try:
            with open(MODE_FILE) as f:
                cur = f.read().strip()
            if cur in MODES and self.mode.get_selected() != MODES.index(cur):
                self.mode.set_selected(MODES.index(cur))
        except OSError:
            pass
        self._syncing = False
        return True


if __name__ == "__main__":
    sys.exit(Panel().run([]))
