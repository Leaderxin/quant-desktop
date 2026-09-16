<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';
import { useSettingsStore } from '@/stores/settings';
import { positionProfit, formatProfit } from '@/utils/position';
import { formatPrice } from '@/utils/format';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const settings = useSettingsStore();
const paused = ref(false);
const page = ref(0);
const profitVisible = ref(false);
let unlistenProfit: UnlistenFn | null = null;
let privacyVersion = 0;
let profitQueue: Promise<void> = Promise.resolve();

async function initializePrivacy() {
  unlistenProfit?.();
  unlistenProfit = await listen<boolean>('ticker-profit-changed', ({ payload }) => {
    privacyVersion++;
    profitVisible.value = payload;
  });
  const version = privacyVersion;
  const visible = await invoke<boolean>('get_ticker_profit_visible');
  if (version === privacyVersion) profitVisible.value = visible;
}

function toggleProfit() {
  // Immediately remove amounts and tooltips; requests remain ordered for rapid clicks.
  profitVisible.value = false;
  profitQueue = profitQueue.then(async () => {
    await invoke<boolean>('toggle_ticker_profit');
    const version = privacyVersion;
    const visible = await invoke<boolean>('get_ticker_profit_visible');
    if (version === privacyVersion) profitVisible.value = visible;
    openError.value = '';
  }).catch((e) => {
    profitVisible.value = false;
    openError.value = `盈亏显示切换失败，请重试：${e}`;
  });
}
let cycleTimer: ReturnType<typeof setInterval> | null = null;
let unlistenTheme: UnlistenFn | null = null;
let unlistenDatasource: UnlistenFn | null = null;

let unlistenWatchlist: UnlistenFn | null = null;

const initFailed = ref(false);

onMounted(async () => {
  try {
    await initializePrivacy();
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    await watchlist.fetchWatchlist();
    await quoteStore.startListening();
    startCycle();
    startThemeListen();
    startDatasourceListen();
    startWatchlistListener();
  } catch (e) {
    initFailed.value = true;
    console.error('[TickerBar] init failed:', e);
  }
});

onUnmounted(() => {
  unlistenProfit?.();
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
  if (unlistenTheme) unlistenTheme();
  if (unlistenDatasource) unlistenDatasource();
  if (unlistenWatchlist) unlistenWatchlist();
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
    if (!paused.value && tickerItems.value.length > 2) {
      page.value = (page.value + 2) % tickerItems.value.length;
    }
  }, 3000);
}

const tickerItems = computed(() =>
  watchlist.items
    .filter((item) => item.ticker_enabled)
    .map((item) => {
      const q = quoteStore.getQuote(item.code, item.market);
      return {
        name: item.name,
        code: item.code,
        price: q?.price ?? null,
        changePct: q?.change_pct ?? null,
        profit: positionProfit(q?.price, item.cost_price, item.quantity),
      };
    })
);

// 可见集合变化时回到第一屏，避免列表变短后观众从半截开始看。
//
// 必须监听 length 而不是 tickerItems 本身：tickerItems 依赖 quote store，
// 行情每次轮询都会重算，直接监听它会每 2 秒重置一次 page，翻页将永远
// 停在第一屏。
watch(
  () => tickerItems.value.length,
  () => {
    page.value = 0;
  }
);

const visibleItems = computed(() => {
  const items = tickerItems.value;
  if (items.length === 0) return [];
  if (items.length === 1) return [items[0]];
  const count = Math.min(2, items.length);
  const result = [];
  for (let i = 0; i < count; i++) {
    result.push(items[(page.value + i) % items.length]);
  }
  return result;
});

const retryHintVisible = ref(false);
const openError = ref('');

// ── Dragging ──
// Uses Tauri's startDragging() API (Win32 DefWindowProc) for smooth
// OS-level window dragging on both Windows 10 and 11.
// Position is auto-saved by the Rust WindowEvent::Moved handler in lib.rs.
// Click vs drag detection via mousemove threshold:
// - Click (mouse moves <3px): @click fires → opens main window
// - Drag (mouse moves ≥3px): startDragging() triggers OS drag → @click does NOT fire
//   because startDragging() enters a Win32 modal drag loop that consumes mouseup.
//   Document-level mousemove listener ensures we catch fast mouse movements
//   that leave the ticker bar element.

