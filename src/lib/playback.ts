import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { type Unsubscribe } from '@tauri-apps/api/event';

// Start playback of audio from a file path
export async function playAudioFromPath(path: string): Promise<void> {
  const result = await invoke('play_audio', { path }) as {
    success: boolean;
    error?: string;
  };
  
  if (!result.success) {
    throw new Error(result.error || "Unknown error playing audio");
  }
}

// Start playback of audio from base64 data
export async function playAudioFromBase64(audioSrc: string): Promise<void> {
  // Extract the base64 data from the data URL
  const dataMatch = audioSrc.match(/^data:([^;]+);base64,(.+)$/);
  if (!dataMatch) {
    throw new Error("Invalid audio data URL format");
  }
  
  const mimeType = dataMatch[1];
  const base64Data = dataMatch[2];
  
  // Use native playback with base64 data
  const result = await invoke('play_audio_from_base64', { 
    base64_data: base64Data, 
    mime_type: mimeType 
  }) as {
    success: boolean;
    error?: string;
  };
  
  if (!result.success) {
    throw new Error(result.error || "Unknown error playing audio");
  }
}

// Stop audio playback
export async function stopPlayback(): Promise<void> {
  const result = await invoke('stop_audio') as {
    success: boolean;
    error?: string;
  };
  
  if (!result.success) {
    throw new Error(result.error || "Error stopping playback");
  }
}

// Set up listener for audio playback stopped event
export function setupPlaybackListener(callback: () => void): Promise<Unsubscribe> {
  return listen<{ playback_id: string }>('audio-playback-stopped', () => {
    callback();
  });
}