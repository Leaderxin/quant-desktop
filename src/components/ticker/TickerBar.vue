<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';
import { useSettingsStore } from '@/stores/settings';
import { useTickerWindowHeight } from '@/composables/useTickerWindowHeight';
import { formatPrice } from '@/utils/format';
import { nextPageStart, windowItems } from '@/utils/paging';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const settings = useSettingsStore();

// 行情条窗口高度跟着内容走（见 composable 注释），避免系统缩放切换后高度不匹配。
// 初始加载完成（或失败）前内容是「暂无自选」占位行，量它会把窗口先压扁再
// 弹回（启动时 40→24→39 的跳变）；ready 之前 composable 不动窗口。
const tickerContent = ref<HTMLElement | null>(null);
const heightReady = ref(false);
useTickerWindowHeight(tickerContent, { ready: heightReady });

const paused = ref(false);
const page = ref(0);
let cycleTimer: ReturnType<typeof setInterval> | null = null;
let unlistenTheme: UnlistenFn | null = null;
let unlistenDatasource: UnlistenFn | null = null;
let unlistenWatchlist: UnlistenFn | null = null;
let unlistenSettings: UnlistenFn | null = null;
/** 当前手势挂出去的监听清理函数，见 onMouseDown。 */
let stopMouseTracking: (() => void) | null = null;
let openErrorTimer: ReturnType<typeof setTimeout> | null = null;

const initFailed = ref(false);

/**
 * 「打开主窗口」失败。行情条会短暂变成错误行，几秒后自动退回行情显示 ——
 * 常驻不清的话，一次偶发的 IPC 失败会把行情条永久占成错误页。
 */
const openError = ref(false);
const OPEN_ERROR_MS = 5000;

/** 每屏展示几只。设置页可配，默认 2。 */
const perPage = computed(() => settings.tickerItemsPerPage);

/**
 * 透明背景是窗口级的视觉效果，而底板画在 body 上（见 ticker.html），
 * 不在 Vue 组件里 —— 所以开关要落到 document.body 的 class 上。
 */
watch(
  () => settings.tickerTransparent,
  (v) => {
    document.body.classList.toggle('ticker-transparent', v);
  },
  { immediate: true },
);

watch(
  () => settings.colorScheme,
  (v) => {
    settings.applyColorScheme(v);
  },
  { immediate: true },
);

onMounted(async () => {
  try {
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    settings.applyColorScheme(settings.colorScheme);
    await watchlist.fetchWatchlist();
    await quoteStore.startListening();
    startCycle();
    startThemeListen();
    startDatasourceListen();
    startWatchlistListener();
    startSettingsListen();
    heightReady.value = true;
  } catch (e) {
    initFailed.value = true;
    // 失败态（错误行）也是终态，按它设窗口高度没问题
    heightReady.value = true;
    console.error('[TickerBar] init failed:', e);
  }
});

onUnmounted(() => {
  stopMouseTracking?.();
  clearOpenError();
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
  if (unlistenTheme) unlistenTheme();
  if (unlistenDatasource) unlistenDatasource();
  if (unlistenWatchlist) unlistenWatchlist();
  if (unlistenSettings) unlistenSettings();
});

function startWatchlistListener() {
  listen('watchlist-changed', () => {
    watchlist.fetchWatchlist().catch((e) => {
      console.error('[TickerBar] watchlist-changed refresh failed:', e);
    });
  }).then((unlisten) => {
    unlistenWatchlist = unlisten;
  }).catch((e) => {
    console.error('[TickerBar] Failed to listen watchlist-changed:', e);
  });
}

/**
 * 监听主窗口播出的设置变更。
 *
 * 按 payload 就地更新而不是重拉整份设置：这个窗口读不到主窗口的 store，
 * 而 `fetchSettings()` 会连带拉数据源列表、指数池、自启状态等 5 个命令 ——
 * 为一个轮播条数开关付这个代价不值得。
 */
