import { useRef, useEffect } from 'react';

export function useWaveform(audioLevel: number, isRecording: boolean) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationRef = useRef<number>();
  const historyRef = useRef<number[]>([]);

  useEffect(() => {
    if (!canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const bars = 20;
    const barWidth = canvas.width / bars;

    // Initialize history if empty
    if (historyRef.current.length === 0) {
      historyRef.current = new Array(bars).fill(0);
    }

    if (!isRecording) {
      // Clear canvas when not recording
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      historyRef.current = new Array(bars).fill(0);
      return;
    }

    const animate = () => {
      // Shift history
      historyRef.current.shift();
      historyRef.current.push(audioLevel);

      // Clear canvas
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Draw bars
      historyRef.current.forEach((level, i) => {
        const height = level * canvas.height * 0.8;
        const x = i * barWidth;
        const y = (canvas.height - height) / 2;

        ctx.fillStyle = `rgba(99, 102, 241, ${0.3 + level * 0.7})`;
        ctx.fillRect(x + 2, y, barWidth - 4, height);
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [audioLevel, isRecording]);

  return canvasRef;
}
