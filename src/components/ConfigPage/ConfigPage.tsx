import React, { useState, useEffect } from 'react';
import { Settings, Clock, BarChart3 } from 'lucide-react';
import { Button } from '../common/Button';
import { Input } from '../common/Input';
import { Toggle } from '../common/Toggle';
import { useConfig } from '../../hooks/useConfig';
import { TranscriptionHistory } from './TranscriptionHistory';
import { UsageStats } from './UsageStats';
import type { AppConfig } from '../../types/config';

type TabType = 'settings' | 'history' | 'stats';

export const ConfigPage: React.FC = () => {
  const { config, saveConfig } = useConfig();
  const [localConfig, setLocalConfig] = useState<AppConfig | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState('');
  const [activeTab, setActiveTab] = useState<TabType>('settings');

  useEffect(() => {
    document.body.classList.add('config-page');
    return () => {
      document.body.classList.remove('config-page');
    };
  }, []);

  useEffect(() => {
    if (config) {
      setLocalConfig(config);
    }
  }, [config]);

  const handleSave = async () => {
    if (!localConfig) return;

    setIsSaving(true);
    setSaveMessage('');
    try {
      await saveConfig(localConfig);
      setSaveMessage('Configuration saved successfully!');
      setTimeout(() => setSaveMessage(''), 3000);
    } catch (err) {
      setSaveMessage('Failed to save configuration: ' + err);
    } finally {
      setIsSaving(false);
    }
  };

  if (!localConfig) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center">
        <div className="text-gray-600 dark:text-gray-400">Loading...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900" style={{ padding: '40px 48px' }}>
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-bold mb-6 text-gray-900 dark:text-white">FluxVoice</h1>

        {/* Tab Navigation */}
        <div className="flex gap-1 mb-6 border-b border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeTab === 'settings'
                ? 'border-amber-500 text-amber-600 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
            }`}
          >
            <Settings className="w-4 h-4" />
            Settings
          </button>
          <button
            onClick={() => setActiveTab('history')}
            className={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeTab === 'history'
                ? 'border-amber-500 text-amber-600 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
            }`}
          >
            <Clock className="w-4 h-4" />
            History
          </button>
          <button
            onClick={() => setActiveTab('stats')}
            className={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeTab === 'stats'
                ? 'border-amber-500 text-amber-600 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
            }`}
          >
            <BarChart3 className="w-4 h-4" />
            Stats
          </button>
        </div>

        {/* Settings Tab */}
        {activeTab === 'settings' && (
          <>
            <div className="space-y-6">
            {/* Azure Settings */}
            <div className="bg-white dark:bg-gray-800 rounded-lg shadow" style={{ padding: '24px 32px' }}>
              <h2 className="text-xl font-semibold mb-4 text-gray-900 dark:text-white">Azure Configuration</h2>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    Speech Service Key
                  </label>
                  <Input
                    type="password"
                    value={localConfig.azure.speechKey}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        azure: { ...localConfig.azure, speechKey: e.target.value },
                      })
                    }
                    placeholder="Enter your Azure Speech Service key"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    Speech Service Region
                  </label>
                  <Input
                    type="text"
                    value={localConfig.azure.speechRegion}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        azure: { ...localConfig.azure, speechRegion: e.target.value },
                      })
                    }
                    placeholder="e.g., eastus, westus"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    OpenAI Endpoint
                  </label>
                  <Input
                    type="text"
                    value={localConfig.azure.openaiEndpoint}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        azure: { ...localConfig.azure, openaiEndpoint: e.target.value },
                      })
                    }
                    placeholder="https://your-resource.openai.azure.com"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    OpenAI API Key
                  </label>
                  <Input
                    type="password"
                    value={localConfig.azure.openaiKey}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        azure: { ...localConfig.azure, openaiKey: e.target.value },
                      })
                    }
                    placeholder="Enter your Azure OpenAI key"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    OpenAI Deployment Name
                  </label>
                  <Input
                    type="text"
                    value={localConfig.azure.openaiDeployment}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        azure: { ...localConfig.azure, openaiDeployment: e.target.value },
                      })
                    }
                    placeholder="e.g., gpt-4"
                  />
                </div>
              </div>
            </div>

            {/* General Settings */}
            <div className="bg-white dark:bg-gray-800 rounded-lg shadow" style={{ padding: '24px 32px' }}>
              <h2 className="text-xl font-semibold mb-4 text-gray-900 dark:text-white">General Settings</h2>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                      Enable Text Polishing
                    </label>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                      Use Azure OpenAI to improve transcribed text
                    </p>
                  </div>
                  <Toggle
                    checked={localConfig.features.textPolishingEnabled}
                    onChange={(checked) =>
                      setLocalConfig({
                        ...localConfig,
                        features: { ...localConfig.features, textPolishingEnabled: checked },
                      })
                    }
                  />
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                      Auto-Insert Text
                    </label>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                      Automatically insert text into active window
                    </p>
                  </div>
                  <Toggle
                    checked={localConfig.features.autoInsertEnabled}
                    onChange={(checked) =>
                      setLocalConfig({
                        ...localConfig,
                        features: { ...localConfig.features, autoInsertEnabled: checked },
                      })
                    }
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    Speech Language
                  </label>
                  <Input
                    type="text"
                    value={localConfig.language.speechLanguage}
                    onChange={(e) =>
                      setLocalConfig({
                        ...localConfig,
                        language: { ...localConfig.language, speechLanguage: e.target.value },
                      })
                    }
                    placeholder="e.g., en-US, zh-CN"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">
                    Current Hotkey
                  </label>
                  <div className="px-4 py-2 bg-gray-100 dark:bg-gray-700 rounded-lg font-mono text-sm">
                    {localConfig.hotkey.modifier1}
                    {localConfig.hotkey.modifier2 && ` + ${localConfig.hotkey.modifier2}`}
                    {` + ${localConfig.hotkey.key}`}
                  </div>
                  <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    Default: Ctrl + Shift + Z (Press to activate/deactivate recording)
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* Save Button */}
          <div className="flex justify-end gap-4 mt-12 pt-8 border-t border-gray-200 dark:border-gray-700">
            {saveMessage && (
              <div
                className={`px-4 py-2 rounded-lg ${
                  saveMessage.includes('success')
                    ? 'bg-green-100 text-green-700'
                    : 'bg-red-100 text-red-700'
                }`}
              >
                {saveMessage}
              </div>
            )}
            <Button onClick={handleSave} disabled={isSaving}>
              {isSaving ? 'Saving...' : 'Save Configuration'}
            </Button>
          </div>
          </>
        )}

        {/* History Tab */}
        {activeTab === 'history' && <TranscriptionHistory />}

        {/* Stats Tab */}
        {activeTab === 'stats' && <UsageStats />}
      </div>
    </div>
  );
};
