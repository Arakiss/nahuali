#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The README hero requires Ghostty's real Kitty image layer so the canonical
# raster mascot is captured exactly as the TUI renders it. VHS is retained only
# as an explicit portable fallback test; it must never replace the hero asset.
CAPTURE_BACKEND="${NAHUALI_TUI_CAPTURE_BACKEND:-auto}"
if [[ "$CAPTURE_BACKEND" != "vhs" \
  && "$(uname -s)" == "Darwin" \
  && -d "/Applications/Ghostty.app" ]]; then
  exec "$ROOT/scripts/render-readme-tui-ghostty.sh"
fi
if [[ "$CAPTURE_BACKEND" != "vhs" ]]; then
  printf '%s\n' \
    'The canonical README GIF requires macOS and /Applications/Ghostty.app.' \
    'Set NAHUALI_TUI_CAPTURE_BACKEND=vhs only to validate the text fallback; it will not overwrite the hero asset.' >&2
  exit 1
fi

CAPTURE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-tui.XXXXXX")"
TAPE="$CAPTURE_DIR/nahuali-tui.tape"
EMPTY="$CAPTURE_DIR/nahuali-tui-empty.png"
CERTIFIED="$CAPTURE_DIR/nahuali-tui-certified.png"
BLOCKED="$CAPTURE_DIR/nahuali-tui-blocked.png"
CANDIDATE_GIF="$CAPTURE_DIR/nahuali-tui.gif"
CANDIDATE_PNG="$CAPTURE_DIR/nahuali-tui.png"

cleanup() {
  if [[ -d "$CAPTURE_DIR" ]]; then
    find "$CAPTURE_DIR" -type f -delete
    find "$CAPTURE_DIR" -depth -type d -empty -delete
  fi
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
sed "s|__CAPTURE_DIR__|$CAPTURE_DIR|g" assets/nahuali-tui.tape > "$TAPE"
vhs "$TAPE"

# Every published frame comes from this one fresh process. Direct screenshots
# avoid seeking through VHS delta frames, which can expose a partial redraw.
for screenshot in "$EMPTY" "$CERTIFIED" "$BLOCKED"; do
  [[ -s "$screenshot" ]] || {
    printf 'VHS did not produce the expected screenshot: %s\n' "$screenshot" >&2
    exit 1
  }
  dimensions="$(magick identify -format '%wx%h' "$screenshot")"
  if [[ "$dimensions" != "1400x900" ]]; then
    printf 'Unexpected screenshot dimensions for %s: %s\n' "$screenshot" "$dimensions" >&2
    exit 1
  fi
done

# VHS currently captures terminal text canvases but not Kitty/Sixel image
# layers. The README tape therefore exercises Nahuali's real half-block
# fallback. Guard the fixed empty-state hero region so a protocol or layout
# regression cannot silently publish a GIF with a missing mascot.
hero_colors="$(
  magick "$EMPTY" \
    -crop 520x360+690+250 +repage \
    -format '%k' info:
)"
if ((hero_colors < 64)); then
  printf 'Empty-state mascot is missing or visually collapsed: %s colors in hero region\n' \
    "$hero_colors" >&2
  exit 1
fi

magick -delay 300 \
  "$EMPTY" \
  "$CERTIFIED" \
  "$BLOCKED" \
  -loop 0 "$CANDIDATE_GIF"
cp "$CERTIFIED" "$CANDIDATE_PNG"

frames="$(magick identify "$CANDIDATE_GIF" | wc -l | tr -d ' ')"
bad_dimensions="$(magick identify -format '%wx%h\n' "$CANDIDATE_GIF" | awk '$0 != "1400x900" { print }')"
png_dimensions="$(magick identify -format '%wx%h' "$CANDIDATE_PNG")"
if [[ "$frames" != "3" || -n "$bad_dimensions" || "$png_dimensions" != "1400x900" ]]; then
  printf 'Unexpected capture contract: frames=%s bad_gif_dimensions=%s png_dimensions=%s\n' \
    "$frames" "${bad_dimensions:-none}" "$png_dimensions" >&2
  exit 1
fi

printf 'Portable TUI fallback validated (%s, %s scenes); canonical README assets unchanged.\n' \
  "$png_dimensions" "$frames"
printf 'Empty-state mascot contract: %s colors in the real TUI hero region\n' \
  "$hero_colors"
