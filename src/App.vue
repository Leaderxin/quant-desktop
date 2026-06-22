<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, onErrorCaptured } from 'vue';
import { NConfigProvider, darkTheme, lightTheme, NMessageProvider, type GlobalThemeOverrides } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import { useUpdaterStore } from '@/stores/updater';
import AppLayout from '@/components/layout/AppLayout.vue';
import UpdateDialog from '@/components/updater/UpdateDialog.vue';
import { useUpdateCheck } from '@/composables/useUpdateCheck';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();
const quote = useQuoteStore();
// Initialize updater event listeners early so backend events during
// startup are not missed (Pinia stores are lazy-initialized).
useUpdaterStore().initListeners();
// Composables must be called at setup top-level (not inside lifecycle
// hooks) per Vue 3 convention — ensures hooks are registered correctly
// even if the component is activated/deactivated by <KeepAlive>.
const { performStartupCheck } = useUpdateCheck();

const initError = ref<string | null>(null);
const initReady = ref(false);
const appError = ref<string | null>(null);

// 全局错误边界 — 捕获子组件中的未处理错误，防止静默崩溃
onErrorCaptured((err, instance, info) => {
  const componentName = instance?.$?.type?.__name
    || (instance?.$ as any)?.type?.name
    || 'Unknown';
  const msg = `[${componentName}] ${String(err).slice(0, 200)}`;
  console.error('[App] onErrorCaptured:', msg, info);

  if (!appError.value) {
    appError.value = `界面错误: ${msg}`;
  }
  // 阻止错误继续传播到浏览器控制台
  return false;
});

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

    // Startup update check (non-blocking, gated by trading session)
    performStartupCheck();
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
        :app-error="appError"
        @retry="handleRetry"
        @dismiss-app-error="appError = null"
      />
    </NMessageProvider>
    <UpdateDialog />
  </NConfigProvider>
</template>