let isDragging = false;

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  isDragging = false;
  if (initFailed.value) {
    return;
  }
  const startX = e.clientX;
  const startY = e.clientY;

  const onMouseMove = (ev: MouseEvent) => {
    if (isDragging) return;
    if (Math.abs(ev.clientX - startX) > 3 || Math.abs(ev.clientY - startY) > 3) {
      isDragging = true;
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      getCurrentWindow().startDragging().catch((err) => {
        console.error('[TickerBar] startDragging failed:', err);
      });
    }
  };

  const onMouseUp = () => {
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
  };

  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', onMouseUp);
}

async function handleClick(event: MouseEvent | KeyboardEvent) {
  if (event instanceof MouseEvent && (event.button !== 0 || isDragging)) return;
  if (initFailed.value) {
    if (cycleTimer) { clearInterval(cycleTimer); cycleTimer = null; }
    if (unlistenTheme) { unlistenTheme(); unlistenTheme = null; }
    if (unlistenDatasource) { unlistenDatasource(); unlistenDatasource = null; }
    if (unlistenWatchlist) { unlistenWatchlist(); unlistenWatchlist = null; }
    quoteStore.stopListening();

    initFailed.value = false;
    retryHintVisible.value = true;
    try {
      await initializePrivacy();
    await settings.fetchSettings();
      settings.applyTheme(settings.theme);
      await watchlist.fetchWatchlist();
      await quoteStore.startListening();
      startCycle();
      startThemeListen();
      startDatasourceListen();
      startWatchlistListener();
      retryHintVisible.value = false;
    } catch (e) {
      initFailed.value = true;
      retryHintVisible.value = false;
      console.error('[TickerBar] retry failed:', e);
    }
    return;
  }
  await invoke('show_main_window').catch((e) => { console.error('[TickerBar] show_main_window failed:', e); });
}
</script>

<template>
  <div
    class="ticker-bar"
    :class="{ 'profit-hidden': !profitVisible }"
    :title="openError || undefined"
    role="button"
    tabindex="0"
    aria-label="显示主界面"
    @keydown.enter="handleClick"
    @keydown.space.prevent="handleClick"
    @mousedown="onMouseDown"
    @mousedown.middle.prevent
    @auxclick.middle.stop.prevent="toggleProfit"
    @mouseenter="paused = true"
    @mouseleave="paused = false"
    @click="handleClick"
  >
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
        <span v-else class="ticker-change ticker-muted">--</span>
        <span v-if="profitVisible" class="ticker-profit tabular-nums"
          :class="item.profit !== null && item.profit > 0 ? 'up' : item.profit !== null && item.profit < 0 ? 'down' : 'ticker-muted'"
          :title="`持仓盈亏：${formatProfit(item.profit)}`"
        >{{ formatProfit(item.profit, true) }}</span>
      </div>
    </template>
    <!-- 区分两种为空：诚然没有自选，与有自选但全部关闭了播报 -->
    <div v-else class="ticker-empty">
      {{ watchlist.items.length === 0 ? '暂无自选' : '暂未设置播报标的' }}
    </div>
  </div>
</template>

<style scoped>
.ticker-bar {
  position: relative;
  box-sizing: border-box;
  width: 100%;
  height: 100vh;
  background: transparent;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  user-select: none;
  cursor: grab;
  overflow: hidden;
  padding: var(--space-1) 6px;
  transition: background var(--transition-fast);
}
.ticker-bar:hover {
  background: rgba(255, 255, 255, 0.03);
}
.ticker-row {
  height: 15px;
  flex-shrink: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 46px 48px 70px;
  align-items: center;
  gap: 4px;
  line-height: 1.4;
}
.profit-hidden .ticker-row { grid-template-columns: minmax(0, 1fr) 46px 48px; }
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
.ticker-values {
  display: contents;
}
.ticker-price, .ticker-na, .ticker-change, .ticker-profit {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ticker-price {
  flex-shrink: 0;
  font-weight: var(--font-weight-semibold);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  text-align: right;
  color: var(--color-text-primary);
}
.ticker-na {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  text-align: right;
}
.ticker-change {
  flex-shrink: 0;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  text-align: right;
}
.ticker-profit {
  flex-shrink: 0;
  text-align: right;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ticker-muted { color: var(--color-text-tertiary); }
</style>
