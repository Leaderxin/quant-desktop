<script setup lang="ts">
import { onMounted } from 'vue';
import { NConfigProvider, darkTheme, lightTheme, NMessageProvider } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import AppLayout from '@/components/layout/AppLayout.vue';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();
const quote = useQuoteStore();

onMounted(async () => {
  await settings.fetchSettings();
  settings.applyTheme(settings.theme);
  await watchlist.fetchWatchlist();
  await quote.startListening();
});
</script>

<template>
  <NConfigProvider :theme="settings.theme === 'dark' ? darkTheme : lightTheme">
    <NMessageProvider>
      <AppLayout />
    </NMessageProvider>
  </NConfigProvider>
</template>
