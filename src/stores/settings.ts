// src/stores/settings.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Record<string, string>>({});
  const datasources = ref<[string, string][]>([]);
  const activeDatasource = ref('eastmoney');
  const theme = ref<'dark' | 'light'>('dark');

  async function fetchSettings() {
    try {
      settings.value = await invoke<Record<string, string>>('get_settings');
      activeDatasource.value = settings.value['active_datasource'] || 'eastmoney';
      theme.value = (settings.value['theme'] as 'dark' | 'light') || 'dark';
      datasources.value = await invoke<[string, string][]>('list_datasources');
    } catch (e) {
      console.error('Failed to fetch settings:', e);
    }
  }

  async function setSetting(key: string, value: string) {
    await invoke('set_setting', { key, value });
    settings.value[key] = value;
  }

  async function switchDatasource(name: string) {
    await invoke('switch_datasource', { name });
    activeDatasource.value = name;
  }

  function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', theme.value);
    setSetting('theme', theme.value);
  }

  function applyTheme(t: 'dark' | 'light') {
    theme.value = t;
    const root = document.documentElement;
    root.setAttribute('data-theme', t);
    // Force CSS variables directly to bypass any selector issues
    if (t === 'light') {
      root.style.setProperty('--color-bg', '#ffffff');
      root.style.setProperty('--color-card-bg', '#fafafa');
      root.style.setProperty('--color-text-primary', '#1a1a1a');
      root.style.setProperty('--color-text-secondary', '#666666');
      root.style.setProperty('--color-border', '#e8e8e8');
      root.style.setProperty('--color-header-bg', '#f0f0f0');
    } else {
      root.style.setProperty('--color-bg', '#1e1e2e');
      root.style.setProperty('--color-card-bg', '#252536');
      root.style.setProperty('--color-text-primary', '#e0e0e0');
      root.style.setProperty('--color-text-secondary', '#888888');
      root.style.setProperty('--color-border', '#333344');
      root.style.setProperty('--color-header-bg', '#1a1a2e');
    }
  }

  return { settings, datasources, activeDatasource, theme, fetchSettings, setSetting, switchDatasource, toggleTheme, applyTheme };
});
