<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useQuoteStore } from '@/stores/quote';
import { useWatchlistStore } from '@/stores/watchlist';

const quoteStore = useQuoteStore();
const watchlist = useWatchlistStore();
const paused = ref(false);

onMounted(async () => {
  await watchlist.fetchWatchlist();
  await quoteStore.startListening();
});

onUnmounted(() => {
  quoteStore.stopListening();
});

// Build ticker items: for each watchlist stock, get the latest quote
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

// Double the items for seamless looping
const displayItems = computed(() => {
  return [...tickerItems.value, ...tickerItems.value];
});
</script>

<template>
  <div
    class="ticker-bar"
    @mouseenter="paused = true"
    @mouseleave="paused = false"
  >
    <div class="ticker-track" :class="{ paused }">
      <span
        v-for="(item, i) in displayItems"
        :key="`${item.code}-${i}`"
        class="ticker-item"
      >
        <span class="ticker-name">{{ item.name }}</span>
        <span
          v-if="item.price !== null"
          class="ticker-price"
          :class="item.changePct !== null && item.changePct >= 0 ? 'up' : 'down'"
        >
          {{ item.price.toFixed(2) }}
        </span>
        <span v-if="item.price === null" class="ticker-na">--</span>
        <span
          v-if="item.changePct !== null"
          class="ticker-change"
          :class="item.changePct >= 0 ? 'up' : 'down'"
        >
          {{ item.changePct >= 0 ? '+' : '' }}{{ item.changePct.toFixed(2) }}%
        </span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.ticker-bar {
  width: 100vw;
  height: 32px;
  overflow: hidden;
  background-color: #0d1117;
  border-top: 1px solid #222;
  display: flex;
  align-items: center;
  user-select: none;
  cursor: default;
}
.ticker-track {
  display: flex;
  gap: 24px;
  white-space: nowrap;
  animation: scroll-left 30s linear infinite;
  padding: 0 16px;
}
.ticker-track.paused {
  animation-play-state: paused;
}
.ticker-item {
  display: flex;
  gap: 6px;
  align-items: center;
  flex-shrink: 0;
}
.ticker-name {
  color: #999;
  font-size: 12px;
}
.ticker-price {
  font-weight: 600;
  font-size: 12px;
}
.ticker-na {
  color: #666;
  font-size: 12px;
}
.ticker-change {
  font-size: 12px;
}
.up { color: #ef5350; }
.down { color: #66bb6a; }

@keyframes scroll-left {
  0% { transform: translateX(0); }
  100% { transform: translateX(-50%); }
}
</style>
