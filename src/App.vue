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
 * naive-ui 主题覆盖 —— 把组件库的语义色映射到本应用的 CSS 令牌上。
 *
 * 为什么必须显式覆盖「卡片/弹窗/菜单底色」这几个：naive-ui 的 darkTheme 给
 * `cardColor` / `modalColor` / `popoverColor` 的默认值是 #48484e（一块中性灰），
 * 而本应用的 surface-3 是 #252d3f（偏蓝的深色）。不覆盖的话，删除分组确认框、
 * 添加自选弹窗、右键菜单都会渲染成一块明显偏灰的板子，和周围界面不像一套东西。
 *
 * 取色依据 src/assets/styles/variables.css：弹窗/菜单用 surface-3（该文件里
 * 标注为 "Highest elevation (modals)"），正文 primary、说明文字 secondary，
 * 边框用 border-1（比页面分隔线 border-0 强一档，弹窗需要更清晰的轮廓）。
 *
 * 色值在这里是字面量而不是 var()：naive-ui 的主题覆盖在 JS 里做颜色运算
 * （derive 出 hover/pressed），CSS 变量在它那里只是不透明字符串，塞进去算不出来。
 * 现有代码里的 primaryColor 已经是这个写法，保持一致。
 */
const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const isDark = settings.theme === 'dark';
  const c = isDark
    ? {
        surface0: '#0d1117',
        surface3: '#252d3f',
        border0: '#1e293b',
        border1: '#30363d',
        textPrimary: '#e6edf3',
        textSecondary: '#8b949e',
        textTertiary: '#6e7681',
        primary: '#58a6ff',
        primaryHover: '#79b8ff',
        primaryPressed: '#388bfd',
        error: '#f85149',
        errorHover: '#ff7b72',
        errorPressed: '#d13a33',
        accentDim: 'rgba(88, 166, 255, 0.12)',
        mask: 'rgba(0, 0, 0, 0.5)',
      }
    : {
        surface0: '#ffffff',
        surface3: '#e2e5ea',
        border0: '#d0d7de',
        border1: '#c0c7cf',
        textPrimary: '#1f2328',
        textSecondary: '#656d76',
        textTertiary: '#8b949e',
        primary: '#0969da',
        primaryHover: '#2180e0',
        primaryPressed: '#085bb8',
        error: '#d1242f',
        errorHover: '#e5484d',
        errorPressed: '#a40e26',
        accentDim: 'rgba(9, 105, 218, 0.08)',
        // 浅色下遮罩要够重才能隔离背景，太浅会让弹窗与背后的表格糊在一起
        mask: 'rgba(31, 35, 40, 0.4)',
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
      // 浮层统一用最高一级表面色
      cardColor: c.surface3,
      modalColor: c.surface3,
      popoverColor: c.surface3,
      bodyColor: c.surface0,
      // 正文与标题用主文字色；弱化文字用 tertiary 而不是 naive-ui 默认的
      // 「主色叠透明度」—— 那套算法在深色底上会压到 3:1 以下
      textColorBase: c.textPrimary,
      textColor1: c.textPrimary,
      textColor2: c.textPrimary,
      textColor3: c.textTertiary,
      hoverColor: c.accentDim,
    },
    Card: {
      borderColor: c.border1,
      titleTextColor: c.textPrimary,
      textColor: c.textSecondary,
    },
    Dialog: {
      border: `1px solid ${c.border1}`,
      titleTextColor: c.textPrimary,
      // 弹窗正文是解释性文字，用 secondary 让标题保持主导
      textColor: c.textSecondary,
      iconColorError: c.error,
      iconColorWarning: c.error,
    },
    Modal: {
      maskColor: c.mask,
    },
    Popover: {
      color: c.surface3,
      textColor: c.textPrimary,
      border: `1px solid ${c.border1}`,
    },
    Dropdown: {
      color: c.surface3,
      optionTextColor: c.textPrimary,
      optionColorHover: c.accentDim,
      optionTextColorHover: c.primary,
      optionTextColorActive: c.primary,
      dividerColor: c.border1,
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
