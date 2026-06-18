<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
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

// Override Naive UI's default green accent with our blue
const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#58a6ff',
    primaryColorHover: '#79b8ff',
    primaryColorPressed: '#388bfd',
    primaryColorSuppl: '#58a6ff',
    infoColor: '#58a6ff',
    infoColorHover: '#79b8ff',
    infoColorPressed: '#388bfd',
    infoColorSuppl: '#58a6ff',
  },
};

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
