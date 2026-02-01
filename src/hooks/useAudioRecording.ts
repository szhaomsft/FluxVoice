import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

// Global lock to prevent concurrent operations
let isOperationInProgress = false;

// Play a short beep sound using Web Audio API
function playStartSound() {
  try {
    const audioContext = new AudioContext();
    const oscillator = audioContext.createOscillator();
    const gainNode = audioContext.createGain();

    oscillator.connect(gainNode);
    gainNode.connect(audioContext.destination);

    oscillator.frequency.value = 880; // A5 note
    oscillator.type = 'sine';

    gainNode.gain.setValueAtTime(0.3, audioContext.currentTime);
    gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.15);

    oscillator.start(audioContext.currentTime);
    oscillator.stop(audioContext.currentTime + 0.15);
  } catch (err) {
    console.error('Failed to play start sound:', err);
  }
}

export function useAudioRecording() {
  const {
    recordingDuration,
    setRecordingState,
    setAudioLevel,
    setTranscription,
    setError,
    setUploadSize,
    setRecordingStartTime,
    setRecordingDuration,
    addToHistory,
  } = useAppStore();
  const [intervalId, setIntervalId] = useState<number | null>(null);
  const [durationIntervalId, setDurationIntervalId] = useState<number | null>(null);

  const startRecording = useCallback(async () => {
    // Prevent concurrent operations
    if (isOperationInProgress) {
      console.log('startRecording blocked - operation in progress');
      return;
    }
    isOperationInProgress = true;

    // Clear any leftover intervals from previous recording
    if (intervalId) {
      clearInterval(intervalId);
      setIntervalId(null);
    }
    if (durationIntervalId) {
      clearInterval(durationIntervalId);
      setDurationIntervalId(null);
    }

    try {
      setError(null);
      setUploadSize(null);
      setRecordingDuration(0);
      setAudioLevel(0);

      // Play start sound
      playStartSound();

      const startTime = Date.now();
      setRecordingStartTime(startTime);
      setRecordingState('recording');
      await invoke('start_recording');

      // Start polling audio level for waveform
      const id = window.setInterval(async () => {
        try {
          const level = await invoke<number>('get_audio_level');
          setAudioLevel(level);
        } catch (err) {
          console.error('Failed to get audio level:', err);
        }
      }, 50); // Update 20 times per second

      setIntervalId(id);

      // Start duration timer
      const durationId = window.setInterval(() => {
        const elapsed = Math.floor((Date.now() - startTime) / 1000);
        setRecordingDuration(elapsed);
      }, 100);

      setDurationIntervalId(durationId);
    } catch (err) {
      console.error('Failed to start recording:', err);
      setError(err as string);
      setRecordingState('idle'); // Immediately return to idle so user can retry
      setRecordingStartTime(null);
      // Clear error message after 3 seconds
      setTimeout(() => {
        setError(null);
      }, 3000);
    } finally {
      isOperationInProgress = false;
    }
  }, [intervalId, durationIntervalId, setRecordingState, setAudioLevel, setError, setUploadSize, setRecordingStartTime, setRecordingDuration]);

  const stopRecording = useCallback(async () => {
    // Prevent multiple stop calls
    if (isOperationInProgress) {
      console.log('stopRecording blocked - operation in progress');
      return;
    }
    isOperationInProgress = true;

    // Capture recording duration before clearing
    const capturedDuration = recordingDuration;

    // Always clear intervals first
    if (intervalId) {
      clearInterval(intervalId);
      setIntervalId(null);
    }
    if (durationIntervalId) {
      clearInterval(durationIntervalId);
      setDurationIntervalId(null);
    }

    setRecordingState('processing');
    setRecordingStartTime(null);

    let audioData: number[] | null = null;

    // Step 1: Stop recording (must always succeed to reset backend state)
    try {
      audioData = await invoke<number[]>('stop_recording');
      // Set upload size (audioData is array of bytes)
      if (audioData) {
        setUploadSize(audioData.length);
      }
    } catch (err) {
      console.error('Failed to stop recording:', err);
      setError(err as string);
      setRecordingState('idle'); // Immediately return to idle so user can retry
      setAudioLevel(0);
      setUploadSize(null);
      setRecordingDuration(0);
      isOperationInProgress = false;
      // Clear error message after 3 seconds
      setTimeout(() => {
        setError(null);
      }, 3000);
      return;
    }

    // Step 2: Transcribe (can fail independently)
    try {
      const result = await invoke<{
        original: string;
        polished: string | null;
        final_text: string;
      }>('transcribe_and_insert', {
        audioData,
      });

      setTranscription(result.final_text);
      // Add to history if we got a result
      if (result.final_text && result.final_text.trim()) {
        const timestamp = Date.now();
        // Update UI immediately
        addToHistory(result.original, result.polished, result.final_text, audioData ?? undefined, timestamp);
        // Save to backend (writes to disk immediately)
        try {
          await invoke('save_history_item', {
            item: {
              original: result.original,
              polished: result.polished,
              final_text: result.final_text,
              timestamp,
              audio_data: audioData ?? null,
            },
          });
          console.log('History item saved to backend');
        } catch (err) {
          console.error('Failed to save history item to backend:', err);
        }
        // Update usage stats
        try {
          await invoke('update_stats', {
            characters: result.final_text.length,
            durationSecs: capturedDuration,
          });
          console.log('Stats updated');
        } catch (err) {
          console.error('Failed to update stats:', err);
        }
      }
      setRecordingState('idle');
      setAudioLevel(0);
      setRecordingDuration(0);
    } catch (err) {
      console.error('Failed to transcribe:', err);
      setError(err as string);
      setRecordingState('idle'); // Immediately return to idle so user can retry
      setAudioLevel(0);
      setRecordingDuration(0);
      // Clear error message after 3 seconds
      setTimeout(() => {
        setError(null);
      }, 3000);
    } finally {
      isOperationInProgress = false;
    }
  }, [intervalId, durationIntervalId, recordingDuration, setRecordingState, setTranscription, setAudioLevel, setError, setUploadSize, setRecordingStartTime, setRecordingDuration, addToHistory]);

  // Reset function to clear state after error recovery
  const resetState = useCallback(() => {
    if (intervalId) {
      clearInterval(intervalId);
      setIntervalId(null);
    }
    if (durationIntervalId) {
      clearInterval(durationIntervalId);
      setDurationIntervalId(null);
    }
    setRecordingState('idle');
    setError(null);
    setAudioLevel(0);
    setRecordingDuration(0);
    setRecordingStartTime(null);
  }, [intervalId, durationIntervalId, setRecordingState, setError, setAudioLevel, setRecordingDuration, setRecordingStartTime]);

  return {
    startRecording,
    stopRecording,
    resetState,
  };
}
