{
  runCommand,
  python3,
  lib,
  stdenv,
  vroca,
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
        python3 ${../tests/test_packaged_client.py} ${vroca.tts}/bin/tts
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
