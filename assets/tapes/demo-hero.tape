# Demo: piz hero - core features showcase
Output assets/demo.gif

Set Shell "pwsh"
Set FontFamily "Cascadia Code"
Set FontSize 16
Set Width 1000
Set Height 600
Set Padding 15
Set TypingSpeed 50ms
Set Theme "Catppuccin Mocha"
Set Framerate 24

# Show version
Type "piz --version"
Enter
Sleep 2s

# Pipe mode - translate natural language, output command only
Type "piz --pipe find all files larger than 100MB"
Enter
Sleep 5s

Sleep 1s

# Another pipe example
Type "piz --pipe compress the src folder"
Enter
Sleep 5s

Sleep 1s

# Explain mode - break down a command
Type "piz -e 'tar -czf archive.tar.gz src/'"
Enter
Sleep 25s

Sleep 3s
