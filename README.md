# FluxVoice

A voice input method application with Azure Speech transcription and OpenAI polishing capabilities.

## Features

- **Always-on-top floating window** - Minimal, transparent UI that stays visible
- **Global hotkey activation** - Hold Ctrl+Shift+Z, or choose Caps Lock on Windows, to record
- **Azure Fast Transcription** - Real-time speech-to-text using Azure Cognitive Services
- **Multilingual Transcription** - Auto-detects and transcribes across multiple languages continuously (de-DE, en-AU, en-CA, en-GB, en-IN, en-US, es-ES, es-MX, fr-CA, fr-FR, it-IT, ja-JP, ko-KR, zh-CN)
- **AI Text Polishing** - Optional enhancement with Azure OpenAI (configurable model deployment)
- **Auto-insertion** - Automatically paste transcribed text into active windows
- **Waveform visualization** - Real-time audio level display while recording
- **Configurable settings** - Full customization of Azure credentials, hotkeys, and preferences

### Export recordings and transcripts

Open **Settings > History > Export All** and choose a destination directory.
FluxVoice creates a fresh `fluxvoice-export-<UTC timestamp>` subfolder without
overwriting existing exports. It contains original audio files (`.wav`, `.ogg`,
or `.mp3`, without conversion), matching UTF-8 `.txt` files with original,
processed (when available), and final text, and a `transcripts.json` index.
Entries are ordered chronologically; numbered UTC filenames remain unique even
when recordings share a timestamp.

Export includes all saved history, not just visible entries. Wait for the latest
recording to finish saving before exporting; pending background saves are not
included in the snapshot. Entries without saved audio still get transcripts,
and the completion message reports the missing-audio count. Invalid data and
write errors are shown explicitly; a failed export may leave a partial folder.
Exported audio and transcripts may contain sensitive information; keep the
destination private.

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

### Multilingual Mode

FluxVoice supports Azure's multilingual transcription model, which can detect and transcribe across multiple languages within a single audio recording.

Enable it in **Settings → Speech Languages → Multilingual** toggle.

- **Multilingual ON**: The API uses the multi-lingual speech model to auto-detect and transcribe across supported languages (de-DE, en-AU, en-CA, en-GB, en-IN, en-US, es-ES, es-MX, fr-CA, fr-FR, it-IT, ja-JP, ko-KR, zh-CN). No locale selection needed.
- **Multilingual OFF**: Select specific locale(s) for language identification. Multiple locales enable auto-detection of the single best-matching locale per audio.

### Speech-to-Text Model

Choose **Settings → General Settings → Speech-to-Text Model**, then save:

| Model | Request mode | Recording format |
| --- | --- | --- |
| Fast STT (default) | Standard Fast Transcription | 16 kHz mono Opus/OGG |
| LLM Speech | `enhancedMode: { enabled: true, task: "transcribe" }` | 16 kHz mono Opus/OGG |
| MAI-Transcribe-2 (public preview) | `enhancedMode: { enabled: true, model: "MAI-Transcribe-2" }` | 16 kHz mono MP3 at 48 kbps |

All three use the configured Azure Speech key/region and API version
`2025-10-15`. Existing configurations default to Fast STT. The recording captures
its settings when it starts, so model or post-processing changes apply to the
next recording. Polish and translation still use the separate OpenAI
post-processing settings; selecting LLM Speech does not automatically translate.

LLM Speech and MAI require a region/resource supporting the selected model.
Model availability and language support differ; service errors are shown rather
than silently falling back to Fast STT. See [LLM Speech](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/llm-speech)
and [MAI-Transcribe-2](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/mai-transcribe).

MAI supports automatic multilingual detection when **Multilingual** is enabled.
When disabled, select one language hint; regional locales are converted to MAI
language codes (`en-US` → `en`, `zh-CN` → `zh`, `zh-HK` → `yue`). MAI recordings
are encoded directly from captured PCM to MP3 with the bundled LAME encoder;
no FFmpeg installation or external conversion process is required. At 48 kbps,
MP3 uses approximately 6 KB per second compared with 32 KB per second for
16-bit mono WAV, plus frame/padding overhead. MP3 is lossy, so recognition quality
and latency should be evaluated with representative speech. History playback
and export handle MP3, Opus/OGG, and existing WAV recordings.

The `mp3lame-encoder` and `mp3lame-sys` crates are LGPL-3.0 dependencies.
Binary distributors must comply with their applicable license and relinking/source
requirements; see the [encoder project](https://github.com/DoumanAsh/mp3lame-encoder)
and [native binding project](https://github.com/DoumanAsh/mp3lame-sys).

### Hotkey

Default hotkey is **Ctrl+Shift+Z**. Hold the shortcut to record, then release it
to stop and transcribe.

For a single key close to your left hand on Windows, open **Settings > General
Settings > Recording Shortcut**, select **Caps Lock**, and click **Save
Configuration**. The change takes effect immediately and is saved for future
launches. Hold Caps Lock to record and release it to transcribe. While this
shortcut is active, FluxVoice consumes physical Caps Lock presses globally so
they do not toggle capitalization; the existing Caps Lock on/off state is
preserved. Other keys and software-generated key events are unaffected. Switch
back to **Ctrl + Shift + Z** and save, or exit FluxVoice, to restore normal Caps
Lock behavior. Release the recording shortcut before changing it.

## Usage

1. Launch FluxVoice - a small floating window will appear
2. Hold your recording shortcut (**Ctrl+Shift+Z** by default)
3. Speak clearly into your microphone
4. Release the shortcut to stop recording
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

### Measuring recognition latency

Run `npm run tauri dev` and make a normal recording. Lines prefixed with
`[latency]` report milliseconds spent stopping and preparing audio, encoding,
speech requests and retries, post-processing, insertion, and saving history/stats.
The frontend summary distinguishes release-handler-to-result from
release-handler-to-idle; these totals include IPC but exclude OS hotkey delivery
and the final UI paint. Request-to-headers timings combine connection setup,
upload, service processing, and network transit; they do not isolate Azure
compute time. Nested stage timings should not be added to their enclosing totals.
Timing diagnostics contain durations, status codes, and processing mode, not
credentials or transcript text.

History and usage statistics are saved in a serialized background queue after a
result is received, without delaying the return to idle. Save failures appear in
the floating window when idle. Allow pending saves to finish before closing the
app.

Speech and OpenAI share a pooled HTTP client that retains idle connections for
up to 10 minutes. Recording startup asynchronously warms the configured service
connections with unauthenticated HEAD requests; OpenAI is warmed only when
post-processing is enabled. These requests send no audio or text and do not
invoke a model. Warmup does not delay recording or change inference settings.
Servers can still close idle connections, and warming a connection does not
warm the model itself or eliminate model-generation latency.

## License

For demonstration purposes.
