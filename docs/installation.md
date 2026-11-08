# Installing Vroca With Nix

Vroca is packaged through `pkgs.callPackage`, exported as `pkgs.vroca` by an
optional overlay, and available as this flake's default package. There is no
installer script or automatic edit to global Nix configuration. Use an existing
Nix installation with flake support.

The expected local checkout is `~/dev/exocortex/vroca_tts`. Nix copies the
selected source and dependencies to the store; installed commands do not run
Python files directly from the mutable checkout.

## Package Contents

| Platform | Default package commands |
| --- | --- |
| `x86_64-linux`, `aarch64-linux` | `vroca`, `vroca-daemon`, `vroca-panel`, `vroca-overlay` |
| `aarch64-darwin` | `vroca`, `vroca-daemon`, `vroca-panel`; `Applications/Vroca.app` |

The existing local model set is included: Kokoro, Supertonic, LibriTTS,
ZipVoice/vocoder, and ASR. Model downloads are hash-pinned Nix build inputs;
the daemon does not fetch models at runtime. The first build downloads a
substantial dependency and model closure and measures pitch for unnamed voices.
Pitch measurement sources remain separate from daemon and GUI code so normal
application edits do not repeat that work.

The Mac bundle also includes `share/vroca/hammerspoon/vroca.lua`. Loading the
module binds no keys until explicitly configured. Intel macOS is not advertised.

### Kokoro Model Pin Review

The original Kokoro hash failed during the Mac package build. Inspection of
the archive fetched from the existing sherpa-onnx release URL found an updated
v1.0 voice table: its README records `em_santa` added at speaker ID 53, with
existing IDs 0–52 unchanged. The developer approved pinning that archive on
2026-10-28. `voices.py` now includes the appended voice; existing indices and
names are retained.

