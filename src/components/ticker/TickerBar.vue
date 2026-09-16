<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

import MarketBreadthRow from './MarketBreadthRow.vue';
import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';
import { useSettingsStore } from '@/stores/settings';
import { formatPrice } from '@/utils/format';
import { tickerRows, tickerPage, tickerGroups, TICKER_ROW_HEIGHT } from '@/utils/ticker';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const settings = useSettingsStore();
const paused = ref(false);
const page = ref(0);
const marketVisible = ref(false);
let unlistenMarket: UnlistenFn | null = null;
let marketVersion = 0;
async function initializeMarket() {
  unlistenMarket?.();
  unlistenMarket = await listen<boolean>('ticker-market-changed', ({ payload }) => {
    marketVersion++;
    marketVisible.value = payload;
    syncRows();
  });
  const version = marketVersion;
  const visible = await invoke<boolean>('get_ticker_market_visible');
  if (version === marketVersion) marketVisible.value = visible;
  syncRows();
}
const rowCount = ref(tickerRows(window.innerHeight));
function syncRows() { rowCount.value = tickerRows(window.innerHeight - (marketVisible.value ? 15 : 0)); }
window.addEventListener('resize', syncRows);
const resizeError = ref('');
type ResizeEdge = 'top' | 'bottom';
let pendingResize: { rows: number; edge: ResizeEdge } | null = null;
let resizing = false;
async function requestRows(rows: number, edge: ResizeEdge = 'bottom') {
  pendingResize = { rows: Math.max(minimumRows.value, Math.min(30, rows)), edge };
  if (resizing) return;
  resizing = true;
  try {
    while (pendingResize !== null) {
      const next = pendingResize;
      pendingResize = null;
      rowCount.value = await invoke<number>('set_ticker_rows', next);
      resizeError.value = '';
    }
  } catch (e) {
    pendingResize = null;
    resizeError.value = `调整失败：${e}`;
    syncRows();
  } finally {
    resizing = false;
  }
}
let resizeStart: { y: number; rows: number; edge: ResizeEdge } | null = null;
function startResize(e: PointerEvent, edge: ResizeEdge) {
  if (e.button !== 0) return;
  resizeStart = { y: e.screenY, rows: rowCount.value, edge };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}
function moveResize(e: PointerEvent) {
  if (!resizeStart) return;
  const direction = resizeStart.edge === 'top' ? -1 : 1;
  const rows = Math.max(2, Math.min(30, resizeStart.rows + Math.round(direction * (e.screenY - resizeStart.y) / TICKER_ROW_HEIGHT)));
  if (rows !== (pendingResize?.rows ?? rowCount.value)) void requestRows(rows, resizeStart.edge);
}
let cycleTimer: ReturnType<typeof setInterval> | null = null;
let unlistenTheme: UnlistenFn | null = null;
let unlistenDatasource: UnlistenFn | null = null;

let unlistenWatchlist: UnlistenFn | null = null;

const initFailed = ref(false);

onMounted(async () => {
  try {
    await initializeMarket();
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
  unlistenMarket?.();
  window.removeEventListener('resize', syncRows);
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
    if (!paused.value && !resizeStart && groups.value.rotating.length > groups.value.slots) {
      page.value = (page.value + groups.value.slots) % groups.value.rotating.length;
    }
  }, 3000);
}

const tickerItems = computed(() =>
  watchlist.items
    .filter((item) => item.ticker_enabled)
    .map((item) => {
      const q = quoteStore.getQuote(item.code, item.market);
      return {
        key: `${item.market}:${item.code}`,
        ticker_enabled: item.ticker_enabled,
        ticker_pinned: item.ticker_pinned,
        name: item.name,
        code: item.code,
        price: q?.price ?? null,
        changePct: q?.change_pct ?? null,
      };
    })
);

const minimumRows = computed(() => Math.min(30, Math.max(2,
  watchlist.items.filter((item) => item.ticker_enabled && item.ticker_pinned).length + 1)));
const groups = computed(() => tickerGroups(tickerItems.value, rowCount.value));
// Only membership/order/row changes reset rotation, never price updates.
watch(
  [() => tickerItems.value.map((item) => `${item.key}:${item.ticker_pinned}`).join('|'), rowCount],
  () => { page.value = 0; }
);
const visibleItems = computed(() => [
  ...groups.value.pinned.map((item) => ({ ...item, fixed: true })),
  ...tickerPage(groups.value.rotating, page.value, groups.value.slots).map((item) => ({ ...item, fixed: false })),
]);

