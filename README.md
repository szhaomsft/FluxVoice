# FluxVoice

A voice input method application with Azure Speech transcription and OpenAI polishing capabilities.

## Features

- **Always-on-top floating window** - Minimal, transparent UI that stays visible
- **Global hotkey activation** - Press Ctrl+Shift+Z to start/stop recording
- **Azure Fast Transcription** - Real-time speech-to-text using Azure Cognitive Services
- **AI Text Polishing** - Optional enhancement with Azure OpenAI GPT-4.1
- **Auto-insertion** - Automatically paste transcribed text into active windows
- **Waveform visualization** - Real-time audio level display while recording
- **Configurable settings** - Full customization of Azure credentials, hotkeys, and preferences

## Prerequisites

- Rust (install from https://rustup.rs/)
- Node.js (v18 or higher)
- Azure Speech Service subscription
- Azure OpenAI subscription (optional, for text polishing)

## Installation

1. Install dependencies:
```bash
npm install
```

2. Build and run in development mode:
```bash
npm run tauri dev
```

3. Build for production:
```bash
npm run tauri build
```

## Configuration

### First Time Setup

1. Click on the floating window to open the configuration page
2. Enter your Azure Speech Service credentials
3. (Optional) Enter Azure OpenAI credentials for text polishing
4. Configure language settings and features
5. Click "Save Configuration"

### Hotkey

Default hotkey is **Ctrl+Shift+Z**. Press once to start recording, press again to stop.

## Usage

1. Launch FluxVoice - a small floating window will appear
2. Press **Ctrl+Shift+Z** to start voice recording
3. Speak clearly into your microphone
4. Press **Ctrl+Shift+Z** again to stop recording
5. Text will be transcribed, polished (if enabled), and auto-inserted

## Architecture

- **Backend**: Rust with Tauri 2.x
- **Frontend**: React 18 + TypeScript + Tailwind CSS
- **State Management**: Zustand
- **APIs**: Azure Speech Services + Azure OpenAI

## Troubleshooting

- **No audio**: Check microphone permissions and default device
- **Transcription errors**: Verify Azure credentials and internet connection
- **Text not inserting**: Enable auto-insert in settings, ensure target app accepts input
- **Hotkey not working**: Check for conflicts with other applications

## License

For demonstration purposes.
