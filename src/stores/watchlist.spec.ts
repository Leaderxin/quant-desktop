// src/stores/watchlist.spec.ts
//
// 自选 store 的派生状态与 IPC 边界。
//
// 只 mock 掉 Tauri 的 invoke —— pinia 与 store 本体都是真的。要验的是两件在界面上
// 看不出来的事：
// 1. 派生顺序的来源（组内顺序在关联表上，不在股票池的返回顺序里）；
// 2. IPC 参数名的拼写。Rust 端参数是 snake_case、JS 端要传 camelCase，
//    拼错了 Tauri 只会返回一个反序列化错误，而这类错误在界面上表现为"点了没反应"。
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { WatchGroup, WatchItem, WatchlistSnapshot } from '@/types';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { useWatchlistStore } from './watchlist';

function item(id: number, code: string, over: Partial<WatchItem> = {}): WatchItem {
  return {
    id,
    code,
    market: 'CN',
    name: `名${code}`,
    added_at: '2026-01-01T00:00:00',
    ticker_enabled: true,
    ticker_order: 0,
    ...over,
  };
}

function group(id: number, name: string, watch_ids: number[]): WatchGroup {
  return { id, name, sort_order: id, created_at: '2026-01-01T00:00:00', watch_ids };
}

function snapshot(items: WatchItem[], groups: WatchGroup[]): WatchlistSnapshot {
  return { items, groups, default_group_id: groups[0]?.id ?? 0 };
}

/** 让 get_watchlist 返回给定快照，并完成一次加载。 */
async function load(snap: WatchlistSnapshot) {
  invokeMock.mockResolvedValueOnce(snap);
  const store = useWatchlistStore();
  await store.fetchWatchlist();
  return store;
}

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

describe('派生状态', () => {
  it('visibleItems 按组内顺序排，不是按股票池的返回顺序', async () => {
    // 池子按 id 升序返回：a(1) b(2) c(3)；组内顺序却是 c, a
    const store = await load(
      snapshot(
        [item(1, 'a'), item(2, 'b'), item(3, 'c')],
        [group(1, '自选股', [3, 1])],
      ),
    );
    expect(store.visibleItems.map((i) => i.code)).toEqual(['c', 'a']);
    // b 在池子里、不在此分组 → 不该出现
    expect(store.visibleItems.some((i) => i.code === 'b')).toBe(false);
  });

  it('visibleItems 丢掉关联表里残留的悬空 id，不渲染空洞', async () => {
    const store = await load(
      snapshot([item(1, 'a')], [group(1, '自选股', [1, 999])]),
    );
    expect(store.visibleItems.map((i) => i.id)).toEqual([1]);
  });

  it('activeGroupId 在当前分组消失后回落到默认分组', async () => {
    const store = await load(
      snapshot([item(1, 'a')], [group(1, '自选股', [1]), group(2, '科技', [])]),
    );
    store.activeGroupId = 2;

    // 科技被删了：快照里只剩默认分组
    invokeMock.mockResolvedValueOnce(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));
    await store.fetchWatchlist();

    expect(store.activeGroupId).toBe(1);
    expect(store.activeGroup?.name).toBe('自选股');
  });

  it('activeGroup 在没有分组时为 null', async () => {
    const store = await load(snapshot([], []));
    expect(store.activeGroup).toBeNull();
    expect(store.visibleItems).toEqual([]);
  });

  it('tickerItems 只取开启播报的，按 ticker_order 排，同序时按 id 兜底', async () => {
    const store = await load(
      snapshot(
        [
          item(1, 'a', { ticker_enabled: true, ticker_order: 2 }),
          item(2, 'b', { ticker_enabled: false, ticker_order: 0 }),
          item(3, 'c', { ticker_enabled: true, ticker_order: 0 }),
          item(4, 'd', { ticker_enabled: true, ticker_order: 0 }),
          item(5, 'e', { ticker_enabled: true, ticker_order: 1 }),
        ],
        [group(1, '自选股', [1, 2, 3, 4, 5])],
      ),
    );
    // b 关播报被排除；c/d 同为 0 时按 id 升序 → c 在 d 前
    expect(store.tickerItems.map((i) => i.code)).toEqual(['c', 'd', 'e', 'a']);
  });

  it('tickerItems 跨分组，播报范围与分组正交', async () => {
    const store = await load(
      snapshot(
        [item(1, 'a'), item(2, 'b')],
        [group(1, '自选股', [1]), group(2, '科技', [2])],
      ),
    );
    expect(store.tickerItems.map((i) => i.code)).toEqual(['a', 'b']);
  });

  it('groupsOf 返回一只股票所属的全部分组', async () => {
    const store = await load(
      snapshot(
        [item(7, 'x')],
        [group(1, '自选股', [7]), group(2, '科技', [7]), group(3, '银行', [])],
      ),
    );
    expect(store.groupsOf(7).map((g) => g.name)).toEqual(['自选股', '科技']);
    expect(store.groupsOf(99)).toEqual([]);
    expect(store.groupName(2)).toBe('科技');
  });
});