The new unpacked hash is
`sha256-X/mZcz8+AjfFfUDkpVJ0HOp+bT2uIy36I9RXMhl0qA4=`.
The [upstream model guide](https://k2-fsa.github.io/sherpa/onnx/vroca/pretrained_models/kokoro.html)
documents the original 53-voice mapping; the
[downloaded release archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2)
contains the update note. Future changes to the release asset will fail the
fixed-output hash check and require another review.

## Build Or Run Without Installing

```sh
cd ~/dev/exocortex/vroca_tts
nix build
./result/bin/vroca-daemon
```

In another terminal:

```sh
cd ~/dev/exocortex/vroca_tts
./result/bin/vroca status
./result/bin/vroca say "Vroca is ready."
./result/bin/vroca speed 1.2
./result/bin/vroca quit
```

Alternatively, `nix run .#vroca-daemon` runs the foreground daemon and
`nix run . -- status` runs the bundled client. Package installation alone does
not start a daemon. Keep service-managed and foreground instances separate.

## Declarative Home Manager Installation

Add the local Git checkout as an input in the flake that owns your Home Manager
config:

```nix
inputs.vroca = {
  url = "git+file:///Users/q/dev/exocortex/vroca_tts";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Use the corresponding absolute home path on Linux. Nix does not expand `~`
inside an input URL. The Git input selects tracked source files rather than
copying unrelated local files into the store. Local inputs are lockfile
snapshots; update the `vroca` input through your configuration repository's
normal review process when selecting a new checkout revision.

For commands without an automatically started service, add the package to an
existing Home Manager module receiving `inputs`:

```nix
{ inputs, pkgs, ... }: {
  home.packages = [ inputs.vroca.packages.${pkgs.stdenv.hostPlatform.system}.default ];
}
```

For login startup, import the reusable module instead:

```nix
{ inputs, ... }: {
  imports = [ inputs.vroca.homeModules.default ];
  services.vroca.enable = true;
}
```

The module builds Vroca with the consuming configuration's `pkgs` and adds it
to `home.packages`. `services.vroca.package` can override the package, for
example with the flake's pinned package. Activation uses the existing Home
Manager workflow; no activation is performed by this repository's build.
Changes to `~/dev/nix-config` require separate approval.

The service installs only when enabled and starts only the daemon. Linux panel
and overlay commands are included but are not started automatically.
When adopting the module, replace an existing `vroca` service declaration rather
than keeping two definitions of the same unit.

## Using The Overlay Or callPackage

The flake exports `overlays.default`:

```nix
{ inputs, pkgs, ... }: {
  nixpkgs.overlays = [ inputs.vroca.overlays.default ];
  home.packages = [ pkgs.vroca ];
}
```

With a source path already available to the evaluating configuration, the
equivalent conventional package expression is:

```nix
pkgs.callPackage (vrocaSource + "/nix/package.nix") { }
```

`vrocaSource` may be the local Git flake input. No overlay is required to use
the Home Manager module. Existing flake outputs `vroca`, `vroca-daemon`,
`vroca-pitch-tables`, and `vroca-panel` remain available; `vroca-overlay` is Linux-only.
The default output now contains the command bundle rather than just the daemon.

## Service And Runtime Behavior

Approved packaging choices, 2026-10-28:

| Behavior | Linux | Apple Silicon macOS |
| --- | --- | --- |
| Service | systemd user unit `vroca`, attached to the graphical session | Home Manager LaunchAgent `org.nix-community.home.vroca`, in the GUI session |
| Runtime | Existing `$XDG_RUNTIME_DIR`; legacy `/tmp` fallback when unset | `$XDG_RUNTIME_DIR` if supplied, otherwise `~/Library/Caches/vroca` |
| Logs | `vroca log` follows the service journal | `vroca log` follows `~/Library/Logs/vroca.log` |
| Restart | On failure, after 3 seconds | On unsuccessful exit, throttled to 3 seconds |
| `vroca quit` | Clean shutdown; service stays stopped | Clean shutdown; agent stays stopped |

The Mac daemon creates its runtime directory with mode `0700`; wrappers reject
a symlink, non-directory, or directory owned by another user. The client never
creates a runtime directory or auto-starts the daemon. Runtime selection applies
only to Vroca processes, not the global session environment. Explicit runtime
paths must be short enough for Unix sockets (macOS limits are especially tight).

Use the service manager to restart after `quit`:

```sh
# Linux
systemctl --user start vroca

# Mac, when the Home Manager agent has been activated
launchctl kickstart "gui/$(id -u)/org.nix-community.home.vroca"
```

Use `vroca stop` to stop speech while leaving the daemon available.

## Mac Panel And Hotkeys

Build the default bundle, start the daemon in one terminal, then open the panel:

```sh
nix build
./result/bin/vroca-daemon
# In another terminal:
./result/bin/vroca-panel
# Or launch the Mac application:
open ./result/Applications/Vroca.app
```

The panel shows daemon/model/playback status, engine and searchable voices,
preview/selection, speed, master volume, voice trim, and playback/queue/model
controls. On Mac, aligner and overlay/RSVP settings are hidden. Socket waits run
off the UI thread. Closing the panel leaves the daemon running; **Quit daemon**
stops both. When the daemon is unavailable, the panel says how to start it and
keeps polling; it does not register or start a Mac service.

**Read clipboard** uses the existing `read` command: when idle it reads the
clipboard, and when speech is active it stops speech. **Pause / Resume** toggles
pause; **Previous** and **Next** move between sentences. These controls preserve
the existing daemon API, allowing a later Rust daemon to replace Python.

For a manual check, copy a short paragraph, press Read clipboard, change speed
and volume, pause/resume, and select/preview a voice. Changes made through `vroca`
should appear in the panel. Verify actual audio and clipboard behavior on the
target Mac; automated GTK tests use a fake daemon.

The Hammerspoon module uses Nix-substituted absolute paths, asynchronous
`hs.task` calls for `vroca`, and `hs.application.launchOrFocus` for the app. It
does not depend on Hammerspoon inheriting the terminal's PATH. A reviewable
example enables the developer-approved Linux/Speechify layout with Option
replacing Super:

```lua
-- Replace this with the installed package/profile's share path.
vroca = dofile(os.getenv("HOME") .. "/.nix-profile/share/vroca/hammerspoon/vroca.lua")
vroca.setup()
```

| Shortcut | Action |
| --- | --- |
| Option+Z | Read clipboard / stop active speech |
| Option+Shift+Z | Open or focus panel |
| Option+X | Pause / resume |
| Option+C / Option+V | Previous / next sentence |
| Option+Shift+C / Option+Shift+V | Slower / faster |

For a build-only trial, use the absolute checkout's
`result/share/vroca/hammerspoon/vroca.lua` instead. Keep that result symlink
until installed. `vroca.stop()` removes this module's bindings; `setup` first
removes its previous bindings. Inspect existing shortcuts before enabling a
table. `setup({})` enables no keys; a supplied binding table replaces the
defaults. Hammerspoon needs macOS Accessibility permission for global hotkeys;
follow its setup prompt. Grant no extra permissions solely for this module.
See the official [task API](https://www.hammerspoon.org/docs/hs.task.html),
[application API](https://www.hammerspoon.org/docs/hs.application.html), and
[Hammerspoon setup](https://www.hammerspoon.org/go/).

Installing the package does not edit active Hammerspoon configuration. Editing
that configuration and activating the Home Manager service remain separate,
explicitly approved machine changes. Home Manager can expose the bundle through
its normal macOS application linking; no `/Applications` installer is supplied.

## Client And Preferences

The client forwards every command's arguments and waits up to 65 seconds after
submission for the daemon's reply, allowing synchronous synthesis and the
HTTP engine's existing timeout. Long commands no longer lose their reply to
`socat`'s default half-second wait after stdin closes.

Preferences live in `$XDG_CONFIG_HOME/vroca` (normally `~/.config/vroca`).
Provider keys must remain outside Nix expressions and the store. The Linux
service still reads the optional user-managed `vroca/env` file; the Mac module
does not source shell environment files. Local engines work without provider
credentials.

## Migrating From `tts`

The public command was renamed from `tts` to `vroca` on 2026-11-07. What
changed and what still works:

| Item | Old | New | Compatibility |
| --- | --- | --- | --- |
| Commands | `tts`, `tts-daemon`, `tts-panel`, `tts-overlay` | `vroca`, `vroca-daemon`, `vroca-panel`, `vroca-overlay` | `tts*` symlinks ship in the bundle |
| Flake outputs | `.#tts`, `.#tts-daemon`, … | `.#vroca-cli`, `.#vroca-daemon`, … | `.#vroca` is still the bundle |
| systemd unit | `tts` | `vroca` | `vroca log` follows the new unit |
| Runtime files | `tts.sock`, `tts-state.json`, `tts-mode` | `vroca.sock`, `vroca-state.json`, `vroca-mode` | None; old and new daemons do not see each other |
| Config dir | `~/.config/tts` | `~/.config/vroca` | Daemon reads the old dir when the new one is absent; `mv ~/.config/tts ~/.config/vroca` finishes it |
| Env vars | `TTS_*` | `VROCA_*` | Any `TTS_*` value is copied to `VROCA_*` when unset |

Deployment in `~/dev/nix-config` still refers to `tts` bindings, the Linux
`tts-home.nix` unit, and the `TTS_API_KEY` secret. Those work through the
aliases and fallbacks above; update them, then remove the aliases.

## Checks

```sh
cd ~/dev/exocortex/vroca_tts
nix flake check
```

The flake checks build the complete package and exercise the installed client
against an isolated Unix socket, including argument forwarding, large replies,
missing-daemon behavior, and Mac runtime selection. They do not activate a
service or read your clipboard. On Linux, a daemon check uses isolated runtime
and preferences directories plus `mpv --ao=null` to verify real synthesis,
voice metadata, controls, and graceful cleanup without audible playback.

On Mac, `mpv` initializes AppKit even with null audio; that initialization
aborts in the build sandbox. Run the same daemon test in the logged-in session:

```sh
nix build
python3 tests/test_packaged_daemon.py ./result/bin
nix develop --command python3 tests/test_panel.py
```

This uses temporary state, never reads the clipboard, and plays no audible
sound. It verifies the actual package rather than a fake player. No sandbox
relaxation or global Nix configuration change is required.

The GTK smoke test requires a graphical login session and briefly opens a test
window. It checks control commands, voice filtering, read-only status refresh,
responsiveness during delayed preview, and the Mac unavailable state using an
isolated fake daemon. Flake checks also test the packaged Hammerspoon module
with mocked APIs; they do not register global keys. Human focus behavior,
Accessibility permission, and app launch through Finder/Hammerspoon still need
manual verification.

Validation on 2026-10-29: the Mac app launches through `open` and remains
running without startup errors. The native fake-daemon GTK test and packaged
Hammerspoon tests pass. A duplicate-start cleanup bug discovered during this
check was fixed: rejected instances now leave a live owner's socket and state
alone. The packaged daemon regression test passes duplicate refusal, preserved
socket inode/live status, synthesis, controls, and cleanup. This does not verify
simultaneous startup or service-manager lifecycle.

The installed Nix version may warn that `homeModules` is an unknown flake
output. It is the intentional Home Manager module export; the separate module
evaluation below checks its option types and platform behavior.

`tests/home-manager.nix` evaluates the module against real Home Manager option
types for Linux and Mac, including the disabled case. Supply the source paths
from the consuming configuration's inputs:

```sh
nix eval --json --file tests/home-manager.nix --apply \
  'test: test { nixpkgs = /nix/store/<nixpkgs-source>; homeManager = /nix/store/<home-manager-source>; }'
```

See [`macos-compatibility.md`](macos-compatibility.md) for service activation,
actual audio, clipboard, and later UI validation. A package build alone does
not prove those human-operated flows.

Validation on 2026-10-28: the Apple Silicon bundle build and seven packaged
client tests pass. The native null-audio daemon test passes startup, model
synthesis, voice catalogue/selection, speed, and graceful cleanup. Home Manager
evaluations pass for Linux, Mac, and disabled cases; all three advertised
platforms' package interfaces and the overlay evaluate. Linux binaries were not
built or run on this Mac. No user service, profile, global Nix configuration,
or deployment repository was changed; `flake.lock` is unchanged.
