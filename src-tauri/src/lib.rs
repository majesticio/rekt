use std::fs::File;
use std::io::{Read, Write, BufReader};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use base64::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::Serialize;
use tauri::{AppHandle, Manager, State, Emitter};
use tempfile::NamedTempFile;

// State for the recording system
#[derive(Default)]
struct RecordingState {
    is_recording: AtomicBool,
    audio_data: Mutex<Vec<i16>>,
    channels: Mutex<u16>,
    sample_rate: Mutex<u32>,
}

// State for audio playback
#[derive(Default)]
struct AudioPlaybackState {
    is_playing: AtomicBool,
    current_playback_id: Mutex<Option<String>>,
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

// We'll use a background thread to handle audio recording
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
        
        // Set up the stop flag
        (*self.stop_flag).store(false, Ordering::SeqCst);
        
        // Clone what we need for the thread
        let stop_flag = self.stop_flag.clone();
        let state = state.clone();
        
        // Create the recording thread
        let handle = thread::spawn(move || {
            println!("Recording thread started");
            
            // Set up the audio host
            let host = cpal::default_host();
            
            // Get the default input device
            let device = match host.default_input_device() {
                Some(device) => device,
                None => {
                    println!("Error: No input device available");
                    return;
                }
            };
            
            println!("Using input device: {}", device.name().unwrap_or_else(|_| "unknown".to_string()));
            
            // Get the default config for the device
            let config = match device.default_input_config() {
                Ok(config) => config,
                Err(err) => {
                    println!("Error getting default input config: {}", err);
                    return;
                }
            };
            
            println!("Using input config: {:?}", config);
            
            // Store the audio configuration in the state
            if let Ok(mut channels) = state.channels.lock() {
                *channels = config.channels();
            }
            
            if let Ok(mut sample_rate) = state.sample_rate.lock() {
                *sample_rate = config.sample_rate().0;
            }
            
            println!("Recording with {} channels at {} Hz", config.channels(), config.sample_rate().0);
            
            let err_fn = |err| {
                println!("An error occurred on the audio stream: {}", err);
            };
            
            // Create the stream
            let stream = match config.sample_format() {
                SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        // Add the data to our buffer
                        if let Ok(mut audio_data) = state.audio_data.lock() {
                            // For I16 format, we can directly use the data
                            audio_data.extend_from_slice(data);
                        }
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::U16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        // Convert to i16 and add to our buffer
                        if let Ok(mut audio_data) = state.audio_data.lock() {
                            for &sample in data {
                                // Convert u16 to i16 (offset binary conversion)
                                // u16 range is 0 to 65535, i16 range is -32768 to 32767
                                let sample = ((sample as i32) - 32768) as i16;
                                audio_data.push(sample);
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // Convert to i16 and add to our buffer
                        if let Ok(mut audio_data) = state.audio_data.lock() {
                            for &sample in data {
                                // Properly convert f32 to i16 with clamping
                                let sample = (sample.max(-1.0).min(1.0) * i16::MAX as f32) as i16;
                                audio_data.push(sample);
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                _ => {
                    println!("Unsupported sample format");
                    return;
                }
            };
            
            let stream = match stream {
                Ok(stream) => stream,
                Err(err) => {
                    println!("Error building input stream: {}", err);
                    return;
                }
            };
            
            // Start recording
            if let Err(err) = stream.play() {
                println!("Error starting audio stream: {}", err);
                return;
            }
            
            // Keep recording until the stop flag is set
            while !(*stop_flag).load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }
            
            // Stop the stream when done
            drop(stream);
            
            println!("Recording thread stopped");
        });
        
        self.join_handle = Some(handle);
        
        Ok(())
    }
    
    fn stop(&mut self) -> Result<(), String> {
        // Signal the thread to stop
        (*self.stop_flag).store(true, Ordering::SeqCst);
        
        // Wait for the thread to finish
        if let Some(handle) = self.join_handle.take() {
            match handle.join() {
                Ok(_) => Ok(()),
                Err(_) => Err("Failed to join recording thread".to_string()),
            }
        } else {
            Err("No recording in progress".to_string())
        }
    }
}

#[tauri::command]
fn start_recording(state: State<'_, Arc<RecordingState>>, recorder: State<'_, Mutex<BackgroundRecorder>>) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }

    // Clear any previous recording data
    let mut audio_data = state.audio_data.lock().unwrap();
    audio_data.clear();
    drop(audio_data); // Release the lock
    
