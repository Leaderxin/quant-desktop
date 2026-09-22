// src/stores/settings.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import {
  parseBool,
  parseColumns,
  parseCount,
  parseDefaultSort,
  parseJsonArray,
  clampTopN,
  clampTickerItems,
  type ColumnKey,
  type DefaultSort,
} from '@/utils/prefs';

/** 设置项默认值（前端侧兜底）。后端 `init_defaults()` 会在首启时写入同名的键，
 *  这里的值只在键缺失（旧库、手工删键）时生效。 */
const DEFAULTS = {
  sector_top_n: '5',
  ticker_items_per_page: '2',
} as const;

/** 指数区默认勾选（与后端 `datasource::DEFAULT_INDEX_CODES` 一致）。
 *  仅在 `index_codes` 键损坏/缺失时兜底 —— 正常路径下总是从设置里读。 */
const FALLBACK_INDEX_CODES = [
  's_sh000001', 's_sz399001', 's_sz399006', 's_sh000688',
  's_sh000698', 's_sh000905', 's_sh000680',
];

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Record<string, string>>({});
  const datasources = ref<[string, string][]>([]);
  /** 指数候选池 (代码, 名称)。名字由后端提供，见 datasource::INDEX_POOL_NAMES。 */
  const indexPool = ref<[string, string][]>([]);
  const activeDatasource = ref('tencent');
  const theme = ref<'dark' | 'light'>('light');
  const autoLaunch = ref(false);
  const isPortable = ref(false);
  const isStoreBuild = ref(false);
  // fetchSettings 是否已完成（无论成败）。依赖 isPortable/isStoreBuild 的 UI
  // 必须同时等待它，否则商店/便携构建启动时会先以 false 渲染、闪现本应隐藏的控件。
  const loaded = ref(false);
  const error = ref<string | null>(null);

  // 自更新 UI 可见性的单一判定点（状态栏按钮等）。行为层面的拦截在
  // updater store 的 checkForUpdate 内统一处理，此处只管"要不要显示"。
  const updaterAvailable = computed(() => loaded.value && !isPortable.value && !isStoreBuild.value);

  // ── 派生配置 ──
  //
  // 全部做成 computed 而不是各自维护一份 ref：设置的唯一真源是 `settings` 这个
  // 字符串表，写回后派生值自动更新，不存在「本地 state 与库里不一致」的窗口。

  /** 顶栏展示哪些指数、以及从左到右的顺序。 */
  const indexCodes = computed<string[]>(() => {
    const parsed = parseJsonArray<string>(settings.value['index_codes'], []);
    return parsed.length > 0 ? parsed : FALLBACK_INDEX_CODES;
  });

  const marketOverviewVisible = computed(() =>
    parseBool(settings.value['market_overview_visible'], true));

  const sectorTopN = computed(() =>
    parseCount(settings.value['sector_top_n'] ?? DEFAULTS.sector_top_n, 5, clampTopN));

  const tickerVisible = computed(() => parseBool(settings.value['ticker_visible'], true));

  const tickerTransparent = computed(() =>
    parseBool(settings.value['ticker_transparent'], false));

  const tickerItemsPerPage = computed(() =>
    parseCount(
      settings.value['ticker_items_per_page'] ?? DEFAULTS.ticker_items_per_page,
      2,
      clampTickerItems,
    ));

  const watchlistColumns = computed<ColumnKey[]>(() =>
    parseColumns(settings.value['watchlist_columns']));

  const watchlistDefaultSort = computed<DefaultSort | null>(() =>
    parseDefaultSort(settings.value['watchlist_default_sort']));

  const colorScheme = computed<'cn' | 'us'>(() =>
    settings.value['color_scheme'] === 'us' ? 'us' : 'cn');

  async function fetchSettings() {
    try {
      settings.value = await invoke<Record<string, string>>('get_settings');
      activeDatasource.value = settings.value['active_datasource'] || 'tencent';
      theme.value = (settings.value['theme'] as 'dark' | 'light') || 'light';
      datasources.value = await invoke<[string, string][]>('list_datasources');
      indexPool.value = await invoke<[string, string][]>('list_index_pool');
      autoLaunch.value = await invoke<boolean>('get_autostart');
      isPortable.value = await invoke<boolean>('get_portable_mode');
      isStoreBuild.value = await invoke<boolean>('is_store_build');
    } catch (e) {
      console.error('Failed to fetch settings:', e);
      error.value = `加载设置失败: ${e}`;
    } finally {
      loaded.value = true;
    }
  }

  async function toggleAutoLaunch() {
    try {
      const newValue = !autoLaunch.value;
      // Persist to DB first so that on restart the app knows the desired state.
      await setSetting('auto_launch', String(newValue));
      // Then toggle the OS-level autostart. The backend picks the mechanism:
      // StartupTask for Store builds, registry Run key otherwise.
      await invoke('set_autostart', { enabled: newValue });
      autoLaunch.value = newValue;
    } catch (e) {
      console.error('[settings] toggleAutoLaunch failed:', e);
      error.value = `自动启动切换失败: ${e}`;
    }
  }

  async function setSetting(key: string, value: string) {
    try {
      await invoke('set_setting', { key, value });
      settings.value[key] = value;
      // 行情条是独立窗口，读不到本窗口的 store，只能靠事件同步。
      // 主窗口自己也会收到这个事件，但它不注册监听（见 TickerBar），所以不会成环。
      emit('settings-changed', { key, value }).catch((e) => {
        console.error('[settings] Failed to emit settings-changed:', e);
      });
    } catch (e) {
      console.error(`[settings] setSetting('${key}') failed:`, e);
      error.value = `保存设置失败: ${e}`;
    }
  }

  // ── 各配置项的写入入口 ──
  // 统一在这里做序列化与夹取，调用方（设置页各分区）只管传语义上的值。

  async function setIndexCodes(codes: string[]) {
    await setSetting('index_codes', JSON.stringify(codes));
  }

  async function setMarketOverviewVisible(v: boolean) {
    await setSetting('market_overview_visible', v ? '1' : '0');
  }

  async function setSectorTopN(n: number) {
    await setSetting('sector_top_n', String(clampTopN(n)));
  }

  /**
   * 显示/隐藏行情条。走 IPC 命令而不是前端窗口 API —— 窗口的显示需要补
   * always_on_top / skip_taskbar / WS_EX_TOOLWINDOW / 位置还原，这些只在
   * Rust 侧实现一次（见 `crate::set_ticker_visible`），与托盘菜单共用。
   */
  async function setTickerVisible(visible: boolean) {
    try {
      await invoke('set_ticker_visible', { visible });
      settings.value['ticker_visible'] = visible ? '1' : '0';
      emit('settings-changed', { key: 'ticker_visible', value: settings.value['ticker_visible'] })
        .catch((e) => console.error('[settings] Failed to emit settings-changed:', e));
    } catch (e) {
      console.error('[settings] setTickerVisible failed:', e);
      error.value = `切换行情条显示失败: ${e}`;
    }
  }

  async function setTickerTransparent(v: boolean) {
    await setSetting('ticker_transparent', v ? '1' : '0');
  }

  async function setTickerItemsPerPage(n: number) {
    await setSetting('ticker_items_per_page', String(clampTickerItems(n)));
  }

  async function setWatchlistColumns(cols: ColumnKey[]) {
    await setSetting('watchlist_columns', JSON.stringify(cols));
  }

  async function setWatchlistDefaultSort(sort: DefaultSort | null) {
    await setSetting('watchlist_default_sort', sort ? JSON.stringify(sort) : '');
  }

  async function setColorScheme(scheme: 'cn' | 'us') {
    await setSetting('color_scheme', scheme);
    applyColorScheme(scheme);
  }

  async function switchDatasource(name: string) {
    const previous = activeDatasource.value;
    try {
      await invoke('switch_datasource', { name });
      activeDatasource.value = name;
      settings.value['active_datasource'] = name;
      emit('datasource-changed', { datasource: name }).catch((e) => {
        console.error('[settings] Failed to emit datasource-changed:', e);
      });
    } catch (e) {
      activeDatasource.value = previous;
      error.value = `数据源切换失败: ${e}`;
      console.error('[settings] switchDatasource failed:', e);
    }
  }

  async function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', theme.value);
    await setSetting('theme', theme.value);
    emit('theme-changed', { theme: theme.value }).catch((e) => {
      console.error('[settings] Failed to emit theme-changed:', e);
    });
  }

  function applyTheme(t: 'dark' | 'light') {
    theme.value = t;
    document.documentElement.setAttribute('data-theme', t);
    // NOTE: does NOT emit 'theme-changed' — only toggleTheme() broadcasts.
    // If applyTheme emitted, the ticker's theme-changed listener would call
    // applyTheme again, creating an infinite event loop between windows.
  }

  /**
   * 应用涨跌配色方案（A 股红涨绿跌 / 欧美绿涨红跌）。
   *
   * 实现是切 `<html data-color-scheme>`，由 variables.css 里的覆盖块翻转
   * `--color-up` / `--color-down` 及其派生色。不做成给每个组件传 prop ——
   * 涨跌色散落在表格、图表、指数卡、板块榜等十几处，逐个传参必然会漏。
   */
  function applyColorScheme(scheme: 'cn' | 'us') {
    document.documentElement.setAttribute('data-color-scheme', scheme);
  }

  /**
   * 应用另一个窗口广播过来的设置变更（见 `setSetting` 里的 `settings-changed`）。
   *
   * 只更新本地 state，不写库也不回广播 —— 变更源头是发起窗口，它已经落过库了，
   * 回广播会形成两个窗口之间的无限往返。
   *
   * 为什么按 payload 就地更新而不是重拉整份设置：行情条是独立窗口，
   * `fetchSettings()` 会顺带拉数据源列表、指数池、自启状态等 5 个命令，
   * 为一个开关付这个代价不值得。
   */
  function applyRemoteSetting(key: string, value: string) {
    settings.value[key] = value;
  }

  return {
    settings, datasources, indexPool, activeDatasource, theme, autoLaunch,
    isPortable, isStoreBuild, loaded, updaterAvailable, error,
    // 派生配置
    indexCodes, marketOverviewVisible, sectorTopN, tickerVisible, tickerTransparent,
    tickerItemsPerPage, watchlistColumns, watchlistDefaultSort, colorScheme,
    // 动作
    fetchSettings, setSetting, switchDatasource, toggleTheme, toggleAutoLaunch, applyTheme,
    applyColorScheme, applyRemoteSetting,
    setIndexCodes, setMarketOverviewVisible, setSectorTopN, setTickerVisible,
    setTickerTransparent, setTickerItemsPerPage, setWatchlistColumns,
    setWatchlistDefaultSort, setColorScheme,
  };
});
