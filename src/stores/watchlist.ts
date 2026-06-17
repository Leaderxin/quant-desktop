// src/stores/watchlist.ts
import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { WatchItem } from '@/types';

export const useWatchlistStore = defineStore('watchlist', () => {
  const items = ref<WatchItem[]>([]);
  const loading = ref(false);

  async function fetchWatchlist() {
    loading.value = true;
    try {
      items.value = await invoke<WatchItem[]>('get_watchlist');
    } catch (e) {
      console.error('Failed to fetch watchlist:', e);
    } finally {
      loading.value = false;
    }
  }

  async function addStock(code: string, market: string, name: string) {
    await invoke('add_watch', { code, market, name });
    await fetchWatchlist();
  }

  async function removeStock(code: string, market: string) {
    await invoke('remove_watch', { code, market });
    await fetchWatchlist();
  }

  async function reorder(ids: number[]) {
    await invoke('reorder_watch', { ids });
    await fetchWatchlist();
  }

  return { items, loading, fetchWatchlist, addStock, removeStock, reorder };
});