function startSettingsListen() {
  listen<{ key: string; value: string }>('settings-changed', (event) => {
    settings.applyRemoteSetting(event.payload.key, event.payload.value);
  }).then((unlisten) => {
    unlistenSettings = unlisten;
  }).catch((e) => {
    console.error('[TickerBar] Failed to listen settings-changed:', e);
  });
}

function startThemeListen() {
  listen<{ theme: string }>('theme-changed', (event) => {
    const t = event.payload.theme as 'dark' | 'light';
    settings.applyTheme(t);
  }).then((unlisten) => {
    unlistenTheme = unlisten;
  }).catch((e) => {
    console.error('[TickerBar] Failed to listen theme-changed:', e);
  });
}

function startDatasourceListen() {
  listen<{ datasource: string }>('datasource-changed', (event) => {
    settings.activeDatasource = event.payload.datasource;
  }).then((unlisten) => {
    unlistenDatasource = unlisten;
  }).catch((e) => {
    console.error('[TickerBar] Failed to listen datasource-changed:', e);
  });
}

function startCycle() {
  cycleTimer = setInterval(() => {
    if (paused.value) return;
    // nextPageStart 在「一屏装得下」时归零，所以短列表不会每隔 3 秒把同一批
    // 数据重排一次（那在界面上是无意义的抖动）。
    page.value = nextPageStart(page.value, tickerItems.value.length, perPage.value);
  }, 3000);
}

/**
 * 播报列表。顺序取自 store 的 `tickerItems`（按 `ticker_order` 排）——
 * 轮播顺序由设置页独立维护，与自选表的组内顺序无关。
 */
const tickerItems = computed(() =>
  watchlist.tickerItems.map((item) => {
    const q = quoteStore.getQuote(item.code, item.market);
    return {
      name: item.name,
      code: item.code,
      price: q?.price ?? null,
      changePct: q?.change_pct ?? null,
    };
  })
);

// 可见集合变化时回到第一屏，避免列表变短后观众从半截开始看。
//
// 必须监听 length 而不是 tickerItems 本身：tickerItems 依赖 quote store，
// 行情每次轮询都会重算，直接监听它会每 2 秒重置一次 page，翻页将永远
// 停在第一屏。
//
// perPage 也要一起监听：每屏条数变大后当前 page 可能已越过列表末尾，
// 虽然 visibleItems 里取模不会越界，但会让人看到半截窗口。
watch(
  [() => tickerItems.value.length, perPage],
  () => {
    page.value = 0;
  }
);

const visibleItems = computed(() =>
  windowItems(tickerItems.value, page.value, perPage.value),
);

const retryHintVisible = ref(false);

// ── Dragging ──
// Uses Tauri's startDragging() API (Win32 DefWindowProc) for smooth
// OS-level window dragging on both Windows 10 and 11.
// Position is auto-saved by the Rust WindowEvent::Moved handler in lib.rs.
//
// 点击 / 拖动靠按下后的位移区分：
// - 点击(位移 ≤ 阈值)：@click 照常触发 → 打开主窗口
// - 拖动(位移 > 阈值)：startDragging() 走 Win32 模态拖拽循环，它把 mouseup
//   吃掉，所以 @click 不会触发。
//
// ⚠ 关键在于「mouseup 被吃掉」不止发生在拖动之后：窗口失焦、或 OS 抢走
// mouseup，都会让挂在 document 上的 mousemove 留在原地。而它闭包里存的是
// 上一次按下的起点，于是用户只是把鼠标移到行情条上悬停(buttons 为 0)，
// 就会拿陈旧的起点算出一个巨大位移、误判成拖动并调用 startDragging() ——
// 窗口无端跳一下，紧跟着的那次点击也进不了 handleClick。
// 所以下面有三道收尾：按下前先收上一次的、事件里发现按键已松开就收、失焦也收。
/** 判定为拖动的最小位移(CSS px)。留点余量：手抖 4~5px 是常事，按 3px 判会
 *  把正常点击吃掉，主窗口反而打不开。 */
