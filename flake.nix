{
  description = "Vroca — Text-to-Speech & Assistive Reading Subsystem";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      # Everything derived from this repository's own code lives here, so the
      # repo builds like any other package and the deployment repository only
      # wires services and hotkeys around `self.packages.${system}`.
      #
      # The wrappers previously lived in the deployment repo's tts-home.nix;
      # they moved here verbatim (comments included) so that `nix build` on
      # this flake is self-contained.
      mkPackages = system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          lib = pkgs.lib;
          isLinux = pkgs.stdenv.hostPlatform.isLinux;

          src = ./python_impl;

          # Models are static release tarballs, so they live in the store: no
          # huggingface download at runtime, nothing to warm on first use.
          fetchModel = { name, url, hash }: pkgs.fetchzip {
            inherit url hash name;
            stripRoot = true;
          };

          models = {
            supertonic = fetchModel {
              name = "tts-supertonic";
              url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2";
              hash = "sha256-UNN4akgwwdqLdC/T8avL/JGDhLyZ1Y57kqgzfxeayPA=";
            };
            # v1_0, not the newer v1_1: v1_1 has 103 voices but only three are English
            # (the rest are zf_/zm_ Chinese, numbered not named). v1_0 has 53 with real
            # names, 28 of them English, and is smaller.
            kokoro = fetchModel {
              name = "tts-kokoro";
              url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2";
              hash = "sha256-hbMZyZhRD5AD+WMsXlgoPgBj0gQ03G/5lLQqoUZFB58=";
            };
            libritts = fetchModel {
              name = "tts-libritts";
              url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-libritts_r-medium.tar.bz2";
              hash = "sha256-+/KdxdOywTff0LFay/9+RUjm16zlfzNawX+bX5BAWXk=";
            };
            # Zero-shot voice cloning from a reference clip. RTF 0.55 measured here.
            # Only offered when ~/.config/tts/voices has wavs in it.
            zipvoice = fetchModel {
              name = "tts-zipvoice";
              url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-zipvoice-distill-int8-zh-en-emilia.tar.bz2";
              hash = "sha256-iV8AC5cWLAH+ZXAYAfakupMaJfNXnWuIEJCFXuXpQck=";
            };
            # Forced alignment for word highlighting. ~78ms/sentence against 500-2600ms
            # of synthesis, so it is the default aligner. Also transcribes cloning
            # references, so a dropped-in wav needs no hand-written transcript.
            asr = fetchModel {
              name = "tts-asr";
              url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-zipformer-small-en-2023-06-26.tar.bz2";
              hash = "sha256-kxgNbcFiXJvyR/a7fF/RqNvAs7qiHiuKwj9Zp4Fap5g=";
            };
          };

          # Bare .onnx, not a tarball, so fetchurl -- zipvoice needs it to turn its
          # generated features into audio.
          vocoder = pkgs.fetchurl {
            url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/vocoder-models/vocos_24khz.onnx";
            hash = "sha256-vLO5cOOEFhxNY08LuemZ/xxHGzTJvAsQSaUBQGXtPMA=";
          };

          modelDirs = builtins.toJSON (
            lib.mapAttrs (_: v: "${v}") models // { vocoder = "${vocoder}"; }
          );

          pyEnv = pkgs.python3.withPackages (ps: [ ps.sherpa-onnx ]);

          # Isolate synthesis measuring sources from GUI scripts so editing overlay.py
          # or panel.py never forces pitchTables (and 904 voice measurements) to rerun.
          measureSrc = pkgs.runCommand "tts-measure-src" { } ''
            mkdir -p $out
            cp ${src}/measure.py ${src}/engines.py ${src}/voices.py $out/
          '';

          # Supertonic and LibriTTS ship no voice names, so the catalogue describes them
          # by measured pitch. Done here rather than at daemon start: 904 libritts
          # voices is ~1min of synthesis, which nobody wants on every login.
          pitchTables = pkgs.runCommand "tts-pitch-tables" { nativeBuildInputs = [ pyEnv ]; } ''
            mkdir -p $out
            for e in supertonic libritts; do
              case $e in
                supertonic) d=${models.supertonic} ;;
                libritts)   d=${models.libritts} ;;
              esac
              PYTHONPATH=${measureSrc} python3 ${measureSrc}/measure.py "$e" "$d" "$out/$e.json"
            done
          '';

          # pygobject resolves namespaces at import time from GI_TYPELIB_PATH, so every
          # library the GUIs touch has to be listed -- gtk4 pulls pango/gdk-pixbuf/
          # graphene in as separate typelibs. makeSearchPathOutput, not makeSearchPath:
          # pango's default output isn't `out` and the typelibs live there.
          gtkTypelibs = lib.makeSearchPathOutput "out" "lib/girepository-1.0" (with pkgs; [
            gtk4
            gtk4-layer-shell
            glib
            pango
            gdk-pixbuf
            graphene
            harfbuzz
            gobject-introspection # ships cairo-1.0.typelib, which gtk4 needs
          ]);

          runtimeDeps = [ pkgs.mpv ]
            ++ lib.optionals isLinux [ pkgs.wl-clipboard pkgs.xclip ];

          daemon = pkgs.writeShellApplication {
            name = "tts-daemon";
            runtimeInputs = runtimeDeps ++ [ pyEnv ];
            text = ''
              export TTS_MODEL_DIRS=${lib.escapeShellArg modelDirs}
              export TTS_PITCH_DIR=${pitchTables}
              # 4, not 8: onnxruntime oversubscribes the 4 physical cores and 8
              # measured *slower* (RTF 0.97 -> 1.30 on kokoro).
              export TTS_THREADS=4
              # TTS_API_BASE / TTS_API_KEY_FILE are deliberately NOT set here.
              # Remote-provider settings come from the user unit's
              # EnvironmentFile=-%E/tts/env: a key written into a derivation
              # ends up in /nix/store, which is world readable.
              export PYTHONPATH=${src}
              exec python3 ${src}/daemon.py
            '';
          };

          # Subtitle strip along the bottom edge. Linux-only: it needs the wlroots
          # layer-shell protocol.
          overlay = pkgs.writeShellApplication {
            name = "tts-overlay";
            runtimeInputs = [ (pkgs.python3.withPackages (ps: with ps; [ pygobject3 pycairo ])) ];
            text = ''
              export GI_TYPELIB_PATH=${gtkTypelibs}
              # gtk4-layer-shell has to win the symbol race against libwayland-client,
              # which it can't when python dlopens GTK late. Its shipped preload shim
              # is the supported fix; without it init_for_window silently no-ops and
              # the bar renders as an ordinary focus-stealing window.
              export LD_PRELOAD=${pkgs.gtk4-layer-shell.out}/lib/libgtk4-layer-shell.so
              export PYTHONPATH=${src}
              exec python3 ${src}/overlay.py "''${1:-subtitle}"
            '';
          };

          # Config panel (Super+Shift+Z). Plain toplevel, so no layer-shell preload.
          panel = pkgs.writeShellApplication {
            name = "tts-panel";
            runtimeInputs = [
              (pkgs.python3.withPackages (ps: with ps; [ pygobject3 pycairo ]))
              pkgs.systemd # panel starts the unit itself if the socket is missing
            ];
            text = ''
              export GI_TYPELIB_PATH=${gtkTypelibs}
              export PYTHONPATH=${src}
              exec python3 ${src}/panel.py
            '';
          };

          # One client for every binding. socat rather than a second python start-up:
          # a keypress should not pay interpreter boot time.
          ttsctl = pkgs.writeShellApplication {
            name = "tts";
            runtimeInputs = [ pkgs.socat pkgs.systemd ];
            text = ''
              sock="''${XDG_RUNTIME_DIR:-/tmp}/tts.sock"
              if [ "''${1:-}" = "log" ]; then
                exec journalctl --user -u tts -u tts-overlay -f
              fi
              [ -S "$sock" ] || { echo "tts daemon not running (tts log)" >&2; exit 1; }
              if [ "''${1:-}" = "say" ] || [ "''${1:-}" = "speak" ] || [ "''${1:-}" = "queue" ]; then
                cmd="$1"
                shift
                printf '%s %s' "$cmd" "$*" | socat - "UNIX-CONNECT:$sock"
                echo
              else
                printf '%s' "''${1:-read}" | socat - "UNIX-CONNECT:$sock"
                echo
              fi
            '';
          };
        in
        {
          tts-daemon = daemon;
          tts = ttsctl;
          tts-pitch-tables = pitchTables;
          default = daemon;
        } // lib.optionalAttrs isLinux {
          tts-overlay = overlay;
          tts-panel = panel;
        };
    in
    {
      packages = forAllSystems mkPackages;

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          pyEnv = pkgs.python3.withPackages (ps: with ps; [
            sherpa-onnx pygobject3 pycairo numpy
          ]);
        in
        {
          default = pkgs.mkShell {
            packages = [
              pyEnv
              pkgs.uv
              pkgs.mpv
              pkgs.socat

              # Rust toolchain for the staged replacement in rust_impl/.
              # Taken from nixpkgs rather than a rust-overlay input: the
              # migration needs a stable toolchain, not a pinned nightly, and
              # avoiding the extra input keeps flake.lock to one entry.
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
            ]
            # systemd and the GTK layer-shell stack are Linux-only. Listing
            # them unconditionally made the advertised darwin shells fail to
            # evaluate at all, systemd first and gtk4-layer-shell behind it.
            ++ nixpkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.systemd
              pkgs.gtk4
              pkgs.gtk4-layer-shell
            ];
          };
        });
    };
}
