# Rekt - Voice Recorder Application

A cross-platform voice recording application built with Tauri, Svelte, and Rust.

## Overview

Rekt is a simple yet powerful voice recording application that uses the native audio capabilities of your system via Rust's CPAL audio library. It demonstrates how to build a cross-platform audio application using Tauri's combination of web technologies and Rust.

## Features

- Audio recording with microphone access
- Customizable audio settings (channels, sample rate)
- WAV file export
- Dark/Light theme
- Responsive design

## How It Works: Audio Recording Architecture

### High-Level Overview

The application combines a Svelte frontend with a Rust backend to create a seamless audio recording experience:

1. The Svelte frontend provides the user interface
2. Tauri connects the frontend to the Rust backend
3. The Rust backend handles the audio recording using CPAL
4. Audio data is stored in memory and then saved as a WAV file

### Frontend-Backend Communication

When the user presses the Record button, the frontend sends a command to the Rust backend via Tauri's invoke system. The backend starts a recording thread that captures audio from the microphone and stores it in memory. When the user stops recording, the backend saves the audio data to a WAV file.

### Technical Details of Audio Recording

#### Microphone Access and Audio Capturing

The application uses [CPAL (Cross-Platform Audio Library)](https://github.com/RustAudio/cpal) to access the audio hardware. Here's how it works:

1. **Device Discovery**: The app identifies available audio input devices on the system
   ```rust
   let host = cpal::default_host();
   let device = host.default_input_device();
   ```

2. **Configuration**: The app configures audio parameters like sample rate and channels
   ```rust
   let config = device.default_input_config();
   ```

3. **Audio Stream**: A background thread creates an audio stream that captures data from the microphone
   ```rust
   let stream = device.build_input_stream(
       &config.into(),
       move |data: &[i16], _| {
           // Store audio data
           audio_data.extend_from_slice(data);
       },
       error_callback,
       None
   );
   ```

4. **Format Handling**: The app handles different audio formats (i16, u16, f32) by converting them to a consistent format (i16) for storage
   ```rust
   // For f32 samples
   let sample = (sample.max(-1.0).min(1.0) * i16::MAX as f32) as i16;
   
   // For u16 samples
   let sample = ((sample as i32) - 32768) as i16;
   ```

5. **Data Storage**: Audio data is stored in a thread-safe Mutex-protected Vector
   ```rust
   if let Ok(mut audio_data) = state.audio_data.lock() {
       audio_data.extend_from_slice(data);
   }
   ```

#### WAV File Creation

When recording stops, the app creates a WAV file using the [hound](https://github.com/ruuda/hound) library:

```rust
let spec = hound::WavSpec {
    channels,
    sample_rate,
    bits_per_sample: 16,
    sample_format: hound::SampleFormat::Int,
};

let mut writer = hound::WavWriter::create(&filepath, spec)?;

for &sample in audio_data.iter() {
    writer.write_sample(sample)?;
}

writer.finalize()?;
```

#### Thread Safety

Audio recording happens in a separate thread to prevent blocking the UI. Thread synchronization is handled using:

- `Arc` (Atomic Reference Counting) for shared ownership of data between threads
- `Mutex` for exclusive access to shared data
- `AtomicBool` for thread-safe flag variables

```rust
struct RecordingState {
    is_recording: AtomicBool,
    audio_data: Mutex<Vec<i16>>,
    channels: Mutex<u16>,
    sample_rate: Mutex<u32>,
}
```

### Audio Configuration

Users can customize:

- **Channels**: Mono (1) or Stereo (2)
- **Sample Rate**: 8kHz, 16kHz, 22.05kHz, 44.1kHz, or 48kHz

These settings affect the quality and file size of the recording.

## Hardware Compatibility and Considerations

### Pros

1. **Cross-Platform Compatibility**: Works on Windows, macOS, and Linux
2. **Native Performance**: Uses native audio APIs through CPAL
3. **Configurable Quality**: Adjustable sample rate and channel settings
4. **Low Latency**: Direct access to system audio APIs

### Cons

1. **Device Variations**: Different hardware may have different supported formats
2. **Permission Requirements**: Requires microphone permissions which may vary by OS
3. **Format Limitations**: Some systems may not support all sample rates or formats

### Potential Issues

1. **Audio Driver Compatibility**: Some audio drivers may not work correctly with CPAL
2. **Sample Rate Support**: Not all hardware supports all sample rates (particularly 48kHz)
3. **Channel Mismatch**: Using stereo with a mono microphone may cause issues
4. **Resource Usage**: High sample rates can consume more CPU and memory
5. **Permission Denials**: If microphone access is denied, the app can't function

### Best Practices

1. **Default Settings**: The app starts with system defaults when possible
2. **Error Handling**: Graceful fallbacks when optimal settings aren't available
3. **User Feedback**: Clear status messages about audio configuration issues
4. **Format Detection**: The app attempts to detect and use the best format for each device

## Development

### Prerequisites

- Node.js (v14+)
- Rust (1.60+)
- Tauri CLI

### Setup

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## License

[MIT License](LICENSE)


## NEXT STEPS
How We Got the Voice Recorder App Working

  Key Steps Taken:

  1. Fixed Rust Lifetime Issues:
    - Changed borrowed references (&self, &RecordingState) to use Arc<RecordingState> for thread-safe reference counting
    - Made sure data shared between threads was properly ownership-transferred
  2. Fixed Audio Configuration:
    - Added storage for audio parameters (channels, sample rate)
    - Added functions to get and set audio configuration
    - Made sure audio settings were consistent throughout recording and saving
  3. Enhanced the UI:
    - Created a modern interface with light/dark theme
    - Added configuration options for audio recording
    - Implemented recording timer and visual feedback
    - Added device selection and audio parameter controls
  4. Built and Run the Application:
    - Built the backend with cargo build --release
    - Built the frontend with npm run build
    - Used npm run tauri dev for development
    - Used the release binary for production

  What Works:

  1. Audio Recording:
    - Records audio from microphone using CPAL library
    - Supports different audio formats (i16, u16, f32)
    - Properly converts between sample formats
    - Creates WAV files with correct parameters
  2. Configuration Options:
    - Channels selection (mono/stereo)
    - Sample rate selection (8kHz to 48kHz)
    - Recording mode selection (toggle or hold)
    - Theme switching (light/dark)
  3. User Interface Features:
    - Visual recording feedback with timer
    - Settings panel with configuration options
    - Audio playback of recordings
    - Status messages for user feedback

  What Doesn't Work or Has Issues:

  1. AppImage Building:
    - The AppImage bundling fails with "failed to run linuxdeploy"
    - This is a packaging issue specific to Arch Linux
    - The application still works when running the binary directly
  2. Permission Handling:
    - No explicit error handling for microphone permission denial
    - No platform-specific permission request mechanisms
    - Could lead to unclear errors if microphone access is denied
  3. Svelte Deprecation Warnings:
    - Event handling syntax (on:click etc.) is using deprecated methods
    - These warnings don't affect functionality but should be updated

  What We're Still Missing:

  1. Advanced Audio Features:
    - Audio visualization (waveform)
    - Audio trimming/editing
    - Multiple recording tracks
    - Audio effects or processing
  2. Platform Integration:
    - Proper installation packages for different Linux distributions
    - Integration with system audio controls
    - Notifications for recording status
  3. User Experience Improvements:
    - Keyboard shortcuts
    - Drag and drop interface for recordings
    - Recording organization/management
    - Export to different audio formats

  Next Steps to Consider:

  1. Fix the AppImage Building:
    - Install proper dependencies for linuxdeploy
    - Configure the build process to use a local linuxdeploy instance
  2. Improve Error Handling:
    - Add better detection and messaging for permission issues
    - Handle audio device disconnection gracefully
  3. Update Deprecated Svelte Syntax:
    - Replace on:click with onclick and similar event handlers
  4. Add Missing Features:
    - Audio visualization
    - More export options
    - Advanced recording management

  Overall, the app is functional for basic voice recording needs, with the release binary working properly. The main remaining issue is with
  packaging, not core functionality.