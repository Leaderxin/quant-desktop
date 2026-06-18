// src/stores/settings.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Record<string, string>>({});
  const datasources = ref<[string, string][]>([]);
  const activeDatasource = ref('sina');
  const theme = ref<'dark' | 'light'>('dark');

  async function fetchSettings() {
    try {
      settings.value = await invoke<Record<string, string>>('get_settings');
      activeDatasource.value = settings.value['active_datasource'] || 'sina';
      theme.value = (settings.value['theme'] as 'dark' | 'light') || 'dark';
      datasources.value = await invoke<[string, string][]>('list_datasources');
    } catch (e) {
      console.error('Failed to fetch settings:', e);
    }
  }

  async function setSetting(key: string, value: string) {
    try {
      await invoke('set_setting', { key, value });
      settings.value[key] = value;
    } catch (e) {
      console.error(`[settings] setSetting('${key}') failed:`, e);
    }
  }

  async function switchDatasource(name: string) {
    const previous = activeDatasource.value;
    try {
      await invoke('switch_datasource', { name });
      activeDatasource.value = name;
      settings.value['active_datasource'] = name;
      emit('datasource-changed', { datasource: name }).catch((e) => {
        console.error('[settings] Failed to emit datasource-changed:', e);
      });
    } catch (e) {
      activeDatasource.value = previous;
      console.error('[settings] switchDatasource failed:', e);
    }
  }

  async function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', theme.value);
    await setSetting('theme', theme.value);
    emit('theme-changed', { theme: theme.value }).catch((e) => {
      console.error('[settings] Failed to emit theme-changed:', e);
    });
  }

  function applyTheme(t: 'dark' | 'light') {
    theme.value = t;
    document.documentElement.setAttribute('data-theme', t);
    // NOTE: does NOT emit 'theme-changed' — only toggleTheme() broadcasts.
    // If applyTheme emitted, the ticker's theme-changed listener would call
    // applyTheme again, creating an infinite event loop between windows.
  }

  return { settings, datasources, activeDatasource, theme, fetchSettings, setSetting, switchDatasource, toggleTheme, applyTheme };
});
