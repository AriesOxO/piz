# Demo: piz basic usage - natural language to shell command
Output assets/demo-basic.gif

Set Shell "bash"
Set FontSize 18
Set Width 900
Set Height 500
Set Padding 20
Set TypingSpeed 60ms
Set Theme "Catppuccin Mocha"

# Show version
Type "piz --version"
Enter
Sleep 2s

# Basic translate
Type "piz list all rust files modified today"
Enter
Sleep 8s

# Press 'n' to cancel (don't actually execute)
Type "n"
Sleep 2s

# Another example
Type "piz show disk usage sorted by size"
Enter
Sleep 8s

Type "n"
Sleep 2s

# Pipe mode
Type "piz --pipe compress the src folder"
Enter
Sleep 8s

Sleep 2s
