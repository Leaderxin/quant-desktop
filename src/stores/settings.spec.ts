// @vitest-environment happy-dom
//
// settings store 的派生配置与跨窗口同步契约。
//
// 两件在界面上看不出来的事：
// 1. 配置值全部是「字符串设置表」上的 computed，坏数据必须退回可用默认值而不是
//    抛错 —— 一个被手工改坏的键不该让整个界面白屏；
// 2. `applyRemoteSetting` **只能**改本地 state。它若是回写库或回广播，主窗口与
//    行情条之间就会无限往返（两边互相通知对方"设置变了"）。
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';

const { invokeMock, emitMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  emitMock: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ emit: emitMock }));

import { useSettingsStore } from './settings';

/** 让 fetchSettings 走完一轮（它会依次拉 6 个命令）。 */
function mockBackend(settings: Record<string, string>) {
  invokeMock.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case 'get_settings':
        return settings;
      case 'list_datasources':
        return [['tencent', '腾讯证券']];
      case 'list_index_pool':
        return [['s_sh000001', '上证指数'], ['s_sz399001', '深证成指']];
      case 'get_autostart':
        return false;
      case 'get_portable_mode':
        return false;
      case 'is_store_build':
        return false;
      default:
        // 任何写操作都不该在只读用例里发生 —— 让它显式失败而不是静默通过
        throw new Error(`unexpected command: ${cmd}`);
    }
  });
}

/**
 * 加载一份设置。
 *
 * 每次都重建 pinia：多个用例会在同一个 it 里连续 load 多次来对比不同配置，
 * 若复用同一个 pinia，`useSettingsStore()` 返回的是同一个实例，先取到的 store
 * 会被后一次 load 覆盖 —— 断言读到的全是最后一次的值，测试看着通过实际没验。
 */
async function load(settings: Record<string, string> = {}) {
  setActivePinia(createPinia());
  mockBackend(settings);
  const store = useSettingsStore();
  await store.fetchSettings();
  return store;
}

beforeEach(() => {
  invokeMock.mockReset();
  emitMock.mockClear();
});

describe('派生配置的解析与兜底', () => {
  it('index_codes 缺失或损坏时退回默认的 7 个', async () => {
    const a = await load({});
    expect(a.indexCodes).toHaveLength(7);
    expect(a.indexCodes[0]).toBe('s_sh000001');

    const b = await load({ index_codes: 'not json' });
    expect(b.indexCodes).toHaveLength(7);

    // JSON 合法但不是数组 —— 直接返回它会让 IndexBar 在 `.map` 上崩
    const c = await load({ index_codes: '{"a":1}' });
    expect(c.indexCodes).toHaveLength(7);
  });

  it('index_codes 合法时按原样与顺序使用', async () => {
    const store = await load({ index_codes: '["s_sh000300","s_sh000001"]' });
    expect(store.indexCodes).toEqual(['s_sh000300', 's_sh000001']);
  });

  it('sector_top_n 越界夹到 1–50，非法值退回 5', async () => {
    expect((await load({ sector_top_n: '0' })).sectorTopN).toBe(1);
    expect((await load({ sector_top_n: '999' })).sectorTopN).toBe(50);
    expect((await load({ sector_top_n: '12' })).sectorTopN).toBe(12);
    expect((await load({ sector_top_n: 'abc' })).sectorTopN).toBe(5);
    expect((await load({})).sectorTopN).toBe(5);
  });

  it('ticker_items_per_page 越界夹到 1–4，非法值退回 2', async () => {
    expect((await load({ ticker_items_per_page: '0' })).tickerItemsPerPage).toBe(1);
    expect((await load({ ticker_items_per_page: '9' })).tickerItemsPerPage).toBe(4);
    expect((await load({ ticker_items_per_page: '3' })).tickerItemsPerPage).toBe(3);
    expect((await load({ ticker_items_per_page: 'x' })).tickerItemsPerPage).toBe(2);
    expect((await load({})).tickerItemsPerPage).toBe(2);
  });

  it('布尔型设置只认 "1"/"0"，缺键时取各自默认值', async () => {
    const store = await load({
      market_overview_visible: '0',
      ticker_visible: '0',
      ticker_transparent: '1',
    });
    expect(store.marketOverviewVisible).toBe(false);
    expect(store.tickerVisible).toBe(false);
    expect(store.tickerTransparent).toBe(true);
  });

  it('color_scheme 只认 cn/us，其余退回 cn', async () => {
    expect((await load({ color_scheme: 'us' })).colorScheme).toBe('us');
    expect((await load({ color_scheme: 'rainbow' })).colorScheme).toBe('cn');
    expect((await load({})).colorScheme).toBe('cn');
  });

  it('watchlist_columns 过滤未知列并补回必选列', async () => {
    const store = await load({ watchlist_columns: '["price","bogus"]' });
    expect(store.watchlistColumns).toEqual(['code', 'name', 'price']);
  });

  it('watchlist_default_sort 非法时是 null（不排序）', async () => {
    expect((await load({})).watchlistDefaultSort).toBeNull();
    expect((await load({ watchlist_default_sort: '{' })).watchlistDefaultSort).toBeNull();
    const ok = await load({ watchlist_default_sort: '{"key":"change_pct","order":"ascend"}' });
    expect(ok.watchlistDefaultSort).toEqual({ key: 'change_pct', order: 'ascend' });
  });

  it('indexPool 从后端拉取（名字来自后端常量，前端不另抄一份）', async () => {
    const store = await load({});
    expect(store.indexPool).toEqual([['s_sh000001', '上证指数'], ['s_sz399001', '深证成指']]);
  });
});