describe('IPC 参数拼写', () => {
  // Rust 端参数是 snake_case，Tauri 在 JS 侧按 camelCase 取名。
  // 拼错时 Tauri 抛的是反序列化错误，界面上只表现为"点了没反应"。
  it('moveUp 传 groupId + watchId', async () => {
    const store = await load(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));
    invokeMock.mockResolvedValueOnce(undefined); // move_group_member_up
    invokeMock.mockResolvedValueOnce(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));

    await store.moveUp(1);
    expect(invokeMock).toHaveBeenCalledWith('move_group_member_up', {
      groupId: 1,
      watchId: 1,
    });
  });

  it('addStock 带上当前分组 id', async () => {
    const store = await load(
      snapshot([], [group(1, '自选股', []), group(2, '科技', [])]),
    );
    store.activeGroupId = 2;
    invokeMock.mockResolvedValueOnce(undefined); // add_watch
    invokeMock.mockResolvedValueOnce(snapshot([], [group(1, '自选股', []), group(2, '科技', [])]));

    await store.addStock('sh600519', 'CN', '贵州茅台');
    expect(invokeMock).toHaveBeenCalledWith('add_watch', {
      code: 'sh600519',
      market: 'CN',
      name: '贵州茅台',
      groupId: 2,
    });
  });

  it('setWatchGroups 传 watchId + groupIds', async () => {
    const store = await load(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));
    invokeMock.mockResolvedValueOnce(undefined);
    invokeMock.mockResolvedValueOnce(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));

    await store.setWatchGroups(1, [1, 2]);
    expect(invokeMock).toHaveBeenCalledWith('set_watch_groups', {
      watchId: 1,
      groupIds: [1, 2],
    });
  });

  it('reorderTicker 传 ids', async () => {
    const store = await load(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));
    invokeMock.mockResolvedValueOnce(undefined);
    invokeMock.mockResolvedValueOnce(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));

    await store.reorderTicker([3, 1, 2]);
    expect(invokeMock).toHaveBeenCalledWith('reorder_ticker', { ids: [3, 1, 2] });
  });

  it('deleteGroup 返回后端救回的孤儿数', async () => {
    const store = await load(
      snapshot([item(1, 'a')], [group(1, '自选股', [1]), group(2, '科技', [])]),
    );
    invokeMock.mockResolvedValueOnce(3); // delete_watch_group
    invokeMock.mockResolvedValueOnce(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));

    await expect(store.deleteGroup(2)).resolves.toBe(3);
  });
});

describe('setTickerEnabled 的乐观更新', () => {
  it('发 IPC 之前本地已改，失败时从库里重新同步且不置全局错误', async () => {
    const store = await load(snapshot([item(1, 'a')], [group(1, '自选股', [1])]));

    invokeMock.mockRejectedValueOnce(new Error('boom')); // set_watch_ticker_enabled
    invokeMock.mockResolvedValueOnce(
      snapshot([item(1, 'a', { ticker_enabled: false })], [group(1, '自选股', [1])]),
    );

    const pending = store.setTickerEnabled(1, false);
    // async 函数在首个 await 前同步执行 —— 此刻本地已经翻过来了
    expect(store.items[0].ticker_enabled).toBe(false);

    await pending;
    // 失败后以库里的值为准（而不是回滚到 await 前捕获的旧值：
    // 快速连点时会用旧值覆盖后发的意图）
    expect(store.items[0].ticker_enabled).toBe(false);
    // 单次开关失败不该让整张表被错误态替换
    expect(store.error).toBeNull();
  });
});
