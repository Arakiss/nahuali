Output "assets/nahuali-demo.gif"

Set Shell "bash"
Set Width 880
Set Height 1000
Set FontSize 22
Set FontFamily "Menlo"
Set Padding 20
Set Margin 24
Set MarginFill "#120f0e"
Set WindowBar Rings
Set BorderRadius 10
Set Framerate 30
Set CursorBlink false
Set TypingSpeed 18ms
Set Theme { "name": "Nahuali", "black": "#171311", "red": "#e16b66", "green": "#72b58b", "yellow": "#d9a45f", "blue": "#7eadd1", "magenta": "#c38db8", "cyan": "#78b9b0", "white": "#e8ddd3", "brightBlack": "#786d66", "brightRed": "#ef8a82", "brightGreen": "#8fc9a2", "brightYellow": "#e7bd7d", "brightBlue": "#9fc4df", "brightMagenta": "#d5a7ca", "brightCyan": "#98cec6", "brightWhite": "#fff8f0", "background": "#171311", "foreground": "#e8ddd3", "selection": "#493a34", "cursor": "#d99a7b" }

Hide
Type "export NAHUALI_DEMO_HOME=$(mktemp -d)"
Enter
Type "export NAHUALI_DEMO_DB=launch_demo"
Enter
Type "export COLORTERM=truecolor"
Enter
Type "unset NO_COLOR"
Enter
Type "clear"
Enter
Show

Type "scripts/run-launch-demo.sh seed"
Enter
Sleep 3s
Type "scripts/run-launch-demo.sh recall"
Enter
Sleep 3s

Type "scripts/run-launch-demo.sh explore"
Enter
Sleep 2s
Type "j"
Sleep 3s
Type "q"

Type "clear"
Enter
Type "scripts/run-launch-demo.sh contradict"
Enter
Sleep 3s
Type "scripts/run-launch-demo.sh mcp-recall"
Enter
Sleep 4s

Type "scripts/run-launch-demo.sh explore"
Enter
Sleep 2s
Type "j"
Sleep 2s
Type "j"
Sleep 3s
Type "q"

Type "clear"
Enter
Type "scripts/run-launch-demo.sh inspect"
Enter
Sleep 4s