    // Start the background recorder
    let mut recorder = recorder.lock().unwrap();
    recorder.start(state.inner().clone())?;
    
    state.is_recording.store(true, Ordering::SeqCst);
    println!("Recording started");
    
    Ok(())
}

// Helper function to get app data directory
fn get_app_data_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    // Tauri v2 returns Result directly
    let path = app_handle
        .app_handle()
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    Ok(path)
}

#[tauri::command]
async fn stop_recording(
    app_handle: AppHandle,
    state: State<'_, Arc<RecordingState>>,
    recorder: State<'_, Mutex<BackgroundRecorder>>,
) -> Result<AudioRecordingResponse, String> {
    if !state.is_recording.load(Ordering::SeqCst) {
        return Err("Not recording".to_string());
    }

    // Stop the background recorder
    {
        let mut recorder = recorder.lock().unwrap();
        recorder.stop()?;
    }
    
    state.is_recording.store(false, Ordering::SeqCst);
    println!("Recording stopped");
    
    // Get the app's data directory
    let app_dir = get_app_data_dir(&app_handle)?;

    // Create the directory if it doesn't exist
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    // Create a filename with timestamp
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("recording_{}.wav", timestamp);
    let filepath = app_dir.join(filename);

    // Get the saved audio configuration
    let channels = *state.channels.lock().unwrap();
    let sample_rate = *state.sample_rate.lock().unwrap();
    
    // Use default values if nothing was set
    let channels = if channels == 0 { 1 } else { channels };
    let sample_rate = if sample_rate == 0 { 44100 } else { sample_rate };
    
    println!("Creating WAV with {} channels at {} Hz", channels, sample_rate);

    // Create a WAV writer
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&filepath, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;

    // Write the audio data
    let audio_data = state.audio_data.lock().unwrap();
    
    // If there's no audio data, create a dummy silent file
    if audio_data.is_empty() {
        println!("No audio data recorded, creating silent file");
        // Create 1 second of silence (sample_rate * channels samples)
        for _ in 0..sample_rate * channels as u32 {
            writer.write_sample(0i16)
                .map_err(|e| format!("Failed to write audio data: {}", e))?;
        }
    } else {
        println!("Writing {} samples to WAV file", audio_data.len());
        // Write the actual audio data
        for &sample in audio_data.iter() {
            writer.write_sample(sample)
                .map_err(|e| format!("Failed to write audio data: {}", e))?;
        }
    }

    writer.finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(AudioRecordingResponse {
        success: true,
        path: Some(filepath.to_string_lossy().to_string()),
        error: None,
    })
}

