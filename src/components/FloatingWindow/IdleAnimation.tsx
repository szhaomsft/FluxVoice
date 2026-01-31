import React from 'react';

export const IdleAnimation: React.FC = () => {
  return (
    <div className="flex items-center justify-center gap-1.5 flex-1">
      {[...Array(7)].map((_, i) => (
        <div
          key={i}
          className="w-1.5 bg-gradient-to-t from-amber-500 to-orange-400 rounded-full"
          style={{
            animation: `idleWave 1.5s ease-in-out infinite`,
            animationDelay: `${i * 0.12}s`,
            height: '12px',
          }}
        />
      ))}
      <style>{`
        @keyframes idleWave {
          0%, 100% {
            height: 12px;
            opacity: 0.5;
          }
          50% {
            height: 36px;
            opacity: 1;
          }
        }
      `}</style>
    </div>
  );
};
