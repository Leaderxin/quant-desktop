<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue';
import { NConfigProvider, darkTheme, lightTheme, NMessageProvider, type GlobalThemeOverrides } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import AppLayout from '@/components/layout/AppLayout.vue';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();
const quote = useQuoteStore();

const initError = ref<string | null>(null);
const initReady = ref(false);

// Naive UI primary color overrides — match our CSS --color-accent per theme
const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const isDark = settings.theme === 'dark';
  return {
    common: {
      primaryColor: isDark ? '#58a6ff' : '#0969da',
      primaryColorHover: isDark ? '#79b8ff' : '#2180e0',
      primaryColorPressed: isDark ? '#388bfd' : '#085bb8',
      primaryColorSuppl: isDark ? '#58a6ff' : '#0969da',
      infoColor: isDark ? '#58a6ff' : '#0969da',
      infoColorHover: isDark ? '#79b8ff' : '#2180e0',
      infoColorPressed: isDark ? '#388bfd' : '#085bb8',
      infoColorSuppl: isDark ? '#58a6ff' : '#0969da',
      borderColor: isDark ? '#1e293b' : '#d0d7de',
      dividerColor: isDark ? '#1e293b' : '#d0d7de',
    },
  };
});

onMounted(async () => {
  try {
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    await watchlist.fetchWatchlist();
    await quote.startListening();
    initReady.value = true;
  } catch (e) {
    initError.value = `应用启动失败: ${String(e).slice(0, 200)}`;
    console.error('[App] init failed:', e);
  }
});

onUnmounted(() => {
  quote.stopListening();
});

function handleRetry() {
  initError.value = null;
  location.reload();
}
</script>

<template>
  <NConfigProvider :theme="settings.theme === 'dark' ? darkTheme : lightTheme" :theme-overrides="themeOverrides">
    <NMessageProvider>
      <AppLayout
        :init-error="initError"
        :init-ready="initReady"
        :quote-error="quote.error"
        @retry="handleRetry"
      />
    </NMessageProvider>
  </NConfigProvider>
</template>
