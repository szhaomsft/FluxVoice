import { useState } from 'react';
import { Clock, Copy, Check, Trash2, Sparkles } from 'lucide-react';
import { useTranscriptionHistory } from '../../hooks/useTranscriptionHistory';

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

export const TranscriptionHistory: React.FC = () => {
  const { transcriptionHistory, clearHistory } = useTranscriptionHistory();
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const handleCopy = async (text: string, key: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const handleClearHistory = () => {
    if (confirm('Clear all transcription history?')) {
      clearHistory();
    }
  };

  if (transcriptionHistory.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow" style={{ padding: '24px 32px' }}>
        <div className="text-center py-12 text-gray-500 dark:text-gray-400">
          <Clock className="w-16 h-16 mx-auto mb-4 opacity-50" />
          <p className="text-lg font-medium">No transcriptions yet</p>
          <p className="text-sm mt-2">Your recent transcriptions will appear here</p>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-white dark:bg-gray-800 rounded-lg shadow" style={{ padding: '24px 32px' }}>
      <div className="flex items-center justify-between mb-4">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {transcriptionHistory.length} transcription{transcriptionHistory.length !== 1 ? 's' : ''}
        </p>
        <button
          onClick={handleClearHistory}
          className="flex items-center gap-1 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
        >
          <Trash2 className="w-4 h-4" />
          Clear All
        </button>
      </div>
      <div className="space-y-4 max-h-[60vh] overflow-y-auto">
        {transcriptionHistory.map((item) => (
          <div
            key={item.timestamp}
            className="p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg"
          >
            {/* Original transcription */}
            <div className="group flex items-start gap-3 mb-2">
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs font-medium text-gray-500 dark:text-gray-400">Original</span>
                </div>
                <p className="text-sm text-gray-700 dark:text-gray-300 break-words">
                  {item.original}
                </p>
              </div>
              <button
                onClick={() => handleCopy(item.original, `${item.timestamp}-original`)}
                className="flex-shrink-0 p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 opacity-0 group-hover:opacity-100 transition-opacity"
                title="Copy original"
              >
                {copiedKey === `${item.timestamp}-original` ? (
                  <Check className="w-4 h-4 text-green-500" />
                ) : (
                  <Copy className="w-4 h-4" />
                )}
              </button>
            </div>

            {/* Polished transcription (if available) */}
            {item.polished && item.polished !== item.original && (
              <div className="group flex items-start gap-3 pt-2 border-t border-gray-200 dark:border-gray-600">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <Sparkles className="w-3 h-3 text-amber-500" />
                    <span className="text-xs font-medium text-amber-600 dark:text-amber-400">Polished</span>
                  </div>
                  <p className="text-sm text-gray-900 dark:text-gray-100 break-words">
                    {item.polished}
                  </p>
                </div>
                <button
                  onClick={() => handleCopy(item.polished!, `${item.timestamp}-polished`)}
                  className="flex-shrink-0 p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 opacity-0 group-hover:opacity-100 transition-opacity"
                  title="Copy polished"
                >
                  {copiedKey === `${item.timestamp}-polished` ? (
                    <Check className="w-4 h-4 text-green-500" />
                  ) : (
                    <Copy className="w-4 h-4" />
                  )}
                </button>
              </div>
            )}

            {/* Timestamp */}
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
              {formatTimestamp(item.timestamp)}
            </p>
          </div>
        ))}
      </div>
    </div>
  );
};
