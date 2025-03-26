import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
export function setupPlaybackListener(callback: () => void): Promise<UnlistenFn> {
  return listen<{ playback_id: string }>('audio-playback-stopped', () => {
    callback();
  });
}