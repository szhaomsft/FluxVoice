import React from 'react';
import { Mic, Loader2, AlertCircle } from 'lucide-react';
import { useAppStore } from '../../store/appStore';

export const StatusIndicator: React.FC = () => {
  const { recordingState } = useAppStore();

  const getIcon = () => {
    switch (recordingState) {
      case 'recording':
        return <Mic className="w-4 h-4 text-red-500 animate-pulse" />;
      case 'processing':
        return <Loader2 className="w-4 h-4 text-blue-500 animate-spin" />;
      case 'error':
        return <AlertCircle className="w-4 h-4 text-red-500" />;
      default:
        return <Mic className="w-4 h-4 text-gray-400" />;
    }
  };

  const getLabel = () => {
    switch (recordingState) {
      case 'recording':
        return 'Recording...';
      case 'processing':
        return 'Processing...';
      case 'error':
        return 'Error';
      default:
        return 'Ready';
    }
  };

  return (
    <div className="flex items-center gap-2">
      {getIcon()}
      <span className="text-xs font-medium text-gray-700 dark:text-gray-300">{getLabel()}</span>
    </div>
  );
};
