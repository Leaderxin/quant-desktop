// src/ticker.ts — independent Vue app for ticker bar window
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import TickerBar from './components/ticker/TickerBar.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';

// Disable default browser context menu
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(TickerBar);
app.use(createPinia());
app.mount('#app');
