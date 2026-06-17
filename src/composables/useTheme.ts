// src/composables/useTheme.ts
import { ref } from 'vue';

const currentTheme = ref<'dark' | 'light'>('dark');

export function useTheme() {
  function applyTheme(theme: 'dark' | 'light') {
    currentTheme.value = theme;
    document.documentElement.setAttribute('data-theme', theme);
  }

  function toggle() {
    applyTheme(currentTheme.value === 'dark' ? 'light' : 'dark');
  }

  return { currentTheme, applyTheme, toggle };
}
