import { TrayIcon } from '@tauri-apps/api/tray';
import { startRecording, stopRecording } from './recording';

// Current default settings to use for tray recording
let deviceName = "";
let channels = 1;
let sampleRate = 44100;
let isRecording = false;

// Setup tray recording functionality
export async function setupTrayRecording(currentDevice: string, currentChannels: number, currentSampleRate: number) {
  // Store current settings to use for tray recording
  deviceName = currentDevice;
  channels = currentChannels;
  sampleRate = currentSampleRate;
  
  // Get existing tray (created in Rust)
  const tray = await TrayIcon.get();
  
  if (!tray) {
    console.error("Failed to get tray icon");
    return;
  }
  
  // Set up click event listener
  tray.onEvent((event) => {
    // Only handle click events
    if (event.type !== 'Click') return;
    
    // Check if it's the left mouse button
    if (event.button === 'left') {
      // Handle mouse down - start recording
      if (event.buttonState === 'down' && !isRecording) {
        handleTrayRecordStart();
      }
      
      // Handle mouse up - stop recording
      if (event.buttonState === 'up' && isRecording) {
        handleTrayRecordStop();
      }
    }
  });
  
  console.log("Tray recording setup completed");
}

// Start recording when tray is clicked
async function handleTrayRecordStart() {
  if (isRecording) return;
  
  try {
    // Start recording with current device settings
    await startRecording(deviceName, channels, sampleRate);
    isRecording = true;
    
    // Change tray tooltip
    const tray = await TrayIcon.get();
    if (tray) {
      await tray.setTooltip("Voice Recorder (Recording...)");
    }
    
    console.log("Tray recording started");
  } catch (error) {
    console.error("Failed to start tray recording:", error);
  }
}

// Stop recording when tray is released
async function handleTrayRecordStop() {
  if (!isRecording) return;
  
  try {
    // Stop the recording
    const result = await stopRecording();
    isRecording = false;
    
    // Change tray tooltip back
    const tray = await TrayIcon.get();
    if (tray) {
      await tray.setTooltip("Voice Recorder (Hold to Record)");
    }
    
    console.log("Tray recording stopped, saved to:", result.audioPath);
  } catch (error) {
    console.error("Failed to stop tray recording:", error);
  }
}

// Update tray recording settings when user changes them
export function updateTrayRecordingSettings(device: string, ch: number, rate: number) {
  deviceName = device;
  channels = ch;
  sampleRate = rate;
}