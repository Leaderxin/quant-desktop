// 必须在 App 之前引入：日志桥要赶在应用模块图求值之前装好。
import './bootstrapLog';
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import './assets/styles/variables.css';
import './assets/styles/dark.css';
import './assets/styles/settings.css';

// Disable default browser context menu
document.addEventListener('contextmenu', (e) => e.preventDefault());

const app = createApp(App);
app.use(createPinia());
app.mount('#app');
