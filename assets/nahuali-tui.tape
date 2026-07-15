Output assets/nahuali-tui.gif

Set Shell "bash"
Set Width 1400
Set Height 860
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
Type "target/debug/nahuali --database readme remember 'Maya owns the release notes' --mention Maya --tag release"
Enter
Type "target/debug/nahuali --database readme claim Maya owns 'release notes' --source-last --confidence 0.94"
Enter
Type "target/debug/nahuali --database readme intention 'Verify the next release on its exact commit' --kind task --priority high"
Enter
Type "clear"
Enter
Type "target/debug/nahuali --database readme explore"
Enter
Sleep 1s
Show
Sleep 3s
Type "j"
Sleep 2s
Type "j"
Sleep 2s
Type "Tab"
Sleep 2s
Type "q"
