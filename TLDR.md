# Rekt: Rust-Tauri-Svelte Audio Recording App

## Architecture Overview

### Recording Flow
1. **Frontend (Svelte)** sends a command to start recording
2. **Rust Backend** uses cpal library to capture audio input
3. **Rust Thread Management** handles recording in background thread
4. **Audio Processing** writes audio data to WAV file using hound library
5. **State Management** uses Arc, Mutex and AtomicBool for thread safety
6. **IPC** notifies frontend of recording status changes via events

### Playback Flow
1. **Frontend** requests audio playback via Tauri command
2. **File Access** Rust provides path to recording via IPC
3. **Web Audio API** frontend handles actual playback using browser APIs
4. **UI Updates** frontend manages playback controls and visualization

### Data Persistence
- Recordings stored in app data directory
- Metadata handled by Rust with serialization to disk
- Frontend accesses recording list through Tauri commands

### Key Components
- **Tauri Commands**: Bridge between TypeScript and Rust
- **cpal**: Rust library for audio input/output
- **hound**: WAV file creation and manipulation
- **Svelte Stores**: Frontend state management for UI

## Communication Examples

### Recording Commands
```typescript
// Frontend: Start recording
await invoke('start_recording');

// Frontend: Stop recording
const result = await invoke('stop_recording');
const recordingPath = result.path;

// Frontend: Configure audio before recording
await invoke('set_audio_config', { 
  channels: selectedChannels, 
  sampleRate: selectedSampleRate 
});
```

```rust
// Backend: Start recording command handler
#[tauri::command]
pub fn start_recording(state: State<RecordingState>) -> Result<(), String> {
    // Initialize recording thread with cpal
    // Return Result to frontend
}

// Backend: Stop recording command handler
#[tauri::command]
pub fn stop_recording(state: State<RecordingState>) -> Result<RecordingResult, String> {
    // Stop recording thread
    // Save WAV file with hound
    // Return file path to frontend
}
```

### Playback Communication
```typescript
// Frontend: Request playback
await invoke('play_audio', { path: audioPath });

// Frontend: Listen for playback events
await listen('audio-playback-stopped', (event) => {
  // Update UI when playback ends
});
```

### Device & File Management
```typescript
// Frontend: Get available devices
const devices = await invoke('get_audio_devices');

// Frontend: Retrieve audio file as base64
const audioData = await invoke('get_audio_data', { path });
```

This communication pattern demonstrates how Tauri's IPC bridges the TypeScript frontend with the Rust audio processing backend.