const DRAG_THRESHOLD_PX = 6;

let isDragging = false;

function onMouseDown(e: MouseEvent) {
  // 中键/右键既不拖动也不打开主窗口，别为它们挂监听。
  if (e.button !== 0) return;
  // 上一次手势没收到 mouseup 的话，它的监听还挂着 —— 先收干净再挂新的，
  // 否则两份并存，旧的那份会拿旧起点抢先判定。
  stopMouseTracking?.();
  isDragging = false;
  if (initFailed.value) {
    return;
  }
  const startX = e.clientX;
  const startY = e.clientY;

  function cleanup() {
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', cleanup);
    window.removeEventListener('blur', cleanup);
    stopMouseTracking = null;
  }

  function onMouseMove(ev: MouseEvent) {
    // 左键已经松开却还收到 mousemove —— 那次 mouseup 被吞了，现在这只是
    // 悬停产生的事件。不拦住它，下面就会拿上次的起点误判成拖动。
    if (!(ev.buttons & 1)) { cleanup(); return; }
    if (isDragging) return;
    if (
      Math.abs(ev.clientX - startX) > DRAG_THRESHOLD_PX ||
      Math.abs(ev.clientY - startY) > DRAG_THRESHOLD_PX
    ) {
      isDragging = true;
      cleanup();
      getCurrentWindow().startDragging().catch((err) => {
        // 没真的拖起来，就别把这次手势算成拖动 —— 否则它之后的点击全被吞掉。
        isDragging = false;
        console.error('[TickerBar] startDragging failed:', err);
      });
    }
  }

  stopMouseTracking = cleanup;
  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', cleanup);
  // 按下期间窗口失焦(Alt-Tab、系统模态框)后，mouseup 不会再送进这个 webview，
  // 只能靠 blur 兜底。
  window.addEventListener('blur', cleanup);
}

function clearOpenError() {
  if (openErrorTimer) {
    clearTimeout(openErrorTimer);
    openErrorTimer = null;
  }
  openError.value = false;
}

async function handleClick(event: MouseEvent | KeyboardEvent) {
  // 键盘走 keydown，没有 button；鼠标事件里中键/右键也不该打开主窗口。
  if (event instanceof MouseEvent && (event.button !== 0 || isDragging)) return;
  if (initFailed.value) {
    if (cycleTimer) { clearInterval(cycleTimer); cycleTimer = null; }
    if (unlistenTheme) { unlistenTheme(); unlistenTheme = null; }
    if (unlistenDatasource) { unlistenDatasource(); unlistenDatasource = null; }
    if (unlistenWatchlist) { unlistenWatchlist(); unlistenWatchlist = null; }
    if (unlistenSettings) { unlistenSettings(); unlistenSettings = null; }
    quoteStore.stopListening();

    initFailed.value = false;
    retryHintVisible.value = true;
    // 「重连中...」是过渡态，重连期间别按它改窗口高度
    heightReady.value = false;
    try {
      await settings.fetchSettings();
      settings.applyTheme(settings.theme);
      settings.applyColorScheme(settings.colorScheme);
      await watchlist.fetchWatchlist();
      await quoteStore.startListening();
      startCycle();
      startThemeListen();
      startDatasourceListen();
      startWatchlistListener();
      startSettingsListen();
      retryHintVisible.value = false;
      heightReady.value = true;
    } catch (e) {
      initFailed.value = true;
      retryHintVisible.value = false;
      heightReady.value = true;
      console.error('[TickerBar] retry failed:', e);
    }
    return;
  }
  // 上一次失败留下的错误行，点一下就当重试，先撤掉
  clearOpenError();
  try {
    await invoke('show_main_window');
  } catch (e) {
    console.error('[TickerBar] show_main_window failed:', e);
    openError.value = true;
    openErrorTimer = setTimeout(clearOpenError, OPEN_ERROR_MS);
  }
}
</script>

