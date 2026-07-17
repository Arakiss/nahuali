Output "__CAPTURE_DIR__/nahuali-tui-raw.gif"

# Portable half-block fallback contract only. The README hero is captured from
# real Ghostty windows by scripts/render-readme-tui-ghostty.sh.

Set Shell "bash"
Set Width 1400
Set Height 900
Set FontSize 18
Set FontFamily "Menlo"
Set Padding 20
Set Margin 24
Set MarginFill "#120f0e"
Set WindowBar Rings
Set BorderRadius 10
Set Framerate 30
Set CursorBlink false
Set TypingSpeed 1ms
Set Theme { "name": "Nahuali", "black": "#171311", "red": "#e16b66", "green": "#72b58b", "yellow": "#d9a45f", "blue": "#7eadd1", "magenta": "#c38db8", "cyan": "#78b9b0", "white": "#e8ddd3", "brightBlack": "#786d66", "brightRed": "#ef8a82", "brightGreen": "#8fc9a2", "brightYellow": "#e7bd7d", "brightBlue": "#9fc4df", "brightMagenta": "#d5a7ca", "brightCyan": "#98cec6", "brightWhite": "#fff8f0", "background": "#171311", "foreground": "#e8ddd3", "selection": "#493a34", "cursor": "#d99a7b" }

Hide
Type "export NAHUALI_DEMO_HOME=__CAPTURE_DIR__/home"
Enter
Type "mkdir -p $NAHUALI_DEMO_HOME"
Enter
Type "export NAHUALI_DEMO_DB=readme_tui"
Enter
Type "export NAHUALI_HOME=$NAHUALI_DEMO_HOME"
Enter
Type "export COLORTERM=truecolor"
Enter
Type "export NAHUALI_TUI_FORCE_HALF_BLOCKS=1"
Enter
Type "unset NO_COLOR"
Enter
Type "set -e"
Enter
Type "printf '\033[2J\033[H'"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB explore"
Enter
Wait+Screen /PROOF/
Show
Sleep 1s
Screenshot "__CAPTURE_DIR__/nahuali-tui-empty.png"
Sleep 1s

Hide
Type "q"
Sleep 500ms
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB remember 'The release review approved launch after QA passed.' --mention Release --tag release --scope project:Nahuali >/dev/null"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB claim Release status 'approved after QA' --source-last --confidence 0.97 --scope project:Nahuali >/dev/null"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB link Release status 'approved after QA' --source-last --confidence 0.96 --scope project:Nahuali >/dev/null"
Enter
Type "printf '%064d' 0 > __CAPTURE_DIR__/signing.seed && chmod 600 __CAPTURE_DIR__/signing.seed"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB checkpoint-policy-init --origin readme-demo --key-id readme --key-file __CAPTURE_DIR__/signing.seed --output __CAPTURE_DIR__/policy.json >/dev/null"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB checkpoint-sign --policy __CAPTURE_DIR__/policy.json --key-id readme --key-file __CAPTURE_DIR__/signing.seed --output __CAPTURE_DIR__/checkpoint.json >/dev/null"
Enter
Type "printf '\033[2J\033[H'"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB explore --checkpoint __CAPTURE_DIR__/checkpoint.json --policy __CAPTURE_DIR__/policy.json"
Enter
Wait+Screen /CURRENT/
Show
Sleep 1s
Type "/"
Type "Release"
Enter
Wait+Screen /Release/
Type "j"
Wait+Screen /approved after QA/
Sleep 1s
Screenshot "__CAPTURE_DIR__/nahuali-tui-certified.png"
Sleep 1s

Hide
Type "q"
Sleep 500ms
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB claim Release status 'approved without QA' --confidence 0.99 --scope project:Nahuali >/dev/null"
Enter
Type "printf '\033[2J\033[H'"
Enter
Type "target/debug/nahuali --database $NAHUALI_DEMO_DB explore --checkpoint __CAPTURE_DIR__/checkpoint.json --policy __CAPTURE_DIR__/policy.json --checkpoint-mode historical"
Enter
Wait+Screen /EARLIER STATE/
Show
Sleep 1s
Type "/"
Type "without QA"
Enter
Wait+Screen /approved without QA/
Sleep 1s
Screenshot "__CAPTURE_DIR__/nahuali-tui-blocked.png"
Sleep 1s
Hide
Type "q"
