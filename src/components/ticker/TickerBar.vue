<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';
import { useSettingsStore } from '@/stores/settings';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const settings = useSettingsStore();
const paused = ref(false);
const page = ref(0);
let cycleTimer: ReturnType<typeof setInterval> | null = null;

onMounted(async () => {
  await settings.fetchSettings();
  settings.applyTheme(settings.theme);
  await watchlist.fetchWatchlist();
  await quoteStore.startListening();
  startCycle();
});

onUnmounted(() => {
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
});

function startCycle() {
  cycleTimer = setInterval(() => {
    if (!paused.value && tickerItems.value.length > 2) {
      page.value = (page.value + 2) % tickerItems.value.length;
    }
  }, 3000);
}

const tickerItems = computed(() => {
  return watchlist.items.map(item => {
    const q = quoteStore.getQuote(item.code, item.market);
    return {
      name: item.name,
      code: item.code,
      price: q?.price ?? null,
      changePct: q?.change_pct ?? null,
    };
  });
});

const visibleItems = computed(() => {
  const items = tickerItems.value;
  if (items.length === 0) return [];
  const result = [];
  for (let i = 0; i < 2; i++) {
    const idx = (page.value + i) % items.length;
    result.push(items[idx]);
  }
  return result;
});

function pauseCycle() {
  paused.value = true;
}

function resumeCycle() {
  paused.value = false;
}

async function handleClick() {
  try {
    await invoke('show_main_window');
  } catch (e) {
    console.error('Failed to show main window:', e);
  }
}
</script>

<template>
  <div
    class="ticker-bar"
    @mouseenter="pauseCycle"
    @mouseleave="resumeCycle"
    @click="handleClick"
  >
    <template v-if="visibleItems.length > 0">
      <div
        v-for="item in visibleItems"
        :key="item.code"
        class="ticker-row"
      >
        <span class="ticker-name">{{ item.name }}</span>
        <span
          v-if="item.price !== null"
          class="ticker-price"
          :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'"
        >
          {{ item.price.toFixed(2) }}
        </span>
        <span v-else class="ticker-na">--</span>
        <span
          v-if="item.changePct !== null"
          class="ticker-change"
          :class="item.changePct >= 0 ? 'up' : 'down'"
        >
          {{ item.changePct >= 0 ? '+' : '' }}{{ item.changePct.toFixed(2) }}%
        </span>
      </div>
    </template>
    <div v-else class="ticker-empty">
      暂无自选
    </div>
  </div>
</template>

<style scoped>
.ticker-bar {
  width: 100%;
  height: 100%;
  background-color: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  user-select: none;
  cursor: pointer;
  overflow: hidden;
  padding: 2px 8px;
}
.ticker-bar:hover {
  border-color: var(--color-text-secondary);
}
.ticker-row {
  display: flex;
  align-items: center;
  gap: 6px;
  line-height: 1.3;
}
.ticker-name {
  color: var(--color-text-secondary);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 56px;
}
.ticker-price {
  font-weight: 600;
  font-size: 11px;
  min-width: 52px;
  text-align: right;
  color: var(--color-text-primary);
}
.ticker-na {
  color: var(--color-text-secondary);
  font-size: 11px;
  min-width: 52px;
  text-align: right;
}
.ticker-change {
  font-size: 11px;
  min-width: 54px;
  text-align: right;
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.ticker-empty {
  color: var(--color-text-secondary);
  font-size: 11px;
  text-align: center;
  width: 100%;
}
</style>