<template>
  <div
    class="ticker-bar"
    :class="{ 'is-transparent': settings.tickerTransparent }"
    role="button"
    tabindex="0"
    aria-label="显示主界面"
    @keydown.enter="handleClick"
    @keydown.space.prevent="handleClick"
    @mousedown="onMouseDown"
    @mouseenter="paused = true"
    @mouseleave="paused = false"
    @click="handleClick"
  >
    <div ref="tickerContent" class="ticker-content">
      <template v-if="initFailed">
        <div class="ticker-row ticker-error-row">
          <span class="ticker-error-text" :title="'点击重试'">QuantDesktop</span>
          <span class="ticker-retry-hint">· 点击重试</span>
        </div>
      </template>
      <template v-else-if="retryHintVisible">
        <div class="ticker-row ticker-error-row">
          <span class="ticker-error-text">重连中...</span>
        </div>
      </template>
      <!-- 打开主窗口没成功。和上面的重连失败同一套行内样式，几秒后自动退回
           行情显示(见 OPEN_ERROR_MS)，不会把行情条永久占成错误页。 -->
      <template v-else-if="openError">
        <div class="ticker-row ticker-error-row">
          <span class="ticker-error-text">打开失败</span>
          <span class="ticker-retry-hint">· 点击重试</span>
        </div>
      </template>
      <template v-else-if="visibleItems.length > 0">
        <div v-for="item in visibleItems" :key="item.code" class="ticker-row">
          <span class="ticker-name">{{ item.name }}</span>
          <span
            v-if="item.price !== null"
            class="ticker-price tabular-nums"
            :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'"
          >{{ formatPrice(item.price) }}</span>
          <span v-else class="ticker-na">--</span>
          <span
            v-if="item.changePct !== null"
            class="ticker-change tabular-nums"
            :class="item.changePct >= 0 ? 'up' : 'down'"
          >{{ item.changePct >= 0 ? '+' : '' }}{{ item.changePct.toFixed(2) }}%</span>
        </div>
      </template>
      <!-- 区分两种为空：诚然没有自选，与有自选但全部关闭了播报 -->
      <div v-else class="ticker-empty">
        {{ watchlist.items.length === 0 ? '暂无自选' : '暂未设置播报标的' }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.ticker-bar {
  width: 100%;
  height: 100%;
  background: transparent;
  user-select: none;
  cursor: grab;
  overflow: hidden;
  transition: background var(--transition-fast);
}
.ticker-bar:hover {
  background: rgba(255, 255, 255, 0.03);
}
/* 透明模式下不能有 hover 底色：底板已经去掉了，悬停时冒出一块半透明矩形
   会像渲染残留。光标仍会变成 grab，交互提示没有丢。 */
.ticker-bar.is-transparent:hover {
  background: transparent;
}
/* 内容层：高度由内容撑开（不要写 height:100%，否则量高度会自我循环）。
   useTickerWindowHeight 量的就是这个元素，再据此设窗口高度。 */
.ticker-content {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: var(--space-1) var(--space-2);
}
.ticker-row {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  line-height: 1.4;
}
.ticker-name {
  flex: 1;
  min-width: 0;
  color: var(--color-text-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ticker-price {
  flex-shrink: 0;
  font-weight: var(--font-weight-semibold);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  width: 46px;
  text-align: right;
  color: var(--color-text-primary);
}
.ticker-na {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  width: 46px;
  text-align: right;
}
.ticker-change {
  flex-shrink: 0;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  width: 48px;
  text-align: right;
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.ticker-empty {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-align: center;
  width: 100%;
}
.ticker-error-row {
  justify-content: center;
}
.ticker-error-text {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  letter-spacing: 0.05em;
}
.ticker-retry-hint {
  color: var(--color-warning);
  font-size: 9px;
  opacity: 0.7;
}
</style>
