Output assets/nahuali-tui.gif

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
Set Theme { "name": "Nahuali", "black": "#171311", "red": "#e16b66", "green": "#72b58b", "yellow": "#d9a45f", "blue": "#7eadd1", "magenta": "#c38db8", "cyan": "#78b9b0", "white": "#e8ddd3", "brightBlack": "#786d66", "brightRed": "#ef8a82", "brightGreen": "#8fc9a2", "brightYellow": "#e7bd7d", "brightBlue": "#9fc4df", "brightMagenta": "#d5a7ca", "brightCyan": "#98cec6", "brightWhite": "#fff8f0", "background": "#171311", "foreground": "#e8ddd3", "selection": "#493a34", "cursor": "#d99a7b" }

Hide
Type "export HOME=$(mktemp -d)"
Enter
Type "export COLORTERM=truecolor"
Enter
Type "unset NO_COLOR"
Enter
Type "target/debug/nahuali --database readme remember 'Hrafn recalls the signed release decision and its evidence.' --mention Hrafn --tag release --scope project:Nahuali"
Enter
Type "target/debug/nahuali --database readme claim Hrafn retrieves 'the signed release decision' --source-last --confidence 0.97 --scope project:Nahuali"
Enter
Type "target/debug/nahuali --database readme link Hrafn retrieves 'the signed release decision' --source-last --confidence 0.96 --scope project:Nahuali"
Enter
Type "target/debug/nahuali --database readme intention 'Verify the next release on its exact commit' --kind task --priority high --source-last --scope project:Nahuali"
Enter
Type "clear"
Enter
Type "target/debug/nahuali --database readme explore"
Enter
Sleep 1s
Show
Sleep 2s
Type "j"
Sleep 5s
Type "q"
