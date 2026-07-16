#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW="/tmp/nahuali-tui-raw.gif"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-tui-gif.XXXXXX")"

cleanup() {
  rm -f "$RAW"
  rm -r "$WORK"
}
trap cleanup EXIT

for command in vhs ffmpeg magick; do
  command -v "$command" >/dev/null 2>&1 || {
    printf '%s is required to render the README TUI GIF\n' "$command" >&2
    exit 1
  }
done

if [[ ! -x "$ROOT/target/debug/nahuali" ]]; then
  printf 'Build the CLI first with: cargo build -p nahuali-cli\n' >&2
  exit 1
fi

cd "$ROOT"
vhs assets/nahuali-tui.tape

# VHS records terminal cell updates as optimized GIF frames. Extract the two
# stable end states as opaque RGB images so README renderers never expose a
# partial terminal redraw. The middle scene is the checked-in exact TUI capture.
ffmpeg -loglevel error -y -ss 2.5 -i "$RAW" \
  -frames:v 1 -vf format=rgb24 "$WORK/mascot.png"
ffmpeg -loglevel error -y -ss 11.5 -i "$RAW" \
  -frames:v 1 -vf format=rgb24 "$WORK/blocked.png"

magick -delay 300 \
  "$WORK/mascot.png" \
  "$ROOT/assets/nahuali-tui.png" \
  "$WORK/blocked.png" \
  -loop 0 "$ROOT/assets/nahuali-tui.gif"

dimensions="$(magick identify -format '%wx%h' "$ROOT/assets/nahuali-tui.gif[0]")"
frames="$(magick identify "$ROOT/assets/nahuali-tui.gif" | wc -l | tr -d ' ')"
if [[ "$dimensions" != "1400x900" || "$frames" != "3" ]]; then
  printf 'Unexpected GIF contract: dimensions=%s frames=%s\n' "$dimensions" "$frames" >&2
  exit 1
fi

printf 'README TUI GIF rendered: %s (%s, %s scenes)\n' \
  "$ROOT/assets/nahuali-tui.gif" "$dimensions" "$frames"
