import { useState } from 'react';
import { Clock, Copy, Check, Trash2 } from 'lucide-react';
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
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const handleCopy = async (text: string, index: number) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 2000);
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
      <div className="space-y-3 max-h-[60vh] overflow-y-auto">
        {transcriptionHistory.map((item, index) => (
          <div
            key={item.timestamp}
            className="group flex items-start gap-3 p-3 bg-gray-50 dark:bg-gray-700/50 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-900 dark:text-gray-100 break-words">
                {item.text}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {formatTimestamp(item.timestamp)}
              </p>
            </div>
            <button
              onClick={() => handleCopy(item.text, index)}
              className="flex-shrink-0 p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 opacity-0 group-hover:opacity-100 transition-opacity"
              title="Copy to clipboard"
            >
              {copiedIndex === index ? (
                <Check className="w-4 h-4 text-green-500" />
              ) : (
                <Copy className="w-4 h-4" />
              )}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};
