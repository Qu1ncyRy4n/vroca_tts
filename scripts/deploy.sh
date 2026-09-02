#!/usr/bin/env bash
# Rebuild the system against THIS working tree, without touching the flake lock.
#
# The deployment repo pins Vroca by content hash, so every commit here normally
# requires `nix flake update vroca_tts` before it reaches the running daemon.
# That is the correct model for a dependency and the wrong one for a repository
# you are editing every few minutes: the lock is stale constantly, and the churn
# is noise rather than signal.
#
# `--override-input` bypasses the lock for one invocation. Nothing is recorded,
# nothing goes out of sync, and the lock stays meaningful for actual releases.
#
# The override uses `git+file:` rather than `path:` on purpose. `path:` has no
# git awareness and copies the entire directory into the Nix store -- including
# `rust_impl/target`, which is 461 MB of build output and changes on every
# `cargo build`. `git+file:` respects `.gitignore`, so the copy is small and its
# hash only moves when tracked files do. Uncommitted edits are still picked up;
# Nix warns that the tree is dirty and uses it anyway.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOTFILES="${VROCA_DOTFILES:-$HOME/dev/nix-config}"
HOST="${VROCA_HOST:-desktop}"
DRY=0

usage() {
    cat <<EOF
usage: scripts/deploy.sh [--dry] [--host HOST]

Rebuilds the system with the Vroca flake input overridden to this working tree,
then restarts the user service.

  --dry          evaluate and show what would be built; change nothing
  --host HOST    NixOS configuration to build (default: ${HOST})

environment:
  VROCA_DOTFILES  deployment repo (default: \$HOME/dev/nix-config)
  VROCA_HOST      default host name
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dry)  DRY=1; shift ;;
        --host) HOST="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [ ! -e "$DOTFILES/flake.nix" ]; then
    echo "no flake.nix in $DOTFILES -- set VROCA_DOTFILES" >&2
    exit 1
fi

# Warn rather than fail. Nix will use a dirty tree, but it is worth knowing that
# what you are about to deploy is not what is committed.
if ! git -C "$REPO" diff --quiet HEAD 2>/dev/null; then
    echo "note: working tree has uncommitted changes; deploying them anyway"
fi

OVERRIDE="git+file://$REPO"
echo "deploying $REPO"
echo "  -> $DOTFILES#$HOST"
echo "  -> override vroca_tts = $OVERRIDE"
echo

if [ "$DRY" -eq 1 ]; then
    sudo nixos-rebuild dry-build --flake "$DOTFILES#$HOST" \
        --override-input vroca_tts "$OVERRIDE"
    echo
    echo "dry run only; nothing changed"
    exit 0
fi

sudo nixos-rebuild switch --flake "$DOTFILES#$HOST" \
    --override-input vroca_tts "$OVERRIDE"

# A rebuild installs the new store path but does not necessarily replace a
# running process, so the daemon can keep serving the old code with no sign that
# anything is stale. Restart explicitly and show what is actually running.
echo
echo "restarting tts.service"
systemctl --user restart tts.service
sleep 2
systemctl --user show tts.service -p MainPID -p ActiveState -p NRestarts
