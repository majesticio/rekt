use std::fs::File;
use std::io::{Read, Write, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use base64::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::Serialize;
use tauri::{AppHandle, Manager, State, Emitter};
use tempfile::NamedTempFile;

//
// ====== AUDIO INPUT (RECORDING) STATE ======
//

struct AudioInputStream {
    #[allow(dead_code)] // We only hold the stream to keep it alive
    stream: Box<dyn StreamTrait>,
}

unsafe impl Send for AudioInputStream {}
unsafe impl Sync for AudioInputStream {}

#[derive(Default)]
struct RecordingState {
    is_recording: AtomicBool,
    audio_data: Mutex<Vec<i16>>,
    channels: Mutex<u16>,
    sample_rate: Mutex<u32>,
    input_stream: Mutex<Option<AudioInputStream>>,
}

/// Background recorder spawns a thread that keeps recording
struct BackgroundRecorder {
    join_handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl Default for BackgroundRecorder {
    fn default() -> Self {
        Self {
            join_handle: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl BackgroundRecorder {
    fn start(&mut self, state: Arc<RecordingState>) -> Result<(), String> {
        // Make sure we're not already recording
        if self.join_handle.is_some() {
            return Err("Already recording".to_string());
        }
        
        state.is_recording.store(false, Ordering::SeqCst); // Reset in case.
        self.stop_flag.store(false, Ordering::SeqCst);

        // Clone arcs for the thread
        let stop_flag = Arc::clone(&self.stop_flag);
        let thread_state = Arc::clone(&state);

        // Create the thread
        let handle = thread::spawn(move || {
            println!("Recording thread started");
            
            // Clear audio buffer before new recording
            {
                let mut audio_data = thread_state.audio_data.lock().unwrap();
                audio_data.clear();
            }

            // ALWAYS initialize the input stream each time
            let host = cpal::default_host();

            // Get the default input device
            let device = match host.default_input_device() {
                Some(dev) => dev,
                None => {
                    println!("Error: No input device available");
                    return;
                }
            };

            println!("Using input device: {}", device.name().unwrap_or_else(|_| "unknown".to_string()));

            // Get default config for this device
            let config = match device.default_input_config() {
                Ok(cfg) => cfg,
                Err(e) => {
                    println!("Error getting default input config: {}", e);
                    return;
                }
            };

            // Store the actual device format into the state
            let actual_channels = config.channels();
            let actual_sample_rate = config.sample_rate().0;

            {
                let mut ch_lock = thread_state.channels.lock().unwrap();
                let mut sr_lock = thread_state.sample_rate.lock().unwrap();
                *ch_lock = actual_channels;
                *sr_lock = actual_sample_rate;
            }

            println!("Recording with {} channel(s) at {} Hz", actual_channels, actual_sample_rate);

            let err_fn = |err| eprintln!("An error occurred on the input stream: {}", err);

            let i16_state = Arc::clone(&thread_state);
            let u16_state = Arc::clone(&thread_state);
            let f32_state = Arc::clone(&thread_state);

            // Build the input stream using the actual default config
            let stream = match config.sample_format() {
                SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if i16_state.is_recording.load(Ordering::SeqCst) {
                            if let Ok(mut audio_data) = i16_state.audio_data.lock() {
                                audio_data.extend_from_slice(data);
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::U16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if u16_state.is_recording.load(Ordering::SeqCst) {
                            if let Ok(mut audio_data) = u16_state.audio_data.lock() {
                                for &sample in data {
                                    let sample = ((sample as i32) - 32768) as i16;
                                    audio_data.push(sample);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if f32_state.is_recording.load(Ordering::SeqCst) {
                            if let Ok(mut audio_data) = f32_state.audio_data.lock() {
                                for &sample in data {
                                    let clamped = sample.clamp(-1.0, 1.0);
                                    let converted = (clamped * i16::MAX as f32) as i16;
                                    audio_data.push(converted);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                _ => {
                    println!("Unsupported sample format.");
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    println!("Error building input stream: {}", e);
                    return;
                }
            };

            // Store the stream in our state so it won't get dropped
            {
                let mut input_stream = thread_state.input_stream.lock().unwrap();
                *input_stream = Some(AudioInputStream {
                    stream: Box::new(stream),
                });
            }

            // Start the stream
            if let Err(e) = thread_state.input_stream.lock().unwrap().as_ref().unwrap().stream.play() {
                println!("Error starting input stream: {}", e);
                return;
            }

            // Indicate recording is now active
            thread_state.is_recording.store(true, Ordering::SeqCst);

            // Keep the thread alive until we stop
            while !stop_flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }

            // Turn off recording
            thread_state.is_recording.store(false, Ordering::SeqCst);

            println!("Recording thread stopped");
        });

        self.join_handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        // Signal the thread to stop
        self.stop_flag.store(true, Ordering::SeqCst);

        // Join the thread if it exists
        if let Some(handle) = self.join_handle.take() {
            handle.join().map_err(|_| "Failed to join recording thread".to_string())?;
        } else {
            return Err("No recording in progress".to_string());
        }

        Ok(())
    }
}

//
// ====== AUDIO OUTPUT (PLAYBACK) STATE ======
//

struct AudioOutputStream {
    #[allow(dead_code)] // Kept alive
    stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
}

unsafe impl Send for AudioOutputStream {}
unsafe impl Sync for AudioOutputStream {}

#[derive(Default)]
struct AudioPlaybackState {
    is_playing: AtomicBool,
    current_playback_id: Mutex<Option<String>>,
    output_stream: Mutex<Option<AudioOutputStream>>,
    device_initialized: AtomicBool,
}

#[derive(Debug, Serialize)]
struct AudioRecordingResponse {
    success: bool,
    path: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AudioDataResponse {
    success: bool,
    data: Option<String>,
    mime_type: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AudioPlaybackResponse {
    success: bool,
    is_playing: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct AudioPlaybackEvent {
    playback_id: String,
}

#[derive(Debug, Serialize)]
struct AudioConfigResponse {
    success: bool,
    device_name: String,
    available_devices: Vec<AudioDeviceInfo>,
    current_device: AudioDeviceInfo,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AudioDeviceInfo {
    name: String,
    channels: u16,
    sample_rate: u32,
    formats: Vec<String>,
}

//
// ========== Tauri Commands ==========
//

// Start recording
#[tauri::command]
fn start_recording(
    state: State<'_, Arc<RecordingState>>,
    recorder: State<'_, Mutex<BackgroundRecorder>>,
) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }

    // Clear old data
    {
        let mut audio_data = state.audio_data.lock().unwrap();
        audio_data.clear();
    }

    // Actually start the background recorder
    let mut bg_recorder = recorder.lock().unwrap();
    bg_recorder.start(Arc::clone(state.inner()))?;

    state.is_recording.store(true, Ordering::SeqCst);
    println!("Recording started");

    Ok(())
}

// Stop recording and write WAV file
#[tauri::command]
async fn stop_recording(
    app_handle: AppHandle,
    state: State<'_, Arc<RecordingState>>,
    recorder: State<'_, Mutex<BackgroundRecorder>>,
) -> Result<AudioRecordingResponse, String> {
    if !state.is_recording.load(Ordering::SeqCst) {
        return Err("Not recording".to_string());
    }

    // Stop background recorder
    {
        let mut bg_recorder = recorder.lock().unwrap();
        bg_recorder.stop()?;
    }

    state.is_recording.store(false, Ordering::SeqCst);
    println!("Recording stopped");

    // Determine where to save
    let app_dir = app_handle
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    // Make a filename
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("recording_{}.wav", timestamp);
    let filepath = app_dir.join(filename);

    // Retrieve the actual channels and sample rate we used
    let channels = *state.channels.lock().unwrap();
    let sample_rate = *state.sample_rate.lock().unwrap();
    println!("Writing WAV with {} channel(s) at {} Hz", channels, sample_rate);

    // Create WAV
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&filepath, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    let audio_data = state.audio_data.lock().unwrap();

    if audio_data.is_empty() {
        println!("No audio data recorded, creating 1s silent file...");
        for _ in 0..(sample_rate * channels as u32) {
            writer.write_sample(0i16)
                .map_err(|e| format!("Failed to write sample: {}", e))?;
        }
    } else {
        println!("Writing {} samples...", audio_data.len());
        for &sample in audio_data.iter() {
            writer.write_sample(sample)
                .map_err(|e| format!("Failed to write sample: {}", e))?;
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(AudioRecordingResponse {
        success: true,
        path: Some(filepath.to_string_lossy().to_string()),
        error: None,
    })
}

// Return the recorded file as base64
#[tauri::command]
async fn get_audio_data(path: String) -> Result<AudioDataResponse, String> {
    let mut file = File::open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Base64 encode
    let base64_data = BASE64_STANDARD.encode(&buffer);

    // Infer MIME
    let mime_type = if path.ends_with(".wav") {
        "audio/wav"
    } else if path.ends_with(".mp3") {
        "audio/mp3"
    } else {
        "audio/octet-stream"
    };

    Ok(AudioDataResponse {
        success: true,
        data: Some(base64_data),
        mime_type: mime_type.to_string(),
        error: None,
    })
}

// Check if currently recording
#[tauri::command]
fn is_recording(state: State<'_, Arc<RecordingState>>) -> bool {
    state.is_recording.load(Ordering::SeqCst)
}

// Check if currently playing
#[tauri::command]
fn is_playing(playback_state: State<'_, AudioPlaybackState>) -> bool {
    playback_state.is_playing.load(Ordering::SeqCst)
}

// List available audio input devices
#[tauri::command]
fn get_audio_devices() -> Result<AudioConfigResponse, String> {
    let host = cpal::default_host();

    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to get input devices: {}", e))?
        .collect::<Vec<_>>();

    let default_device = host
        .default_input_device()
        .ok_or_else(|| "No default input device available.".to_string())?;

    let default_name = default_device
        .name()
        .unwrap_or_else(|_| "Unknown Device".to_string());
    let default_config = default_device
        .default_input_config()
        .map_err(|e| format!("Failed to get default config: {}", e))?;
    let supported_configs = default_device
        .supported_input_configs()
        .map_err(|e| format!("Failed to get supported configs: {}", e))?
        .collect::<Vec<_>>();
    let formats = supported_configs
        .iter()
        .map(|cfg| format!("{:?}", cfg.sample_format()))
        .collect();

    // Collect info
    let available_devices = devices
        .iter()
        .filter_map(|dev| {
            let name = dev.name().ok()?;
            let cfg = dev.default_input_config().ok()?;
            let sup = dev.supported_input_configs().ok()?.collect::<Vec<_>>();
            let fmts = sup.iter()
                .map(|c| format!("{:?}", c.sample_format()))
                .collect::<Vec<_>>();

            Some(AudioDeviceInfo {
                name,
                channels: cfg.channels(),
                sample_rate: cfg.sample_rate().0,
                formats: fmts,
            })
        })
        .collect::<Vec<_>>();

    Ok(AudioConfigResponse {
        success: true,
        device_name: default_name.clone(),
        current_device: AudioDeviceInfo {
            name: default_name,
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate().0,
            formats,
        },
        available_devices,
        error: None,
    })
}

// Set user-chosen config (currently just stored; not used in build_input_stream)
#[tauri::command]
fn set_audio_config(state: State<'_, Arc<RecordingState>>, channels: u16, sample_rate: u32) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Cannot change config while recording.".to_string());
    }

    // Simple validations
    if !(1..=2).contains(&channels) {
        return Err("Invalid number of channels (must be 1 or 2).".to_string());
    }
    let valid_rates = [8000, 16000, 22050, 44100, 48000];
    if !valid_rates.contains(&sample_rate) {
        return Err(format!(
            "Invalid sample rate {}, must be one of {:?}",
            sample_rate, valid_rates
        ));
    }

    *state.channels.lock().unwrap() = channels;
    *state.sample_rate.lock().unwrap() = sample_rate;

    println!("Audio config set to {} ch, {} Hz", channels, sample_rate);
    Ok(())
}

// Get the currently stored config (not necessarily the device's default)
#[tauri::command]
fn get_current_audio_config(state: State<'_, Arc<RecordingState>>) -> Result<AudioDeviceInfo, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default input device available.".to_string())?;
    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());

    let device_config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get device config: {}", e))?;

    let stored_channels = *state.channels.lock().unwrap();
    let stored_rate = *state.sample_rate.lock().unwrap();

    // If stored is zero (never set), fallback to device default
    let channels = if stored_channels == 0 {
        device_config.channels()
    } else {
        stored_channels
    };
    let sample_rate = if stored_rate == 0 {
        device_config.sample_rate().0
    } else {
        stored_rate
    };

    let supported_configs = device
        .supported_input_configs()
        .map_err(|e| format!("Failed to get supported input configs: {}", e))?
        .collect::<Vec<_>>();
    let formats = supported_configs
        .iter()
        .map(|c| format!("{:?}", c.sample_format()))
        .collect::<Vec<_>>();

    Ok(AudioDeviceInfo {
        name,
        channels,
        sample_rate,
        formats,
    })
}

//
// ====== Playback commands ======
//

// Start playback from a file
#[tauri::command]
async fn play_audio(
    path: String,
    app_handle: AppHandle,
    playback_state: State<'_, AudioPlaybackState>,
) -> Result<AudioPlaybackResponse, String> {
    stop_audio_internal(&playback_state); // Stop any existing audio

    let playback_id = nanoid::nanoid!();
    *playback_state.current_playback_id.lock().unwrap() = Some(playback_id.clone());
    playback_state.is_playing.store(true, Ordering::SeqCst);

    let path_clone = path.clone();
    let playback_id_clone = playback_id.clone();

    // Possibly re-init output device if needed
    let mut need_init = !playback_state.device_initialized.load(Ordering::SeqCst);
    let mut stream_handle_option = None;

    // Try to get existing stream
    {
        let output_guard = playback_state.output_stream.lock().unwrap();
        if let Some(ref existing_output) = *output_guard {
            stream_handle_option = Some(existing_output.handle.clone());
        } else {
            need_init = true;
        }
    }

    if need_init {
        use rodio::OutputStream;
        match OutputStream::try_default() {
            Ok((stream, handle)) => {
                if let Ok(mut out) = playback_state.output_stream.lock() {
                    *out = Some(AudioOutputStream {
                        stream,
                        handle: handle.clone(),
                    });
                }
                playback_state.device_initialized.store(true, Ordering::SeqCst);
                stream_handle_option = Some(handle);
            }
            Err(e) => {
                playback_state.is_playing.store(false, Ordering::SeqCst);
                return Err(format!("Failed to create output stream: {}", e));
            }
        }
    }

    let stream_handle = match stream_handle_option {
        Some(h) => h,
        None => {
            playback_state.is_playing.store(false, Ordering::SeqCst);
            return Err("Failed to get output stream handle".to_string());
        }
    };

    // Playback in a separate thread
    thread::spawn(move || {
        use rodio::{Decoder, Sink};

        let file = match File::open(&path_clone) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file for playback: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        let buf_reader = BufReader::new(file);
        let source = match Decoder::new(buf_reader) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error decoding file: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error creating Sink: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        sink.append(source);
        sink.sleep_until_end();

        let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
    });

    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: true,
        error: None,
    })
}

// Stop any playback
#[tauri::command]
fn stop_audio(playback_state: State<'_, AudioPlaybackState>) -> Result<AudioPlaybackResponse, String> {
    stop_audio_internal(&playback_state);
    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: false,
        error: None,
    })
}

// Internal helper
fn stop_audio_internal(playback_state: &AudioPlaybackState) {
    if playback_state.is_playing.load(Ordering::SeqCst) {
        playback_state.is_playing.store(false, Ordering::SeqCst);
        *playback_state.current_playback_id.lock().unwrap() = None;
        // Actual stopping is done because rodio Sinks run in another thread
    }
}

// Play audio from base64 data
#[tauri::command]
async fn play_audio_from_base64(
    base64_data: String,
    mime_type: String,
    app_handle: AppHandle,
    playback_state: State<'_, AudioPlaybackState>,
) -> Result<AudioPlaybackResponse, String> {
    stop_audio_internal(&playback_state);

    let playback_id = nanoid::nanoid!();
    *playback_state.current_playback_id.lock().unwrap() = Some(playback_id.clone());
    playback_state.is_playing.store(true, Ordering::SeqCst);

    let audio_data = BASE64_STANDARD
        .decode(base64_data.as_bytes())
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // We create a temporary file
    let extension = if mime_type.contains("wav") {
        ".wav"
    } else if mime_type.contains("mp3") {
        ".mp3"
    } else {
        ".wav" // fallback
    };

    let mut temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    temp_file.write_all(&audio_data)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    let path_clone = temp_file.path().to_path_buf();
    let playback_id_clone = playback_id.clone();

    // Possibly re-init output device
    let mut need_init = !playback_state.device_initialized.load(Ordering::SeqCst);
    let mut stream_handle_option = None;
    {
        let out_guard = playback_state.output_stream.lock().unwrap();
        if let Some(ref output) = *out_guard {
            stream_handle_option = Some(output.handle.clone());
        } else {
            need_init = true;
        }
    }

    if need_init {
        use rodio::OutputStream;
        match OutputStream::try_default() {
            Ok((stream, handle)) => {
                if let Ok(mut out) = playback_state.output_stream.lock() {
                    *out = Some(AudioOutputStream {
                        stream,
                        handle: handle.clone(),
                    });
                }
                playback_state.device_initialized.store(true, Ordering::SeqCst);
                stream_handle_option = Some(handle);
            }
            Err(e) => {
                playback_state.is_playing.store(false, Ordering::SeqCst);
                return Err(format!("Failed to create output stream: {}", e));
            }
        }
    }

    let stream_handle = match stream_handle_option {
        Some(h) => h,
        None => {
            playback_state.is_playing.store(false, Ordering::SeqCst);
            return Err("Failed to get output stream handle".to_string());
        }
    };

    // Spawn thread for playback
    thread::spawn(move || {
        use rodio::{Decoder, Sink};

        // Keep the temp file alive
        let file = match File::open(path_clone) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open temp file: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        let buf_reader = BufReader::new(file);
        let source = match Decoder::new(buf_reader) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to decode base64 audio: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed creating Sink: {}", e);
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };

        sink.append(source);
        sink.sleep_until_end();

        let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
        // temp_file drops here
    });

    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: true,
        error: None,
    })
}

//
// ====== Main Tauri Entry ======
//

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    println!("Initializing audio system with correct, per-session device config");
    tauri::Builder::default()
        .manage(Arc::new(RecordingState::default()))
        .manage(Mutex::new(BackgroundRecorder::default()))
        .manage(AudioPlaybackState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Recording
            start_recording,
            stop_recording,
            is_recording,
            get_audio_data,
            set_audio_config,
            get_current_audio_config,
            get_audio_devices,
            // Playback
            play_audio,
            stop_audio,
            is_playing,
            play_audio_from_base64,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
