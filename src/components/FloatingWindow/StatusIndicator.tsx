import React from 'react';
import { Mic, AlertCircle, Upload, Zap } from 'lucide-react';
import { useAppStore } from '../../store/appStore';

// Format bytes to human readable string
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export const StatusIndicator: React.FC = () => {
  const { recordingState, error, uploadSize } = useAppStore();

  const getIcon = () => {
    switch (recordingState) {
      case 'recording':
        return <Mic className="w-4 h-4 text-red-500 animate-pulse" />;
      case 'processing':
        return <Upload className="w-4 h-4 text-blue-500 animate-pulse" />;
      case 'error':
        return <AlertCircle className="w-4 h-4 text-red-500" />;
      default:
        return (
          <div className="relative">
            <Zap className="w-4 h-4 text-amber-500 animate-pulse" />
            <div className="absolute inset-0 animate-ping">
              <Zap className="w-4 h-4 text-amber-400 opacity-30" />
            </div>
          </div>
        );
    }
  };

  const getLabel = () => {
    switch (recordingState) {
      case 'recording':
        return 'Recording...';
      case 'processing':
        return uploadSize ? `Uploading ${formatBytes(uploadSize)}...` : 'Processing...';
      case 'error':
        return error ? `Error: ${error}` : 'Error';
      default:
        return (
          <span className="font-bold bg-gradient-to-r from-amber-500 via-orange-500 to-red-500 bg-clip-text text-transparent animate-pulse">
            FluxVoice
          </span>
        );
    }
  };

  return (
    <div className="flex items-center gap-2 overflow-hidden">
      <div className="flex-shrink-0">{getIcon()}</div>
      <span
        className="text-xs font-medium text-gray-700 dark:text-gray-300 truncate"
        title={recordingState === 'error' && error ? error : undefined}
      >
        {getLabel()}
      </span>
    </div>
  );
};
