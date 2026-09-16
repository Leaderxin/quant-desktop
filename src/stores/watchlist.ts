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
    try {
      await invoke('add_watch', { code, market, name });
      await fetchWatchlist();
    } catch (e) {
      error.value = `添加失败: ${e}`;
      console.error('[watchlist] addStock failed:', e);
    }
  }

  async function removeStock(code: string, market: string) {
    error.value = null;
    try {
      await invoke('remove_watch', { code, market });
      await fetchWatchlist();
    } catch (e) {
      error.value = `删除失败: ${e}`;
      console.error('[watchlist] removeStock failed:', e);
    }
  }

  /**
   * 切换某个自选的行情条播报开关。
   *
   * 乐观更新：先改本地再发 IPC，避免拨动开关时有可见延迟；失败则从数据库
   * 重新同步（而非回滚到旧值，理由见 catch 内注释）。
   * 不复用 `error` 字段 —— 该字段会让整张自选表被错误态替换
   * （WatchlistTable 的 `v-if="watchlist.error"`），单次开关失败不值得
   * 清空表格，因此只重新同步 + 记日志。
   *
   * 按 id 查找而不是接收行对象引用，避免依赖 naive-ui 是否原样透传
   * data 中的响应式代理。
   */
  async function setTickerEnabled(id: number, enabled: boolean) {
    const item = items.value.find((i) => i.id === id);
    if (!item) return;
    item.ticker_enabled = enabled;
    try {
      await invoke('set_watch_ticker_enabled', { id, enabled });
    } catch (e) {
      // 失败时从数据库重新同步，而不是回滚到 await 前捕获的值：用户快速
      // 连点（关→开）时，先发的请求后失败会用旧值覆盖后发的意图，且没有
      // 任何环节会再校正（主窗口不监听 watchlist-changed，只有行情条听）。
      await fetchWatchlist();
      // fetchWatchlist 自身失败会置位 error，让整张自选表被错误页替换——
      // 单次开关失败不值得清空表格（两次失败访问同一数据库，是相关的），
      // 复位以免升级为全局错误态。失败详情已由 fetchWatchlist 内部记日志。
      error.value = null;
      console.error('[watchlist] setTickerEnabled failed:', e);
    }
  }

  async function setPosition(id: number, costPrice: number | null, quantity: number | null) {
    await invoke('set_watch_position', { id, costPrice, quantity });
    const item = items.value.find((i) => i.id === id);
    if (item) {
      item.cost_price = costPrice;
      item.quantity = quantity;
    }
  }

  return { items, loading, error, fetchWatchlist, addStock, removeStock, setTickerEnabled, setPosition };
});
