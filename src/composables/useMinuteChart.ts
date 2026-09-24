import { ref, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { AxisCreateTicksParams, AxisTick, KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { MinuteData } from '@/types';
import { mapMinuteBars } from '@/utils/minuteBars';
import { minuteAxisLayout, sessionTicks } from '@/utils/minuteAxis';
import { useChartCore } from './useChartCore';

/**
 * 分时图 composable — 仅用于 MinuteChart 组件。
 * 不含副图指标、K 线懒加载等逻辑，与 K 线图表完全隔离。
 * 数据映射（bar 构造 + 只取当前交易日）见 @/utils/minuteBars；
 * 横轴排版（固定一个交易日）见 @/utils/minuteAxis。
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

  // ---- 横轴：固定一个交易日 ----
  // 分时图横轴永远是 09:30–15:00，曲线自左端向右生长。klinecharts 默认按「最近若干根」
  // 排版、数据不够就贴右边，所以每次数据或尺寸变化后都要把格子重排一次（见 @/utils/minuteAxis）。

  /** bar 的分钟跨度，刻度换算要用；跟随数据更新 */
  let axisBarMinutes = 1;
  let resizeObserver: ResizeObserver | null = null;

  function applySessionAxis() {
    const c = chart.value;
    const bars = klineData.value;
    if (!c || bars.length === 0) return;
    // 图区宽度取主图 bounding：yAxis 是 inside 的，等于整幅宽度
    const width = c.getSize('candle_pane', 'main')?.width ?? 0;
    if (!(width > 0)) return;

    const layout = minuteAxisLayout(width, bars);
    axisBarMinutes = layout.barMinutes;
    c.setBarSpace(layout.barSpace);
    // setBarSpace 超出 barSpaceLimit 会被忽略，留白按实际柱宽算，两者才不打架。
    // 第二个参数 d.ts 里没声明，但实现接受 isUpdate：不传的话留白只被记下、不当场重排。
    const offset = Math.max(0, width - bars.length * c.getBarSpace().bar);
    (c.setOffsetRightDistance as (distance: number, isUpdate?: boolean) => void)(offset, true);
  }

  /** 固定刻度 09:30 / 10:30 / 11:30-13:00 / 14:00 / 15:00，位置跟随实际柱宽 */
  function minuteAxisTicks(params: AxisCreateTicksParams): AxisTick[] {
    const c = chart.value;
    if (!c) return params.defaultTicks;
    const ticks = sessionTicks(params.bounding.width, c.getBarSpace().bar, axisBarMinutes);
    // 图还没量到宽度时退回 klinecharts 自己的刻度，别把横轴清空
    return ticks.length > 0 ? ticks : params.defaultTicks;
  }

  /** 尺寸变化后重排：柱宽不再等于「图宽 / 240」时，左侧又会冒出留白 */
  function observeResize() {
    const el = options.chartRef.value;
    if (!el || typeof ResizeObserver === 'undefined') return;
    resizeObserver?.disconnect();
    resizeObserver = new ResizeObserver(() => {
      // 先让 klinecharts 自己的 ResizeObserver 更新内部尺寸，下一帧再按新宽度重排，
      // 否则这里读到的还是旧宽度
      requestAnimationFrame(() => applySessionAxis());
    });
    resizeObserver.observe(el);
  }

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
          applySessionAxis();
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
        applySessionAxis();
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
    if (!chart.value) return;
    chart.value.overrideXAxis({ createTicks: minuteAxisTicks });
    // 横轴固定为一个交易日，拖拽与缩放都会把它带偏：拖完会被上面的重排拉回来，
    // 缩完柱宽就不再等于「图宽 / 格子数」。分时图本来也不给拖，索性关掉这两个手势。
    chart.value.setScrollEnabled(false);
    chart.value.setZoomEnabled(false);
    observeResize();
  }

  function disposeChart() {
    stopAutoRefresh();
    barSubscriber = null;
    resizeObserver?.disconnect();
    resizeObserver = null;
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
