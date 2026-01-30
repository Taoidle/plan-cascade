/**
 * Application Entry Point
 *
 * Initializes React and mounts the App component.
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import './i18n'; // Initialize i18n before App
import App from './App';
import './styles/globals.css';

// Initialize theme before React renders
function initializeTheme() {
  const theme = localStorage.getItem('plan-cascade-settings');
  if (theme) {
    try {
      const settings = JSON.parse(theme);
      const preferredTheme = settings.state?.theme;
      const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

      if (preferredTheme === 'dark' || (preferredTheme === 'system' && systemDark)) {
        document.documentElement.classList.add('dark');
        document.documentElement.classList.remove('light');
      } else if (preferredTheme === 'light') {
        document.documentElement.classList.add('light');
        document.documentElement.classList.remove('dark');
      }
    } catch {
      // Ignore parse errors
    }
  }
}

initializeTheme();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
