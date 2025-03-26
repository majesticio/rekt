import { invoke } from '@tauri-apps/api/core';

// Types
export type AudioDeviceInfo = {
  name: string;
  channels: number;
  sample_rate: number;
  formats: string[];
};

export type AudioConfigResponse = {
  success: boolean;
  device_name: string;
  available_devices: AudioDeviceInfo[];
  current_device: AudioDeviceInfo;
  error?: string;
};

// Format seconds as MM:SS
export function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

// Load audio configuration
export async function loadAudioConfig(): Promise<{
  audioDevices: AudioDeviceInfo[],
  selectedDevice: string,
  currentDevice: AudioDeviceInfo,
  selectedChannels: number,
  selectedSampleRate: number
}> {
  // First get device list
  const config = await invoke('get_audio_devices') as AudioConfigResponse;
  
  if (!config.success) {
    throw new Error(config.error || 'Failed to load audio config');
  }
  
  let currentDevice: AudioDeviceInfo;
  let selectedChannels: number;
  let selectedSampleRate: number;
  
  // Then get current audio settings which may include user customizations
  try {
    const currentConfig = await invoke('get_current_audio_config') as AudioDeviceInfo;
    currentDevice = currentConfig;
    selectedChannels = currentConfig.channels;
    selectedSampleRate = currentConfig.sample_rate;
  } catch (err) {
    console.error("Error getting current config:", err);
    // Fallback to device defaults
    currentDevice = config.current_device;
    selectedChannels = config.current_device.channels;
    selectedSampleRate = config.current_device.sample_rate;
  }
  
  return {
    audioDevices: config.available_devices,
    selectedDevice: config.device_name,
    currentDevice,
    selectedChannels,
    selectedSampleRate
  };
}

// Apply audio settings
export async function applyAudioSettings(channels: number, sampleRate: number): Promise<void> {
  await invoke('set_audio_config', { 
    channels, 
    sampleRate
  });
}

// Start recording
export async function startRecording(channels: number, sampleRate: number): Promise<void> {
  // Apply selected audio configuration before recording
  await applyAudioSettings(channels, sampleRate);
  await invoke('start_recording');
}

// Stop recording
export async function stopRecording(): Promise<{
  audioPath: string
}> {
  const result = await invoke('stop_recording') as { 
    success: boolean, 
    path: string, 
    error?: string 
  };
  
  if (!result.success || !result.path) {
    throw new Error(result.error || 'Unknown error');
  }
  
  return { audioPath: result.path };
}