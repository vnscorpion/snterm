import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { reportError } from './lib/ipc';

window.addEventListener('error', (e) => reportError(`${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener('unhandledrejection', (e) => reportError(`unhandledrejection: ${String(e.reason)}`));

// Chặn phím tắt trình duyệt (F5, Ctrl+P, Ctrl+F, Ctrl+R, zoom) như v1 tắt AreBrowserAcceleratorKeysEnabled.
window.addEventListener('keydown', (e) => {
  const k = e.key.toLowerCase();
  if (e.key === 'F5' || e.key === 'F3' || e.key === 'F7' || e.key === 'F12') { e.preventDefault(); return; }
  if (e.ctrlKey && !e.shiftKey && !e.altKey && ['p', 'f', 'r', 'g', 'j', 'h', 'o', 'u', 's', 'n', 't', '+', '-', '=', '0'].includes(k)) {
    // Ctrl+=/-/0 được xterm xử lý riêng (zoom terminal); vẫn chặn mặc định của trình duyệt.
    e.preventDefault();
  }
}, { capture: true });
window.addEventListener('contextmenu', (e) => e.preventDefault());

mount(App, { target: document.getElementById('app')! });