describe('写入与跨窗口同步', () => {
  it('setSectorTopN 写库、更新本地、并广播 settings-changed', async () => {
    const store = await load({});
    invokeMock.mockResolvedValueOnce(undefined); // set_setting

    await store.setSectorTopN(20);
    expect(invokeMock).toHaveBeenCalledWith('set_setting', {
      key: 'sector_top_n',
      value: '20',
    });
    // 派生值随之更新 —— 说明设置表是唯一真源，不存在"本地与库里不一致"的窗口
    expect(store.sectorTopN).toBe(20);
    expect(emitMock).toHaveBeenCalledWith('settings-changed', {
      key: 'sector_top_n',
      value: '20',
    });
  });

  it('setSectorTopN 会夹取后再落库，不会把越界值写进去', async () => {
    const store = await load({});
    invokeMock.mockResolvedValueOnce(undefined);
    await store.setSectorTopN(999);
    expect(invokeMock).toHaveBeenCalledWith('set_setting', {
      key: 'sector_top_n',
      value: '50',
    });
  });

  it('applyRemoteSetting 只改本地 —— 不回写、不回广播', async () => {
    const store = await load({});
    invokeMock.mockClear();
    emitMock.mockClear();

    store.applyRemoteSetting('ticker_items_per_page', '4');

    expect(store.tickerItemsPerPage).toBe(4);
    // 回写或回广播会让主窗口与行情条互相通知，形成无限往返
    expect(invokeMock).not.toHaveBeenCalled();
    expect(emitMock).not.toHaveBeenCalled();
  });

  it('setTickerVisible 走 set_ticker_visible 命令（托盘菜单的同一份实现）', async () => {
    const store = await load({ ticker_visible: '1' });
    invokeMock.mockResolvedValueOnce(undefined);

    await store.setTickerVisible(false);
    expect(invokeMock).toHaveBeenCalledWith('set_ticker_visible', { visible: false });
    expect(store.tickerVisible).toBe(false);
    expect(emitMock).toHaveBeenCalledWith('settings-changed', {
      key: 'ticker_visible',
      value: '0',
    });
  });

  it('applyColorScheme 落到 <html data-color-scheme> 上', async () => {
    const store = await load({});
    store.applyColorScheme('us');
    expect(document.documentElement.getAttribute('data-color-scheme')).toBe('us');
    store.applyColorScheme('cn');
    expect(document.documentElement.getAttribute('data-color-scheme')).toBe('cn');
  });
});
