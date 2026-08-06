#!/usr/bin/env python3
"""Subtitle & RSVP overlay for the Vroca TTS reader.

A GTK4 layer-shell surface: on Wayland an ordinary toplevel can't reliably
stay above others or refuse focus, and stealing focus mid-read would defeat
the point. Layer-shell gives an always-on-top, click-through, never-focused strip.

Renders whatever `tts-state.json` says. The daemon owns all state; this only draws.
MODES contains "subtitle", "rsvp", and "scroll_rsvp".
"""
import json
import os
import sys
import tempfile

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gtk4LayerShell", "1.0")
from gi.repository import GLib, Gtk, Gtk4LayerShell as LayerShell  # noqa: E402

RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
STATE = os.path.join(RUNTIME, "tts-state.json")
MODE_FILE = os.path.join(RUNTIME, "tts-mode")
POLL_MS = 50

CSS = b"""
window { background: transparent; }
#bar {
  background: rgba(18,18,22,0.82);
  border-radius: 14px;
  padding: 14px 22px;
  margin: 0 0 48px 0;
}
#text  { color: #f2f2f7; font-size: 19pt; font-weight: 500; }
#meta  { color: #9aa0a6; font-size: 11pt; }
#meta.paused { color: #f5a623; }
progress, trough { min-height: 3px; border-radius: 2px; }
trough { background: rgba(255,255,255,0.14); }
progress { background: #7aa2f7; }
#bar progressbar { min-height: 3px; }
"""


def read_state():
    try:
        with open(STATE) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def read_mode(default):
    try:
        with open(MODE_FILE) as f:
            return f.read().strip() or default
    except OSError:
        return default


