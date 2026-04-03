# Output Configuration
Output demo.mp4
Set FontSize 18
Set Width 1280
Set Height 720
Set Padding 40
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set PlaybackSpeed 1.5 # Speeds up the typing slightly for that "fast" feel

# Initial Pause
Sleep 1s

# --- SCENE 1: The Speed Hook (Markdown Only) ---
Type "ai just show me the terminal command to kill port 8080 do not run it"
Sleep 300ms
Enter

# Watch the markdown-styled bash script fly across the screen
Sleep 3.5s

# Snap clear
Ctrl+L
Sleep 500ms

# --- SCENE 2: The Context Recording (0:20 - 0:35) ---
Type "ai rec npm run dev"
Sleep 500ms
Enter
# Simulate compilation output
Sleep 3s
Type "ai why did it fail?"
Sleep 500ms
Enter
# Wait for explanation
Sleep 4s

Type "clear"
Enter

# --- SCENE 3: Auto-Commit (0:35 - 0:45) ---
Type "git status"
Sleep 500ms
Enter
Sleep 2s

Type "ai commit"
Sleep 500ms
Enter
# Wait for the AI to generate the commit
Sleep 4s

# Final hold frame
Sleep 3s
