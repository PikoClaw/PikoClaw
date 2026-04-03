# Spec: Voice Input (Speech-to-Text)

**Status**: ❌ Todo
**TS source**: `services/voice.ts`, `services/voiceStreamSTT.ts`, `voice/voiceModeEnabled.ts`

---

## Overview

Voice mode allows users to speak to PikoClaw instead of (or in addition to) typing. Audio is captured from the microphone and converted to text via a speech-to-text (STT) service, then submitted as a message.

---

## TS Implementation

The TS codebase uses:
- Browser-native `MediaRecorder` API (when running in Electron/Desktop)
- A custom streaming STT service (`voiceStreamSTT.ts`) that streams audio chunks to an API
- Key term detection (`voiceKeyterms.ts`) for wake words / command words

---

## Rust Implementation Plan

### Approach: OpenAI Whisper API (or Anthropic's future STT)

The simplest approach is to record audio locally and send to OpenAI's Whisper API for transcription.

### Step 1: Audio Capture

Use `cpal` crate for cross-platform audio input:

```toml
[dependencies]
cpal = "0.15"
hound = "3.5"   # WAV encoding
```

```rust
pub async fn record_until_silence(
    silence_threshold_ms: u32,
    max_duration_ms: u32,
) -> Result<Vec<u8>> {
    // capture audio from default input device
    // detect silence (RMS below threshold for N ms)
    // return WAV bytes
}
```

### Step 2: Transcription

```rust
pub async fn transcribe_audio(audio_bytes: &[u8]) -> Result<String> {
    // POST to OpenAI Whisper API or local whisper.cpp
    // Return transcribed text
}
```

Config:
```toml
[voice]
enabled = false
stt_provider = "whisper_api"   # whisper_api | whisper_local | deepgram
api_key = "..."                 # for whisper_api
model = "whisper-1"
language = "en"
```

### Step 3: TUI Integration

- Keybinding to start recording (default: `Ctrl+M` — "M for mic")
- Visual indicator in input bar: `🎤 Recording...` with elapsed time
- On silence or key release: transcribe and populate input bar
- User reviews transcription, then presses Enter to submit

```
┌─ Input ──────────────────────────────────────────────────────┐
│ 🎤 Recording... 3.2s  [Esc to cancel]                        │
└──────────────────────────────────────────────────────────────┘
```

After transcription:
```
┌─ Input ──────────────────────────────────────────────────────┐
│ "What does the authenticate function do?"      [Enter to send]│
└──────────────────────────────────────────────────────────────┘
```

### Step 4: `/voice` Command

```
/voice on   → enable voice mode (Ctrl+M activates recording)
/voice off  → disable voice mode
/voice      → toggle
```

---

## Dependencies

```toml
[dependencies]
cpal = "0.15"              # cross-platform audio capture
hound = "3.5"              # WAV encoding
reqwest = { ... }          # already present, for STT API calls
```

---

## Alternative: Local Whisper

For privacy-conscious users, run whisper.cpp locally:
- Bundle whisper.cpp or call via subprocess
- No network required
- Slower but offline-capable

```toml
[voice]
stt_provider = "whisper_local"
whisper_model_path = "~/.config/pikoclaw/whisper-small.bin"
```

---

## Priority

Medium-low. Nice to have but not core. Implement after hooks, memory, and task system.
