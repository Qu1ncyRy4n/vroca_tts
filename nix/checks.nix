{
  runCommand,
  python3,
  lib,
  stdenv,
  vroca,
  lua,
}:
{
  # Building the bundle checks the real runtime/model closure and platform
  # selection. The CLI check needs no audio device or downloaded models.
  package = vroca;
  client =
    runCommand "vroca-client-check"
      {
        nativeBuildInputs = [ python3 ];
      }
      ''
        export PYTHONDONTWRITEBYTECODE=1
        python3 ${../tests/test_packaged_client.py} ${vroca.vroca-cli}/bin/vroca
        touch "$out"
      '';
}
// lib.optionalAttrs stdenv.hostPlatform.isDarwin {
  hammerspoon = runCommand "vroca-hammerspoon-check" { nativeBuildInputs = [ lua ]; } ''
    lua ${../tests/test_hammerspoon.lua} ${vroca}/share/vroca/hammerspoon/vroca.lua
    touch "$out"
  '';
}
// lib.optionalAttrs stdenv.hostPlatform.isLinux {
  # macOS mpv initializes AppKit even for null audio. Its runtime smoke test
  # must run in the login session, outside the Nix build sandbox (see docs).
  daemon =
    runCommand "vroca-daemon-check"
      {
        nativeBuildInputs = [ python3 ];
      }
      ''
        export PYTHONDONTWRITEBYTECODE=1
        python3 ${../tests/test_packaged_daemon.py} ${vroca}/bin
        touch "$out"
      '';
}