const retryHintVisible = ref(false);

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

async function handleClick() {
  if (isDragging) return;
  if (initFailed.value) {
    if (cycleTimer) { clearInterval(cycleTimer); cycleTimer = null; }
    if (unlistenTheme) { unlistenTheme(); unlistenTheme = null; }
    if (unlistenDatasource) { unlistenDatasource(); unlistenDatasource = null; }
    if (unlistenWatchlist) { unlistenWatchlist(); unlistenWatchlist = null; }
    quoteStore.stopListening();

    initFailed.value = false;
    retryHintVisible.value = true;
    try {
      await initializeMarket();
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
    <MarketBreadthRow v-if="marketVisible" />
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
      <div v-for="(item, index) in visibleItems" :key="item.key" class="ticker-row"
        :class="{ 'ticker-pinned': item.fixed, 'ticker-pinned-last': item.fixed && index === groups.pinned.length - 1 }">
        <span class="ticker-name" :title="item.fixed ? `${item.name} · 已固定，不参与轮播` : item.ticker_pinned ? `${item.name} · 屏幕空间不足，暂时参与轮播` : item.name">{{ item.name }}</span>
        <div class="ticker-values">
        <span
          v-if="item.price !== null"
          class="ticker-price tabular-nums"
          :title="`现价：${formatPrice(item.price)}`"
          :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'"
        >{{ formatPrice(item.price) }}</span>
        <span v-else class="ticker-na">--</span>
        <span
          v-if="item.changePct !== null"
          class="ticker-change tabular-nums"
          :title="`涨跌幅：${item.changePct.toFixed(2)}%`"
          :class="item.changePct >= 0 ? 'up' : 'down'"
        >{{ item.changePct >= 0 ? '+' : '' }}{{ item.changePct.toFixed(2) }}%</span>
        <span v-else class="ticker-change ticker-muted">--</span>
        </div>
      </div>
      <div v-if="groups.pinned.length && !groups.rotating.length" class="ticker-empty-slot">暂无轮播股票</div>
    </template>
    <!-- 区分两种为空：诚然没有自选，与有自选但全部关闭了播报 -->
    <div v-else class="ticker-empty">
      {{ watchlist.items.length === 0 ? '暂无自选' : '暂未设置播报标的' }}
    </div>
    <div v-for="edge in (['top', 'bottom'] as const)" :key="edge"
      class="resize-handle" :class="`resize-handle-${edge}`" role="separator" tabindex="0"
      :aria-label="edge === 'top' ? '从顶部调整显示行数' : '从底部调整显示行数'" aria-orientation="horizontal"
      :aria-valuenow="rowCount" :aria-valuemin="minimumRows" :aria-valuemax="30"
      :title="resizeError || `拖动调整高度（当前 ${rowCount} 行），也可用上下方向键`"
      @mousedown.stop.prevent @click.stop @pointerdown.stop.prevent="startResize($event, edge)" @pointermove="moveResize"
      @pointerup="resizeStart = null" @pointercancel="resizeStart = null" @lostpointercapture="resizeStart = null"
      @keydown.stop
      @keydown.up.stop.prevent="requestRows(rowCount + (edge === 'top' ? 1 : -1), edge)"
      @keydown.down.stop.prevent="requestRows(rowCount + (edge === 'top' ? -1 : 1), edge)"
    />
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
  grid-template-columns: minmax(0, 1fr) 46px 48px;
  align-items: center;
  gap: 4px;
  line-height: 1.4;
}
.ticker-pinned { background: var(--color-bg-elevated); }
.ticker-pinned .ticker-name { font-weight: 600; }
.ticker-pinned-last { box-shadow: inset 0 -1px 0 var(--color-border-0); }
.ticker-empty-slot { height: 15px; font-size: 9px; color: var(--color-text-tertiary); text-align: center; }
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
.ticker-price, .ticker-na, .ticker-change {
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

.ticker-muted { color: var(--color-text-tertiary); }
.resize-handle {
  position: absolute;
  left: 0;
  right: 0;
  height: 4px;
  cursor: ns-resize;
  touch-action: none;
}
.resize-handle-top { top: 0; }
.resize-handle-bottom { bottom: 0; }
.resize-handle:hover, .resize-handle:focus-visible { background: var(--color-accent); outline: none; }
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.ticker-empty {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  text-align: center;
  width: 100%;
}
.ticker-error-row {
  display: flex;
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
