use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use base64::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

// State for the recording system
#[derive(Default)]
struct RecordingState {
    is_recording: AtomicBool,
    audio_data: Mutex<Vec<i16>>,
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
                                // Convert u16 to i16
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
                                // Convert f32 to i16
                                let sample = (sample * i16::MAX as f32) as i16;
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

    // Get the default audio device's sample rate
    let host = cpal::default_host();
    let sample_rate = match host.default_input_device() {
        Some(device) => match device.default_input_config() {
            Ok(config) => config.sample_rate().0,
            Err(_) => 44100, // Fallback to 44.1kHz
        },
        None => 44100, // Fallback to 44.1kHz
    };

    // Create a WAV writer
    let spec = hound::WavSpec {
        channels: 1,
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
        // Create 1 second of silence (44100 samples)
        for _ in 0..44100 {
            writer.write_sample(0i16)
                .map_err(|e| format!("Failed to write audio data: {}", e))?;
        }
    } else {
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


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(RecordingState::default()))
        .manage(Mutex::new(BackgroundRecorder::default()))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            is_recording,
            get_audio_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}