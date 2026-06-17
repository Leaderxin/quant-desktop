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
let themePollTimer: ReturnType<typeof setInterval> | null = null;

onMounted(async () => {
  await settings.fetchSettings();
  settings.applyTheme(settings.theme);
  await watchlist.fetchWatchlist();
  await quoteStore.startListening();
  startCycle();
  startThemePoll();
});

onUnmounted(() => {
  quoteStore.stopListening();
  if (cycleTimer) clearInterval(cycleTimer);
  if (themePollTimer) clearInterval(themePollTimer);
});

function startThemePoll() {
  let lastTheme = settings.theme;
  themePollTimer = setInterval(() => {
    invoke<Record<string, string>>('get_settings')
      .then((all) => {
        const current = (all['theme'] as 'dark' | 'light') || 'dark';
        if (current !== lastTheme) {
          lastTheme = current;
          settings.applyTheme(current);
        }
      })
      .catch(() => {});
  }, 1000);
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
  const result = [];
  for (let i = 0; i < 2; i++) {
    result.push(items[(page.value + i) % items.length]);
  }
  return result;
});

async function handleClick() {
  await invoke('show_main_window').catch(() => {});
}
</script>

<template>
  <div
    class="ticker-bar"
    @mouseenter="paused = true"
    @mouseleave="paused = false"
    @click="handleClick"
  >
    <template v-if="visibleItems.length > 0">
      <div v-for="item in visibleItems" :key="item.code" class="ticker-row">
        <span class="ticker-name">{{ item.name }}</span>
        <span
          v-if="item.price !== null"
          class="ticker-price tabular-nums"
          :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'"
        >{{ item.price.toFixed(2) }}</span>
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
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  user-select: none;
  cursor: pointer;
  overflow: hidden;
  padding: var(--space-1) var(--space-2);
  transition: border-color var(--transition-fast), background var(--transition-fast);
}
.ticker-bar:hover {
  border-color: var(--color-border-1);
  background: var(--color-surface-2);
}
.ticker-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  line-height: 1.4;
}
.ticker-name {
  color: var(--color-text-secondary);
  font-size: var(--text-xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 50px;
}
.ticker-price {
  font-weight: var(--font-weight-semibold);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  min-width: 48px;
  text-align: right;
  color: var(--color-text-primary);
}
.ticker-na {
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  min-width: 48px;
  text-align: right;
  font-family: var(--font-mono);
}
.ticker-change {
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  min-width: 50px;
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
</style>
