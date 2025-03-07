<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  
  // Recording state
  let isRecording = $state(false);
  let audioPath = $state<string | null>(null);
  let audioSrc = $state<string | null>(null);
  let statusMessage = $state("Ready to record. Press the button to start.");
  let isLoading = $state(false);
  let recordingTime = $state(0);
  let recordingTimer: number;
  
  // Audio configuration 
  type AudioDeviceInfo = {
    name: string;
    channels: number;
    sample_rate: number;
    formats: string[];
  };
  
  type AudioConfigResponse = {
    success: boolean;
    device_name: string;
    available_devices: AudioDeviceInfo[];
    current_device: AudioDeviceInfo;
    error?: string;
  };
  
  let audioDevices = $state<AudioDeviceInfo[]>([]);
  let currentDevice = $state<AudioDeviceInfo | null>(null);
  let selectedDevice = $state<string>("");
  let selectedChannels = $state<number>(1);
  let selectedSampleRate = $state<number>(44100);
  let showSettings = $state(false);
  let isRecordButtonPressed = $state(false);
  let recordingMode = $state<'hold' | 'toggle'>('toggle');
  let theme = $state<'light' | 'dark'>('light');
  
  // Initialize theme
  onMount(() => {
    // Check system preference
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      theme = 'dark';
    }
    
    // Load audio config
    loadAudioConfig();
    
    // Check for saved theme preference
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme) {
      theme = savedTheme as 'light' | 'dark';
    }
    
    // Apply theme
    document.documentElement.setAttribute('data-theme', theme);
  });
  
  // Theme toggle
  function toggleTheme() {
    theme = theme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }
  
  // Load audio configuration
  async function loadAudioConfig() {
    try {
      isLoading = true;
      statusMessage = "Loading audio configuration...";
      
      const config = await invoke('get_audio_devices') as AudioConfigResponse;
      
      if (config.success) {
        audioDevices = config.available_devices;
        currentDevice = config.current_device;
        selectedDevice = config.device_name;
        selectedChannels = config.current_device.channels;
        selectedSampleRate = config.current_device.sample_rate;
        
        statusMessage = "Ready to record. Press the button to start.";
      } else {
        statusMessage = `Error: ${config.error || 'Failed to load audio config'}`;
      }
    } catch (error) {
      console.error("Error loading audio config:", error);
      statusMessage = `Error loading audio configuration: ${error}`;
    } finally {
      isLoading = false;
    }
  }
  
  // Recording functions
  async function startRecording() {
    try {
      isLoading = true;
      
      // Apply selected audio configuration before recording
      try {
        await invoke('set_audio_config', { 
          channels: selectedChannels, 
          sampleRate: selectedSampleRate 
        });
      } catch (configError) {
        console.error("Error setting audio config:", configError);
        statusMessage = `Error setting audio config: ${configError}`;
        isLoading = false;
        return;
      }
      
      await invoke('start_recording');
      isRecording = true;
      statusMessage = "Recording...";
      isLoading = false;
      
      // Start recording timer
      recordingTime = 0;
      recordingTimer = setInterval(() => {
        recordingTime++;
      }, 1000);
    } catch (error) {
      console.error("Error starting recording:", error);
      statusMessage = `Error starting recording: ${error}`;
      isLoading = false;
    }
  }

  async function stopRecording() {
    if (!isRecording) return;
    
    try {
      isLoading = true;
      
      // Clear recording timer
      if (recordingTimer) {
        clearInterval(recordingTimer);
      }
      
      const result = await invoke('stop_recording') as { 
        success: boolean, 
        path: string, 
        error?: string 
      };
      
      isRecording = false;
      
      if (result.success && result.path) {
        audioPath = result.path;
        statusMessage = `Recording saved (${formatTime(recordingTime)}). Ready to play.`;
        
        // Get audio data as base64
        try {
          const audioData = await invoke('get_audio_data', { path: result.path }) as {
            success: boolean,
            data?: string,
            mime_type: string,
            error?: string
          };
          
          if (audioData.success && audioData.data) {
            // Create data URL
            audioSrc = `data:${audioData.mime_type};base64,${audioData.data}`;
          } else {
            console.error("Failed to load audio data:", audioData.error);
            statusMessage = `Error loading audio: ${audioData.error}`;
          }
        } catch (err) {
          console.error("Error getting audio data:", err);
          statusMessage = `Error getting audio data: ${err}`;
        }
      } else {
        statusMessage = `Error: ${result.error || 'Unknown error'}`;
      }
      isLoading = false;
    } catch (error) {
      console.error("Error stopping recording:", error);
      statusMessage = `Error stopping recording: ${error}`;
      isRecording = false;
      isLoading = false;
    }
  }

  async function playRecording() {
    if (!audioSrc) {
      statusMessage = "No recording available.";
      return;
    }
    
    try {
      statusMessage = "Playing recording...";
      
      const audio = new Audio(audioSrc);
      
      audio.onended = () => {
        statusMessage = "Playback complete.";
      };
      
      audio.onerror = (e) => {
        console.error("Audio playback error:", e);
        statusMessage = "Error playing recording.";
      };
      
      await audio.play();
    } catch (error) {
      console.error("Error playing recording:", error);
      statusMessage = "Error playing recording.";
    }
  }

  // Format seconds as MM:SS
  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  // Handle recording button events
  function handleRecordClick() {
    if (isLoading) return;
    
    if (recordingMode === 'toggle') {
      if (isRecording) {
        stopRecording();
      } else {
        startRecording();
      }
    } else {
      // For hold mode, handled by mouse down/up
      isRecordButtonPressed = !isRecordButtonPressed;
    }
  }
  
  function handleMouseDown() {
    if (isLoading || recordingMode !== 'hold') return;
    isRecordButtonPressed = true;
    startRecording();
  }

  function handleMouseUp() {
    if (isLoading || recordingMode !== 'hold' || !isRecording) return;
    isRecordButtonPressed = false;
    stopRecording();
  }

  function handleMouseLeave() {
    if (isLoading || recordingMode !== 'hold' || !isRecording) return;
    isRecordButtonPressed = false;
    stopRecording();
  }

  function handlePlayClick() {
    if (!isLoading) {
      playRecording();
    }
  }
  
  function toggleSettings() {
    showSettings = !showSettings;
  }
  
  // Update audio settings when changed
  function handleDeviceChange() {
    // Find the device info for the selected device
    const device = audioDevices.find(d => d.name === selectedDevice);
    if (device) {
      // Update channels and sample rate based on device defaults
      selectedChannels = device.channels;
      selectedSampleRate = device.sample_rate;
    }
  }
  
  function applyAudioSettings() {
    if (isRecording) {
      statusMessage = "Cannot change audio settings while recording";
      return;
    }
    
    invoke('set_audio_config', { 
      channels: selectedChannels, 
      sampleRate: selectedSampleRate 
    }).then(() => {
      statusMessage = "Audio settings applied";
      // Refresh the displayed configuration
      loadAudioConfig();
    }).catch(error => {
      console.error("Error applying audio settings:", error);
      statusMessage = `Error: ${error}`;
    });
  }
