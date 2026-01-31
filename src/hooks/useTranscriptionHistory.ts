import { useEffect, useRef, useCallback } from 'react';
import { useAppStore, TranscriptionHistoryItem } from '../store/appStore';
import { saveAudioData, getAudioData, clearAllAudioData } from '../utils/audioStorage';

const HISTORY_STORAGE_KEY = 'fluxvoice_transcription_history';

export function useTranscriptionHistory() {
  const { transcriptionHistory, addToHistory: storeAddToHistory, clearHistory: storeClearHistory } = useAppStore();
  const isInitialized = useRef(false);

  // Load history from localStorage on mount
  useEffect(() => {
    if (isInitialized.current) return;
    isInitialized.current = true;

    const loadHistory = async () => {
      try {
        const stored = localStorage.getItem(HISTORY_STORAGE_KEY);
        if (stored) {
          const items: TranscriptionHistoryItem[] = JSON.parse(stored);

          // Load audio data from IndexedDB for each item
          const itemsWithAudio = await Promise.all(
            items.slice(0, 20).map(async (item) => {
              const audioData = await getAudioData(item.timestamp);
              return { ...item, audioData: audioData ?? undefined };
            })
          );

          useAppStore.setState({ transcriptionHistory: itemsWithAudio });
        }
      } catch (err) {
        console.error('Failed to load transcription history:', err);
      }
    };

    loadHistory();
  }, []);

  // Listen for storage changes from other windows
  useEffect(() => {
    const handleStorageChange = async (e: StorageEvent) => {
      if (e.key === HISTORY_STORAGE_KEY) {
        if (e.newValue === null) {
          // History was cleared in another window
          storeClearHistory();
        } else {
          try {
            const items: TranscriptionHistoryItem[] = JSON.parse(e.newValue);

            // Load audio data from IndexedDB for each item
            const itemsWithAudio = await Promise.all(
              items.slice(0, 20).map(async (item) => {
                const audioData = await getAudioData(item.timestamp);
                return { ...item, audioData: audioData ?? undefined };
              })
            );

            useAppStore.setState({ transcriptionHistory: itemsWithAudio });
          } catch (err) {
            console.error('Failed to parse history from storage event:', err);
          }
        }
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, [storeClearHistory]);

  // Save history to localStorage when it changes (exclude audioData, it's in IndexedDB)
  useEffect(() => {
    if (!isInitialized.current) return;

    try {
      // Strip audioData before saving to localStorage
      const historyWithoutAudio = transcriptionHistory.map(({ audioData, ...rest }) => rest);
      localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(historyWithoutAudio));
    } catch (err) {
      console.error('Failed to save transcription history:', err);
    }
  }, [transcriptionHistory]);

  // Add to history and save audio to IndexedDB
  const addToHistory = useCallback(async (
    original: string,
    polished: string | null,
    finalText: string,
    audioData?: number[]
  ) => {
    const timestamp = Date.now();

    // Save audio data to IndexedDB if present
    if (audioData && audioData.length > 0) {
      await saveAudioData(timestamp, audioData);
    }

    // Add to store (with audioData for current session)
    storeAddToHistory(original, polished, finalText, audioData, timestamp);
  }, [storeAddToHistory]);

  // Clear history from both store, localStorage, and IndexedDB
  const clearHistory = useCallback(async () => {
    await clearAllAudioData();
    localStorage.removeItem(HISTORY_STORAGE_KEY);
    storeClearHistory();
  }, [storeClearHistory]);

  return {
    transcriptionHistory,
    addToHistory,
    clearHistory,
  };
}
