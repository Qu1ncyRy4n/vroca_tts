{ lib, isDarwin }:
{
  # Scope this environment to Vroca's processes. Do not set a session-wide
  # XDG_RUNTIME_DIR: other applications may use a different lifecycle.
  environment = lib.optionalString isDarwin ''
    export XDG_RUNTIME_DIR="''${XDG_RUNTIME_DIR:-$HOME/Library/Caches/vroca}"
    if [ -e "$XDG_RUNTIME_DIR" ] || [ -L "$XDG_RUNTIME_DIR" ]; then
      if [ -L "$XDG_RUNTIME_DIR" ] || [ ! -d "$XDG_RUNTIME_DIR" ] || [ ! -O "$XDG_RUNTIME_DIR" ]; then
        echo "Vroca runtime directory must be a real directory owned by the current user: $XDG_RUNTIME_DIR" >&2
        exit 1
      fi
    fi
  '';

  # The daemon creates its private runtime directory; the client never starts
  # services or creates runtime state just to check whether a daemon is alive.
  prepare = lib.optionalString isDarwin ''
    (umask 077; mkdir -p -- "$XDG_RUNTIME_DIR")
    chmod 700 -- "$XDG_RUNTIME_DIR"
  '';
}
