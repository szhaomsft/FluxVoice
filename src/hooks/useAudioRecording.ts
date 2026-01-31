import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

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
    try {
      setError(null);
      setUploadSize(null);
      setRecordingDuration(0);

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
      setRecordingState('error');
      setRecordingStartTime(null);
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
      }, 3000);
    }
  }, [setRecordingState, setAudioLevel, setError, setUploadSize, setRecordingStartTime, setRecordingDuration]);

  const stopRecording = useCallback(async () => {
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
      setRecordingState('error');
      setAudioLevel(0);
      setUploadSize(null);
      setRecordingDuration(0);
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
      }, 3000);
      return;
    }

    // Step 2: Transcribe (can fail independently)
    try {
      const result = await invoke<string>('transcribe_and_insert', {
        audioData,
      });

      setTranscription(result);
      // Add to history if we got a result
      if (result && result.trim()) {
        addToHistory(result);
      }
      setRecordingState('idle');
      setAudioLevel(0);
      setRecordingDuration(0);
    } catch (err) {
      console.error('Failed to transcribe:', err);
      setError(err as string);
      setRecordingState('error');
      setAudioLevel(0);
      setRecordingDuration(0);
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
      }, 3000);
    }
  }, [intervalId, durationIntervalId, setRecordingState, setTranscription, setAudioLevel, setError, setUploadSize, setRecordingStartTime, setRecordingDuration, addToHistory]);

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
