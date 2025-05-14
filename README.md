# Snapshot Hoster

A lightweight, efficient application for capturing screenshots and analyzing them with AI. The application can overlay itself on other applications and process the captured content using either local language models or remote AI APIs.

## Features

- **Efficient Screenshot Capture**: Capture screenshots of the entire screen or specific windows
- **AI Analysis**: Process screenshots with either local LLMs or cloud-based APIs (OpenAI, Anthropic)
- **Transparent Overlay**: Display AI analysis as an overlay on the captured application
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Hotkey Support**: Configure global hotkeys for quick operations
- **Lightweight**: Minimal resource usage when not actively processing

## Technology

Built with Rust for efficiency and reliability:

- **egui/eframe**: Lightweight, immediate-mode GUI framework
- **screenshots**: Cross-platform screen capture
- **reqwest**: HTTP client for API integration
- **winit**: Low-level window handling
- **global-hotkey**: Hotkey registration
- **image**: Image processing utilities
- **anyhow/thiserror**: Error handling
- **serde**: Configuration serialization

## Installation

### Prerequisites

- Rust toolchain (1.70 or newer recommended)
- Platform development tools
  - Windows: MSVC build tools
  - Linux: X11 or Wayland development libraries
  - macOS: Xcode command-line tools

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/snapshot-hoster.git
   cd snapshot-hoster
   ```

2. Build the application:
   ```bash
   cargo build --release
   ```

3. Run the application:
   ```bash
   cargo run --release
   ```

The compiled binary will be available in `target/release/snapshot-hoster`.

## Usage

### Basic Operations

1. **Capturing Screenshots**:
   - Use the "Capture Screenshot" button in the UI
   - Or press the configured hotkey (default: Ctrl+Shift+S)

2. **Processing with AI**:
   - After capturing, click "Process with AI"
   - Results will be displayed in the application and can be shown as an overlay

3. **Configuring Settings**:
   - Click the "Settings" button to configure AI providers, hotkeys, and overlay preferences

### AI Integration

#### Local Models

To use a local AI model:
1. Enable "Use Local Model" in settings
2. Provide the path to your local model executable
   - The model should accept image input and produce text output

#### API Integration

To use cloud AI services:
1. Disable "Use Local Model" in settings
2. Select your preferred provider (OpenAI, Anthropic, or Custom)
3. Enter your API key
4. For custom providers, enter the API endpoint URL

## Privacy and Security

- All processing happens on your device when using local models
- When using API services, screenshots are sent to the respective service providers
- No data is permanently stored or shared beyond what's necessary for the requested analysis

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.