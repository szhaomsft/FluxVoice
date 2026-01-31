import React from 'react';
import { useAppStore } from '../../store/appStore';
import { useWaveform } from '../../hooks/useWaveform';

export const Waveform: React.FC = () => {
  const { audioLevel, recordingState } = useAppStore();
  const canvasRef = useWaveform(audioLevel, recordingState === 'recording');

  return (
    <div className="w-full h-full flex items-center justify-center">
      <canvas
        ref={canvasRef}
        width={280}
        height={60}
        className="rounded-lg"
      />
    </div>
  );
};
