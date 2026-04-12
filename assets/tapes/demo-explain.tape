# Demo: piz explain mode
Output assets/demo-explain.gif

Set Shell "bash"
Set FontSize 16
Set Width 980
Set Height 620
Set Padding 20
Set TypingSpeed 45ms
Set Theme "Catppuccin Mocha"

Hide
Type "source ./assets/tapes/demo-session-init.sh"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 300ms
Show

Type "piz -e 'find src -type f -name '\''*.rs'\'''"
Enter
Sleep 4s
