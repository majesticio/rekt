<!-- Svelte component -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  
  // Import our extracted functionality
  import { 
    type AudioDeviceInfo, 
    formatTime, 
    loadAudioConfig, 
    applyAudioSettings as applySettings,
    startRecording as startRec,
    stopRecording as stopRec
  } from '$lib/recording';
  
  import {
    playAudioFromPath,
    stopPlayback as stopAudio,
    setupPlaybackListener
  } from '$lib/playback';
  
  // Recording state
  let isRecording = $state(false);
  let audioPath = $state<string | null>(null);
  let statusMessage = $state("Ready to record. Press the button to start.");
  let isLoading = $state(false);
  let recordingTime = $state(0);
  let recordingTimer: number;
  
  // Audio configuration
  let audioDevices = $state<AudioDeviceInfo[]>([]);
  let currentDevice = $state<AudioDeviceInfo | null>(null);
  let selectedDevice = $state<string>("");
  let selectedChannels = $state<number>(1);
  let selectedSampleRate = $state<number>(44100);
  let showSettings = $state(false);
  let isRecordButtonPressed = $state(false);
  let recordingMode = $state<'hold' | 'toggle'>('toggle');
  let theme = $state<'light' | 'dark'>('light');
  let isPlaying = $state(false);
  
  // Initialize theme and listeners
  onMount(() => {
    // Check system preference
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      theme = 'dark';
    }
    
    // Load audio config
    loadAudioConfiguration();
    
    // Check for saved theme preference
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme) {
      theme = savedTheme as 'light' | 'dark';
    }
    
    // Apply theme
    document.documentElement.setAttribute('data-theme', theme);
    
    // Listen for audio playback events from the backend
    const unlistenPromise = setupPlaybackListener(() => {
      isPlaying = false;
      statusMessage = "Playback complete.";
    });
    
    // Cleanup listener on component unmount
    return () => {
      unlistenPromise.then(unlistenFn => unlistenFn());
    };
  });
  
  // Theme toggle
  function toggleTheme() {
    theme = theme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }
  
  // Load audio configuration
  async function loadAudioConfiguration() {
    try {
      isLoading = true;
      statusMessage = "Loading audio configuration...";
      
      const config = await loadAudioConfig();
      audioDevices = config.audioDevices;
      selectedDevice = config.selectedDevice;
      currentDevice = config.currentDevice;
      selectedChannels = config.selectedChannels;
      selectedSampleRate = config.selectedSampleRate;
      
      statusMessage = "Ready to record. Press the button to start.";
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
      
      await startRec(selectedChannels, selectedSampleRate);
      isRecording = true;
      statusMessage = "Recording...";
      
      // Start recording timer
      recordingTime = 0;
      recordingTimer = setInterval(() => {
        recordingTime++;
      }, 1000);
    } catch (error) {
      console.error("Error starting recording:", error);
      statusMessage = `Error starting recording: ${error}`;
    } finally {
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
      
      const result = await stopRec();
      
      isRecording = false;
      audioPath = result.audioPath;
      statusMessage = `Recording saved (${formatTime(recordingTime)}). Ready to play.`;
    } catch (error) {
      console.error("Error stopping recording:", error);
      statusMessage = `Error stopping recording: ${error}`;
      isRecording = false;
    } finally {
      isLoading = false;
    }
  }

  async function playRecording() {
    if (!audioPath) {
      statusMessage = "No recording available.";
      return;
    }
    
    try {
      statusMessage = "Playing recording...";
      isPlaying = true;
      await playAudioFromPath(audioPath);
    } catch (error) {
      isPlaying = false;
      console.error("Error playing recording:", error);
      statusMessage = `Error playing recording: ${error}`;
    }
  }
  
  async function stopPlayback() {
    try {
      await stopAudio();
      isPlaying = false;
      statusMessage = "Playback stopped.";
    } catch (error) {
      isPlaying = false;
      console.error("Error stopping playback:", error);
      statusMessage = `Error stopping playback: ${error}`;
    }
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
      if (isPlaying) {
        stopPlayback();
      } else {
        playRecording();
      }
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
    
    applySettings(selectedChannels, selectedSampleRate)
      .then(() => {
        statusMessage = "Audio settings applied";
        // Update the current device display but don't reload audio config 
        // which would reset to device defaults
        if (currentDevice) {
          currentDevice = {
            ...currentDevice,
            channels: selectedChannels,
            sample_rate: selectedSampleRate
          };
        }
      })
      .catch(error => {
        console.error("Error applying audio settings:", error);
        statusMessage = `Error: ${error}`;
      });
  }
</script>

<div class="app" data-theme={theme}>
  <main>
    <div class="top-nav">
      <button class="icon-button" onclick={toggleTheme} aria-label="Toggle theme">
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
      <button class="icon-button" onclick={toggleSettings} aria-label="Settings">
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
                onclick={() => recordingMode = recordingMode === 'toggle' ? 'hold' : 'toggle'}
                aria-label="Toggle recording mode"
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
            onchange={handleDeviceChange}
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
            onclick={applyAudioSettings}
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
        onclick={handleRecordClick}
        onmousedown={handleMouseDown}
        onmouseup={handleMouseUp}
        onmouseleave={handleMouseLeave}
        ontouchstart={handleMouseDown}
        ontouchend={handleMouseUp}
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
      
      {#if audioPath}
        <button 
          class="play-button" 
          class:playing={isPlaying}
          onclick={handlePlayClick}
          disabled={isLoading}
          aria-label={isPlaying ? "Stop Playback" : "Play Recording"}
        >
          <div class="button-content">
            {#if isPlaying}
              <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
                <path d="M6 6h4v12H6zm8 0h4v12h-4z"/>
              </svg>
              <span>Stop</span>
            {:else}
              <svg xmlns="http://www.w3.org/2000/svg" height="24" width="24" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z"/>
              </svg>
              <span>Play</span>
            {/if}
          </div>
        </button>
      {/if}
    </div>
    
  </main>
</div>

<!-- Styles are now imported from src/styles/app.css in the layout file -->