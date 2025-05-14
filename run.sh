#!/bin/bash
# Simple run script for ScreenSnap

echo "=== ScreenSnap Simplified ==="

# Start Ollama if not running
if ! pgrep -x "ollama" > /dev/null; then
    echo "Starting Ollama server..."
    ollama serve &
    sleep 3
fi

# Check if we have Llava model
if ! ollama list | grep -q "llava"; then
    echo "Llava model not found. Would you like to pull it? (y/n)"
    read answer
    if [[ "$answer" == "y" ]]; then
        echo "Pulling llava:latest model..."
        ollama pull llava:latest
    fi
fi

# Run the app in interactive mode
echo "Starting ScreenSnap in interactive mode..."
RUST_LOG=info cargo run -- interactive