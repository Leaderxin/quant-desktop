import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { AxisCreateRangeParams, AxisCreateTicksParams, AxisRange, AxisTick, KLineData as KCLineData, DataLoader, YAxisOverride } from 'klinecharts';
import type { MinuteData } from '@/types';
import { beijingNow, mapMinuteBars, sessionKeyOf, sessionPrevClose, sessionUpdate } from '@/utils/minuteBars';
import { inferBarMinutes, minuteAxisLayout, percentText, sessionFits, sessionTicks, symmetricRange } from '@/utils/minuteAxis';
import { useChartCore } from './useChartCore';

/**
 * 分时图 composable — 仅用于 MinuteChart 组件。
 * 不含副图指标、K 线懒加载等逻辑，与 K 线图表完全隔离。
 * 数据映射（bar 构造 + 只取当前交易日）见 @/utils/minuteBars；
 * 坐标轴（横轴固定一个交易日、纵轴以昨收对称）见 @/utils/minuteAxis。
 */
export function useMinuteChart(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
  /**
   * 兜底昨收（实时行情的 `price - change`）。正常从分时数据里算（见 sessionPrevClose），
   * 只有窗口里没有上一交易日时（新股首日）才用得上；都拿不到就退回 klinecharts 默认纵轴。
   */
  prevClose?: MaybeRef<number | undefined>;
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

  let resizeObserver: ResizeObserver | null = null;
  /** 图里装的是哪个交易日的数据（由 sessionKeyOf 得出），用来判断刷新该重挂还是追加 */
  let renderedSession: string | null = null;

  function applySessionAxis() {
    const c = chart.value;
    const bars = klineData.value;
    if (!c || bars.length === 0) return;
    // 图区宽度取主图 bounding：yAxis 是 inside 的，等于整幅宽度
    const width = c.getSize('candle_pane', 'main')?.width ?? 0;
    if (!(width > 0)) return;

    const layout = minuteAxisLayout(width, bars);
    // 图宽装不下一整天时柱宽会被 barSpaceLimit 卡住，自算的排版落不到图上：
    // 留给 klinecharts 自己排，刻度也跟着退回默认（见 minuteAxisTicks）
    if (!sessionFits(width, layout.barMinutes)) return;

    c.setBarSpace(layout.barSpace);
    // 留白按**实际**柱宽算：setBarSpace 可能被 barSpaceLimit 夹住，两者才不会打架。
    // Chart 层的 setOffsetRightDistance 内部就是当场重排（store 的 isUpdate 恒为 true），
    // 不用再传第二个参数
    const offset = Math.max(0, width - bars.length * c.getBarSpace().bar);
    c.setOffsetRightDistance(offset);
  }

  /** 记下图里装的是哪个交易日 —— 每次把 klineData 整份交给图表后都要调 */
  function markRenderedSession() {
    const last = klineData.value[klineData.value.length - 1];
    renderedSession = last ? sessionKeyOf(last.timestamp) : null;
  }

  /** 固定刻度 09:30 / 10:30 / 11:30-13:00 / 14:00 / 15:00，位置跟随实际柱宽 */
  function minuteAxisTicks(params: AxisCreateTicksParams): AxisTick[] {
    const c = chart.value;
    if (!c) return params.defaultTicks;
    // 还没有数据时格子无从对齐：此刻的柱宽是 klinecharts 的默认值，按它摆出来的五个刻度会
    // 全挤在右缘（数据一到 applySessionAxis 就会重排，这里先交回默认刻度）
    if (klineData.value.length === 0) return params.defaultTicks;
    // 跨度直接由数据推（1 分钟线 240 格、5 分钟线 48 格），不依赖 applySessionAxis 跑过没有，
    // 免得某一帧按错的跨度摆刻度
    const barMinutes = inferBarMinutes(klineData.value);
    // 一整天铺不下时固定刻度没有意义：柱宽被 barSpaceLimit 卡住，五个刻度会全被钳到右缘叠起来
    if (!sessionFits(params.bounding.width, barMinutes)) return params.defaultTicks;
    const ticks = sessionTicks(params.bounding.width, c.getBarSpace().bar, barMinutes);
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

  // ---- 纵轴：价格 + 涨跌幅双刻度 ----
  // 分时图的纵轴以昨收为中心对称（0.00% 在正中）：右侧价格刻度、左侧涨跌幅刻度，
  // 两条轴共用同一个区间，所以刻度逐行对应。昨收拿不到时两条轴都不装 ——
  // 已经装着的也要摘掉，见 applyPriceScale。

  const CANDLE_PANE = 'candle_pane';
  const PERCENT_AXIS_ID = 'minute_percent';
  const BASELINE_OVERLAY_ID = 'minute_prev_close';

  /** 从数据里算出的昨收（所画那个交易日的）；窗口里没有上一交易日时为 null */
  let dataPrevClose: number | null = null;
  /** 已经装到图上的昨收，避免每次刷新都重建一遍轴 */
  let installedPrevClose = 0;

  /**
   * 分时图用的昨收。优先取**数据**里算出来的（见 sessionPrevClose）：它和画出来那一天是同一个
   * 交易日，盘前窗口里画昨日行情时也只有它对得上；数据里没有上一交易日（新股首日）才退回
   * 实时行情给的兜底值。都拿不到返回 0，调用方据此退回 klinecharts 的默认纵轴。
   */
  function prevCloseValue(): number {
    const value = dataPrevClose ?? Number(unref(options.prevClose ?? 0));
    return value > 0 ? value : 0;
  }

  /**
   * 装/卸价格纵轴。
   * @param force 数据整份换过（换标的、重试）时为 true：昨收可能恰好没变，但基准线的
   *              时间戳跟着数据走，得重建
   */
  function applyPriceScale(force = false) {
    const c = chart.value;
    if (!c) return;
    const prev = prevCloseValue();

    if (prev <= 0) {
      // 昨收没了（切到停牌股、行情还没到）：把上次装的涨跌幅轴和基准线摘掉。
      // 只保证「别重装」是不够的 —— createRange / displayValueToText 每次重绘都会读活的昨收，
      // 留着它们就会按 prevClose = 0 算出一个以 0 为中心的区间，刻度文字也全空。
      if (installedPrevClose > 0) {
        c.removeYAxis({ id: PERCENT_AXIS_ID });
        c.removeOverlay({ id: BASELINE_OVERLAY_ID });
        installedPrevClose = 0;
      }
      return;
    }
    // 5s 一次的刷新里昨收通常没变：removeYAxis/createOverlay/overrideYAxis 各会触发一次 layout，
    // 没必要每次都来一遍
    if (!force && prev === installedPrevClose) return;
    installedPrevClose = prev;

    // 左侧涨跌幅轴。文字统一交给 displayValueToText：刻度用它不算，
    // 鼠标悬浮时每个 y 轴 widget 还会在轴的位置画一个读数（CrosshairHorizontalLabelView），
    // 那个读数走的是同一条钩子 —— 只用 createTicks 换刻度的话，悬浮读数仍是价格。
    // 区间由下面的 overrideYAxis 给（两条轴要完全相同），这里不碰几何。
    const percentAxis: YAxisOverride & {
      displayValueToText: (value: number, precision: number) => string;
    } = {
      id: PERCENT_AXIS_ID,
      paneId: CANDLE_PANE,
      position: 'left',
      inside: true,
      displayValueToText: (value: number) => percentText(value, prevCloseValue()),
    };
    c.createYAxis(percentAxis);

    // 两条轴共用的区间。gap 也得一起设：默认是「上 20% / 下 10%」的非对称留白，
    // 会把对称区间推歪。昨收无效时把 klinecharts 算好的区间原样交回去，
    // 免得停牌股那种拿不到昨收的标的被画成压在顶上的一条线。
    c.overrideYAxis({
      paneId: CANDLE_PANE,
      gap: { top: 0.05, bottom: 0.05 },
      createRange: (params: AxisCreateRangeParams): AxisRange => {
        const base = prevCloseValue();
        if (base <= 0) return params.defaultRange;
        const { from, to } = symmetricRange(klineData.value, base);
        return {
          from, to, range: to - from,
          realFrom: from, realTo: to, realRange: to - from,
          displayFrom: from, displayTo: to, displayRange: to - from,
        };
      },
    });

    // 昨收基准线（区间正中）。锁住不给拖，默认的点/坐标轴小标签也不画。
    // 颜色跟蜡烛的 noChange 一样写死一个中性灰：深浅主题都读得出来，
    // 也就不必在切主题时重建这条线。
    c.removeOverlay({ id: BASELINE_OVERLAY_ID });
    c.createOverlay({
      id: BASELINE_OVERLAY_ID,
      name: 'horizontalStraightLine',
      paneId: CANDLE_PANE,
      points: [{ timestamp: klineData.value[klineData.value.length - 1]?.timestamp ?? Date.now(), value: prev }],
      lock: true,
      needDefaultPointFigure: false,
      needDefaultXAxisFigure: false,
      needDefaultYAxisFigure: false,
      styles: { line: { style: 'dashed', color: 'rgba(139,148,158,0.55)', size: 1 } },
    });
  }

  // 兜底昨收是异步到的（行情比分钟内数据晚一拍），到了再算一次；
  // 数据里能算出昨收时，上面的守卫会把它挡掉
  if (options.prevClose) {
    watch(() => unref(options.prevClose), () => applyPriceScale());
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
        // 丢掉还没走到的时刻。数据源的钟点要先换算到与 bar 时间戳同一口径再比（见 beijingNow），
        // 否则本机不在 UTC+8 时整段会话都会被判成未来、一并丢光
        const boundary = beijingNow();
        const validBars = mapMinuteBars(data).filter((b) => b.timestamp <= boundary);
        if (validBars.length === 0) return;

        const newLast = validBars[validBars.length - 1];
        // 跨交易日（盘前就开着面板、隔夜没关）要整份重挂：增量回调只会往后追加，
        // 上一个交易日的 240 根会赖在图里，正好画成「昨天的下午 + 今天的早盘」
        const reset = sessionUpdate(renderedSession, validBars) === 'reset';
        klineData.value = validBars;
        dataPrevClose = sessionPrevClose(data);

        if (chart.value && (reset || !barSubscriber)) {
          chart.value.setDataLoader(dataLoader);
          markRenderedSession();
        } else if (barSubscriber && newLast) {
          barSubscriber(newLast);
        }
        applySessionAxis();
        applyPriceScale(reset);
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
        dataPrevClose = sessionPrevClose(data);
      }

      if (signal.aborted) return;
      if (chart.value) {
        chart.value.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
        chart.value.setPeriod(periodToKlinecharts('minute'));
        chart.value.setDataLoader(dataLoader);
        syncPrecision(klineData.value);
        markRenderedSession();
        applySessionAxis();
        // 数据整份换过：昨收可能恰好相等，但基准线的时间戳得跟着新数据走
        applyPriceScale(true);
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
    applyPriceScale();
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

  // 卸载时必须自己收：useChartCore 里那个 onUnmounted 只销毁图表实例，
  // 不会停这里起的 5s 轮询，也不会断开新加的 ResizeObserver —— 切一次周期就漏一份
  onUnmounted(() => disposeChart());

  return {
    loading,
    error,
    initChart,
    loadData,
    disposeChart,
    applyTheme: reapplyStyles,
  };
}
