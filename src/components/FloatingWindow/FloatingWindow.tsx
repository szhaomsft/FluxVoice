import React, { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X } from 'lucide-react';
import { Waveform } from './Waveform';
import { IdleAnimation } from './IdleAnimation';
import { StatusIndicator } from './StatusIndicator';
import { useAudioRecording } from '../../hooks/useAudioRecording';
import { useAppStore } from '../../store/appStore';

export const FloatingWindow: React.FC = () => {
  const { recordingState } = useAppStore();
  const { startRecording, stopRecording } = useAudioRecording();
  const recordingStateRef = useRef(recordingState);
  const isProcessingHotkey = useRef(false);
  const pendingRelease = useRef(false);
  const lastActionTime = useRef(0); // Debounce protection

  // Keep ref in sync with state
  useEffect(() => {
    recordingStateRef.current = recordingState;

    // Reset pending release when returning to idle state
    if (recordingState === 'idle') {
      pendingRelease.current = false;
    }

    // If we have a pending release and now we're recording, stop recording
    if (pendingRelease.current && recordingState === 'recording') {
      pendingRelease.current = false;
      console.log('Processing pending release - stopping recording');
      stopRecording();
    }
  }, [recordingState, stopRecording]);

  useEffect(() => {
    // Listen for hotkey press - start recording
    const unlistenPress = listen('hotkey-pressed', async () => {
      const now = Date.now();
      // Debounce: ignore events within 300ms of last action
      if (now - lastActionTime.current < 300) {
        console.log('Hotkey press ignored - debounce');
        return;
      }

      // Prevent multiple hotkey events from being processed simultaneously
      if (isProcessingHotkey.current) {
        console.log('Hotkey press ignored - already processing');
        return;
      }

      const currentState = recordingStateRef.current;
      console.log('Hotkey pressed, current state:', currentState);

      // Only start if idle - not if processing or already recording
      if (currentState === 'idle') {
        isProcessingHotkey.current = true;
        lastActionTime.current = now;
        pendingRelease.current = false;
        try {
          await startRecording();
        } finally {
          isProcessingHotkey.current = false;
        }
      }
    });

    // Listen for hotkey release - stop recording
    const unlistenRelease = listen('hotkey-released', async () => {
      const now = Date.now();
      // Debounce: ignore events within 300ms of last action
      if (now - lastActionTime.current < 300) {
        console.log('Hotkey release ignored - debounce');
        return;
      }

      const currentState = recordingStateRef.current;
      console.log('Hotkey released, current state:', currentState);

      if (currentState === 'recording') {
        lastActionTime.current = now;
        await stopRecording();
      } else if (isProcessingHotkey.current) {
        // Recording is still starting, mark as pending release
        console.log('Release during start - marking as pending');
        pendingRelease.current = true;
      }
    });

    return () => {
      unlistenPress.then((fn) => fn());
      unlistenRelease.then((fn) => fn());
    };
  }, [startRecording, stopRecording]);

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

  const saveWindowPosition = async () => {
    try {
      const window = getCurrentWindow();
      const position = await window.outerPosition();
      await invoke('save_window_position', { x: position.x, y: position.y });
    } catch (err) {
      console.error('Failed to save window position:', err);
    }
  };

  const handleClose = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await saveWindowPosition();
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
        {recordingState === 'idle' && <IdleAnimation />}
      </div>
    </div>
  );
};
