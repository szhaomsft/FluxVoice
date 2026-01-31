import React, { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X } from 'lucide-react';
import { Waveform } from './Waveform';
import { StatusIndicator } from './StatusIndicator';
import { useAudioRecording } from '../../hooks/useAudioRecording';
import { useAppStore } from '../../store/appStore';

export const FloatingWindow: React.FC = () => {
  const { recordingState } = useAppStore();
  const { startRecording, stopRecording } = useAudioRecording();
  const isRecordingRef = useRef(false);

  // Sync isRecordingRef with actual recording state
  useEffect(() => {
    if (recordingState === 'idle' || recordingState === 'error') {
      isRecordingRef.current = false;
    } else if (recordingState === 'recording') {
      isRecordingRef.current = true;
    }
  }, [recordingState]);

  useEffect(() => {
    // Listen for hotkey events from backend
    const unlisten = listen('hotkey-triggered', async () => {
      console.log('Hotkey triggered, current state:', recordingState);

      if ((recordingState === 'idle' || recordingState === 'error') && !isRecordingRef.current) {
        // Start recording (also recovers from error state)
        isRecordingRef.current = true;
        await startRecording();
      } else if (recordingState === 'recording' && isRecordingRef.current) {
        // Stop recording
        isRecordingRef.current = false;
        await stopRecording();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [recordingState, startRecording, stopRecording]);

  const handleClick = async () => {
    // Open configuration window
    try {
      await invoke('open_config_window');
    } catch (err) {
      console.error('Failed to open config window:', err);
    }
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    // Only start drag on left mouse button and not on interactive elements
    if (e.button === 0 && e.detail === 1) {
      // Delay dragging slightly to allow double-click detection
      setTimeout(async () => {
        try {
          await getCurrentWindow().startDragging();
        } catch (err) {
          // Ignore errors - window may have been closed or drag cancelled
        }
      }, 150);
    }
  };

  const handleClose = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await getCurrentWindow().close();
    } catch (err) {
      console.error('Failed to close window:', err);
    }
  };

  return (
    <div
      className="w-full h-full bg-white/90 dark:bg-gray-900/90 backdrop-blur-sm rounded-xl shadow-2xl border border-gray-200 dark:border-gray-700 cursor-move relative"
      onMouseDown={handleMouseDown}
      onDoubleClick={handleClick}
    >
      <button
        onClick={handleClose}
        onMouseDown={(e) => e.stopPropagation()}
        className="absolute top-1 right-1 p-1 rounded-full hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer"
        title="Close"
      >
        <X size={14} className="text-gray-500 dark:text-gray-400" />
      </button>
      <div className="p-3 h-full flex flex-col gap-2">
        <StatusIndicator />
        {recordingState === 'recording' && <Waveform />}
      </div>
    </div>
  );
};
