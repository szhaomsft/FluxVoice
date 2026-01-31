import { useEffect, useRef, useCallback } from 'react';
import { useAppStore, TranscriptionHistoryItem } from '../store/appStore';

const HISTORY_STORAGE_KEY = 'fluxvoice_transcription_history';

export function useTranscriptionHistory() {
  const { transcriptionHistory, addToHistory, clearHistory: storeClearHistory } = useAppStore();
  const isInitialized = useRef(false);

  // Load history from localStorage on mount
  useEffect(() => {
    if (isInitialized.current) return;
    isInitialized.current = true;

    try {
      const stored = localStorage.getItem(HISTORY_STORAGE_KEY);
      if (stored) {
        const items: TranscriptionHistoryItem[] = JSON.parse(stored);
        // Set all items at once
        useAppStore.setState({ transcriptionHistory: items.slice(0, 20) });
      }
    } catch (err) {
      console.error('Failed to load transcription history:', err);
    }
  }, []);

  // Listen for storage changes from other windows
  useEffect(() => {
    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === HISTORY_STORAGE_KEY) {
        if (e.newValue === null) {
          // History was cleared in another window
          storeClearHistory();
        } else {
          try {
            const items: TranscriptionHistoryItem[] = JSON.parse(e.newValue);
            useAppStore.setState({ transcriptionHistory: items.slice(0, 20) });
          } catch (err) {
            console.error('Failed to parse history from storage event:', err);
          }
        }
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, [storeClearHistory]);

  // Save history to localStorage when it changes
  useEffect(() => {
    if (!isInitialized.current) return;

    try {
      localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(transcriptionHistory));
    } catch (err) {
      console.error('Failed to save transcription history:', err);
    }
  }, [transcriptionHistory]);

  // Clear history from both store and localStorage
  const clearHistory = useCallback(() => {
    localStorage.removeItem(HISTORY_STORAGE_KEY);
    storeClearHistory();
  }, [storeClearHistory]);

  return {
    transcriptionHistory,
    addToHistory,
    clearHistory,
  };
}