class Overlay(Gtk.Application):
    def __init__(self, mode):
        super().__init__(application_id="dev.qix.ttsoverlay")
        self.mode = mode
        self.last = None

    def do_activate(self):
        win = Gtk.ApplicationWindow(application=self)
        LayerShell.init_for_window(win)
        LayerShell.set_layer(win, LayerShell.Layer.OVERLAY)
        LayerShell.set_keyboard_mode(win, LayerShell.KeyboardMode.NONE)
        self._anchor(win, "subtitle", "bottom")

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.set_name("bar")
        box.set_halign(Gtk.Align.CENTER)
        self.text = Gtk.Label(wrap=True, justify=Gtk.Justification.CENTER)
        self.text.set_name("text")
        self.text.set_max_width_chars(80)
        self.meta = Gtk.Label()
        self.meta.set_name("meta")
        self.bar = Gtk.ProgressBar(show_text=False)
        self.bar.set_hexpand(True)
        box.append(self.text)
        box.append(self.bar)
        box.append(self.meta)
        win.set_child(box)

        css = Gtk.CssProvider()
        css.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_display(
            win.get_display(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        self.win = win
        self.hold()
        GLib.timeout_add(POLL_MS, self._tick)

    def _tick(self):
        mode = read_mode(self.mode)
        st = read_state()
        if (st, mode) == self.last:
            return True
        self.last = (st, mode)
        if mode == "off" or not st or not st.get("sentence"):
            self.win.set_visible(False)
            return True
        self._anchor(self.win, mode, st.get("position", "bottom") if st else "bottom")
        MODES.get(mode, render_subtitle)(self, st)
        self.win.set_visible(True)
        return True

    def _anchor(self, win, mode, pos="bottom"):
        key = (mode, pos)
        if getattr(self, "_anchored", None) == key:
            return
        self._anchored = key
        top = (pos == "top")
        bottom = (pos == "bottom" and mode != "rsvp" and mode != "scroll_rsvp")
        for edge, on in ((LayerShell.Edge.BOTTOM, bottom),
                         (LayerShell.Edge.TOP, top),
                         (LayerShell.Edge.LEFT, True),
                         (LayerShell.Edge.RIGHT, True)):
            LayerShell.set_anchor(win, edge, on)


def render_subtitle(app, st):
    words = st["sentence"].split()
    w = st.get("word", -1)
    if words and 0 <= w < len(words):
        parts = [
            (f'<span foreground="#f7d774" weight="bold">{GLib.markup_escape_text(t)}</span>'
             if i == w else
             f'<span foreground="#8b909a">{GLib.markup_escape_text(t)}</span>'
             if i < w else GLib.markup_escape_text(t))
            for i, t in enumerate(words)
        ]
        app.text.set_markup(" ".join(parts))
    else:
        app.text.set_text(st["sentence"])

    total = st["total"] or 1
    app.bar.set_fraction((st["index"] + 1) / total)

    bits = []
    if st["paused"]:
        bits.append("paused")
    bits.append(f"{st['index'] + 1}/{total}")
    qlen = st.get("queue_len", 0)
    if qlen > 0:
        bits.append(f"queue: {qlen}")
    bits.append(f"{st.get('rendered', 0)}/{total} rendered")
    if st.get("rendering") is not None:
        bits.append(f"rendering {st['rendering'] + 1}…")
    ms = st.get("last_render_ms")
    avg = st.get("avg_render_ms")
    if ms is not None:
        bits.append(f"{ms}ms" + (f" (avg {avg}ms)" if avg else ""))
    bits.append(f"{st['speed']}x")
    app.meta.set_text("   ".join(bits))

    cls = [c for c in app.meta.get_css_classes() if c != "paused"]
    app.meta.set_css_classes(cls + (["paused"] if st["paused"] else []))


def orp(word):
    n = len(word)
    if n <= 1:
        return 0
    if n <= 5:
        return 1
    return min(int(n * 0.3), n - 1)


def render_rsvp(app, st):
    words = st["sentence"].split()
    w = st.get("word", -1)
    if not words:
        app.text.set_text("")
        return
    w = min(max(w, 0), len(words) - 1)
    word = words[w]
    k = orp(word)
    pre, piv, post = word[:k], word[k], word[k + 1:]
    esc = GLib.markup_escape_text
    pad = max(len(pre), len(post))
    font_sz = st.get("font_size", 24)
    app.text.set_markup(
        f'<span font_family="monospace" size="{font_sz * 1024}">'
        f'{"&#160;" * (pad - len(pre))}{esc(pre)}'
        f'<span foreground="#f7d774">{esc(piv)}</span>'
        f'{esc(post)}{"&#160;" * (pad - len(post))}</span>')
    total = st["total"] or 1
    qlen = st.get("queue_len", 0)
    qstr = f"   queue: {qlen}" if qlen else ""
    app.bar.set_fraction((st["index"] + 1) / total)
    app.meta.set_text(f"{st['index'] + 1}/{total}   {st['speed']}x   engine: {st.get('engine','kokoro')}{qstr}"
                      + ("   paused" if st["paused"] else ""))


def render_scroll_rsvp(app, st):
    words = st["sentence"].split()
    w = st.get("word", -1)
    if not words:
        app.text.set_text("")
        return
    w = min(max(w, 0), len(words) - 1)
    vis = st.get("words_visible", 3)
    start = max(0, w - vis)
    end = min(len(words), w + vis + 1)
    
    word = words[w]
    k = orp(word)
    pre, piv, post = word[:k], word[k], word[k + 1:]
    
    left_context = " ".join(words[start:w])
    left_str = (left_context + " " if left_context else "") + pre
    
    right_context = " ".join(words[w + 1:end])
    right_str = post + (" " + right_context if right_context else "")
    
    max_side = max(len(left_str), len(right_str))
    esc = GLib.markup_escape_text
    
    left_padded = ("&#160;" * (max_side - len(left_str))) + esc(left_str)
    right_padded = esc(right_str) + ("&#160;" * (max_side - len(right_str)))
    
    font_sz = st.get("font_size", 24)
    app.text.set_markup(
        f'<span font_family="monospace" size="{font_sz * 1024}">'
        f'<span foreground="#565f89">{left_padded}</span>'
        f'<span foreground="#f7d774" weight="bold">{esc(piv)}</span>'
        f'<span foreground="#565f89">{right_padded}</span>'
        f'</span>'
    )
    total = st["total"] or 1
    qlen = st.get("queue_len", 0)
    qstr = f"   queue: {qlen}" if qlen else ""
    app.bar.set_fraction((st["index"] + 1) / total)
    app.meta.set_text(f"{st['index'] + 1}/{total}   {st['speed']}x   engine: {st.get('engine','kokoro')}{qstr}"
                      + ("   paused" if st["paused"] else ""))


MODES = {"subtitle": render_subtitle, "rsvp": render_rsvp, "scroll_rsvp": render_scroll_rsvp}


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "subtitle"
    sys.exit(Overlay(mode).run([]))
