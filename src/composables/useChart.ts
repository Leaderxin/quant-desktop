import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { KLineData, PeriodType, SubIndicatorType, MainOverlayType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { subPaneHeight } from '@/utils/paneSize';
import { minuteKSpan } from './minutePeriod';
import { useChartCore } from './useChartCore';

/** 固定副图 pane ID — 切换指标时复用同一个子窗格 */
const SUB_PANE_ID = 'sub_indicator_pane';

/**
 * K 线图 composable — 用于日/周/月 K 及分钟 K 线图。
 * 含历史数据懒加载、MA 主图叠加、副图指标切换等逻辑。
 */
export function useChart(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
  subIndicator?: MaybeRef<SubIndicatorType>;
  mainOverlay?: MaybeRef<MainOverlayType>;
}) {
  const settings = useSettingsStore();
  const { chart, loading, error, currentPeriod, volIndicatorConfig, periodToKlinecharts, syncPrecision: syncPrecisionCore, initChartCore, disposeChart: disposeChartCore, reapplyStyles } = useChartCore(options);

  let abortController: AbortController | null = null;
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

  /**
   * 数据代次：每次 `loadData` 自增，异步请求回来时对不上就整批丢掉。
   *
   * 图表实例与 `allData` 都是跨标的复用的。切股时，上一只股票的 forward /
   * 增量请求可能还在路上：它回来后会往 `allData` 里 prepend 旧标的的历史，
   * 再通过旧 loader 的 callback 把同一段数据 prepend 进图表 —— 症状是切换后
   * 多出一段本不属于该股的历史（例如新标的 2010 年上市，图上却从 2000 年
   * 开始）。abortController 只覆盖主请求，覆盖不到 forward/增量，所以这里
   * 再加一道代次闸门。
   */
  let dataEpoch = 0;
  /**
   * 上一次 `loadData` 的目标（code:market:period）。
   * 换股/换周期时先停掉旧的增量定时器；同一目标的周期重载（新浪每轮全量拉）
   * 不能停，否则一次失败会把自动刷新永久关掉。
   */
  let lastLoadKey = '';

  // 累积全部已加载的 K 线数据（初始 + 历次懒加载），按时间升序
  const allData = ref<KCLineData[]>([]);
  // 标记是否还有更多历史数据可加载
  const hasMoreForward = ref(true);

  /** subscribeBar 回调引用，增量推送数据到图表避免全量重绘导致的抖动 */
  let barSubscriber: ((bar: KCLineData) => void) | null = null;

  // ---- 日期格式化 ----

  function formatDate(ts: number): string {
    const d = new Date(ts);
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  }

  // ---- 数据映射 ----

  function mapKLineToChart(data: KLineData[]): KCLineData[] {
    return data.map((d) => {
      const ts = new Date(d.date).getTime();
      return {
        timestamp: isNaN(ts) ? 0 : ts,
        open: d.open,
        high: d.high,
        low: d.low,
        close: d.close,
        volume: d.volume,
      };
    });
  }

  // ---- 懒加载历史数据 ----

  async function loadMoreHistory(): Promise<KCLineData[]> {
    const epoch = dataEpoch;
    if (!hasMoreForward.value || loading.value) return [];
    // 分钟 K 不做前向加载：腾讯 mkline 不按 end_date 翻页，传了也返回最近一段，
    // 前向结果会与已有数据整段重叠；再 prepend 进图表就是两端重复的 K 线。
    if (minuteKSpan(currentPeriod.value) !== null) {
      hasMoreForward.value = false;
      return [];
    }
    loading.value = true;
    try {
      const earliest = allData.value[0];
      const endDate = earliest ? formatDate(earliest.timestamp - 86_400_000) : undefined;
      const count = 200;
      const data = await invoke<KLineData[]>('get_kline', {
        code: unref(options.code),
        market: unref(options.market),
        period: currentPeriod.value,
        endDate,
        count,
      });
      // 切股/切周期后旧请求才回来：直接丢弃，既不改 allData，也不把翻页结果
      // 交回给旧 loader 的 callback（否则会 prepend 到新标的的图上）。
      if (epoch !== dataEpoch) return [];
      const newBars = mapKLineToChart(data);
      let uniqueBars: KCLineData[] = [];
      if (newBars.length > 0) {
        const existing = new Set(allData.value.map((d) => d.timestamp));
        uniqueBars = newBars.filter((d) => !existing.has(d.timestamp));
        if (uniqueBars.length > 0) {
          allData.value = [
            ...uniqueBars.sort((a, b) => a.timestamp - b.timestamp),
            ...allData.value,
          ];
        }
      }
      klineData.value = allData.value;
      hasMoreForward.value = uniqueBars.length > 0 && newBars.length >= 100;
      // 只把去重后的 bar 交给图表：forward 是 prepend，重复项会直接叠进图里。
      return uniqueBars;
    } catch (e) {
      console.error('[useChart] forward load failed:', e);
      return [];
    } finally {
      // 旧代次的请求不能把新代次的 loading 清掉
      if (epoch === dataEpoch) loading.value = false;
    }
  }

  // ---- 数据加载器 ----

  const klineData = ref<KCLineData[]>([]);

  const dataLoader: DataLoader = {
    getBars: async (params) => {
      if (params.type === 'init') {
        params.callback(allData.value, {
          forward: hasMoreForward.value,
          backward: false,
        });
      } else if (params.type === 'forward') {
        const epoch = dataEpoch;
        if (!hasMoreForward.value) {
          params.callback([], { forward: false, backward: false });
          return;
        }
        const newBars = await loadMoreHistory();
        // 请求属于旧标的：连 callback 都不能调 —— 它会 prepend 到当前图表。
        // 旧图表实例的 _loading 会被切股时的 setSymbol/setDataLoader 重置，不用收尾。
        if (epoch !== dataEpoch) return;
        params.callback(newBars, {
          forward: hasMoreForward.value,
          backward: false,
        });
      } else {
        params.callback([], {
          forward: hasMoreForward.value,
          backward: false,
        });
      }
    },
    subscribeBar: ({ callback }) => {
      barSubscriber = callback;
    },
    unsubscribeBar: () => {
      barSubscriber = null;
    },
  };

  // ---- 自动刷新 ----

  function getRefreshInterval(period: PeriodType): number {
    const span = minuteKSpan(period);
    if (span !== null) return span <= 5 ? 8000 : 20000;
    switch (period) {
      case 'daily':  return 30000;
      case 'weekly': return 60000;
      case 'monthly':return 60000;
      default:       return 30000;
    }
  }

  function startAutoRefresh(period: PeriodType) {
    stopAutoRefresh();

    // Sina doesn't support incremental refresh; use old full-reload behavior
    if (settings.activeDatasource === 'sina') {
      const interval = getRefreshInterval(period);
      refreshTimer = setInterval(() => {
        if (!loading.value) {
          loadData(period);
        }
      }, interval);
      return;
    }

    // K-line: incremental refresh
    const interval = getRefreshInterval(period);
    refreshTimer = setInterval(async () => {
      if (loading.value) return;
      const epoch = dataEpoch;
      try {
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
          count: 10,
        });
        // 切股/切周期后旧定时器的请求才回来：丢弃，别把旧标的的最新几根
        // 合并进新标的的 allData，更别推给新图表。
        if (epoch !== dataEpoch || period !== currentPeriod.value) return;
        const newBars = mapKLineToChart(data);
        if (newBars.length > 0) {
          const map = new Map(allData.value.map((d) => [d.timestamp, d]));
          for (const bar of newBars) {
            map.set(bar.timestamp, bar);
          }
          allData.value = [...map.values()].sort((a, b) => a.timestamp - b.timestamp);
          klineData.value = allData.value;
          if (barSubscriber) {
            for (const bar of newBars) {
              barSubscriber(bar);
            }
          } else if (chart.value) {
            chart.value.setDataLoader(dataLoader);
          }
        }
      } catch (e) {
        console.error('[useChart] incremental update failed:', e);
      }
    }, interval);
  }

  function stopAutoRefresh() {
    if (refreshTimer !== null) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
  }

  // ---- 副图指标 ----

  /**
   * 副图高度。klinecharts 的 pane 高度是**固定像素**，主图吃掉剩余空间；
   * 默认 100px 在小视口下会占到图区三分之一以上，喧宾夺主。口径见 utils/paneSize
   * （分时图的成交量副图共用同一份，切周期时高度不跳）。
   */
  function currentSubPaneHeight(): number {
    return subPaneHeight(options.chartRef.value?.clientHeight ?? 0);
  }

  function applySubPaneHeight() {
    if (!chart.value) return;
    chart.value.setPaneOptions({ id: SUB_PANE_ID, height: currentSubPaneHeight() });
  }

  /** 图区高度随详情面板/窗口变化时同步副图比例（不监听会一直停留在初始像素值） */
  let paneResizeObserver: ResizeObserver | null = null;

  function observePaneResize() {
    const el = options.chartRef.value;
    if (!el || typeof ResizeObserver === 'undefined' || paneResizeObserver) return;
    paneResizeObserver = new ResizeObserver(() => {
      // 等 klinecharts 自己的 ResizeObserver 先按新高度排完，下一帧再改副图
      requestAnimationFrame(applySubPaneHeight);
    });
    paneResizeObserver.observe(el);
  }

  function syncSubIndicator(name: SubIndicatorType) {
    if (!chart.value) return;
    // v10：createIndicator(value, isStack?)，paneId 须写入 value 内；同一 paneId 且 isStack=false 会替换旧指标。
    // VOL 需带自定义外观（v10 overrideIndicator 仅对已存在指标生效，故在创建时一次性传入）。
    const create = name === 'VOL' ? volIndicatorConfig(SUB_PANE_ID) : { name, paneId: SUB_PANE_ID };
    chart.value.createIndicator(create as any);
    // 切换指标可能重建 pane（回到默认 100px），每次创建后都按当前图高再定一次
    applySubPaneHeight();
  }

  // ---- 主图叠加指标 ----

  /** 主图叠加的指标名集合 — 切换时用于移除另一个 */
  const MAIN_OVERLAY_NAMES: MainOverlayType[] = ['MA', 'BOLL'];

  /**
   * 同步主图叠加指标（MA / BOLL）。
   * 二者均为价格类指标（series: 'price'），叠加在蜡烛 pane（candle_pane）上，而非副图窗格。
   * isStack=true 表示叠加，避免替换主图；切换时先移除另一个再确保当前指标存在。
   */
  function syncMainOverlay(name: MainOverlayType) {
    if (!chart.value) return;
    for (const other of MAIN_OVERLAY_NAMES) {
      if (other !== name) {
        chart.value.removeIndicator({ paneId: 'candle_pane', name: other });
      }
    }
    const existing = chart.value.getIndicators({ paneId: 'candle_pane', name });
    if (existing.length === 0) {
      const config = name === 'MA'
        ? { name, calcParams: [5, 10, 20, 60], paneId: 'candle_pane' }
        : { name, paneId: 'candle_pane' };
      chart.value.createIndicator(config, true);
    }
  }

  // ---- 数据加载 ----

  async function loadData(period: PeriodType) {
    const loadKey = `${unref(options.code)}:${unref(options.market)}:${period}`;
    if (loadKey !== lastLoadKey) {
      // 换股/换周期：旧定时器必须立刻停。新标的加载失败时 catch 不会走到
      // startAutoRefresh，旧定时器却会用新代码拉数据、推进仍显示旧标的的图表。
      stopAutoRefresh();
      lastLoadKey = loadKey;
    }
    // 先换代号：从这一刻起，旧标的的 forward/增量请求回来都会被丢掉。
    dataEpoch += 1;
    if (abortController) {
      abortController.abort();
    }
    abortController = new AbortController();
    const { signal } = abortController;

    loading.value = true;
    error.value = '';

    // 重置累积数据和分页状态（切换股票/周期时重新开始）
    allData.value = [];
    hasMoreForward.value = true;

    try {
      const data = await invoke<KLineData[]>('get_kline', {
        code: unref(options.code),
        market: unref(options.market),
        period: period,
      });
      if (signal.aborted) return;

      if (data.length) {
        const mapped = mapKLineToChart(data);
        allData.value = mapped.sort((a, b) => a.timestamp - b.timestamp);
        const isSina = settings.activeDatasource === 'sina';
        // 分钟 K 不做前向加载：mkline 不按 end_date 翻页，见 loadMoreHistory。
        hasMoreForward.value = !isSina && minuteKSpan(period) === null;
        klineData.value = mapped;
      } else {
        // 空响应也要清掉上一只股票的数据：syncPrecision 还会读 klineData，
        // 留着旧精度会让新标的（比如 ETF 3 位 vs 股票 2 位）显示错。
        klineData.value = [];
        hasMoreForward.value = false;
      }

      if (signal.aborted) return;
      if (chart.value) {
        chart.value.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
        chart.value.setPeriod(periodToKlinecharts(period));
        chart.value.setDataLoader(dataLoader);
        syncPrecisionCore(klineData.value);
      }
      startAutoRefresh(period);
    } catch (e) {
      if (signal.aborted) return;
      error.value = `加载数据失败: ${String(e).slice(0, 160)}`;
      console.error('[useChart] loadData failed:', e);
    } finally {
      if (!signal.aborted) {
        loading.value = false;
      }
    }
  }

  // ---- 初始化 ----

  function initChart(period: PeriodType) {
    const isNew = initChartCore(period);

    if (isNew) {
      // 首次创建副图指标（VOL 自定义外观在 volIndicatorConfig 中一次性传入）
      if (options.subIndicator) {
        syncSubIndicator(unref(options.subIndicator)!);
      }
    }

    if (!chart.value) return;
    observePaneResize();

    if (period !== 'minute') {
      // 叠加主图指标（MA/BOLL）到蜡烛 pane
      syncMainOverlay(unref(options.mainOverlay) ?? 'MA');

      // 周期切换后确保副图指标还在
      if (options.subIndicator) {
        const currentSub = unref(options.subIndicator)!;
        const existingSub = chart.value.getIndicators({ paneId: SUB_PANE_ID });
        if (existingSub.length === 0 || existingSub[0]?.name !== currentSub) {
          syncSubIndicator(currentSub);
        }
      }
      applySubPaneHeight();
    }
  }

  function disposeChart() {
    stopAutoRefresh();
    barSubscriber = null;
    paneResizeObserver?.disconnect();
    paneResizeObserver = null;
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
    disposeChartCore();
  }

  // 组件卸载时 useChartCore 只销毁图表实例，这里补上自己起的 ResizeObserver
  onUnmounted(() => {
    paneResizeObserver?.disconnect();
    paneResizeObserver = null;
  });

  // ---- 主题/副图监听 ----

  // 主题或涨跌配色变化：全局样式之外，VOL 的量柱配色是创建指标时的实例级
  // styles（见 volIndicatorConfig 注释），setStyles 压不到已存在的 VOL，
  // 须用 overrideIndicator 重新传入，否则量柱停留在旧配色
  watch(() => [settings.theme, settings.colorScheme], () => {
    reapplyStyles();
    if (chart.value && unref(options.subIndicator) === 'VOL') {
      chart.value.overrideIndicator(volIndicatorConfig(SUB_PANE_ID) as any);
    }
  });

  if (options.subIndicator) {
    watch(() => unref(options.subIndicator)!, (newVal) => {
      syncSubIndicator(newVal);
    });
  }

  if (options.mainOverlay) {
    watch(() => unref(options.mainOverlay)!, (newVal) => {
      syncMainOverlay(newVal);
    });
  }

  return {
    chart,
    loading,
    error,
    klineData,
    initChart,
    loadData,
    disposeChart,
  };
}
