<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { availableMonitors, getCurrentWindow, PhysicalPosition } from '@tauri-apps/api/window';
import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';
import { useSettingsStore } from '@/stores/settings';
import { formatPrice } from '@/utils/format';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const settings = useSettingsStore();
const paused = ref(false);
const page = ref(0);
let cycleTimer: ReturnType<typeof setInterval> | null = null;
let unlistenTheme: UnlistenFn | null = null;
let unlistenDatasource: UnlistenFn | null = null;

let watchlistPollTimer: ReturnType<typeof setInterval> | null = null;

const initFailed = ref(false);

onMounted(async () => {
  try {
    await settings.fetchSettings();
    settings.applyTheme(settings.theme);
    await watchlist.fetchWatchlist();
    await quoteStore.startListening();
    startCycle();
    startThemeListen();
    startDatasourceListen();
    startWatchlistPoll();
  } catch (e) {
    initFailed.value = true;
    console.error('[TickerBar] init failed:', e);
  }
});

onUnmounted(() => {
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
  if (unlistenTheme) unlistenTheme();
  if (unlistenDatasource) unlistenDatasource();
  if (watchlistPollTimer) clearInterval(watchlistPollTimer);
  // Clean up any lingering document-level drag listeners
  document.removeEventListener('mousemove', onDocMouseMove);
  document.removeEventListener('mouseup', onDocMouseUp);
});

function startWatchlistPoll() {
  watchlistPollTimer = setInterval(() => {
    watchlist.fetchWatchlist().catch((e) => { console.error('[TickerBar] poll failed:', e); });
  }, 3000);
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
  watchlist.items.map(item => {
    const q = quoteStore.getQuote(item.code, item.market);
    return {
      name: item.name,
      code: item.code,
      price: q?.price ?? null,
      changePct: q?.change_pct ?? null,
    };
  })
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

// ── Dragging ─────────────────────────────────────────────────────
// Uses document-level listeners so drag continues even when cursor
// leaves the tiny 230×38 window.  Keeps the ticker inside the desktop
// work area (union of all monitors).  Throttled via rAF.
// ──────────────────────────────────────────────────────────────────
let dragging = false;
let dragScreenX = 0;
let dragScreenY = 0;
let winStartPhysicalX = 0;
let winStartPhysicalY = 0;
let scaleFactor = 1;
let setPosPending = false;

let clampMinX = 0;
let clampMinY = 0;
let clampMaxX = Infinity;
let clampMaxY = Infinity;
let tickerPhysicalW = 230;
let tickerPhysicalH = 38;

function clamp(val: number, min: number, max: number): number {
  return val < min ? min : val > max ? max : val;
}

function onDocMouseMove(e: MouseEvent) {
  if (e.buttons !== 1) return;
  const dx = Math.abs(e.screenX - dragScreenX);
  const dy = Math.abs(e.screenY - dragScreenY);
  if (!dragging && (dx > 4 || dy > 4)) {
    dragging = true;
  }
  if (dragging && !setPosPending) {
    setPosPending = true;
    const physicalDx = Math.round((e.screenX - dragScreenX) * scaleFactor);
    const physicalDy = Math.round((e.screenY - dragScreenY) * scaleFactor);
    const newX = clamp(winStartPhysicalX + physicalDx, clampMinX, clampMaxX - tickerPhysicalW);
    const newY = clamp(winStartPhysicalY + physicalDy, clampMinY, clampMaxY - tickerPhysicalH);
    getCurrentWindow().setPosition(new PhysicalPosition(newX, newY));
    requestAnimationFrame(() => { setPosPending = false; });
  }
}

function onDocMouseUp(_e: MouseEvent) {
  if (!dragging) {
    handleClick();
  }
  dragging = false;
  setPosPending = false;
  document.removeEventListener('mousemove', onDocMouseMove);
  document.removeEventListener('mouseup', onDocMouseUp);
}

async function onMouseDown(e: MouseEvent) {
  dragScreenX = e.screenX;
  dragScreenY = e.screenY;
  dragging = false;
  setPosPending = false;

  try {
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    winStartPhysicalX = pos.x;
    winStartPhysicalY = pos.y;
    scaleFactor = await win.scaleFactor();

    const monitors = await availableMonitors();
    if (monitors.length > 0) {
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const m of monitors) {
        const mPos = m.position;
        const mSize = m.size;
        if (mPos.x < minX) minX = mPos.x;
        if (mPos.y < minY) minY = mPos.y;
        if (mPos.x + mSize.width > maxX) maxX = mPos.x + mSize.width;
        if (mPos.y + mSize.height > maxY) maxY = mPos.y + mSize.height;
      }
      clampMinX = minX;
      clampMinY = minY;
      clampMaxX = maxX;
      clampMaxY = maxY;
    }
    tickerPhysicalW = Math.round(230 * scaleFactor);
    tickerPhysicalH = Math.round(38 * scaleFactor);
  } catch {
    winStartPhysicalX = 0;
    winStartPhysicalY = 0;
    scaleFactor = 1;
    clampMinX = 0;
    clampMinY = 0;
    clampMaxX = Infinity;
    clampMaxY = Infinity;
  }

  document.removeEventListener('mousemove', onDocMouseMove);
  document.removeEventListener('mouseup', onDocMouseUp);
  document.addEventListener('mousemove', onDocMouseMove);
  document.addEventListener('mouseup', onDocMouseUp);
}

async function handleClick() {
  if (initFailed.value) {
    if (cycleTimer) { clearInterval(cycleTimer); cycleTimer = null; }
    if (unlistenTheme) { unlistenTheme(); unlistenTheme = null; }
    if (unlistenDatasource) { unlistenDatasource(); unlistenDatasource = null; }
    if (watchlistPollTimer) { clearInterval(watchlistPollTimer); watchlistPollTimer = null; }
    quoteStore.stopListening();

    initFailed.value = false;
    retryHintVisible.value = true;
    try {
      await settings.fetchSettings();
      settings.applyTheme(settings.theme);
      await watchlist.fetchWatchlist();
      await quoteStore.startListening();
      startCycle();
      startThemeListen();
      startDatasourceListen();
      startWatchlistPoll();
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
    @mouseenter="paused = true"
    @mouseleave="paused = false"
    @mousedown="onMouseDown"
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
      </div>
    </template>
    <div v-else class="ticker-empty">暂无自选</div>
  </div>
</template>

<style scoped>
.ticker-bar {
  width: 100%;
  height: 100%;
  background: transparent;
  display: flex;
  flex-direction: column;
  justify-content: center;
  user-select: none;
  cursor: grab;
  overflow: hidden;
  padding: var(--space-1) var(--space-2);
  transition: background var(--transition-fast);
}
.ticker-bar:hover {
  background: rgba(255, 255, 255, 0.03);
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
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
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
