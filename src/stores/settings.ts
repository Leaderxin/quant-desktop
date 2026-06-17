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
    document.documentElement.setAttribute('data-theme', t);
  }

  return { settings, datasources, activeDatasource, theme, fetchSettings, setSetting, switchDatasource, toggleTheme, applyTheme };
});
