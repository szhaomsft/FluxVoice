import React from 'react';

export const ProcessingAnimation: React.FC = () => {
  return (
    <div className="flex items-center justify-center gap-2 flex-1">
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          className="w-2 h-2 rounded-full bg-gradient-to-r from-blue-500 to-purple-500"
          style={{
            animation: `processingBounce 1.4s ease-in-out infinite`,
            animationDelay: `${i * 0.16}s`,
          }}
        />
      ))}
      <style>{`
        @keyframes processingBounce {
          0%, 80%, 100% {
            transform: scale(0.6);
            opacity: 0.4;
          }
          40% {
            transform: scale(1.2);
            opacity: 1;
          }
        }
      `}</style>
    </div>
  );
};
