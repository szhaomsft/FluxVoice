import { useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore, TranscriptionHistoryItem } from '../store/appStore';

// Backend history item type (uses snake_case)
interface BackendHistoryItem {
  original: string;
  polished: string | null;
  final_text: string;
  timestamp: number;
  audio_data: number[] | null;
}

// Convert backend format to frontend format
function fromBackend(item: BackendHistoryItem): TranscriptionHistoryItem {
  return {
    original: item.original,
    polished: item.polished,
    finalText: item.final_text,
    timestamp: item.timestamp,
    audioData: item.audio_data ?? undefined,
  };
}

// Convert frontend format to backend format
function toBackend(item: TranscriptionHistoryItem): BackendHistoryItem {
  return {
    original: item.original,
    polished: item.polished,
    final_text: item.finalText,
    timestamp: item.timestamp,
    audio_data: item.audioData ?? null,
  };
}

export function useTranscriptionHistory() {
  const { transcriptionHistory, addToHistory: storeAddToHistory, clearHistory: storeClearHistory } = useAppStore();
  const isInitialized = useRef(false);

  // Load history from backend on mount
  useEffect(() => {
    if (isInitialized.current) return;
    isInitialized.current = true;

    const loadHistory = async () => {
      try {
        const items = await invoke<BackendHistoryItem[]>('load_history');
        const frontendItems = items.map(fromBackend);
        useAppStore.setState({ transcriptionHistory: frontendItems });
        console.log(`Loaded ${items.length} history items from backend`);
      } catch (err) {
        console.error('Failed to load transcription history:', err);
      }
    };

    loadHistory();
  }, []);

  // Add to history and save to backend immediately
  const addToHistory = useCallback(async (
    original: string,
    polished: string | null,
    finalText: string,
    audioData?: number[]
  ) => {
    const timestamp = Date.now();

    const item: TranscriptionHistoryItem = {
      original,
      polished,
      finalText,
      timestamp,
      audioData,
    };

    // Add to store for immediate UI update
    storeAddToHistory(original, polished, finalText, audioData, timestamp);

    // Save to backend (writes to disk immediately)
    try {
      await invoke('save_history_item', { item: toBackend(item) });
      console.log('History item saved to backend');
    } catch (err) {
      console.error('Failed to save history item to backend:', err);
    }
  }, [storeAddToHistory]);

  // Clear history from both store and backend
  const clearHistory = useCallback(async () => {
    try {
      await invoke('clear_history');
      storeClearHistory();
      console.log('History cleared');
    } catch (err) {
      console.error('Failed to clear history:', err);
    }
  }, [storeClearHistory]);

  return {
    transcriptionHistory,
    addToHistory,
    clearHistory,
  };
}
