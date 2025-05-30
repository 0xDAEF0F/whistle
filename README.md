# Whistle
[![Twitter Follow](https://img.shields.io/twitter/follow/Alex?style=social)](https://x.com/0xdaef0f)

Whistle is a desktop application that efficiently transcribes audio to text and polishes text
through AI. Perfect for note-taking, content creation, and accessibility.

## Features

- **Audio Transcription**: Quickly convert speech to text with high accuracy using fast-whisper
- **Text Polishing**: Clean up transcribed text using DeepSeek V3 for better readability
- **Global Shortcuts**: Control recording and text processing even when the app is in background
- **System Tray Integration**: Access functionality quickly from your system tray
- **Clipboard Integration**: Seamlessly work with your clipboard content

## Installation

### Download

Download the latest release from the [Releases](https://github.com/0xDAEF0F/whistle/releases)
page. If you are on macOS, you can download the `.dmg` file and double click on
it to install and move it to your applications folder just like any other app. and also you

Notes: since the app is not signed, you will need to remove it from the
quarantine list after downloading it and putting it in your applications folder.

```bash
xattr -dr com.apple.quarantine /Applications/whistle.app
```

if you are not sure about whether it is quarantined, you can run the following command (to verify)

```bash
xattr -l /Applications/whistle.app
```

if you are having permission issues when reinstalling the app for the _second_
time, run the following to reset permissions:

```bash
tccutil reset All com.whistle.app
```

### Build from Source

If you prefer to build from source:

1. **Prerequisites**:

   - [Rust](https://www.rust-lang.org/tools/install)
   - [Tauri CLI](https://v2.tauri.app/reference/cli/)
   - [Node.js](https://nodejs.org/en/download)
     Note: you can install pnpm with `npm install -g pnpm`

2. **Setup**:

   ```bash
   # Clone the repository
   git clone https://github.com/0xDAEF0F/whistle.git
   cd whistle

   ### Build for production (this will run `pnpm install`, too)
   cargo tauri build --release

   # Run in development mode
   cargo tauri dev
   ```

## Usage

### Basic Controls

1. **Start/Stop Recording**: Press `Cmd+Option+R` (Mac) or `Ctrl+Alt+R` (Windows/Linux)
2. **Polish Clipboard Text**: Press `Cmd+Option+C` (Mac) or `Ctrl+Alt+C` (Windows/Linux)
3. **Access Menu**: Right-click on the system tray icon

### Customizing Shortcuts

1. Open the application window
2. Use the shortcut configuration panel to set your preferred key combinations

### Configuration

Shortcuts are stored in `~/.config/whistle/shortcuts.json`

## Troubleshooting

- **No audio recording**: Ensure microphone permissions are granted in system settings
- **Transcription errors**: Try speaking more clearly or in a quieter environment
- **Shortcut conflicts**: Change shortcuts if they conflict with other applications

## License

This project is licensed under the MIT License.

## Contact

For support or suggestions, contact alex t. at [aletapia@proton.me](mailto:aletapia@proton.me).
