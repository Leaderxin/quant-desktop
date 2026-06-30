// src/ticker.ts — independent Vue app for ticker bar window
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import { getCurrentWindow } from '@tauri-apps/api/window';
import TickerBar from './components/ticker/TickerBar.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';

// Disable default browser context menu
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(TickerBar);
app.use(createPinia());
app.mount('#app');

// Hide ticker window from taskbar — called from frontend as a safety net,
// since the underlying ITaskbarList::DeleteTab COM call requires the window
// to be fully visible (already processed by the window manager) to take effect.
getCurrentWindow().setSkipTaskbar(true);
