sock="${XDG_RUNTIME_DIR:-/tmp}/tts.sock"
if [ ! -S "$sock" ]; then
  echo "tts daemon not running (start tts-daemon; tts log for service logs)" >&2
  exit 1
fi

if [ "$#" -eq 0 ]; then
  set -- read
fi

# All arguments belong to the single legacy request. Previously only speech
# commands forwarded arguments, so even `tts speed 1.2` sent a bare `speed`.
# socat otherwise exits 0.5 s after stdin EOF. Commands that synchronously
# synthesize or reload a model need time to return their complete reply; allow
# the remote engine's existing 60 s timeout plus a small control-path margin.
printf '%s' "$*" | socat -t 65 - "UNIX-CONNECT:$sock"
printf '\n'
