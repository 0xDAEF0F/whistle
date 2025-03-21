# Whistle

## Overview

Whistle is a simple yet powerful application designed to transcribe audio to text efficiently. Built
with modern technologies like `fast-whisper` and `tauri`, it offers a seamless experience for users
who need accurate and quick transcription services.

## Features

- **Audio Transcription**: Convert audio into text with high accuracy.
- **Text Polishing**: Automatically polish text by passing it through deepseekV3 to provide more
  context to your profile.

## Installation

### Prerequisites

- **Rust**: Ensure you have Rust installed. You can download it from
  [rust-lang.org](https://www.rust-lang.org/).
- **Tauri CLI**: Install Tauri CLI by running:
  ```bash
  cargo install tauri-cli
  ```

### Steps

1. **Clone the Repository**:

   ```bash
   git clone https://github.com/yourusername/transcribe-app.git
   cd transcribe-app/src-tauri
   ```

2. **Build the Application**:

   ```bash
   cargo tauri build
   ```

3. **Run the Application**:
   ```bash
   cargo tauri dev
   ```

## Usage

1. **Launch the Application**: After building, run the application using the command above.
2. **Start Transcription**: Click on the 'Transcribe' button to begin the process.
3. **View Results**: Once completed, the transcribed text will be displayed on the screen.
4. **Copy to Clipboard**: Use the clipboard manager to copy the text for further use.

## Contributing

We welcome contributions! Please fork the repository and submit a pull request for any improvements or bug fixes.

## License

This project is licensed under the MIT License.

## Contact

For any inquiries or support, please contact alex t. at [aletapia@proton.me](mailto:aletapia@proton.me).
