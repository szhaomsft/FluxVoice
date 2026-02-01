import { create } from 'zustand';
import type { AppConfig, RecordingState } from '../types/config';

export interface TranscriptionHistoryItem {
  original: string;
  polished: string | null;
  finalText: string;
  timestamp: number;
  audioData?: number[]; // Opus/OGG audio data for playback
}

interface AppStore {
  // State
  recordingState: RecordingState;
  audioLevel: number;
  transcription: string;
  config: AppConfig | null;
  error: string | null;
  uploadSize: number | null; // Size of audio data being uploaded in bytes
  recordingStartTime: number | null; // Timestamp when recording started
  recordingDuration: number; // Current recording duration in seconds
  transcriptionHistory: TranscriptionHistoryItem[]; // Recent transcriptions

  // Actions
  setRecordingState: (state: RecordingState) => void;
  setAudioLevel: (level: number) => void;
  setTranscription: (text: string) => void;
  setConfig: (config: AppConfig) => void;
  setError: (error: string | null) => void;
  setUploadSize: (size: number | null) => void;
  setRecordingStartTime: (time: number | null) => void;
  setRecordingDuration: (duration: number) => void;
  addToHistory: (original: string, polished: string | null, finalText: string, audioData?: number[], timestamp?: number) => void;
  clearHistory: () => void;
}

export const useAppStore = create<AppStore>((set) => ({
  recordingState: 'idle',
  audioLevel: 0,
  transcription: '',
  config: null,
  error: null,
  uploadSize: null,
  recordingStartTime: null,
  recordingDuration: 0,
  transcriptionHistory: [],

  setRecordingState: (state) => set({ recordingState: state }),
  setAudioLevel: (level) => set({ audioLevel: level }),
  setTranscription: (text) => set({ transcription: text }),
  setConfig: (config) => set({ config }),
  setError: (error) => set({ error }),
  setUploadSize: (size) => set({ uploadSize: size }),
  setRecordingStartTime: (time) => set({ recordingStartTime: time }),
  setRecordingDuration: (duration) => set({ recordingDuration: duration }),
  addToHistory: (original, polished, finalText, audioData, timestamp) =>
    set((state) => ({
      transcriptionHistory: [
        { original, polished, finalText, timestamp: timestamp ?? Date.now(), audioData },
        ...state.transcriptionHistory,
      ],
    })),
  clearHistory: () => set({ transcriptionHistory: [] }),
}));
