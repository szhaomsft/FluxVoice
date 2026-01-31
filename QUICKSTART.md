# FluxVoice - Quick Start Guide

## ✅ Current Status

- ✅ **npm packages**: All installed successfully
- ✅ **Tailwind CSS**: Configured
- ✅ **All code**: Fully implemented (Backend + Frontend)
- ❌ **Rust**: Not installed (required for Tauri)

## 🚀 Quick Setup (Windows)

### Option 1: Run the Setup Script
```bash
setup.bat
```

### Option 2: Manual Installation

**1. Install Rust** (Required - takes ~5 minutes)

Visit **https://rustup.rs/** and download `rustup-init.exe`, then run it.

Or use PowerShell:
```powershell
# Download and run Rust installer
Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "rustup-init.exe"
.\rustup-init.exe
```

After installation, **restart your terminal**.

**2. Verify Installation**
```bash
rustc --version
cargo --version
```

**3. Run FluxVoice**
```bash
npm run tauri dev
```

## 📱 First Run Configuration

1. **Launch the app** - A small floating window appears
2. **Click the window** - Opens configuration page
3. **Enter Azure credentials**:
   - Azure Speech Service Key
   - Azure Speech Region (e.g., "eastus")
   - (Optional) Azure OpenAI credentials
4. **Save configuration**
5. **Start using**: Press **Ctrl+F12** to record voice

## 🎯 How to Use

1. Press **Ctrl+F12** → Recording starts (waveform animates)
2. Speak into your microphone
3. Press **Ctrl+F12** again → Recording stops
4. App transcribes → polishes (if enabled) → inserts text

## 🔑 Azure Setup

### Get Azure Speech Service Credentials:
1. Go to https://portal.azure.com
2. Create a "Speech Service" resource
3. Copy the **Key** and **Region**
4. Paste into FluxVoice config page

### (Optional) Get Azure OpenAI:
1. Create "Azure OpenAI" resource
2. Deploy a GPT-4 model
3. Copy **Endpoint**, **Key**, and **Deployment Name**

## 🛠 Development Commands

```bash
# Development mode (hot reload)
npm run tauri dev

# Production build
npm run tauri build

# Build frontend only
npm run build

# Check for errors
cd src-tauri && cargo check
```

## 📁 Project Structure

```
FluxVoice/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── audio/      # Microphone recording
│   │   ├── azure/      # API integrations
│   │   ├── config/     # Settings persistence
│   │   ├── hotkey/     # Global shortcuts
│   │   └── input/      # Text injection
│   └── Cargo.toml      # Rust dependencies
├── src/                # React frontend
│   ├── components/     # UI components
│   ├── hooks/          # Custom hooks
│   ├── store/          # State management
│   └── types/          # TypeScript types
└── package.json        # Node dependencies
```

## 🐛 Troubleshooting

### "rustc: command not found"
- Install Rust from https://rustup.rs/
- Restart your terminal after installation

### "No audio detected"
- Check microphone permissions in Windows settings
- Verify default microphone is selected

### "Transcription failed"
- Verify Azure Speech Service credentials
- Check internet connection
- Ensure region matches your Azure resource

### "Text not inserting"
- Enable "Auto-Insert Text" in settings
- Make sure target application accepts keyboard input
- Click on the target app before stopping recording

## 🎨 Features

- ✅ Always-on-top floating window
- ✅ Global hotkey (Ctrl+F12)
- ✅ Real-time waveform visualization
- ✅ Azure Speech transcription
- ✅ Azure OpenAI text polishing
- ✅ Auto-insert into active window
- ✅ Dark/Light theme support
- ✅ Persistent configuration

## 📞 Support

For issues or questions:
- Check the troubleshooting section above
- Review console logs in development mode
- Verify all prerequisites are installed
