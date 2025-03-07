#\!/bin/bash

# Set path to our local linuxdeploy
export PATH="$PWD/.tools:$PATH"

# Make linuxdeploy executable
chmod +x .tools/linuxdeploy

# Build the application with our local linuxdeploy
LINUXDEPLOY="$PWD/.tools/linuxdeploy" cargo tauri build

