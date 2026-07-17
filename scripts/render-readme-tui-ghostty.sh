#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CAPTURE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nahuali-ghostty-tui.XXXXXX")"
GHOSTTY_APP="/Applications/Ghostty.app"
BIN="$ROOT/target/debug/nahuali"
DATABASE="readme_tui"
PIDS=()

cleanup() {
  for pid in "${PIDS[@]:-}"; do
    kill "$pid" 2>/dev/null || true
  done
  if [[ -d "$CAPTURE_DIR" ]]; then
    find "$CAPTURE_DIR" -type f -delete
    find "$CAPTURE_DIR" -depth -type d -empty -delete
  fi
}
trap cleanup EXIT

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "the Ghostty README capture requires macOS" >&2
  exit 1
fi
if [[ ! -d "$GHOSTTY_APP" ]]; then
  echo "Ghostty is required at $GHOSTTY_APP" >&2
  exit 1
fi
for command_name in cargo magick open pgrep screencapture shasum swiftc; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the Ghostty README capture" >&2
    exit 1
  fi
done

cd "$ROOT"
cargo build --locked -p nahuali-cli

prepare_store() {
  local state="$1"
  local state_dir="$CAPTURE_DIR/states/$state"
  local home="$state_dir/home"
  mkdir -p "$home"

  export NAHUALI_HOME="$home"
  "$BIN" --database "$DATABASE" remember \
    "The release review approved launch after QA passed." \
    --mention Release --tag release --scope project:Nahuali >/dev/null
  "$BIN" --database "$DATABASE" claim Release status \
    "approved after QA" \
    --source-last --confidence 0.97 --scope project:Nahuali >/dev/null
  "$BIN" --database "$DATABASE" link Release status \
    "approved after QA" \
    --source-last --confidence 0.96 --scope project:Nahuali >/dev/null

  printf '%064d' 0 > "$state_dir/signing.seed"
  chmod 600 "$state_dir/signing.seed"
  "$BIN" --database "$DATABASE" checkpoint-policy-init \
    --origin readme-demo \
    --key-id readme \
    --key-file "$state_dir/signing.seed" \
    --output "$state_dir/policy.json" >/dev/null
  "$BIN" --database "$DATABASE" checkpoint-sign \
    --policy "$state_dir/policy.json" \
    --key-id readme \
    --key-file "$state_dir/signing.seed" \
    --output "$state_dir/checkpoint.json" >/dev/null

  if [[ "$state" == "blocked" ]]; then
    "$BIN" --database "$DATABASE" claim Release status \
      "approved without QA" \
      --confidence 0.99 --scope project:Nahuali >/dev/null
  fi
  unset NAHUALI_HOME
}

mkdir -p "$CAPTURE_DIR/states/empty/home"
prepare_store certified
prepare_store blocked

cat > "$CAPTURE_DIR/launch-state.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

root="${1:?repository root is required}"
capture_dir="${2:?capture directory is required}"
state="${3:?capture state is required}"
database="${4:?database is required}"

export COLORTERM=truecolor
unset NAHUALI_TUI_FORCE_HALF_BLOCKS
unset NO_COLOR
export NAHUALI_HOME="$capture_dir/states/$state/home"

args=(--database "$database" explore)
if [[ "$state" == "certified" ]]; then
  args+=(
    --checkpoint "$capture_dir/states/$state/checkpoint.json"
    --policy "$capture_dir/states/$state/policy.json"
  )
elif [[ "$state" == "blocked" ]]; then
  args+=(
    --checkpoint "$capture_dir/states/$state/checkpoint.json"
    --policy "$capture_dir/states/$state/policy.json"
    --checkpoint-mode historical
  )
fi

cd "$root"
exec "$root/target/debug/nahuali" "${args[@]}"
EOF
chmod +x "$CAPTURE_DIR/launch-state.sh"

cat > "$CAPTURE_DIR/list-windows.swift" <<'EOF'
import CoreGraphics
import Foundation

let raw = CGWindowListCopyWindowInfo(
    [.optionOnScreenOnly, .excludeDesktopElements],
    kCGNullWindowID
) as? [[String: Any]] ?? []

for window in raw {
    let owner = window[kCGWindowOwnerName as String] as? String ?? ""
    guard owner == "Ghostty" else { continue }
    let number = window[kCGWindowNumber as String] as? Int ?? 0
    let pid = window[kCGWindowOwnerPID as String] as? Int ?? 0
    let layer = window[kCGWindowLayer as String] as? Int ?? -1
    print("\(number)\tpid=\(pid)\tlayer=\(layer)")
}
EOF
swiftc "$CAPTURE_DIR/list-windows.swift" -o "$CAPTURE_DIR/list-windows"

