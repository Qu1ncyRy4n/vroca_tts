# Agent input security — the watched spool proposal

**Status: proposal rejected as written. No spool exists. Whether any spool is
still useful is open (`rust-spec.md` §7.13, D1).**

This records why the watched-spool proposal, which first appeared in the
deploying NixOS configuration's roadmap, should not be built in that form, and
what any later spool would have to meet. Vroca owns the daemon, so the
decision lives here.

## The proposal

A file or named pipe, `/tmp/tts-speak`, watched by an `inotify` thread in
`tts-daemon`. Any agent or script could run `echo "Task complete" >
/tmp/tts-speak` to make the daemon speak. The daemon would create it with mode
`0o666` "so background agents running under different user contexts can
notify speech", and symlink it to `~/.config/tts/speak`.

## Why not as written

- **Any local user could make the desktop speak.** A world-writable path
  accepts input from every account and every service on the machine, not just
  the owning user. Speech is an output channel the user trusts, and anything
  that can write the file can put words in it.
- **`/tmp` is shared, so another user can create the path first.** If
  `/tmp/tts-speak` already exists when the daemon starts, it may belong to
  someone else. It could be a regular file they keep writing to, a FIFO they
  hold open, or a symlink they control. The daemon then reads input from, or
  follows a link chosen by, another account.
- **"Different user contexts" is the wrong goal.** Every legitimate caller
  today, including LLM agents, shell scripts, and the panel, runs as the same
  user as the daemon. Giving other users access widens the trust boundary for
  no current caller.
- **It duplicates an interface that already works.** The `tts` CLI and the
  legacy socket at `$XDG_RUNTIME_DIR/tts.sock` already accept `say` and
  `queue` from any local script. The runtime directory is mode `0700`, so the
  owning user is the trust boundary (`vroca.md`; mismatch D11 records that the
  socket's `0o666` mode bits are misleading but grant nothing). An agent that
  can run `echo` can run `tts queue "Task complete"`.

## Requirements for any future spool

If a spool is still wanted after the structured protocol (`rust-spec.md`)
lands, it must:

1. Live in `$XDG_RUNTIME_DIR` (for example `$XDG_RUNTIME_DIR/tts-speak`),
   never in `/tmp` or another shared directory.
2. Be created by the daemon with mode `0600` and owned by the daemon's user.
   At startup, refuse to use a pre-existing path that is a symlink or is not
   owned by that user.
3. Apply the same size limit, input validation, and queue semantics as the
   socket. Prefer `queue` over `say`, so a write cannot interrupt active
   reading.
4. Be a thin adapter onto the public local API, not a second command parser.
5. Not be symlinked into `~/.config`. Configuration directories hold
   configuration, and a link there invites tools to treat it as a regular
   file.

Cross-user notification, for example a system service announcing something
to the logged-in user, is a separate feature. It needs its own design with an
explicit allowlist of senders, not a world-writable file.

## TODO

- Resolve `rust-spec.md` §7.13: is a spool still useful once the structured
  protocol exists? If not, close D1 permanently. If so, implement against the
  requirements above.
