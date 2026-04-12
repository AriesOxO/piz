# Demo: piz multi-candidate mode
Output assets/demo-multi.gif

Set Shell "bash"
Set FontSize 16
Set Width 980
Set Height 560
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

Type "piz -n 3 'find large files'"
Enter
Sleep 6s
