import { ref, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { MinuteData } from '@/types';
import { mapMinuteBars } from '@/utils/minuteBars';
import { useChartCore } from './useChartCore';

/**
 * 分时图 composable — 仅用于 MinuteChart 组件。
 * 不含副图指标、K 线懒加载等逻辑，与 K 线图表完全隔离。
 * 数据映射（bar 构造 + 只取当前交易日）见 @/utils/minuteBars。
 */
export function useMinuteChart(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
}) {
  const { chart, loading, error, periodToKlinecharts, syncPrecision, initChartCore, disposeChart: coreDispose, reapplyStyles } = useChartCore(options);

  let abortController: AbortController | null = null;
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

  /** subscribeBar 回调引用，增量推送数据到图表避免全量重绘导致的抖动 */
  let barSubscriber: ((bar: KCLineData) => void) | null = null;

  // ---- 数据加载器 ----

  const klineData = ref<KCLineData[]>([]);

  const dataLoader: DataLoader = {
    getBars: async (params) => {
      if (params.type === 'init') {
        params.callback(klineData.value, { forward: false, backward: false });
      } else if (params.type === 'forward') {
        params.callback([], { forward: false, backward: false });
      } else {
        params.callback([], { forward: false, backward: false });
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

  function startAutoRefresh() {
    stopAutoRefresh();

    // 分时图增量刷新：通过 barSubscriber 推送增量 bar 避免全量重绘闪烁
    refreshTimer = setInterval(async () => {
      if (loading.value) return;
      try {
        const data = await invoke<MinuteData[]>('get_intraday', {
          code: unref(options.code),
          market: unref(options.market),
        });
        const allBars = mapMinuteBars(data);
        if (allBars.length > 0) {
          const now = Date.now();
          const validBars = allBars.filter((b) => b.timestamp <= now);
          const newLast = validBars[validBars.length - 1];
          klineData.value = validBars;
          if (barSubscriber) {
            if (newLast) barSubscriber(newLast);
          } else if (chart.value) {
            chart.value.setDataLoader(dataLoader);
          }
        }
      } catch (e) {
        console.error('[useMinuteChart] incremental update failed:', e);
      }
    }, 5000);
  }

  function stopAutoRefresh() {
    if (refreshTimer !== null) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
  }

  // ---- 数据加载 ----

  async function loadData() {
    if (abortController) {
      abortController.abort();
    }
    abortController = new AbortController();
    const { signal } = abortController;

    loading.value = true;
    error.value = '';

    try {
      const data = await invoke<MinuteData[]>('get_intraday', {
        code: unref(options.code),
        market: unref(options.market),
      });
      if (signal.aborted) return;

      if (data.length) {
        klineData.value = mapMinuteBars(data);
      }

      if (signal.aborted) return;
      if (chart.value) {
        chart.value.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
        chart.value.setPeriod(periodToKlinecharts('minute'));
        chart.value.setDataLoader(dataLoader);
        syncPrecision(klineData.value);
      }
      startAutoRefresh();
    } catch (e) {
      if (signal.aborted) return;
      error.value = `加载数据失败: ${String(e).slice(0, 160)}`;
      console.error('[useMinuteChart] loadData failed:', e);
    } finally {
      if (!signal.aborted) {
        loading.value = false;
      }
    }
  }

  // ---- 初始化 ----

  function initChart() {
    initChartCore('minute');
  }

  function disposeChart() {
    stopAutoRefresh();
    barSubscriber = null;
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
    coreDispose();
  }

  return {
    loading,
    error,
    initChart,
    loadData,
    disposeChart,
    applyTheme: reapplyStyles,
  };
}
