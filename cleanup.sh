#!/bin/bash
# Script to clean up and rebuild the ScreenSnap project

echo "=== Cleaning up and rebuilding ScreenSnap ==="

# First, let's remove any problematic files
echo "Removing existing compiled artifacts..."
rm -rf target

# Check if we need to fix the window_finder.rs file
if grep -q "unclosed delimiter" src/capture/window_finder.rs 2>/dev/null; then
    echo "Previous window_finder.rs had errors, replacing it..."
    rm src/capture/window_finder.rs
fi

# Create necessary directories if they don't exist
mkdir -p src/ai src/capture src/utils

# Ensure run.sh is executable
chmod +x run.sh

# Build the project
echo "Building ScreenSnap..."
cargo build

if [ $? -eq 0 ]; then
    echo "✅ Build successful! You can now run the application with ./run.sh"
else
    echo "❌ Build failed. Please check the error messages above."
fi