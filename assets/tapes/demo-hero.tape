# Demo: piz hero - deterministic feature showcase
Output assets/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1080
Set Height 640
Set Padding 20
Set TypingSpeed 45ms
Set Theme "Catppuccin Mocha"
Set Framerate 24

Hide
Type "source ./assets/tapes/demo-session-init.sh"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 300ms
Show

Type "piz --version"
Enter
Sleep 1s

Type "printf 'list all rust files in src\n' | piz --pipe"
Enter
Sleep 2s

Type "piz -d 'show rust files in src'"
Enter
Sleep 3s
Type "n"
Enter
Sleep 1s

Type "piz -n 3 'find large files'"
Enter
Sleep 5s