capture_state() {
  local state="$1"
  local output="$2"
  local title="Nahuali-README-${state}-$$"
  local pid=""
  local window_id=""
  local probe="$CAPTURE_DIR/${state}.probe.png"
  local previous_signature=""

  open -na "$GHOSTTY_APP" --args \
    --config-default-files=false \
    --window-save-state=never \
    --confirm-close-surface=false \
    --shell-integration=none \
    --window-width=120 \
    --window-height=43 \
    --window-position-x=120 \
    --window-position-y=40 \
    --window-padding-x=20 \
    --window-padding-y=14 \
    --font-family=Menlo \
    --font-size=18 \
    --background='#171311' \
    --foreground='#e8ddd3' \
    --cursor-color='#d99a7b' \
    --selection-background='#493a34' \
    --title="$title" \
    --macos-titlebar-style=hidden \
    --macos-window-shadow=false \
    -e "$CAPTURE_DIR/launch-state.sh" "$ROOT" "$CAPTURE_DIR" "$state" "$DATABASE"

  for _ in {1..100}; do
    pid="$(
      pgrep -f "/Applications/Ghostty.app/Contents/MacOS/ghostty.*--title=$title" \
        | head -1 || true
    )"
    [[ -n "$pid" ]] && break
    sleep 0.1
  done
  if [[ -z "$pid" ]]; then
    echo "Ghostty process not found for $state" >&2
    return 1
  fi
  PIDS+=("$pid")

  for _ in {1..100}; do
    window_id="$(
      "$CAPTURE_DIR/list-windows" \
        | awk -F '\t' -v pid="$pid" \
          '$0 ~ ("pid=" pid "\t") && $0 ~ /layer=0/ { print $1; exit }'
    )"
    [[ -n "$window_id" ]] && break
    sleep 0.1
  done
  if [[ -z "$window_id" ]]; then
    echo "Ghostty window not found for $state (pid $pid)" >&2
    return 1
  fi

  sleep 6
  for _ in {1..60}; do
    if ! screencapture -x -o -l"$window_id" "$probe" 2>/dev/null; then
      sleep 0.25
      continue
    fi
    local dimensions
    local colors
    local signature
    dimensions="$(magick identify -format '%wx%h' "$probe")"
    colors="$(magick "$probe" -format '%k' info:)"
    signature="$(magick "$probe" -format '%#' info:)"
    if [[ "$dimensions" == "1360x931" \
      && "$colors" -ge 800 \
      && "$signature" == "$previous_signature" ]]; then
      mv "$probe" "$output"
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      printf 'Captured %s: %s, %s colors\n' "$state" "$dimensions" "$colors"
      return 0
    fi
    previous_signature="$signature"
    sleep 0.25
  done

  echo "Ghostty never reached a stable complete frame for $state" >&2
  return 1
}

EMPTY="$CAPTURE_DIR/empty.png"
CERTIFIED="$CAPTURE_DIR/certified.png"
BLOCKED="$CAPTURE_DIR/blocked.png"
CANDIDATE_GIF="$CAPTURE_DIR/nahuali-tui.gif"
CANDIDATE_PNG="$CAPTURE_DIR/nahuali-tui.png"

capture_state empty "$EMPTY"
capture_state certified "$CERTIFIED"
capture_state blocked "$BLOCKED"

empty_hero_colors="$(
  magick "$EMPTY" -crop 620x500+650+120 +repage -format '%k' info:
)"
certified_corner_colors="$(
  magick "$CERTIFIED" -crop 160x150+1200+780 +repage -format '%k' info:
)"
blocked_corner_colors="$(
  magick "$BLOCKED" -crop 160x150+1200+780 +repage -format '%k' info:
)"
if ((empty_hero_colors < 200 \
  || certified_corner_colors < 100 \
  || blocked_corner_colors < 100)); then
  printf 'Kitty mascot capture contract failed: empty=%s certified=%s blocked=%s colors\n' \
    "$empty_hero_colors" "$certified_corner_colors" "$blocked_corner_colors" >&2
  exit 1
fi

magick -delay 300 "$EMPTY" "$CERTIFIED" "$BLOCKED" -loop 0 "$CANDIDATE_GIF"
cp "$EMPTY" "$CANDIDATE_PNG"

frames="$(magick identify "$CANDIDATE_GIF" | wc -l | tr -d ' ')"
bad_dimensions="$(
  magick identify -format '%wx%h\n' "$CANDIDATE_GIF" \
    | awk '$0 != "1360x931" { print }'
)"
if [[ "$frames" != "3" || -n "$bad_dimensions" ]]; then
  printf 'Unexpected Ghostty GIF contract: frames=%s bad_dimensions=%s\n' \
    "$frames" "${bad_dimensions:-none}" >&2
  exit 1
fi

mv "$CANDIDATE_PNG" "$ROOT/assets/nahuali-tui.png"
mv "$CANDIDATE_GIF" "$ROOT/assets/nahuali-tui.gif"

printf 'README TUI GIF rendered from real Ghostty windows: %s (1360x931, 3 scenes)\n' \
  "$ROOT/assets/nahuali-tui.gif"
printf 'Kitty mascot contract: empty=%s certified=%s blocked=%s colors\n' \
  "$empty_hero_colors" "$certified_corner_colors" "$blocked_corner_colors"
printf 'SHA-256: %s\n' "$(shasum -a 256 "$ROOT/assets/nahuali-tui.gif" | awk '{print $1}')"