#[tauri::command]
async fn get_audio_data(path: String) -> Result<AudioDataResponse, String> {
    // Read the file
    let mut file = File::open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    // Encode the data as base64
    let base64_data = BASE64_STANDARD.encode(&buffer);
    
    // Determine the MIME type based on file extension
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

#[tauri::command]
fn is_recording(state: State<'_, Arc<RecordingState>>) -> bool {
    state.is_recording.load(Ordering::SeqCst)
}

#[tauri::command]
fn is_playing(playback_state: State<'_, AudioPlaybackState>) -> bool {
    playback_state.is_playing.load(Ordering::SeqCst)
}

#[tauri::command]
fn get_audio_devices() -> Result<AudioConfigResponse, String> {
    let host = cpal::default_host();
    
    // Get available input devices
    let devices = match host.input_devices() {
        Ok(devices) => devices.collect::<Vec<_>>(),
        Err(err) => return Err(format!("Failed to get input devices: {}", err)),
    };
    
    let default_device = match host.default_input_device() {
        Some(device) => device,
        None => return Err("No default input device available".to_string()),
    };
    
    let default_name = default_device.name().unwrap_or_else(|_| "Unknown Device".to_string());
    let default_name_clone = default_name.clone();
    
    // Get the default config
    let default_config = match default_device.default_input_config() {
        Ok(config) => config,
        Err(err) => return Err(format!("Failed to get default input config: {}", err)),
    };
    
    // Get supported configs
    let supported_configs = match default_device.supported_input_configs() {
        Ok(configs) => configs.collect::<Vec<_>>(),
        Err(err) => return Err(format!("Failed to get supported input configs: {}", err)),
    };
    
    // Get formats from the supported configs
    let formats = supported_configs
        .iter()
        .map(|config| format!("{:?}", config.sample_format()))
        .collect::<Vec<_>>();
    
    // Get device info for each available device
    let available_devices = devices
        .iter()
        .filter_map(|device| {
            let name = match device.name() {
                Ok(name) => name,
                Err(_) => return None,
            };
            
            let config = match device.default_input_config() {
                Ok(config) => config,
                Err(_) => return None,
            };
            
            let supported_configs = match device.supported_input_configs() {
                Ok(configs) => configs.collect::<Vec<_>>(),
                Err(_) => return None,
            };
            
            let formats = supported_configs
                .iter()
                .map(|config| format!("{:?}", config.sample_format()))
                .collect::<Vec<_>>();
            
            Some(AudioDeviceInfo {
                name: name.clone(),
                channels: config.channels(),
                sample_rate: config.sample_rate().0,
                formats,
            })
        })
        .collect::<Vec<_>>();
    
    Ok(AudioConfigResponse {
        success: true,
        device_name: default_name,
        available_devices,
        current_device: AudioDeviceInfo {
            name: default_name_clone,
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate().0,
            formats,
        },
        error: None,
    })
}

#[tauri::command]
fn set_audio_config(state: State<'_, Arc<RecordingState>>, channels: u16, sample_rate: u32) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Cannot change audio configuration while recording".to_string());
    }
    
    // Check if parameters are valid
    if channels < 1 || channels > 2 {
        return Err("Invalid number of channels. Must be 1 or 2.".to_string());
    }
    
    let valid_sample_rates = [8000, 16000, 22050, 44100, 48000];
    if !valid_sample_rates.contains(&sample_rate) {
        return Err(format!("Invalid sample rate: {}. Must be one of {:?}", sample_rate, valid_sample_rates));
    }
    
    // Set the values
    *state.channels.lock().unwrap() = channels;
    *state.sample_rate.lock().unwrap() = sample_rate;
    
    println!("Audio config set: {} channels at {} Hz", channels, sample_rate);
    
    Ok(())
}

#[tauri::command]
fn get_current_audio_config(state: State<'_, Arc<RecordingState>>) -> Result<AudioDeviceInfo, String> {
    let host = cpal::default_host();
    
    let device = match host.default_input_device() {
        Some(device) => device,
        None => return Err("No default input device available".to_string()),
    };
    
    let name = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
    
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(err) => return Err(format!("Failed to get default input config: {}", err)),
    };
    
    // Get the current configured values
    let channels = *state.channels.lock().unwrap();
    let sample_rate = *state.sample_rate.lock().unwrap();
    
    // Use the values from state if available, otherwise use defaults
    let channels = if channels == 0 { config.channels() } else { channels };
    let sample_rate = if sample_rate == 0 { config.sample_rate().0 } else { sample_rate };
    
    println!("Current audio config: {} channels at {} Hz", channels, sample_rate);
    
    let supported_configs = match device.supported_input_configs() {
        Ok(configs) => configs.collect::<Vec<_>>(),
        Err(err) => return Err(format!("Failed to get supported input configs: {}", err)),
    };
    
    let formats = supported_configs
        .iter()
        .map(|config| format!("{:?}", config.sample_format()))
        .collect::<Vec<_>>();
    
    Ok(AudioDeviceInfo {
        name,
        channels,
        sample_rate,
        formats,
    })
}

