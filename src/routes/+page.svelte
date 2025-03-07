<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  
  let isRecording = $state(false);
  let audioPath = $state<string | null>(null);
  let audioSrc = $state<string | null>(null);
  let statusMessage = $state("Ready to record. Hold button to start.");
  let isLoading = $state(false);
  let useSimulation = $state(false);

  async function startRecording() {
    try {
      isLoading = true;
      await invoke('start_recording');
      isRecording = true;
      statusMessage = "Recording... Release to stop";
      isLoading = false;
    } catch (error) {
      console.error("Error starting recording:", error);
      statusMessage = `Error starting recording: ${error}`;
      isLoading = false;
      
      // Enable simulation mode if we can't access the microphone
      useSimulation = true;
    }
  }

  async function stopRecording() {
    if (!isRecording) return;
    
    try {
      isLoading = true;
      
      // If using simulation, generate test audio
      if (useSimulation) {
        try {
          await invoke('simulate_audio_data');
        } catch (err) {
          console.error("Error simulating audio:", err);
        }
      }
      
      const result = await invoke('stop_recording') as { 
        success: boolean, 
        path: string, 
        error?: string 
      };
      
      isRecording = false;
      
      if (result.success && result.path) {
        audioPath = result.path;
        statusMessage = "Recording saved. Ready to play.";
        
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

  // Handle mouse events for recording
  function handleMouseDown() {
    if (!isLoading) {
      startRecording();
    }
  }

  function handleMouseUp() {
    if (isRecording && !isLoading) {
      stopRecording();
    }
  }

  function handleMouseLeave() {
    if (isRecording && !isLoading) {
      stopRecording();
    }
  }

  function handlePlayClick() {
    if (!isLoading) {
      playRecording();
    }
  }
</script>

<main>
  <h1>Voice Recorder</h1>
  {#if useSimulation}
    <p class="subtitle">Using simulated audio (microphone access unavailable)</p>
  {:else}
    <p class="subtitle">Using microphone for recording</p>
  {/if}
  
  <div class="controls">
    <button 
      class="record-button" 
      class:recording={isRecording}
      class:loading={isLoading}
      onmousedown={handleMouseDown}
      onmouseup={handleMouseUp}
      onmouseleave={handleMouseLeave}
      ontouchstart={handleMouseDown}
      ontouchend={handleMouseUp}
      disabled={isLoading}
    >
      {#if isLoading}
        Processing...
      {:else if isRecording}
        Recording...
      {:else}
        Hold to Record
      {/if}
    </button>
    
    <button 
      class="play-button" 
      onclick={handlePlayClick}
      disabled={!audioSrc || isLoading}
    >
      Play Recording
    </button>
  </div>
  
  <p class="status">{statusMessage}</p>
  
  {#if audioSrc}
    <audio controls src={audioSrc} class="audio-player"></audio>
  {/if}
</main>

<style>
  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 1.5;
    color: #0f0f0f;
    background-color: #f6f6f6;
  }

  main {
    max-width: 600px;
    margin: 0 auto;
    padding: 2rem;
    text-align: center;
  }

  h1 {
    margin-bottom: 0.5rem;
    font-size: 2rem;
    font-weight: 600;
  }
  
  .subtitle {
    margin-bottom: 2rem;
    color: #666;
    font-size: 0.9rem;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  button {
    border-radius: 8px;
    border: none;
    padding: 1rem;
    font-size: 1.2rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .record-button {
    background-color: #f44336;
    color: white;
    height: 120px;
    border-radius: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  }

  .record-button:active, .record-button.recording {
    background-color: #d32f2f;
    transform: scale(0.98);
  }

  .record-button.loading {
    background-color: #ff9800;
  }

  .play-button {
    background-color: #4CAF50;
    color: white;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  }

  .play-button:active {
    background-color: #388E3C;
    transform: scale(0.98);
  }

  .status {
    font-size: 1rem;
    color: #555;
    min-height: 1.5rem;
    margin-bottom: 1rem;
  }
  
  .audio-player {
    width: 100%;
    margin-top: 1rem;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: #f6f6f6;
      background-color: #2f2f2f;
    }

    .status {
      color: #aaa;
    }
    
    .subtitle {
      color: #aaa;
    }
  }
</style>