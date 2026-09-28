<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, onErrorCaptured } from 'vue';
import { NConfigProvider, darkTheme, lightTheme, NMessageProvider, NDialogProvider, type GlobalThemeOverrides } from 'naive-ui';
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

/**
 * naive-ui 主题覆盖 —— 只映射语义色（主色/信息/错误/边框/文字），**不碰表面色**。
 *
 * cardColor / modalColor / popoverColor 有意保持 naive-ui 默认：弹窗、右键
 * 菜单、卡片和自选表的底色维持默认的中性灰/白观感，是用户 2026-09 的明确
 * 选择 —— 设置页提交(905baa4)曾把它们统一覆盖成 surface-3，看过之后被否。
 *
 * ⚠ NDataTable 的 tdColor/thColor 默认派生自 cardColor（darkTheme 默认
 * neutralCard = rgb(24,24,28)，与页面 #0d1117 近融，正是自选表一直以来的
 * 旧观感）。以后若要再覆盖 cardColor，必须同时给 DataTable 单独覆盖，否则
 * 整张自选表会跟着浮层一起换底色 —— 905baa4 的回归就是这么来的。
 *
 * 色值在这里是字面量而不是 var()：naive-ui 的主题覆盖在 JS 里做颜色运算
 * （derive 出 hover/pressed），CSS 变量在它那里只是不透明字符串，塞进去算不出来。
 * 现有代码里的 primaryColor 已经是这个写法，保持一致。
 */
const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const isDark = settings.theme === 'dark';
  const c = isDark
    ? {
        border0: '#1e293b',
        textPrimary: '#e6edf3',
        textTertiary: '#6e7681',
        primary: '#58a6ff',
        primaryHover: '#79b8ff',
        primaryPressed: '#388bfd',
        error: '#f85149',
        errorHover: '#ff7b72',
        errorPressed: '#d13a33',
        accentDim: 'rgba(88, 166, 255, 0.12)',
      }
    : {
        border0: '#d0d7de',
        textPrimary: '#1f2328',
        textTertiary: '#8b949e',
        primary: '#0969da',
        primaryHover: '#2180e0',
        primaryPressed: '#085bb8',
        error: '#d1242f',
        errorHover: '#e5484d',
        errorPressed: '#a40e26',
        accentDim: 'rgba(9, 105, 218, 0.08)',
      };

  return {
    common: {
      primaryColor: c.primary,
      primaryColorHover: c.primaryHover,
      primaryColorPressed: c.primaryPressed,
      primaryColorSuppl: c.primary,
      infoColor: c.primary,
      infoColorHover: c.primaryHover,
      infoColorPressed: c.primaryPressed,
      infoColorSuppl: c.primary,
      errorColor: c.error,
      errorColorHover: c.errorHover,
      errorColorPressed: c.errorPressed,
      errorColorSuppl: c.error,
      borderColor: c.border0,
      dividerColor: c.border0,
      // 正文与标题用主文字色；弱化文字用 tertiary 而不是 naive-ui 默认的
      // 「主色叠透明度」—— 那套算法在深色底上会压到 3:1 以下
      textColorBase: c.textPrimary,
      textColor1: c.textPrimary,
      textColor2: c.textPrimary,
      textColor3: c.textTertiary,
      hoverColor: c.accentDim,
    },
  };
});

onMounted(async () => {
  try {
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    // 涨跌配色必须在首屏渲染前落到 <html> 上，否则会先按默认的红涨绿跌画一帧，
    // 选绿涨红跌的用户能看到明显闪变。
    settings.applyColorScheme(settings.colorScheme);
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
      <!-- NDialogProvider 提供 useDialog()（自选分组删除确认用的就是它）。
           放在 NMessageProvider 内部，两者都能被 AppLayout 子树取到。 -->
      <NDialogProvider>
        <AppLayout
          :init-error="initError"
          :init-ready="initReady"
          :quote-error="quote.error"
          :app-error="appError"
          @retry="handleRetry"
          @dismiss-app-error="appError = null"
        />
      </NDialogProvider>
    </NMessageProvider>
    <UpdateDialog />
  </NConfigProvider>
</template>
