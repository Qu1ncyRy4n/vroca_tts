# macOS Compatibility Plan

Vroca can run as a reproducibly packaged, per-user daemon on Apple Silicon
macOS. The deployment mechanism is a Nix-built executable supervised by a
declarative `launchd` LaunchAgent. It does not require Homebrew, `brew
services`, or an installation script that modifies Nix configuration.

This is a plan, not an approved deployment change. Vroca owns its packages and
wrappers; `~/dev/nix-config` owns the user LaunchAgent and activation after
separate approval.

## First Slice

The initial macOS target is intentionally limited to:

- `tts-daemon` and `tts` built for `aarch64-darwin`.
- Local Unix-socket commands and local `mpv` playback.
- `read` through one explicit `pbpaste` read of the general clipboard.
- A per-user LaunchAgent that keeps the foreground daemon running while the
  user is logged in.

It explicitly excludes the GTK overlay, RSVP modes, GTK panel, Wayland/X11
selection backends, `systemd`, `journalctl`, and global hotkeys. A later native
macOS UI or hotkey helper must remain a daemon client, not a second runtime-state
owner.

## Why LaunchAgent

Home Manager's Darwin `launchd.agents.<name>` is the appropriate declarative
service owner. It generates and activates a plist in the logged-in user's
`~/Library/LaunchAgents`; the daemon needs that user's clipboard, preferences,
audio session, and socket. A system LaunchDaemon has the wrong session boundary.

`brew services` also drives `launchctl`, but makes Homebrew a second mutable
owner of service registration and environment. It is therefore not part of this
design. Launchd socket activation is also out of scope: Vroca owns and binds its
published socket, whereas launchd socket activation would require the daemon to
accept launchd-provided descriptors.

The eventual deployment declaration belongs in `~/dev/nix-config` and should be
shaped like this after its owner approves the change:

```nix
launchd.agents.vroca = {
  enable = true;
  domain = "gui";
  config = {
    ProgramArguments = [ "${vroca.tts-daemon}/bin/tts-daemon" ];
    KeepAlive = true;
    ProcessType = "Background";
    EnvironmentVariables = {
      XDG_RUNTIME_DIR = runtimeDir;
      XDG_CONFIG_HOME = "/Users/q/.config";
    };
    StandardOutPath = "/Users/q/Library/Logs/vroca.log";
    StandardErrorPath = "/Users/q/Library/Logs/vroca.log";
  };
};
```

`runtimeDir` must be a short, private, user-owned directory created
declaratively before the agent starts. Both the agent and interactive `tts`
client must receive the same `XDG_RUNTIME_DIR`; the daemon's Python temporary
directory fallback and the client's `/tmp` fallback are not guaranteed to agree.

## Required Decisions

1. Define the Darwin runtime-directory location, lifecycle, and permissions.
   It must preserve the daemon-owned `$XDG_RUNTIME_DIR/tts.sock` interface and
   safely recover only Vroca's stale socket.
2. Define `tts quit` under launchd. Linux currently leaves a cleanly exited
   systemd service stopped; `KeepAlive = true` would restart it. Do not change
   that behavior implicitly.
3. Define Darwin `tts log`. The Linux `journalctl` command cannot carry over;
   the first slice may return an explicit unsupported response.
4. Decide global-hotkey scope separately. nix-darwin and Home Manager do not
   declaratively register arbitrary system-wide hotkeys. A native GUI-session
   helper would need a chosen API and verified TCC permissions.

## Implementation Sequence

1. In this repository, make the `tts` wrapper platform-aware: retain `socat` on
   Darwin, but include `systemd` and `journalctl` behavior only on Linux.
2. Give `tts-daemon` and `tts` one shared Darwin runtime-directory rule while
   preserving supplied `XDG_RUNTIME_DIR` values and the existing socket
   protocol.
3. Add Darwin selection tests for the existing `pbpaste` path and Darwin
   evaluation checks for `tts-daemon` and `tts`. Do not add overlay or panel
   outputs to this slice.
4. On an Apple Silicon Mac, build the two packages and run a null-audio daemon
   smoke test before changing deployment configuration.
5. After explicit approval in `~/dev/nix-config`, declare the Home Manager
   LaunchAgent, package installation, runtime directory, and log directory.
6. Validate login start, crash restart, clean shutdown, stale-socket recovery,
   player cleanup, clipboard denial/empty text, and no-UI CLI operation.

## Validation

Run these on the target Apple Silicon Mac before deployment activation:

```sh
nix build --no-link .#packages.aarch64-darwin.tts-daemon
nix build --no-link .#packages.aarch64-darwin.tts
nix develop --command python3 -m unittest python_impl/test_daemon.py
```

After approved LaunchAgent activation, use `TTS_MPV_ARGS=--ao=null` for the
first lifecycle smoke test and inspect the agent with:

```sh
launchctl print "gui/$(id -u)/<declared-label>"
tts status
tts say "macOS socket smoke test"
```

`x86_64-darwin` is not a current target: the pinned nixpkgs no longer supports
it. Do not advertise it as supported until a separate dependency and validation
decision is recorded.

## Sources

- [Home Manager LaunchAgents](https://github.com/nix-community/home-manager/blob/master/modules/launchd/default.nix)
- [nix-darwin launchd module](https://github.com/nix-darwin/nix-darwin/blob/master/modules/launchd/default.nix)
- [Apple launchd guide](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- [Apple launchd plist reference](https://www.manpagez.com/man/5/launchd.plist/)
- [Apple NSPasteboard](https://developer.apple.com/documentation/AppKit/NSPasteboard)
- [gtk4-layer-shell](https://github.com/wmww/gtk4-layer-shell/blob/main/README.md)
