import { create } from 'zustand';
import type { AppConfig, RecordingState } from '../types/config';

interface AppStore {
  // State
  recordingState: RecordingState;
  audioLevel: number;
  transcription: string;
  config: AppConfig | null;
  error: string | null;
  uploadSize: number | null; // Size of audio data being uploaded in bytes

  // Actions
  setRecordingState: (state: RecordingState) => void;
  setAudioLevel: (level: number) => void;
  setTranscription: (text: string) => void;
  setConfig: (config: AppConfig) => void;
  setError: (error: string | null) => void;
  setUploadSize: (size: number | null) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  recordingState: 'idle',
  audioLevel: 0,
  transcription: '',
  config: null,
  error: null,
  uploadSize: null,

  setRecordingState: (state) => set({ recordingState: state }),
  setAudioLevel: (level) => set({ audioLevel: level }),
  setTranscription: (text) => set({ transcription: text }),
  setConfig: (config) => set({ config }),
  setError: (error) => set({ error }),
  setUploadSize: (size) => set({ uploadSize: size }),
}));
