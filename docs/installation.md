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
| `x86_64-linux`, `aarch64-linux` | `tts`, `tts-daemon`, `tts-panel`, `tts-overlay` |
| `aarch64-darwin` | `tts`, `tts-daemon` |

The existing local model set is included: Kokoro, Supertonic, LibriTTS,
ZipVoice/vocoder, and ASR. Model downloads are hash-pinned Nix build inputs;
the daemon does not fetch models at runtime. The first build downloads a
substantial dependency and model closure and measures pitch for unnamed voices.
Pitch measurement sources remain separate from daemon and GUI code so normal
application edits do not repeat that work.

Mac UI and Hammerspoon configuration remain follow-up work. The package does
not install a Mac UI or register hotkeys. Intel macOS is not advertised.

### Kokoro Model Pin Review

The original Kokoro hash failed during the Mac package build. Inspection of
the archive fetched from the existing sherpa-onnx release URL found an updated
v1.0 voice table: its README records `em_santa` added at speaker ID 53, with
existing IDs 0–52 unchanged. The developer approved pinning that archive on
2026-10-28. `voices.py` now includes the appended voice; existing indices and
names are retained.

The new unpacked hash is
`sha256-X/mZcz8+AjfFfUDkpVJ0HOp+bT2uIy36I9RXMhl0qA4=`.
The [upstream model guide](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html)
documents the original 53-voice mapping; the
[downloaded release archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2)
contains the update note. Future changes to the release asset will fail the
fixed-output hash check and require another review.

## Build Or Run Without Installing

```sh
cd ~/dev/exocortex/vroca_tts
nix build
./result/bin/tts-daemon
```

In another terminal:

```sh
cd ~/dev/exocortex/vroca_tts
./result/bin/tts status
./result/bin/tts say "Vroca is ready."
./result/bin/tts speed 1.2
./result/bin/tts quit
```

Alternatively, `nix run .#tts-daemon` runs the foreground daemon and
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

The opt-in Home Manager service module is a separate implementation step.
Package installation starts no service.

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

`vrocaSource` may be the local Git flake input. Existing flake outputs `tts`, `tts-daemon`,
`tts-pitch-tables`, and Linux-only `tts-panel`/`tts-overlay` remain available.
The default output now contains the command bundle rather than just the daemon.

## Runtime And Client Behavior

On Mac, the daemon and client honor a supplied `XDG_RUNTIME_DIR`; otherwise
they share `~/Library/Caches/vroca`. The daemon creates that directory with
mode `0700`; wrappers reject symlinks, non-directories, and foreign ownership.
This environment applies only to Vroca, not to the global session.

The CLI forwards all command arguments and waits up to 65 seconds for a reply
after submitting a request. This preserves responses during synchronous
synthesis instead of using socat's half-second default. `tts quit` shuts down
the daemon; use `tts stop` to stop only speech.

For an independently configured service, `tts log` follows the journal on
Linux and `~/Library/Logs/vroca.log` on Mac. Package installation does not set
up logging or start a supervisor. Provider credentials remain outside Nix
expressions and the store.

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
```

This uses temporary state, never reads the clipboard, and plays no audible
sound. It verifies the actual package rather than a fake player. No sandbox
relaxation or global Nix configuration change is required.

Validation on 2026-10-28: the Apple Silicon bundle build, seven client
tests, and native null-audio synthesis/control/cleanup test pass. Package and
overlay interfaces evaluate for all three advertised platforms; Linux binaries
were not built or run here. No profile, service, global Nix configuration, or
deployment repository was changed. `flake.lock` is unchanged.
