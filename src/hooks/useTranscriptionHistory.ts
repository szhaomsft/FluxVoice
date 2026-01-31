import { useEffect, useRef } from 'react';
import { useAppStore, TranscriptionHistoryItem } from '../store/appStore';

const HISTORY_STORAGE_KEY = 'fluxvoice_transcription_history';

export function useTranscriptionHistory() {
  const { transcriptionHistory, addToHistory, clearHistory } = useAppStore();
  const isInitialized = useRef(false);

  // Load history from localStorage on mount
  useEffect(() => {
    if (isInitialized.current) return;
    isInitialized.current = true;

    try {
      const stored = localStorage.getItem(HISTORY_STORAGE_KEY);
      if (stored) {
        const items: TranscriptionHistoryItem[] = JSON.parse(stored);
        // Add items in reverse order so newest ends up first
        items.reverse().forEach((item) => {
          useAppStore.setState((state) => ({
            transcriptionHistory: [
              item,
              ...state.transcriptionHistory.filter((h) => h.timestamp !== item.timestamp),
            ].slice(0, 20),
          }));
        });
      }
    } catch (err) {
      console.error('Failed to load transcription history:', err);
    }
  }, []);

  // Save history to localStorage when it changes
  useEffect(() => {
    if (!isInitialized.current) return;

    try {
      localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(transcriptionHistory));
    } catch (err) {
      console.error('Failed to save transcription history:', err);
    }
  }, [transcriptionHistory]);

  return {
    transcriptionHistory,
    addToHistory,
    clearHistory,
  };
}
