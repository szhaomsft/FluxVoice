import React, { useEffect } from 'react';
import { FloatingWindow } from './components/FloatingWindow/FloatingWindow';
import { ConfigPage } from './components/ConfigPage/ConfigPage';
import { useConfig } from './hooks/useConfig';
import './App.css';

function App() {
  const { config } = useConfig();

  useEffect(() => {
    // Apply theme
    if (config?.ui.theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [config?.ui.theme]);

  // Determine which view to show based on URL
  const isConfigPage = window.location.pathname.includes('/config');

  return (
    <div className="w-full h-full">
      {isConfigPage ? <ConfigPage /> : <FloatingWindow />}
    </div>
  );
}

export default App;
