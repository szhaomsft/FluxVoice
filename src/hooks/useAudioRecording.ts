import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';

export function useAudioRecording() {
  const { setRecordingState, setAudioLevel, setTranscription, setError } = useAppStore();
  const [intervalId, setIntervalId] = useState<number | null>(null);

  const startRecording = useCallback(async () => {
    try {
      setError(null);
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
  }, [setRecordingState, setAudioLevel, setError]);

  const stopRecording = useCallback(async () => {
    try {
      if (intervalId) {
        clearInterval(intervalId);
        setIntervalId(null);
      }

      setRecordingState('processing');
      const audioData = await invoke<number[]>('stop_recording');

      // Send to transcription
      const result = await invoke<string>('transcribe_and_insert', {
        audioData,
      });

      setTranscription(result);
      setRecordingState('idle');
      setAudioLevel(0);
    } catch (err) {
      console.error('Failed to stop recording or transcribe:', err);
      setError(err as string);
      setRecordingState('error');
      // Auto-recover from error after 3 seconds
      setTimeout(() => {
        setRecordingState('idle');
        setError(null);
        setAudioLevel(0);
      }, 3000);
    }
  }, [intervalId, setRecordingState, setTranscription, setAudioLevel, setError]);

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
