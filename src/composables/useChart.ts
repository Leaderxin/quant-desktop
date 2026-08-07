import { ref, watch, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { KLineData, PeriodType, SubIndicatorType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
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
}) {
  const settings = useSettingsStore();
  const { chart, loading, error, currentPeriod, themeColors, periodToKlinecharts, syncPrecision: syncPrecisionCore, initChartCore, disposeChart: disposeChartCore, reapplyStyles } = useChartCore(options);

  let abortController: AbortController | null = null;
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

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

  function formatDateMinuteStamp(ts: number): string {
    const d = new Date(ts);
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}`;
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
    if (!hasMoreForward.value || loading.value) return [];
    const span = minuteKSpan(currentPeriod.value);
    const isSina = settings.activeDatasource === 'sina';
    if (span !== null && isSina) {
      hasMoreForward.value = false;
      return [];
    }
    loading.value = true;
    try {
      const earliest = allData.value[0];
      const endDate = earliest
        ? (span !== null
            ? formatDateMinuteStamp(earliest.timestamp)
            : formatDate(earliest.timestamp - 86_400_000))
        : undefined;
      const count = 200;
      const data = await invoke<KLineData[]>('get_kline', {
        code: unref(options.code),
        market: unref(options.market),
        period: currentPeriod.value,
        endDate,
        count,
      });
      const newBars = mapKLineToChart(data);
      let uniqueCount = 0;
      if (newBars.length > 0) {
        const existing = new Set(allData.value.map((d) => d.timestamp));
        const unique = newBars.filter((d) => !existing.has(d.timestamp));
        uniqueCount = unique.length;
        if (uniqueCount > 0) {
          allData.value = [
            ...unique.sort((a, b) => a.timestamp - b.timestamp),
            ...allData.value,
          ];
        }
      }
      klineData.value = allData.value;
      hasMoreForward.value = uniqueCount > 0 && newBars.length >= 100;
      return newBars;
    } catch (e) {
      console.error('[useChart] forward load failed:', e);
      return [];
    } finally {
      loading.value = false;
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
        if (!hasMoreForward.value) {
          params.callback([], { forward: false, backward: false });
          return;
        }
        const newBars = await loadMoreHistory();
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
      try {
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
          count: 10,
        });
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
   * VOL 副图自定义外观（成交量 + MA5/10/20 均量线，柱子配色随主题）。
   * 注意：不覆盖内置 `figures` —— v10 内置 VOL 的 volume 柱用了一个 styles 回调函数
   * 从 `indicator.styles.bars[0]` 取色，若用静态 figure 覆盖会丢失取色逻辑导致柱子不显示。
   * 因此仅通过 `styles.bars[0]` 传入柱子配色，保留内置 figures/calc。
   */
  function volIndicatorConfig() {
    const colors = themeColors();
    return {
      name: 'VOL',
      paneId: SUB_PANE_ID,
      shortName: '成交量',
      calcParams: [5, 10, 20],
      styles: {
        bars: [
          { upColor: colors.volumeBarUp, downColor: colors.volumeBarDown, noChangeColor: colors.volumeBarNoChange },
        ],
      },
    };
  }

  function syncSubIndicator(name: SubIndicatorType) {
    if (!chart.value) return;
    // v10：createIndicator(value, isStack?)，paneId 须写入 value 内；同一 paneId 且 isStack=false 会替换旧指标。
    // VOL 需带自定义外观（v10 overrideIndicator 仅对已存在指标生效，故在创建时一次性传入）。
    const create = name === 'VOL' ? volIndicatorConfig() : { name, paneId: SUB_PANE_ID };
    chart.value.createIndicator(create as any);
  }

  // ---- 数据加载 ----

  async function loadData(period: PeriodType) {
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
        hasMoreForward.value = !isSina;
        klineData.value = mapped;
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

    if (period !== 'minute') {
      // 叠加价格均线 MA5/MA10/MA20/MA60 到主图（v10：isStack=true 才会叠加，否则会替换主图已有指标）
      const existingMA = chart.value.getIndicators({ name: 'MA' });
      if (existingMA.length === 0) {
        chart.value.createIndicator({
          name: 'MA',
          calcParams: [5, 10, 20, 60],
          paneId: 'candle_pane',
        }, true);
      }

      // 周期切换后确保副图指标还在
      if (options.subIndicator) {
        const currentSub = unref(options.subIndicator)!;
        const existingSub = chart.value.getIndicators({ paneId: SUB_PANE_ID });
        if (existingSub.length === 0 || existingSub[0]?.name !== currentSub) {
          syncSubIndicator(currentSub);
        }
      }
    }
  }

  function disposeChart() {
    stopAutoRefresh();
    barSubscriber = null;
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
    disposeChartCore();
  }

  // ---- 主题/副图监听 ----

  watch(() => settings.theme, () => {
    reapplyStyles();
  });

  if (options.subIndicator) {
    watch(() => unref(options.subIndicator)!, (newVal) => {
      syncSubIndicator(newVal);
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
    applyTheme: reapplyStyles,
  };
}
