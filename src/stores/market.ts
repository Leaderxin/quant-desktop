// src/stores/market.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { MarketOverview } from '@/types';
import { invoke } from '@tauri-apps/api/core';

export type MarketDirection = 'up' | 'down';

/** 面板展开时的自动刷新间隔(ms)。东财接口有频控,60s 较安全。 */
const REFRESH_INTERVAL_MS = 60_000;

export const useMarketStore = defineStore('market', () => {
  const overview = ref<MarketOverview | null>(null);
  const direction = ref<MarketDirection>('up');
  const expanded = ref(true);
  const loading = ref(false);
  const error = ref<string | null>(null);

  let timer: ReturnType<typeof setInterval> | null = null;

  async function fetchOverview() {
    if (loading.value) return;
    loading.value = true;
    error.value = null;
    try {
      overview.value = await invoke<MarketOverview>('get_market_overview', {
        direction: direction.value,
      });
    } catch (e) {
      error.value = `市场概览加载失败: ${e}`;
      console.error('[market store] fetchOverview failed:', e);
    } finally {
      loading.value = false;
    }
  }

  function toggleDirection() {
    direction.value = direction.value === 'up' ? 'down' : 'up';
    fetchOverview();
  }

  function setExpanded(v: boolean) {
    expanded.value = v;
    if (v) {
      // 展开时立即拉一次并启动定时刷新
      fetchOverview();
      startRefresh();
    } else {
      stopRefresh();
    }
  }

  function startRefresh() {
    stopRefresh();
    timer = setInterval(fetchOverview, REFRESH_INTERVAL_MS);
  }

  function stopRefresh() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  return {
    overview,
    direction,
    expanded,
    loading,
    error,
    fetchOverview,
    toggleDirection,
    setExpanded,
    startRefresh,
    stopRefresh,
  };
});
