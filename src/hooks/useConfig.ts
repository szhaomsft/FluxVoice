import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store/appStore';
import type { AppConfig } from '../types/config';

export function useConfig() {
  const { config, setConfig } = useAppStore();

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const cfg = await invoke<AppConfig>('get_config');
      setConfig(cfg);
    } catch (err) {
      console.error('Failed to load config:', err);
    }
  };

  const saveConfig = async (newConfig: AppConfig) => {
    try {
      await invoke('save_config_cmd', { config: newConfig });
      setConfig(newConfig);
    } catch (err) {
      console.error('Failed to save config:', err);
      throw err;
    }
  };

  return {
    config,
    loadConfig,
    saveConfig,
  };
}
