import React, { useEffect, useRef, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X, Minimize2 } from 'lucide-react';
import { Waveform } from './Waveform';
import { IdleAnimation } from './IdleAnimation';
import { ProcessingAnimation } from './ProcessingAnimation';
import { StatusIndicator } from './StatusIndicator';
import { useAudioRecording } from '../../hooks/useAudioRecording';
import { useAppStore } from '../../store/appStore';
import type { AppConfig } from '../../types/config';

const MODE_CYCLE: Array<'none' | 'polish' | 'translate'> = ['none', 'polish', 'translate'];
const MODE_ICONS: Record<string, string> = {
  none: '📝',
  polish: '✏️',
  translate: '🌐',
};
const MODE_COLORS: Record<string, string> = {
  none: 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300',
  polish: 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300',
  translate: 'bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300',
};

export const FloatingWindow: React.FC = () => {
  const { recordingState, setError } = useAppStore();
  const { startRecording, stopRecording } = useAudioRecording();
  const recordingStateRef = useRef(recordingState);
  const isProcessingHotkey = useRef(false);
  const pendingRelease = useRef(false);
  const lastActionTime = useRef(0); // Debounce protection
  const [postProcessingMode, setPostProcessingMode] = useState<'none' | 'polish' | 'translate'>('none');
  const [translateTargetLanguage, setTranslateTargetLanguage] = useState<string>('English');
  const [appVersion, setAppVersion] = useState<string>('');
  const [isCompact, setIsCompact] = useState(false);
  const [isModeLoaded, setIsModeLoaded] = useState(false);
  const [isChangingMode, setIsChangingMode] = useState(false);
  const dragStart = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    invoke<boolean>('get_compact_mode')
      .then((compact) => {
        setIsCompact(compact);
        setIsModeLoaded(true);
      })
      .catch((err) => {
        console.error('Failed to load window mode:', err);
        setError(`Could not load window mode: ${String(err)}`);
      });
  }, [setError]);

  const toggleCompactMode = useCallback(async () => {
    if (!isModeLoaded || isChangingMode) return;
    setIsChangingMode(true);
    try {
      setIsCompact(await invoke<boolean>('set_compact_mode', { compact: !isCompact }));
    } catch (err) {
      console.error('Failed to change window mode:', err);
      setError(`Could not change window mode: ${String(err)}`);
    } finally {
      setIsChangingMode(false);
    }
  }, [isCompact, isModeLoaded, isChangingMode, setError]);

  // Use the native build's commit, not the current checkout's runtime HEAD.
  useEffect(() => {
    Promise.all([getVersion(), invoke<string>('get_build_commit')])
      .then(([version, commit]) => setAppVersion(`${version} (${commit})`))
      .catch((err) => console.error('Failed to load build identity:', err));
  }, []);

  // Load config on mount and whenever the window regains focus
  const loadMode = useCallback(async () => {
    try {
      const config = await invoke<AppConfig>('get_config');
      setPostProcessingMode(config.features.postProcessingMode);
      setTranslateTargetLanguage(config.features.translateTargetLanguage);
    } catch (err) {
      console.error('Failed to load config for mode:', err);
    }
  }, []);

  useEffect(() => {
    loadMode();
  }, [loadMode]);

  // Re-sync mode when window gets focus (e.g. user changed config in ConfigPage)
  useEffect(() => {
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadMode();
      }
    });
    return () => {
      unlistenFocus.then((fn) => fn());
    };
  }, [loadMode]);

  const cycleMode = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    const currentIdx = MODE_CYCLE.indexOf(postProcessingMode);
    const nextMode = MODE_CYCLE[(currentIdx + 1) % MODE_CYCLE.length];
    setPostProcessingMode(nextMode);
    try {
      const config = await invoke<AppConfig>('get_config');
      const updatedConfig = {
        ...config,
        features: { ...config.features, postProcessingMode: nextMode },
      };
      await invoke('save_config_cmd', { config: updatedConfig });
    } catch (err) {
      console.error('Failed to save mode:', err);
    }
  }, [postProcessingMode]);

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
      const currentState = recordingStateRef.current;
      console.log('Hotkey released, current state:', currentState);

      if (currentState === 'recording') {
        lastActionTime.current = Date.now();
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
    if (e.button === 0 && e.detail === 1) {
      dragStart.current = { x: e.screenX, y: e.screenY };
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    const start = dragStart.current;
    if (start && (e.buttons & 1) && Math.hypot(e.screenX - start.x, e.screenY - start.y) >= 4) {
      dragStart.current = null;
      void getCurrentWindow().startDragging().catch((err) => {
        console.error('Failed to drag floating window:', err);
        setError(`Could not move window: ${String(err)}`);
      });
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
      className={`w-full h-full bg-white/90 dark:bg-gray-900/90 backdrop-blur-sm shadow-2xl border border-gray-200 dark:border-gray-700 cursor-move relative ${isCompact ? 'rounded-full' : 'rounded-xl'}`}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={() => { dragStart.current = null; }}
      onMouseLeave={() => { dragStart.current = null; }}
      onDoubleClick={() => { void (isCompact ? toggleCompactMode() : handleClick()); }}
      onContextMenu={isCompact ? (e) => { e.preventDefault(); void handleClick(); } : undefined}
      tabIndex={isCompact ? 0 : undefined}
      role={isCompact ? 'button' : undefined}
      aria-label={isCompact ? 'Tiny FluxVoice status. Double-click or press Enter to expand.' : undefined}
      onKeyDown={isCompact ? (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          void toggleCompactMode();
        }
      } : undefined}
    >
      {isCompact ? (
        <StatusIndicator compact details={`${appVersion ? `v${appVersion}. ` : ''}Double-click or press Enter to expand. Right-click for settings. Drag to move.`} />
      ) : (
        <>
          <button
            onClick={(e) => { e.stopPropagation(); void toggleCompactMode(); }}
            onMouseDown={(e) => e.stopPropagation()}
            onDoubleClick={(e) => e.stopPropagation()}
            disabled={!isModeLoaded || isChangingMode}
            className="absolute top-1 right-7 p-1 rounded-full hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer disabled:opacity-50"
            title="Use tiny status mode"
            aria-label="Use tiny status mode"
          >
            <Minimize2 size={14} className="text-gray-500 dark:text-gray-400" />
          </button>
          <button
            onClick={handleClose}
            onMouseDown={(e) => e.stopPropagation()}
            className="absolute top-1 right-1 p-1 rounded-full hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer"
            title="Close"
          >
            <X size={14} className="text-gray-500 dark:text-gray-400" />
          </button>
          {appVersion && (
            <span className="absolute bottom-1 left-2 text-[8px] text-gray-400 dark:text-gray-600 select-none">
              v{appVersion}
            </span>
          )}
          <div className="p-3 h-full flex flex-col gap-2">
            <div className="pr-10"><StatusIndicator /></div>
            {recordingState === 'recording' && <Waveform />}
            {recordingState === 'processing' && <ProcessingAnimation />}
            {recordingState === 'idle' && <IdleAnimation />}
            {recordingState === 'idle' && (
              <div className="flex justify-center">
                <button
                  onClick={cycleMode}
                  onMouseDown={(e) => e.stopPropagation()}
                  className={`px-2 py-0.5 rounded-full text-[10px] font-medium transition-colors cursor-pointer ${MODE_COLORS[postProcessingMode]}`}
                  title={`Mode: ${postProcessingMode} (click to cycle)`}
                >
                  {MODE_ICONS[postProcessingMode]} {postProcessingMode === 'none' ? 'None' : postProcessingMode === 'polish' ? 'Polish' : translateTargetLanguage}
                </button>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
};