</script>

<div class="app" data-theme={theme}>
  <main>
    <div class="top-nav">
      <button class="icon-button" on:click={toggleTheme} aria-label="Toggle theme">
        {#if theme === 'light'}
          <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.389 5.389 0 0 1-4.4 2.26 5.403 5.403 0 0 1-3.14-9.8c-.44-.06-.9-.1-1.36-.1z"/>
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5zM2 13h2c.55 0 1-.45 1-1s-.45-1-1-1H2c-.55 0-1 .45-1 1s.45 1 1 1zm18 0h2c.55 0 1-.45 1-1s-.45-1-1-1h-2c-.55 0-1 .45-1 1s.45 1 1 1zM11 2v2c0 .55.45 1 1 1s1-.45 1-1V2c0-.55-.45-1-1-1s-1 .45-1 1zm0 18v2c0 .55.45 1 1 1s1-.45 1-1v-2c0-.55-.45-1-1-1s-1 .45-1 1zM5.99 4.58a.996.996 0 0 0-1.41 0 .996.996 0 0 0 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0s.39-1.03 0-1.41L5.99 4.58zm12.37 12.37a.996.996 0 0 0-1.41 0 .996.996 0 0 0 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0a.996.996 0 0 0 0-1.41l-1.06-1.06zm1.06-10.96a.996.996 0 0 0 0-1.41.996.996 0 0 0-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06zM7.05 18.36a.996.996 0 0 0 0-1.41.996.996 0 0 0-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06z"/>
          </svg>
        {/if}
      </button>
      <button class="icon-button" on:click={toggleSettings} aria-label="Settings">
        <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
          <path d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
        </svg>
      </button>
    </div>
    
    <h1>Voice Recorder</h1>
    
    {#if currentDevice}
      <div class="subtitle" transition:fade={{ duration: 200 }}>
        {currentDevice.name} • {currentDevice.channels} ch • {currentDevice.sample_rate / 1000}kHz
      </div>
    {/if}
    
    {#if showSettings}
      <div class="settings-panel" transition:scale={{ duration: 200, start: 0.95 }}>
        <h2>Settings</h2>
        
        <div class="settings-group">
          <label>
            Recording Mode
            <div class="toggle-switch">
              <span class={recordingMode === 'toggle' ? 'active' : ''}>Toggle</span>
              <button 
                class="switch-button" 
                class:toggled={recordingMode === 'hold'}
                on:click={() => recordingMode = recordingMode === 'toggle' ? 'hold' : 'toggle'}
              >
                <span class="switch-thumb"></span>
              </button>
              <span class={recordingMode === 'hold' ? 'active' : ''}>Hold</span>
            </div>
          </label>
        </div>
        
        <div class="settings-group">
          <label for="device-select">Audio Device</label>
          <select 
            id="device-select" 
            bind:value={selectedDevice}
            on:change={handleDeviceChange}
            disabled={isRecording || audioDevices.length === 0}
          >
            {#if audioDevices.length === 0}
              <option value="">No devices found</option>
            {:else}
              {#each audioDevices as device}
                <option value={device.name}>{device.name}</option>
              {/each}
            {/if}
          </select>
        </div>
        
        <div class="settings-group">
          <label for="channels-select">Channels</label>
          <select 
            id="channels-select" 
            bind:value={selectedChannels}
            disabled={isRecording}
          >
            <option value={1}>Mono (1)</option>
            <option value={2}>Stereo (2)</option>
          </select>
        </div>
        
        <div class="settings-group">
          <label for="sample-rate-select">Sample Rate</label>
          <select 
            id="sample-rate-select" 
            bind:value={selectedSampleRate}
            disabled={isRecording}
          >
            <option value={8000}>8 kHz</option>
            <option value={16000}>16 kHz</option>
            <option value={22050}>22.05 kHz</option>
            <option value={44100}>44.1 kHz</option>
            <option value={48000}>48 kHz</option>
          </select>
        </div>
        
        <div class="settings-actions">
          <button 
            class="settings-button" 
            on:click={applyAudioSettings}
            disabled={isRecording}
          >
            Apply Settings
          </button>
        </div>
      </div>
    {/if}
    
    <div class="status-area">
      <p class="status">{statusMessage}</p>
      
      {#if isRecording}
        <div class="recording-indicator" transition:fade={{ duration: 200 }}>
          <div class="recording-dot"></div>
          <span class="recording-time">{formatTime(recordingTime)}</span>
        </div>
      {/if}
    </div>
    
    <div class="controls">
      <button 
        class="record-button" 
        class:recording={isRecording}
        class:loading={isLoading}
        class:pressed={isRecordButtonPressed}
        on:click={handleRecordClick}
        on:mousedown={handleMouseDown}
        on:mouseup={handleMouseUp}
        on:mouseleave={handleMouseLeave}
        on:touchstart={handleMouseDown}
        on:touchend={handleMouseUp}
        disabled={isLoading}
        aria-label={isRecording ? "Stop Recording" : "Start Recording"}
      >
        <div class="button-content">
          {#if isLoading}
            <div class="spinner"></div>
          {:else if isRecording}
            <svg xmlns="http://www.w3.org/2000/svg" height="32" width="32" viewBox="0 0 24 24" fill="currentColor">
              <path d="M6 6h12v12H6z"/>
            </svg>
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" height="32" width="32" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"/>
              <path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/>
            </svg>
          {/if}
          <span class="button-text">
            {#if isLoading}
              Processing...
            {:else if isRecording}
              {recordingMode === 'toggle' ? 'Stop' : 'Recording...'}
            {:else}
              {recordingMode === 'toggle' ? 'Record' : 'Hold to Record'}
            {/if}
          </span>
        </div>
      </button>
      
      {#if audioSrc}
        <button 
          class="play-button" 
          on:click={handlePlayClick}
          disabled={isLoading}
          aria-label="Play Recording"
        >
          <div class="button-content">
            <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            <span>Play</span>
          </div>
        </button>
      {/if}
    </div>
    
    {#if audioSrc}
      <div class="audio-player-container" transition:fade={{ duration: 300 }}>
        <audio controls src={audioSrc} class="audio-player"></audio>
      </div>
    {/if}
  </main>
</div>

<style>
  /* Variables for light/dark themes */
  :root {
    --bg-color: #f8f9fa;
    --card-bg-color: #ffffff;
    --text-color: #212529;
    --secondary-text-color: #6c757d;
    --primary-color: #7c3aed;
    --primary-light: #8b5cf6;
    --primary-dark: #6d28d9;
    --record-color: #ef4444;
    --record-active: #dc2626;
    --play-color: #10b981;
    --play-active: #059669;
    --border-color: #e9ecef;
    --shadow-color: rgba(0, 0, 0, 0.1);
  }

  [data-theme="dark"] {
    --bg-color: #18181b;
    --card-bg-color: #27272a;
    --text-color: #f8fafc;
    --secondary-text-color: #94a3b8;
    --primary-color: #8b5cf6;
    --primary-light: #a78bfa;
    --primary-dark: #7c3aed;
    --record-color: #ef4444;
    --record-active: #b91c1c;
    --play-color: #10b981;
    --play-active: #059669;
    --border-color: #3f3f46;
    --shadow-color: rgba(0, 0, 0, 0.25);
  }

  .app {
    background-color: var(--bg-color);
    color: var(--text-color);
    min-height: 100vh;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
      Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    transition: background-color 0.3s ease, color 0.3s ease;
  }

  main {
    max-width: 600px;
    margin: 0 auto;
    padding: 2rem 1.5rem;
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }

  .top-nav {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
  }

  .icon-button {
    background: none;
    border: none;
    color: var(--text-color);
    padding: 0.5rem;
    border-radius: 50%;
    cursor: pointer;
    transition: background-color 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon-button:hover {
    background-color: var(--shadow-color);
  }

  h1 {
    font-size: 2.25rem;
    font-weight: 700;
    margin-bottom: 0.5rem;
    text-align: center;
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 1rem;
  }

  .subtitle {
    color: var(--secondary-text-color);
    text-align: center;
    margin-bottom: 2rem;
    font-size: 0.9rem;
  }

  .settings-panel {
    background-color: var(--card-bg-color);
    border-radius: 1rem;
    padding: 1.5rem;
    margin-bottom: 2rem;
    box-shadow: 0 4px 6px var(--shadow-color);
    border: 1px solid var(--border-color);
  }

  .settings-group {
    margin-bottom: 1.25rem;
  }

  .settings-group:last-child {
    margin-bottom: 0;
  }
  
  .settings-actions {
    margin-top: 1.5rem;
    display: flex;
    justify-content: flex-end;
  }
  
  .settings-button {
    background-color: var(--primary-color);
    color: white;
    padding: 0.75rem 1.25rem;
    border-radius: 0.5rem;
  }
  
  .settings-button:hover {
    background-color: var(--primary-dark);
  }

  .settings-group label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
    color: var(--text-color);
  }

  .toggle-switch {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .toggle-switch span {
    font-size: 0.875rem;
    color: var(--secondary-text-color);
  }

  .toggle-switch span.active {
    color: var(--text-color);
    font-weight: 500;
  }

  .switch-button {
    position: relative;
    width: 3rem;
    height: 1.5rem;
    background-color: var(--border-color);
    border-radius: 1rem;
    padding: 0;
    border: none;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .switch-button.toggled {
    background-color: var(--primary-color);
  }

  .switch-thumb {
    position: absolute;
    top: 0.125rem;
    left: 0.125rem;
    width: 1.25rem;
    height: 1.25rem;
    background-color: white;
    border-radius: 50%;
    transition: transform 0.2s;
  }

  .switch-button.toggled .switch-thumb {
    transform: translateX(1.5rem);
  }

  select {
    width: 100%;
    padding: 0.75rem;
    border-radius: 0.5rem;
    border: 1px solid var(--border-color);
    background-color: var(--card-bg-color);
    color: var(--text-color);
    font-size: 1rem;
    appearance: none;
    background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%236c757d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.75rem center;
    background-size: 1rem;
  }

  select:focus {
    outline: none;
    border-color: var(--primary-color);
  }

  select:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  .status-area {
    text-align: center;
    min-height: 5rem;
    margin-bottom: 1rem;
  }

  .status {
    color: var(--secondary-text-color);
    font-size: 0.9rem;
    margin-bottom: 0.5rem;
  }

  .recording-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .recording-dot {
    width: 0.75rem;
    height: 0.75rem;
    background-color: var(--record-color);
    border-radius: 50%;
    animation: pulse 1.5s infinite;
  }

  .recording-time {
    font-family: monospace;
    font-size: 1.25rem;
    font-weight: 500;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  button {
    border: none;
    border-radius: 1rem;
    padding: 1rem;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    background-color: var(--card-bg-color);
    color: var(--text-color);
    box-shadow: 0 4px 6px var(--shadow-color);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .button-content {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
  }

  .record-button {
    background-color: var(--record-color);
    color: white;
    height: 5rem;
    border-radius: 2.5rem;
  }

  .record-button:hover {
    background-color: var(--record-active);
  }

  .record-button.recording {
    background-color: var(--record-active);
    animation: pulse-background 1.5s infinite;
  }

  .record-button.pressed {
    transform: scale(0.98);
  }

  .record-button.loading {
    background-color: var(--secondary-text-color);
  }

  .play-button {
    background-color: var(--play-color);
    color: white;
    height: 3.5rem;
    border-radius: 1.75rem;
  }

  .play-button:hover {
    background-color: var(--play-active);
  }

  .play-button:active {
    transform: scale(0.98);
  }

  .audio-player-container {
    margin-top: 1rem;
    background-color: var(--card-bg-color);
    border-radius: 1rem;
    padding: 1rem;
    box-shadow: 0 4px 6px var(--shadow-color);
    border: 1px solid var(--border-color);
  }

  .audio-player {
    width: 100%;
  }

  .spinner {
    width: 1.5rem;
    height: 1.5rem;
    border: 3px solid rgba(255, 255, 255, 0.3);
    border-radius: 50%;
    border-top-color: white;
    animation: spin 1s ease-in-out infinite;
  }

  /* Animations */
  @keyframes pulse {
    0% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
    100% {
      opacity: 1;
    }
  }

  @keyframes pulse-background {
    0% {
      background-color: var(--record-active);
    }
    50% {
      background-color: var(--record-color);
    }
    100% {
      background-color: var(--record-active);
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Media queries */
  @media (min-width: 640px) {
    main {
      padding: 3rem 2rem;
    }
  }
</style>