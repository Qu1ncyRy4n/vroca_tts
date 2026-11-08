# Repository-owned package definitions keep model fetching, Python environments,
# and wrappers together. Deployment configurations only select packages and
# wire services/hotkeys; they do not duplicate these derivations.
{
  lib,
  stdenv,
  buildEnv,
  fetchzip,
  fetchurl,
  runCommand,
  writeShellApplication,
  writeText,
  python3,
  mpv,
  socat,
  coreutils,
  systemd,
  wl-clipboard,
  xclip,
  gtk4,
  gtk4-layer-shell,
  glib,
  pango,
  gdk-pixbuf,
  graphene,
  harfbuzz,
  gobject-introspection,
}:

let
  isLinux = stdenv.hostPlatform.isLinux;
  isDarwin = stdenv.hostPlatform.isDarwin;
  src = ../python_impl;
  runtime = import ./runtime.nix { inherit lib isDarwin; };

  # Models are static release tarballs, so they live in the store: no
  # huggingface download at runtime, nothing to warm on first use.
  fetchModel =
    {
      name,
      url,
      hash,
    }:
    fetchzip {
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
    # (the rest are zf_/zm_ Chinese, numbered not named). v1_0 has real names,
    # 28 English voices, and is smaller. The reviewed 2026-10-28 archive adds
    # em_santa at index 53 without moving existing voices; see installation.md.
    kokoro = fetchModel {
      name = "tts-kokoro";
      url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2";
      hash = "sha256-X/mZcz8+AjfFfUDkpVJ0HOp+bT2uIy36I9RXMhl0qA4=";
    };
    libritts = fetchModel {
      name = "tts-libritts";
      url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-libritts_r-medium.tar.bz2";
      hash = "sha256-+/KdxdOywTff0LFay/9+RUjm16zlfzNawX+bX5BAWXk=";
    };
    # Zero-shot voice cloning from a reference clip. RTF 0.55 measured here.
    # Only offered when ~/.config/vroca/voices has wavs in it.
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
  vocoder = fetchurl {
    url = "https://github.com/k2-fsa/sherpa-onnx/releases/download/vocoder-models/vocos_24khz.onnx";
    hash = "sha256-vLO5cOOEFhxNY08LuemZ/xxHGzTJvAsQSaUBQGXtPMA=";
  };

  modelDirs = builtins.toJSON (lib.mapAttrs (_: v: "${v}") models // { vocoder = "${vocoder}"; });

  pyEnv = python3.withPackages (ps: [ ps.sherpa-onnx ]);

  # Isolate synthesis measuring sources from GUI scripts so editing overlay.py
  # or panel.py never forces pitchTables (and 904 voice measurements) to rerun.
  measureSrc = runCommand "tts-measure-src" { } ''
    mkdir -p $out
    cp ${../python_impl/measure.py} $out/measure.py
    cp ${../python_impl/engines.py} $out/engines.py
    cp ${../python_impl/voices.py} $out/voices.py
  '';

  # Supertonic and LibriTTS ship no voice names, so the catalogue describes them
  # by measured pitch. Done here rather than at daemon start: 904 libritts
  # voices is ~1min of synthesis, which nobody wants on every login.
  pitchTables = runCommand "vroca-pitch-tables" { nativeBuildInputs = [ pyEnv ]; } ''
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
  gtkTypelibs = lib.makeSearchPathOutput "out" "lib/girepository-1.0" (
    [
      gtk4
      glib
      pango
      gdk-pixbuf
      graphene
      harfbuzz
      gobject-introspection # ships cairo-1.0.typelib, which gtk4 needs
    ]
    ++ lib.optionals isLinux [ gtk4-layer-shell ]
  );

  runtimeDeps = [
    mpv
    coreutils
  ]
  ++ lib.optionals isLinux [
    wl-clipboard
    xclip
  ];

  daemon = writeShellApplication {
    name = "vroca-daemon";
    runtimeInputs = runtimeDeps ++ [ pyEnv ];
    text = ''
      ${runtime.environment}
      ${runtime.prepare}
      export VROCA_MODEL_DIRS=${lib.escapeShellArg modelDirs}
      export VROCA_PITCH_DIR=${pitchTables}
      # 4, not 8: onnxruntime oversubscribes the 4 physical cores and 8
      # measured *slower* (RTF 0.97 -> 1.30 on kokoro).
      export VROCA_THREADS=4
      # VROCA_API_BASE / VROCA_API_KEY_FILE are deliberately NOT set here.
      # Remote-provider settings come from the user unit's
      # EnvironmentFile=-%E/vroca/env: a key written into a derivation
      # ends up in /nix/store, which is world readable.
      export PYTHONPATH=${src}
      exec python3 ${src}/daemon.py
    '';
  };

  # Subtitle strip along the bottom edge. Linux-only: it needs the wlroots
  # layer-shell protocol.
  overlay = writeShellApplication {
    name = "vroca-overlay";
    runtimeInputs = [
      (python3.withPackages (
        ps: with ps; [
          pygobject3
          pycairo
        ]
      ))
    ];
    text = ''
      export GI_TYPELIB_PATH=${gtkTypelibs}
      # gtk4-layer-shell has to win the symbol race against libwayland-client,
      # which it can't when python dlopens GTK late. Its shipped preload shim
      # is the supported fix; without it init_for_window silently no-ops and
      # the bar renders as an ordinary focus-stealing window.
      export LD_PRELOAD=${gtk4-layer-shell.out}/lib/libgtk4-layer-shell.so
      export PYTHONPATH=${src}
      exec python3 ${src}/overlay.py "''${1:-subtitle}"
    '';
  };

  # Plain toplevel on both platforms, so no layer-shell preload.
  panel = writeShellApplication {
    name = "vroca-panel";
    passthru.typelibs = gtkTypelibs;
    runtimeInputs = [
      (python3.withPackages (
        ps: with ps; [
          pygobject3
          pycairo
        ]
      ))
    ]
    ++ lib.optionals isLinux [ systemd ]; # Linux can start its user unit.
    text = ''
      ${runtime.environment}
      export GI_TYPELIB_PATH=${gtkTypelibs}
      export PYTHONPATH=${src}
      exec python3 ${src}/panel.py
    '';
  };

  macApp = runCommand "vroca-mac-app" { } ''
    app="$out/Applications/Vroca.app/Contents"
    mkdir -p "$app/MacOS"
    ln -s ${panel}/bin/vroca-panel "$app/MacOS/Vroca"
    cat > "$app/Info.plist" <<'PLIST'
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0"><dict>
      <key>CFBundleExecutable</key><string>Vroca</string>
      <key>CFBundleIdentifier</key><string>dev.qix.ttspanel</string>
      <key>CFBundleName</key><string>Vroca</string>
      <key>CFBundlePackageType</key><string>APPL</string>
      <key>CFBundleVersion</key><string>1</string>
      <key>NSHighResolutionCapable</key><true/>
    </dict></plist>
    PLIST
  '';

  hammerspoonLua = writeText "vroca.lua" (
    lib.replaceStrings [ "@TTS@" "@APP@" ] [ "${ttsctl}/bin/vroca" "${macApp}/Applications/Vroca.app" ] (
      builtins.readFile ../integrations/hammerspoon/vroca.lua
    )
  );
  hammerspoon = runCommand "vroca-hammerspoon" { } ''
    mkdir -p "$out/share/vroca/hammerspoon"
    cp ${hammerspoonLua} "$out/share/vroca/hammerspoon/vroca.lua"
  '';

  # One client for every binding. socat rather than a second python start-up:
  # a keypress should not pay interpreter boot time.
  ttsctl = writeShellApplication {
    name = "vroca";
    runtimeInputs = [
      socat
      coreutils
    ]
    ++ lib.optionals isLinux [ systemd ];
    text = ''
      ${runtime.environment}
      if [ "''${1:-}" = "log" ]; then
        ${
          if isLinux then
            ''
              exec journalctl --user -u vroca -u vroca-overlay -f
            ''
          else
            ''
              exec tail -n 100 -F "$HOME/Library/Logs/vroca.log"
            ''
        }
      fi
      ${builtins.readFile ./vroca.sh}
    '';
  };

  # ponytail: `tts*` compatibility aliases for existing bindings; drop once
  # nix-config hotkeys and services reference `vroca*`.
  legacyAliases = runCommand "vroca-legacy-aliases" { } ''
    mkdir -p "$out/bin"
    ln -s ${ttsctl}/bin/vroca "$out/bin/tts"
    ln -s ${daemon}/bin/vroca-daemon "$out/bin/tts-daemon"
    ln -s ${panel}/bin/vroca-panel "$out/bin/tts-panel"
    ${lib.optionalString isLinux ''ln -s ${overlay}/bin/vroca-overlay "$out/bin/tts-overlay"''}
  '';

in
buildEnv {
  name = "vroca";
  paths = [
    daemon
    ttsctl
    panel
    legacyAliases
  ]
  ++ lib.optionals isLinux [ overlay ]
  ++ lib.optionals isDarwin [
    macApp
    hammerspoon
  ];
  pathsToLink = [
    "/bin"
    "/Applications"
    "/share"
  ];
  passthru = {
    vroca-daemon = daemon;
    vroca-cli = ttsctl;
    vroca-panel = panel;
    vroca-pitch-tables = pitchTables;
  }
  // lib.optionalAttrs isLinux {
    vroca-overlay = overlay;
  };
  meta = {
    description = "Speech synthesis and assistive reader";
    mainProgram = "vroca";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
  };
}
