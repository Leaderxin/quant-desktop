<script setup lang="ts">
import { onMounted } from 'vue';
import { NConfigProvider, darkTheme, lightTheme, NMessageProvider, type GlobalThemeOverrides } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import AppLayout from '@/components/layout/AppLayout.vue';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();
const quote = useQuoteStore();

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
  await settings.fetchSettings();
  settings.applyTheme(settings.theme);
  await watchlist.fetchWatchlist();
  await quote.startListening();
});
</script>

<template>
  <NConfigProvider :theme="settings.theme === 'dark' ? darkTheme : lightTheme" :theme-overrides="themeOverrides">
    <NMessageProvider>
      <AppLayout />
    </NMessageProvider>
  </NConfigProvider>
</template>
