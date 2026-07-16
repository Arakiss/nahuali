#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW="/tmp/nahuali-tui-raw.gif"
EMPTY="/tmp/nahuali-tui-empty.png"
BLOCKED="/tmp/nahuali-tui-blocked.png"

cleanup() {
  rm -f "$RAW" "$EMPTY" "$BLOCKED"
}
trap cleanup EXIT

for command in vhs magick; do
  command -v "$command" >/dev/null 2>&1 || {
    printf '%s is required to render the README TUI GIF\n' "$command" >&2
    exit 1
  }
done

cd "$ROOT"
cargo build --locked -p nahuali-cli
rm -f "$RAW" "$EMPTY" "$BLOCKED"
vhs assets/nahuali-tui.tape

# The tape captures its stable terminal states directly. This avoids seeking
# through VHS's optimized delta frames, which can expose a partial redraw.
for screenshot in "$EMPTY" "$BLOCKED"; do
  [[ -s "$screenshot" ]] || {
    printf 'VHS did not produce the expected screenshot: %s\n' "$screenshot" >&2
    exit 1
  }
done

magick -delay 300 \
  "$EMPTY" \
  "$ROOT/assets/nahuali-tui.png" \
  "$BLOCKED" \
  -loop 0 "$ROOT/assets/nahuali-tui.gif"

dimensions="$(magick identify -format '%wx%h' "$ROOT/assets/nahuali-tui.gif[0]")"
frames="$(magick identify "$ROOT/assets/nahuali-tui.gif" | wc -l | tr -d ' ')"
if [[ "$dimensions" != "1400x900" || "$frames" != "3" ]]; then
  printf 'Unexpected GIF contract: dimensions=%s frames=%s\n' "$dimensions" "$frames" >&2
  exit 1
fi

printf 'README TUI GIF rendered: %s (%s, %s scenes)\n' \
  "$ROOT/assets/nahuali-tui.gif" "$dimensions" "$frames"
