import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';

// Disable default browser context menu
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(App);
app.use(createPinia());
app.mount('#app');
