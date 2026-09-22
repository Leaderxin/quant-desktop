// src/stores/watchlist.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { WatchItem, WatchGroup, WatchlistSnapshot } from '@/types';

export const useWatchlistStore = defineStore('watchlist', () => {
  /** 全局股票池。一只股票只出现一次，可同时属于多个分组。 */
  const items = ref<WatchItem[]>([]);
  const groups = ref<WatchGroup[]>([]);
  const defaultGroupId = ref(0);
  /** 当前选中的分组。首次加载后落到默认分组；该分组被删时回落到默认分组。 */
  const activeGroupId = ref(0);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const itemsById = computed(() => {
    const m = new Map<number, WatchItem>();
    for (const it of items.value) m.set(it.id, it);
    return m;
  });

  const activeGroup = computed<WatchGroup | null>(() =>
    groups.value.find((g) => g.id === activeGroupId.value) ?? null);

  /**
   * 当前分组下要展示的自选，按组内顺序。
   *
   * 顺序取自 `groups[].watch_ids` 而不是股票池的返回顺序 —— 组内顺序存在关联表上，
   * 池表根本不携带它。同时按池表过滤一遍：关联表里若残留已删除的 watch_id
   * （异常中断等），不会渲染出空洞。
   */
  const visibleItems = computed<WatchItem[]>(() => {
    const g = activeGroup.value;
    if (!g) return [];
    const byId = itemsById.value;
    return g.watch_ids
      .map((id) => byId.get(id))
      .filter((it): it is WatchItem => it !== undefined);
  });

  /** 行情条播报范围，按轮播位次排序。跨分组的扁平列表。 */
  const tickerItems = computed<WatchItem[]>(() =>
    items.value
      .filter((it) => it.ticker_enabled)
      .slice()
      .sort((a, b) => a.ticker_order - b.ticker_order || a.id - b.id));

  /** 某只自选当前所属的全部分组。多归属下可能不止一个。 */
  function groupsOf(watchId: number): WatchGroup[] {
    return groups.value.filter((g) => g.watch_ids.includes(watchId));
  }

  function groupName(id: number): string {
    return groups.value.find((g) => g.id === id)?.name ?? '';
  }

  async function fetchWatchlist() {
    loading.value = true;
    error.value = null;
    try {
      const snap = await invoke<WatchlistSnapshot>('get_watchlist');
      items.value = snap.items;
      groups.value = snap.groups;
      defaultGroupId.value = snap.default_group_id;
      // 当前分组不存在了（被删、或被重排后 id 变了）就回落到默认分组
      if (!snap.groups.some((g) => g.id === activeGroupId.value)) {
        activeGroupId.value = snap.default_group_id;
      }
    } catch (e) {
      error.value = `获取自选列表失败: ${e}`;
      console.error('Failed to fetch watchlist:', e);
    } finally {
      loading.value = false;
    }
  }

  /**
   * 统一的写操作包装：跑一条 IPC，成功后重拉快照。
   *
   * 不做乐观更新 —— 分组归属、组内顺序、残留孤儿处理这类语义都在后端，
   * 本地推测一份状态很容易与库里不一致；自选表本身很小，重拉的代价是一次
   * 本地 SQLite 读。唯一例外是 `setTickerEnabled`（见下），它需要拨开关的手感。
   */
  async function mutate(action: () => Promise<unknown>, failureMessage: string) {
    error.value = null;
    try {
      await action();
      await fetchWatchlist();
    } catch (e) {
      error.value = `${failureMessage}: ${e}`;
      console.error(`[watchlist] ${failureMessage} failed:`, e);
    }
  }

  function addStock(code: string, market: string, name: string) {
    return mutate(
      () => invoke('add_watch', { code, market, name, groupId: activeGroupId.value }),
      '添加失败',
    );
  }

  /** 彻底删除：解除全部分组关联。 */
  function removeStock(code: string, market: string) {
    return mutate(() => invoke('remove_watch', { code, market }), '删除失败');
  }

  /**
   * 把自选移出当前分组。若它是最后所属的分组，后端会连股票一起删掉
   * —— 前端菜单文案随之切换为「删除自选」，所以这个行为对用户是可预期的。
   */
  function removeFromActiveGroup(watchId: number) {
    return mutate(
      () => invoke('remove_watch_from_group', { groupId: activeGroupId.value, watchId }),
      '移除失败',
    );
  }

  /** 全量覆盖分组归属（右键「添加到分组」多选提交）。空集合会被后端拒绝。 */
  function setWatchGroups(watchId: number, groupIds: number[]) {
    return mutate(
      () => invoke('set_watch_groups', { watchId, groupIds }),
      '修改分组失败',
    );
  }

  function moveTop(watchId: number) {
    return mutate(
      () => invoke('move_group_member_top', { groupId: activeGroupId.value, watchId }),
      '置顶失败',
    );
  }
  function moveUp(watchId: number) {
    return mutate(
      () => invoke('move_group_member_up', { groupId: activeGroupId.value, watchId }),
      '上移失败',
    );
  }
  function moveDown(watchId: number) {
    return mutate(
      () => invoke('move_group_member_down', { groupId: activeGroupId.value, watchId }),
      '下移失败',
    );
  }
  function reorderGroupMembers(watchIds: number[]) {
    return mutate(
      () => invoke('reorder_group_members', { groupId: activeGroupId.value, watchIds }),
      '调整顺序失败',
    );
  }

  // ── 分组 CRUD ──

  /** 新建分组并切过去。返回新分组 id；失败返回 null。 */
  async function addGroup(name: string): Promise<number | null> {
    error.value = null;
    try {
      const g = await invoke<WatchGroup>('add_watch_group', { name });
      await fetchWatchlist();
      activeGroupId.value = g.id;
      return g.id;
    } catch (e) {
      error.value = `新建分组失败: ${e}`;
      console.error('[watchlist] addGroup failed:', e);
      return null;
    }
  }

  function renameGroup(id: number, name: string) {
    return mutate(() => invoke('rename_watch_group', { id, name }), '重命名失败');
  }

  /** 删除分组（不删自选）。返回被并入默认分组的孤儿数量。 */
  async function deleteGroup(id: number): Promise<number | null> {
    error.value = null;
    try {
      const rescued = await invoke<number>('delete_watch_group', { id });
      await fetchWatchlist();
      return rescued;
    } catch (e) {
      error.value = `删除分组失败: ${e}`;
      console.error('[watchlist] deleteGroup failed:', e);
      return null;
    }
  }

  function reorderGroups(ids: number[]) {
    return mutate(() => invoke('reorder_watch_groups', { ids }), '调整分组顺序失败');
  }

  /**
   * 切换某个自选的行情条播报开关。
   *
   * 这个是**乐观更新**：先改本地再发 IPC，避免拨动开关时有可见延迟；失败则从
   * 数据库重新同步（而非回滚到旧值）—— 用户快速连点（关→开）时，先发的请求后
   * 失败会用旧值覆盖后发的意图，且没有任何环节会再校正。
   *
   * 不复用 `error` 字段 —— 该字段会让整张表被错误态替换，单次开关失败不值得
   * 清空表格，因此只重新同步 + 记日志。
   */
  async function setTickerEnabled(id: number, enabled: boolean) {
    const item = items.value.find((i) => i.id === id);
    if (!item) return;
    item.ticker_enabled = enabled;
    try {
      await invoke('set_watch_ticker_enabled', { id, enabled });
    } catch (e) {
      await fetchWatchlist();
      error.value = null;
      console.error('[watchlist] setTickerEnabled failed:', e);
    }
  }

  /** 批量开关播报（设置页按分组全选/全不选）。 */
  async function setTickerEnabledBulk(ids: number[], enabled: boolean) {
    if (ids.length === 0) return;
    error.value = null;
    try {
      await invoke('set_ticker_enabled_bulk', { ids, enabled });
      await fetchWatchlist();
    } catch (e) {
      error.value = `批量设置播报失败: ${e}`;
      console.error('[watchlist] setTickerEnabledBulk failed:', e);
    }
  }

  /** 重排行情条轮播顺序。传入「全部」视图下播报范围列表的顺序。 */
  async function reorderTicker(ids: number[]) {
    error.value = null;
    try {
      await invoke('reorder_ticker', { ids });
      await fetchWatchlist();
    } catch (e) {
      error.value = `调整播报顺序失败: ${e}`;
      console.error('[watchlist] reorderTicker failed:', e);
    }
  }

  return {
    items, groups, defaultGroupId, activeGroupId, loading, error,
    itemsById, activeGroup, visibleItems, tickerItems,
    groupsOf, groupName,
    fetchWatchlist,
    addStock, removeStock, removeFromActiveGroup, setWatchGroups,
    moveTop, moveUp, moveDown, reorderGroupMembers,
    addGroup, renameGroup, deleteGroup, reorderGroups,
    setTickerEnabled, setTickerEnabledBulk, reorderTicker,
  };
});
