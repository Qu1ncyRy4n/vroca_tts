# macOS Compatibility Plan

Updated 2026-10-28. Approved direction: a working Apple Silicon release with
a UI and Hammerspoon configuration for global hotkeys. The developer approves
interface choices; UI scope and toolkit selection remain unresolved.

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

- `tts-daemon` and `tts` built for `aarch64-darwin`.
- Local Unix-socket commands and local `mpv` playback.
- `read` through one explicit `pbpaste` read of the general clipboard.
- A per-user LaunchAgent that keeps the foreground daemon running while the
  user is logged in. Service configuration is opt-in.

It explicitly excludes the GTK overlay, RSVP modes, GTK panel, Wayland/X11
selection backends, `systemd`, and `journalctl`. This bounds the first milestone,
not the full Mac release. The UI and hotkeys follow below; each remains a daemon
client, not a second runtime-state owner.

## Mac UI And Hammerspoon Milestone

The Mac release must include a UI. Decide whether the initial UI contains only
voice/playback/speed/volume controls and daemon status, also subtitles, or full
RSVP overlays. No toolkit or native application packaging has been chosen.
Present proposed UI behavior and a short manual verification path before
implementation.

Global hotkeys are supplied as Hammerspoon Lua configuration calling `tts`.
Agree bindings and the installed executable path before implementation. Ship
configuration and setup instructions here; changing the user's active
Hammerspoon configuration requires approval. Clipboard access, required macOS
permissions, and focus behavior need verification on the target Mac.

Review the Rust scaffold before substantial new UI work, so the UI language and
toolkit decision can account for the imminent Rust replacement. Preserve the
installed command names and daemon interface through migration.

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
   private Mac cache directory above. The socket remains `tts.sock`.
3. `tts quit` leaves the agent stopped; unsuccessful exits are restarted.
4. `tts log` follows `~/Library/Logs/vroca.log` on Mac.

## Remaining Decisions

1. Define UI scope, user-visible behavior, toolkit, and application launch
   interface. UI is required; controls versus subtitles/RSVP is still open.
2. Define Hammerspoon bindings and how its configuration finds the installed
   `tts` executable. Verify its required permissions rather than adding a
   second hotkey helper.

## Implementation Sequence

| Step | Status |
| --- | --- |
| Platform-aware daemon/client package; no Mac `systemd` dependency | Implemented and built |
| Shared private runtime directory, preserving supplied `XDG_RUNTIME_DIR` | Implemented; fallback and unsafe-path rejection tested |
| Mac package/client checks and native null-audio daemon test | Passed |
| Opt-in Home Manager LaunchAgent and log-directory setup | Implemented and evaluated; not activated |
| `pbpaste` tests and actual clipboard/audio use | Pending |
| Duplicate startup, stale-socket recovery, login start, crash restart, and clean quit under launchd | Pending |
| Rust scope/spec review before substantial UI code | Pending |
| Mac UI and Hammerspoon configuration | Pending interface decisions and implementation |

Next, validate audio and clipboard on the target Mac. After separate approval,
enable the module in `~/dev/nix-config` and verify the activated lifecycle.
Review Rust before substantial UI work, then implement and manually verify the
approved controls, bindings, and any overlay behavior.

## Validation

Run these on the target Apple Silicon Mac before deployment activation:

```sh
nix flake check --no-write-lock-file
nix build --no-write-lock-file
python3 tests/test_packaged_daemon.py ./result/bin
nix develop --command python3 -m unittest python_impl/test_daemon.py
```

The packaged-daemon check must run in the login session: AppKit initialization
in `mpv` aborts inside the build sandbox even with null audio. The existing
`test_daemon.py` suite covers Linux selection fallback; Darwin-specific
clipboard tests are still pending. Neither suite proves real audio output.

After approved LaunchAgent activation, use `TTS_MPV_ARGS=--ao=null` for the
first lifecycle smoke test and inspect the agent with:

```sh
launchctl print "gui/$(id -u)/org.nix-community.home.vroca"
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
