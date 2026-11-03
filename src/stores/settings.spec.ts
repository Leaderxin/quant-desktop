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
import { nextTick } from 'vue';

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
      // 主题模式落定后 store 会把模式同步给原生层（见 settings.ts 的模式 watch）。
      // 它属于启动流程的一部分，不算「只读用例里冒出来的写操作」。
      case 'set_window_theme':
        return undefined;
      default:
        // 任何写操作都不该在只读用例里发生 —— 让它显式失败而不是静默通过
        throw new Error(`unexpected command: ${cmd}`);
    }
  });
}

type Listener = (e: { matches: boolean }) => void;

/**
 * 假的 matchMedia。happy-dom 的实现只按宽度判定媒体查询，深浅要自己造；
 * 而主题解析正是靠它取「系统现在是不是深色」。
 */
function stubSystemTheme(initialDark: boolean) {
  const listeners = new Set<Listener>();
  const mql = {
    matches: initialDark,
    media: '(prefers-color-scheme: dark)',
    addEventListener: (_type: string, cb: Listener) => { listeners.add(cb); },
    removeEventListener: (_type: string, cb: Listener) => { listeners.delete(cb); },
  };
  window.matchMedia = (() => mql) as unknown as typeof window.matchMedia;
  return {
    mql,
    /** 系统换了深浅：改值并通知订阅者（Rust 侧解 pin 之后是走这条）。 */
    change(dark: boolean) {
      mql.matches = dark;
      for (const cb of listeners) cb({ matches: dark });
    },
  };
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
  // 每个用例都从「系统浅色 + 干净的 <html>」开始：主题解析要读 matchMedia，
  // 而 data-theme 是上一个用例留在 DOM 上的
  stubSystemTheme(false);
  document.documentElement.removeAttribute('data-theme');
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

  it('ticker_items_per_page 越界夹到 1–10，非法值退回 2', async () => {
    expect((await load({ ticker_items_per_page: '0' })).tickerItemsPerPage).toBe(1);
    expect((await load({ ticker_items_per_page: '99' })).tickerItemsPerPage).toBe(10);
    // 预设档（1–4）之外的取值就是「自定义」，必须原样读回来 —— 夹成 4 等于把自定义吞掉
    expect((await load({ ticker_items_per_page: '9' })).tickerItemsPerPage).toBe(9);
    expect((await load({ ticker_items_per_page: '3' })).tickerItemsPerPage).toBe(3);
    expect((await load({ ticker_items_per_page: 'x' })).tickerItemsPerPage).toBe(2);
    expect((await load({})).tickerItemsPerPage).toBe(2);
  });

  it('布尔型设置只认 "1"/"0"，缺键时取各自默认值', async () => {
    const store = await load({
      market_overview_visible: '0',
      ticker_visible: '0',
      ticker_transparent: '1',
      ticker_click_wakes: '0',
    });
    expect(store.marketOverviewVisible).toBe(false);
    expect(store.tickerVisible).toBe(false);
    expect(store.tickerTransparent).toBe(true);
    expect(store.tickerClickWakes).toBe(false);
    // 点击唤醒默认开 —— 这是行情条一直以来的行为
    expect((await load({})).tickerClickWakes).toBe(true);
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

  it('setTickerClickWakes 写 "0"/"1" 并广播到行情条', async () => {
    const store = await load({ ticker_click_wakes: '1' });
    expect(store.tickerClickWakes).toBe(true);

    invokeMock.mockResolvedValueOnce(undefined);
    await store.setTickerClickWakes(false);
    expect(invokeMock).toHaveBeenCalledWith('set_setting', {
      key: 'ticker_click_wakes',
      value: '0',
    });
    expect(store.tickerClickWakes).toBe(false);
    expect(emitMock).toHaveBeenCalledWith('settings-changed', {
      key: 'ticker_click_wakes',
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

describe('主题：三态与跟随系统', () => {
  it('store 一建立就按解析出的主题落 data-theme（早于首屏渲染）', () => {
    stubSystemTheme(true);
    setActivePinia(createPinia());
    mockBackend({});

    // 还没 fetchSettings，设置表是空的 —— 模式取兜底的「跟随系统」，
    // 于是首帧就按系统深浅渲染。原先要等 fetchSettings 跑完才落属性，
    // 跟随系统下那会先闪一帧相反的底色。
    useSettingsStore();
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('显式模式就是设置值本身，不看系统深浅', async () => {
    stubSystemTheme(true); // 系统是深色
    const store = await load({ theme: 'light' });

    expect(store.themeMode).toBe('light');
    expect(store.theme).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('跟随系统按系统深浅解析；缺键也归「跟随系统」', async () => {
    stubSystemTheme(true);
    expect((await load({ theme: 'system' })).theme).toBe('dark');
    expect((await load({})).themeMode).toBe('system');
    expect((await load({})).theme).toBe('dark');

    stubSystemTheme(false);
    expect((await load({ theme: 'system' })).theme).toBe('light');
  });

  it('系统主题变化：跟随时实时跟随，显式模式下不采纳', async () => {
    const sys = stubSystemTheme(true);

    const following = await load({ theme: 'system' });
    expect(following.theme).toBe('dark');
    sys.change(false);
    await nextTick();
    expect(following.theme).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');

    // 显式模式下窗口主题是被钉住的，这个事件里的值可能只是应用自己 pin 出来的结果
    const explicit = await load({ theme: 'dark' });
    sys.change(true);
    await nextTick();
    expect(explicit.theme).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('setThemeMode 写库、把**模式**交给原生层，并广播 settings-changed', async () => {
    const store = await load({ theme: 'light' });
    invokeMock.mockClear();
    emitMock.mockClear();
    invokeMock.mockResolvedValue(undefined);

    await store.setThemeMode('dark');
    await nextTick();

    expect(invokeMock).toHaveBeenCalledWith('set_setting', { key: 'theme', value: 'dark' });
    // 传的必须是模式：传解析好的 light/dark 会把窗口主题钉住，而钉住会连
    // WebView 的 prefers-color-scheme 一起冻住 —— 「跟随系统」就再也不生效了
    expect(invokeMock).toHaveBeenCalledWith('set_window_theme', { theme: 'dark' });
    expect(store.themeMode).toBe('dark');
    expect(store.theme).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    // 行情条靠这一条跟着切（写入成功后 setSetting 自己发）
    expect(emitMock).toHaveBeenCalledWith('settings-changed', { key: 'theme', value: 'dark' });
    // 顺手钉住「不再有第二条通道」：theme-changed 是两态时代的产物，留着只会多一份
    // 两边说法不一致的机会（实际主题现在由各窗口自己从 matchMedia 解析）
    expect(emitMock).not.toHaveBeenCalledWith('theme-changed', expect.anything());
  });

  it('切到「跟随系统」时，读的是原生层解 pin **之后**的系统值', async () => {
    const sys = stubSystemTheme(false); // 此刻被钉成亮色，值不可信
    const store = await load({ theme: 'light' });
    invokeMock.mockClear();

    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'set_window_theme') {
        // 模拟 Rust 侧 set_theme(None)：窗口交还给系统之后 matchMedia 才变成真值。
        // 刻意**不通知**监听者 —— 只有这样才分辨得出读的是解 pin 之后的值，
        // 还是切模式之前就存下来的旧值（后者会让界面停在亮色）。
        sys.mql.matches = true;
      }
      return undefined;
    });

    await store.setThemeMode('system');
    await nextTick();

    expect(store.theme).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('applyRemoteSetting 收到 theme：只作用于本窗口，不回写库、不回广播', async () => {
    const store = await load({ theme: 'dark' });
    invokeMock.mockClear();
    emitMock.mockClear();

    store.applyRemoteSetting('theme', 'light');
    await nextTick();

    expect(store.theme).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    // 本窗口的窗口主题要跟着走（行情条没有标题栏，这一趟对 main 是同值空转）
    expect(invokeMock).toHaveBeenCalledWith('set_window_theme', { theme: 'light' });
    // 回写或回广播会让两个窗口互相通知，形成无限往返
    expect(invokeMock).not.toHaveBeenCalledWith('set_setting', expect.anything());
    expect(emitMock).not.toHaveBeenCalled();
  });
});
