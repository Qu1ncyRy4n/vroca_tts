# macOS Compatibility Plan

Updated 2026-10-29. Approved direction: a working Apple Silicon release with
a UI and Hammerspoon configuration for global hotkeys. The developer approves
interface choices. The first UI is the existing GTK controls/status panel,
packaged as `vroca-panel` and `Vroca.app`; subtitles/RSVP are outside this port.

The target is a reproducibly packaged, per-user daemon on Apple Silicon
macOS. The planned deployment is a Nix-built executable supervised by a
declarative `launchd` LaunchAgent. It does not require Homebrew, `brew
services`, or an installation script that modifies Nix configuration.

This is a plan, not an approved deployment change. Vroca owns its packages,
wrappers, and reusable Home Manager module; `~/dev/nix-config` owns
opting into that module and activating the user LaunchAgent after separate
approval.

**Packaging validation:** the Apple Silicon package build, packaged-client
tests, and native null-audio synthesis/control/cleanup test pass. The module
evaluates against Home Manager's option types. Actual audio, clipboard access,
and activated LaunchAgent lifecycle have not been verified. See
[`installation.md`](installation.md) for commands and the test boundary.

## First Milestone — Daemon And Client

The initial macOS target is intentionally limited to:

- `vroca-daemon` and `vroca` built for `aarch64-darwin`.
- Local Unix-socket commands and local `mpv` playback.
- `read` through one explicit `pbpaste` read of the general clipboard.
- A per-user LaunchAgent that keeps the foreground daemon running while the
  user is logged in. Service configuration is opt-in.

It explicitly excludes the GTK overlay, RSVP modes, GTK panel, Wayland/X11
selection backends, `systemd`, and `journalctl`. This bounds the first milestone,
not the full Mac release. The UI and hotkeys follow below; each remains a daemon
client, not a second runtime-state owner.

## Mac UI And Hammerspoon Milestone

The developer approved porting the existing GTK panel on 2026-10-29. It uses
the existing Python daemon and command protocol, with voices, speed, volume,
playback, and status. Mac windows hide aligner and Linux overlay controls.
Model operations run off the GTK thread; a missing daemon produces an explicit
unavailable message rather than a service-start attempt. The combined package
includes `Applications/Vroca.app`; the individual `vroca-panel` output provides
the command. See [installation.md](installation.md#mac-panel-and-hotkeys) for
launch commands, controls, and a short human verification path.

Global hotkeys are supplied as Hammerspoon Lua configuration calling `vroca`.
The packaged module contains absolute Nix paths and binds no keys when loaded.
The approved defaults match Linux/Speechify with Option replacing Super.
They are enabled only by an explicit `setup()` call. Changing the user's active
Hammerspoon configuration requires approval. Clipboard access, required macOS
permissions, and focus behavior need verification on the target Mac.

Inspection found empty Rust GUI binaries and a draft JSON client incompatible
with the production Python command protocol. The approved bounded GTK port
therefore reuses the working daemon; full Rust spec review remains pending.
Preserve installed command names and daemon behavior through migration.

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

The package and module are implemented here. Opting into the service in
`~/dev/nix-config` still requires approval. Import the module and enable it as
described in [`installation.md`](installation.md):

```nix
imports = [ inputs.vroca.homeModules.default ];
services.vroca.enable = true;
```

The module sets `RunAtLoad = true` and `KeepAlive.SuccessfulExit = false`.
The packaged daemon and client share `~/Library/Caches/vroca` when
`XDG_RUNTIME_DIR` is unset. The daemon creates it with mode `0700`; wrappers
reject symlinks, non-directories, and foreign ownership. No global runtime
environment setting or installer script is needed.

## Packaging Decisions — Approved 2026-10-28

1. Conventional `callPackage` package, combined default bundle, optional
   overlay, and opt-in `services.vroca` Home Manager module.
2. Runtime directory: preserve supplied `XDG_RUNTIME_DIR`; otherwise use the
   private Mac cache directory above. The socket remains `vroca.sock`.
3. `vroca quit` leaves the agent stopped; unsuccessful exits are restarted.
4. `vroca log` follows `~/Library/Logs/vroca.log` on Mac.

## Remaining Decisions

1. Select the active configuration's module path and approve activation.
   The package supplies absolute paths to `vroca` and `Vroca.app` internally.
2. Verify its required permissions rather than adding a
   second hotkey helper.

## Implementation Sequence

| Step | Status |
| --- | --- |
| Platform-aware daemon/client package; no Mac `systemd` dependency | Implemented and built |
| Shared private runtime directory, preserving supplied `XDG_RUNTIME_DIR` | Implemented; fallback and unsafe-path rejection tested |
| Mac package/client checks and native null-audio daemon test | Passed |
| Opt-in Home Manager LaunchAgent and log-directory setup | Implemented and evaluated; not activated |
| `pbpaste` tests and actual clipboard/audio use | Pending |
| Sequential duplicate startup preserves existing socket | Packaged regression test passes after cleanup ownership fix |
| Concurrent startup, stale-socket recovery, login start, crash restart, and clean quit under launchd | Pending |
| Rust scaffold inspection for toolkit choice | Done; full scope/spec audit remains pending |
| Mac GTK controls/status panel and app bundle | Implemented; native fake-daemon GTK smoke test passes |
| Packaged Hammerspoon module | Implemented; Option bindings approved; mocked API tests pass; activation pending |

Next, validate audio and clipboard on the target Mac. After separate approval,
enable the module in `~/dev/nix-config` and verify the activated lifecycle.
Manually verify the panel, app focus, approved bindings, and real playback.
Resume the Rust spec review without expanding this port into overlay parity.

## Validation

Run these on the target Apple Silicon Mac before deployment activation:

```sh
nix flake check --no-write-lock-file
nix build --no-write-lock-file
python3 tests/test_packaged_daemon.py ./result/bin
nix develop --command python3 -m unittest python_impl/test_daemon.py
nix develop --command python3 tests/test_panel.py
```

The packaged-daemon check must run in the login session: AppKit initialization
in `mpv` aborts inside the build sandbox even with null audio. The existing
`test_daemon.py` suite covers Linux selection fallback; Darwin-specific
clipboard tests are still pending. Neither suite proves real audio output.

After approved LaunchAgent activation, use `VROCA_MPV_ARGS=--ao=null` for the
first lifecycle smoke test and inspect the agent with:

```sh
launchctl print "gui/$(id -u)/org.nix-community.home.vroca"
vroca status
vroca say "macOS socket smoke test"
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
- [GTK macOS backend and native window controls](https://docs.gtk.org/gtk4/osx.html)
- [Hammerspoon asynchronous tasks](https://www.hammerspoon.org/docs/hs.task.html)
