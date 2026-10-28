// src/stores/settings.ts
import { defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import {
  parseAccelerator,
  parseBool,
  parseColumns,
  parseCount,
  parseDefaultSort,
  parseJsonArray,
  parseThemeMode,
  clampTopN,
  clampTickerItems,
  type ColumnKey,
  type DefaultSort,
  type ThemeMode,
} from '@/utils/prefs';
import { resolveTheme, systemPrefersDark, watchSystemTheme, type ResolvedTheme } from '@/utils/theme';

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
  // ── 主题 ──
  //
  // 库里存的是**模式**（亮色 / 暗色 / 跟随系统），窗口上用的才是解析后的实际主题。
  // 两态存库做不到这件事：「系统现在是不是深色」没有地方待，而且模式一旦被解析掉，
  // 就没法把它交给原生层 —— 而「跟随系统」在原生层是一件事（窗口主题交给系统），
  // 不是一个色值（见 lib.rs 的 set_app_theme）。
  /** 模式：设置页选的那个，唯一真源是设置表。 */
  const themeMode = computed<ThemeMode>(() => parseThemeMode(settings.value['theme']));
  /** 系统深浅。**只在跟随系统下可信** —— 显式主题会把窗口主题钉住，而钉住会连
   *  WebView 的 `prefers-color-scheme` 一起冻在应用自己的主题上（见 utils/theme.ts）。 */
  const systemDark = ref(systemPrefersDark());
  /** 实际生效的主题。所有渲染判断（naive-ui 主题、图表配色、图标）读的都是它 ——
   *  「system」是个模式不是一种颜色，谁把它当颜色用，谁就静默落到亮色分支。 */
  const theme = computed<ResolvedTheme>(() => resolveTheme(themeMode.value, systemDark.value));
  const autoLaunch = ref(false);
  const isPortable = ref(false);
  const isStoreBuild = ref(false);
  // fetchSettings 是否已完成（无论成败）。依赖 isPortable/isStoreBuild 的 UI
  // 必须同时等待它，否则商店/便携构建启动时会先以 false 渲染、闪现本应隐藏的控件。
  const loaded = ref(false);
  const error = ref<string | null>(null);
  /** 老板键注册失败的原因（组合键被占用等）。只作用于设置页那一行。 */
  const bossKeyError = ref<string | null>(null);

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

  /** 老板键；空串表示未设置（见 utils/prefs 的 parseAccelerator）。 */
  const bossKey = computed(() => parseAccelerator(settings.value['boss_key']));

  async function fetchSettings() {
    try {
      settings.value = await invoke<Record<string, string>>('get_settings');
      activeDatasource.value = settings.value['active_datasource'] || 'tencent';
      datasources.value = await invoke<[string, string][]>('list_datasources');
      indexPool.value = await invoke<[string, string][]>('list_index_pool');
      autoLaunch.value = await invoke<boolean>('get_autostart');
      isPortable.value = await invoke<boolean>('get_portable_mode');
      isStoreBuild.value = await invoke<boolean>('is_store_build');
    } catch (e) {
      console.error('[settings] Failed to fetch settings:', e);
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

  /**
   * 老板键。走 IPC 命令而不是 `setSetting` —— 改热键要真的去操作系统注册，
   * 而注册会失败（组合键已被别的程序占着）。这个失败必须留在设置页上：
   * `error` 那条横幅在主界面，用户正对着设置页，看不到它。
   * 失败时 store 里的值不动，所以界面显示的还是旧键 —— 与后端一致
   * （后端失败时会回滚到旧键，见 `set_boss_key`）。
   */
  async function setBossKey(accelerator: string) {
    try {
      await invoke('set_boss_key', { accelerator });
      settings.value['boss_key'] = accelerator;
      bossKeyError.value = null;
    } catch (e) {
      console.error('[settings] setBossKey failed:', e);
      bossKeyError.value = String(e);
    }
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

  /**
   * 把主题模式交给原生层 —— 标题栏 + 「窗口主题是否钉住」。
   *
   * 传下去的必须是**模式**而不是解析后的主题：`"system"` 会让 Rust 调
   * `window.set_theme(None)`，把窗口交还给系统；传 light/dark 则是把窗口钉住，
   * 而钉住会连 WebView 的 `prefers-color-scheme` 一起冻在应用主题上 —— 那正是
   * 「跟随系统」不生效的成因（详见 src-tauri/src/lib.rs 的 set_app_theme）。
   *
   * `<html data-theme>` 只影响网页内容 —— **标题栏是操作系统画的**，不告诉窗口的话
   * 暗色主题下会照旧是白的。命令只作用于 main 窗口，所以行情条调它只是白跑一次 IPC
   * （罕见、且 tao 对同值会提前返回），换来的是「任何改主题的路径都不会漏掉标题栏」。
   *
   * 启动时那一次不靠这里，在 Rust 侧（`set_app_theme`，且在 `main.show()` 之前）——
   * 窗口是先 show 出来、webview 才加载完的，只靠这里会先闪一帧白色标题栏。
   */
  function applyWindowTheme(mode: ThemeMode): Promise<void> {
    return invoke<void>('set_window_theme', { theme: mode }).catch((e) => {
      console.error('[settings] set_window_theme failed:', e);
    });
  }

  // 主题的三条同步链。所有改主题的路径最后都汇到这里，谁都不许自己抄一份 ——
  // 抄的那份迟早漏掉「解 pin」或「重读系统值」中的一步，而漏了在界面上看不出来。
  //
  // 1) 解析出的主题变了 → 落 `<html data-theme>`。
  //    immediate：store 一建立就定好属性。App.vue 在 setup 里就取 store，早于首屏
  //    渲染，所以跟随系统下首帧就是对的（原先要等 fetchSettings 跑完）。
  watch(theme, (t) => {
    document.documentElement.setAttribute('data-theme', t);
  }, { immediate: true });

  // 2) 模式变了 → 交给原生层，然后**重新**读一次系统深浅。
  //    顺序不能反：解 pin 之前 matchMedia 读回来的是应用自己钉的值，先读就会把
  //    「刚从显式切到跟随系统」解析成旧主题（下一帧会自愈，但那一下是闪变）。
  //    不用 immediate：store 刚建立时设置表还空着，那一刻发 IPC 等于拿兜底模式去
  //    改窗口主题 —— 而启动时 Rust 已经按库里的值设过、且早于 `main.show()`。
  watch(themeMode, async (mode) => {
    await applyWindowTheme(mode);
    systemDark.value = systemPrefersDark();
  });

  // 3) 系统主题变化。只在跟随系统时采纳：显式模式下窗口被钉住，这个事件里的值可能
  //    只是我们自己 pin 出来的结果（切模式那一下就会触发一次）。
  //    不接返回值：store 与进程同生命周期，没有要摘监听的时候。
  watchSystemTheme((dark) => {
    if (themeMode.value === 'system') systemDark.value = dark;
  });

  /**
   * 改主题模式（设置页三选一）。
   *
   * 只写库：本窗口与行情条的实际应用都由上面那两条 watch 接手，行情条收到
   * `settings-changed` 后走的也是同一条路（它读不到本窗口的 store）。
   *
   * **不再单独广播 `theme-changed`**：那是两态时代的产物 —— 它带的是解析好的实际
   * 主题，而现在每个窗口都能自己解析（行情条窗口的主题从没被钉过，它的 matchMedia
   * 永远是真系统值），多一条通道只会多一份「两边说法不一致」的机会。
   */
  async function setThemeMode(mode: ThemeMode) {
    await setSetting('theme', mode);
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
   *
   * `theme` 这一项落到设置表后还会触发上面那条模式 watch（原生同步 + 重读系统
   * 深浅）—— 那是必须的：行情条就是靠它跟着切主题的，而它读的是本窗口自己的
   * matchMedia，不是主窗口解析好的结果。
   */
  function applyRemoteSetting(key: string, value: string) {
    settings.value[key] = value;
  }

  return {
    settings, datasources, indexPool, activeDatasource, theme, themeMode, autoLaunch,
    isPortable, isStoreBuild, loaded, updaterAvailable, error, bossKeyError,
    // 派生配置
    indexCodes, marketOverviewVisible, sectorTopN, tickerVisible, tickerTransparent,
    tickerItemsPerPage, watchlistColumns, watchlistDefaultSort, colorScheme, bossKey,
    // 动作
    fetchSettings, setSetting, switchDatasource, setThemeMode, toggleAutoLaunch,
    applyColorScheme, applyRemoteSetting,
    setIndexCodes, setMarketOverviewVisible, setSectorTopN, setTickerVisible,
    setTickerTransparent, setTickerItemsPerPage, setWatchlistColumns,
    setWatchlistDefaultSort, setColorScheme, setBossKey,
  };
});