#[tauri::command]
async fn play_audio(
    path: String,
    app_handle: AppHandle,
    playback_state: State<'_, AudioPlaybackState>,
) -> Result<AudioPlaybackResponse, String> {
    // First, stop any currently playing audio
    stop_audio_internal(&playback_state);
    
    // Generate a unique ID for this playback
    let playback_id = nanoid::nanoid!();
    *playback_state.current_playback_id.lock().unwrap() = Some(playback_id.clone());
    
    // Mark as playing
    playback_state.is_playing.store(true, Ordering::SeqCst);
    
    // Clone the path for the thread
    let path_clone = path.clone();
    let playback_id_clone = playback_id.clone();
    
    // Spawn a thread to handle audio playback
    thread::spawn(move || {
        use rodio::{Decoder, OutputStream, Sink};
        
        // Try to open the audio output stream
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(result) => result,
            Err(err) => {
                println!("Error creating audio output stream: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Try to open the file
        let file = match File::open(&path_clone) {
            Ok(file) => file,
            Err(err) => {
                println!("Failed to open audio file: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        let buf_reader = BufReader::new(file);
        
        // Create a decoder
        let source = match Decoder::new(buf_reader) {
            Ok(source) => source,
            Err(err) => {
                println!("Failed to decode audio file: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Create a sink
        let sink = match Sink::try_new(&stream_handle) {
            Ok(sink) => sink,
            Err(err) => {
                println!("Failed to create audio sink: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Add the source to the sink
        sink.append(source);
        
        // Wait for the sink to finish
        sink.sleep_until_end();
        
        // Notify that playback has completed
        let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
    });
    
    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: true,
        error: None,
    })
}

#[tauri::command]
fn stop_audio(playback_state: State<'_, AudioPlaybackState>) -> Result<AudioPlaybackResponse, String> {
    stop_audio_internal(&playback_state);
    
    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: false,
        error: None,
    })
}

// Internal helper function to stop audio
fn stop_audio_internal(playback_state: &AudioPlaybackState) {
    if playback_state.is_playing.load(Ordering::SeqCst) {
        // Reset the playback state
        playback_state.is_playing.store(false, Ordering::SeqCst);
        *playback_state.current_playback_id.lock().unwrap() = None;
        
        // We can't directly stop the audio because it's playing in another thread
        // The frontend will need to listen for the "audio-playback-stopped" event
    }
}

#[tauri::command]
async fn play_audio_from_base64(
    base64_data: String,
    mime_type: String,
    app_handle: AppHandle,
    playback_state: State<'_, AudioPlaybackState>,
) -> Result<AudioPlaybackResponse, String> {
    // First, stop any currently playing audio
    stop_audio_internal(&playback_state);
    
    // Generate a unique ID for this playback
    let playback_id = nanoid::nanoid!();
    *playback_state.current_playback_id.lock().unwrap() = Some(playback_id.clone());
    
    // Mark as playing
    playback_state.is_playing.store(true, Ordering::SeqCst);
    
    // Decode base64 data
    let audio_data = match BASE64_STANDARD.decode(base64_data.as_bytes()) {
        Ok(data) => data,
        Err(err) => return Err(format!("Failed to decode base64 data: {}", err)),
    };
    
    // Create a temporary file
    let _extension = if mime_type.contains("wav") {
        ".wav"
    } else if mime_type.contains("mp3") {
        ".mp3"
    } else {
        // Default to wav if unknown
        ".wav"
    };
    
    let mut temp_file = match NamedTempFile::new() {
        Ok(file) => file,
        Err(err) => return Err(format!("Failed to create temporary file: {}", err)),
    };
    
    // Write the audio data to the temp file
    if let Err(err) = temp_file.write_all(&audio_data) {
        return Err(format!("Failed to write to temporary file: {}", err));
    }
    
    // Get the path to the temp file (used in the thread)
    let _temp_path = temp_file.path().to_string_lossy().to_string();
    
    // Clone what we need for the thread
    let playback_id_clone = playback_id.clone();
    
    // We need to keep the temp file alive until playback is complete
    // So we move it into the thread
    thread::spawn(move || {
        use rodio::{Decoder, OutputStream, Sink};
        
        // Try to open the audio output stream
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(result) => result,
            Err(err) => {
                println!("Error creating audio output stream: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Try to open the file
        let file = match File::open(temp_file.path()) {
            Ok(file) => file,
            Err(err) => {
                println!("Failed to open temporary file: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        let buf_reader = BufReader::new(file);
        
        // Create a decoder
        let source = match Decoder::new(buf_reader) {
            Ok(source) => source,
            Err(err) => {
                println!("Failed to decode audio data: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Create a sink
        let sink = match Sink::try_new(&stream_handle) {
            Ok(sink) => sink,
            Err(err) => {
                println!("Failed to create audio sink: {}", err);
                // Notify that playback stopped
                let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
                return;
            }
        };
        
        // Add the source to the sink
        sink.append(source);
        
        // Wait for the sink to finish
        sink.sleep_until_end();
        
        // Notify that playback has completed
        let _ = app_handle.emit("audio-playback-stopped", AudioPlaybackEvent { playback_id: playback_id_clone });
        
        // temp_file will be dropped here and automatically deleted
    });
    
    Ok(AudioPlaybackResponse {
        success: true,
        is_playing: true,
        error: None,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(RecordingState::default()))
        .manage(Mutex::new(BackgroundRecorder::default()))
        .manage(AudioPlaybackState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            is_recording,
            get_audio_data,
            get_audio_devices,
            get_current_audio_config,
            set_audio_config,
            play_audio,
            stop_audio,
            play_audio_from_base64,
            is_playing,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}