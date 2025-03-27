import { tray } from '@tauri-apps/api';
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
  
  try {
    // In Tauri 2, use tray.onEvent() directly
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
  } catch (error) {
    console.error("Failed to set up tray event listener:", error);
  }
}

// Start recording when tray is clicked
async function handleTrayRecordStart() {
  if (isRecording) return;
  
  try {
    // Start recording with current device settings
    await startRecording(deviceName, channels, sampleRate);
    isRecording = true;
    
    // Change tray tooltip
    try {
      await tray.setTooltip("Voice Recorder (Recording...)");
    } catch (e) {
      console.error("Failed to update tray tooltip:", e);
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
    try {
      await tray.setTooltip("Voice Recorder (Hold to Record)");
    } catch (e) {
      console.error("Failed to update tray tooltip:", e);
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