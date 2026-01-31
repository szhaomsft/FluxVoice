import { useState, useCallback, useRef } from 'react';
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
  const { setRecordingState, setAudioLevel, setTranscription, setError, setUploadSize } = useAppStore();
  const [intervalId, setIntervalId] = useState<number | null>(null);

  const startRecording = useCallback(async () => {
    try {
      setError(null);
      setUploadSize(null);

      // Play start sound
      playStartSound();

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
    } catch (err) {
      console.error('Failed to start recording:', err);
      setError(err as string);
      setRecordingState('error');
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
      }, 3000);
    }
  }, [setRecordingState, setAudioLevel, setError, setUploadSize]);

  const stopRecording = useCallback(async () => {
    // Always clear interval first
    if (intervalId) {
      clearInterval(intervalId);
      setIntervalId(null);
    }

    setRecordingState('processing');

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
      setRecordingState('idle');
      setAudioLevel(0);
    } catch (err) {
      console.error('Failed to transcribe:', err);
      setError(err as string);
      setRecordingState('error');
      setAudioLevel(0);
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
      }, 3000);
    }
  }, [intervalId, setRecordingState, setTranscription, setAudioLevel, setError, setUploadSize]);

  // Reset function to clear state after error recovery
  const resetState = useCallback(() => {
    if (intervalId) {
      clearInterval(intervalId);
      setIntervalId(null);
    }
    setRecordingState('idle');
    setError(null);
    setAudioLevel(0);
  }, [intervalId, setRecordingState, setError, setAudioLevel]);

  return {
    startRecording,
    stopRecording,
    resetState,
  };
}
