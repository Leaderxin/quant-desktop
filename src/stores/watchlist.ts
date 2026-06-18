// src/stores/watchlist.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { WatchItem } from '@/types';

export const useWatchlistStore = defineStore('watchlist', () => {
  const items = ref<WatchItem[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function fetchWatchlist() {
    loading.value = true;
    error.value = null;
    try {
      items.value = await invoke<WatchItem[]>('get_watchlist');
    } catch (e) {
      error.value = `获取自选列表失败: ${e}`;
      console.error('Failed to fetch watchlist:', e);
    } finally {
      loading.value = false;
    }
  }

  async function addStock(code: string, market: string, name: string) {
    error.value = null;
    await invoke('add_watch', { code, market, name });
    await fetchWatchlist();
  }

  async function removeStock(code: string, market: string) {
    error.value = null;
    await invoke('remove_watch', { code, market });
    await fetchWatchlist();
  }

  async function reorder(ids: number[]) {
    error.value = null;
    await invoke('reorder_watch', { ids });
    await fetchWatchlist();
  }

  return { items, loading, error, fetchWatchlist, addStock, removeStock, reorder };
});
