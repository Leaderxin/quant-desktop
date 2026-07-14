import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { MinuteData, KLineData, PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { getPricePrecision } from '@/utils/format';

/** 判断某周期是否为分钟 K（区别于分时 'minute' 与日/周/月 K）。*/
function isMinuteK(period: PeriodType): boolean {
  return period !== 'minute' && period.endsWith('min');
}

export function useChart(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
}) {
  const settings = useSettingsStore();

  const chart = ref<Chart | null>(null);
  const loading = ref(false);
  const error = ref('');
  let abortController: AbortController | null = null;
  let refreshTimer: ReturnType<typeof setInterval> | null = null;
  const currentPeriod = ref<PeriodType>('minute');

  // 累积全部已加载的 K 线数据（初始 + 历次懒加载），按时间升序
  const allData = ref<KCLineData[]>([]);
  // 标记是否还有更多历史数据可加载
  const hasMoreForward = ref(true);

  /** subscribeBar 回调引用，增量推送数据到图表避免全量重绘导致的抖动 */
  let barSubscriber: ((bar: KCLineData) => void) | null = null;

  /** 将时间戳格式化为 YYYY-MM-DD 字符串，用于 API end_date 参数 */
  function formatDate(ts: number): string {
    const d = new Date(ts);
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  }

  /** 将后端 KLineData 映射为 klinecharts 需要的 KLineData 格式 */
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

  /** 加载更早的历史数据（getBars forward 和预加载共用） */
  async function loadMoreHistory(): Promise<KCLineData[]> {
    if (!hasMoreForward.value || loading.value) return [];
    loading.value = true;
    try {
      const earliest = allData.value[0];
      const endDate = earliest
        ? formatDate(earliest.timestamp - 86400000)
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
      // 仅当返回了新的非重复 bar 且响应量充足时才认为还有更多历史数据
      hasMoreForward.value = uniqueCount > 0 && newBars.length >= 100;
      return newBars;
    } catch (e) {
      console.error('[useChart] forward load failed:', e);
      return [];
    } finally {
      loading.value = false;
    }
  }

  const klineData = ref<KCLineData[]>([]);

  const dataLoader: DataLoader = {
    getBars: async (params) => {
      if (params.type === 'init') {
        if (currentPeriod.value === 'minute') {
          params.callback(klineData.value, { forward: false, backward: false });
        } else {
          params.callback(allData.value, {
            forward: hasMoreForward.value,
            backward: false,
          });
        }
      } else if (params.type === 'forward') {
        // 用户向左拖动（getBars 触发或预加载触发）
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
        // backward: 不需要处理
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

  function themeColors() {
    const isDark = settings.theme === 'dark';
    return {
      lineColor: isDark ? '#58a6ff' : '#0969da',
      gridHColor: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)',
      gridVColor: isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.04)',
      axisColor: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)',
      tickColor: isDark ? '#8b949e' : '#656d76',
      tooltipBg: isDark ? 'rgba(22,27,34,0.95)' : 'rgba(255,255,255,0.95)',
      tooltipText: isDark ? '#c9d1d9' : '#24292f',
      separatorColor: isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)',
      crosshairBg: isDark ? 'rgba(22,27,34,0.9)' : 'rgba(31,35,40,0.85)',
      crosshairText: isDark ? '#c9d1d9' : '#e6edf3',
    };
  }

  function applyChartStyles() {
    if (!chart.value) return;
    const c = themeColors();
    const isDark = settings.theme === 'dark';

    chart.value.setStyles({
      grid: {
        show: true,
        horizontal: { show: true, color: c.gridHColor, size: 1, dashedValue: [2, 2] },
        vertical: { show: true, color: c.gridVColor, size: 1, dashedValue: [2, 2] },
      },
      candle: {
        type: 'area',
        bar: { upColor: '#f85149', downColor: '#3fb950', upBorderColor: '#f85149', downBorderColor: '#3fb950', upWickColor: '#f85149', downWickColor: '#3fb950', noChangeColor: '#8b949e', noChangeBorderColor: '#8b949e', noChangeWickColor: '#8b949e', compareRule: 'previous_close' as any },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          labels: ['时间', '开', '高', '低', '收', '量', '额'],
          title: { show: false } as any,
          rect: { position: 'pointer' as any, paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: c.tooltipBg } as any,
          text: { size: 11, color: c.tooltipText, family: 'var(--font-sans)' } as any,
        } as any,
        priceMark: {
          high: { show: false } as any,
          low: { show: false } as any,
          last: { show: false, extendTexts: [] } as any,
        },
      },
      indicator: {
        ohlc: { upColor: '#f85149', downColor: '#3fb950', noChangeColor: '#8b949e', compareRule: 'previous_close' },
        bars: [] as any,
        // 均线配色：参考主流股票软件（同花顺/东方财富/通达信）白/黄/紫/绿
        lines: (isDark
          ? ['#F1F1F1', '#FFD302', '#E454CE', '#32CD32', '#01C5C4']
          : ['#333333', '#CC8800', '#B8308F', '#1E8C4A', '#0A8A8A']
        ).map(color => ({ style: 'solid', smooth: false, size: 1, color })),
        lastValueMark: { show: false } as any,
        tooltip: { show: true, labels: ['', '', '', '', '', '量', '额'], text: { size: 11, color: c.tooltipText } } as any,
      },
      xAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: true, color: c.axisColor, size: 1 },
        tickLine: { show: false } as any,
        tickText: { size: 10, color: c.tickColor, family: 'var(--font-sans)', marginStart: 0, marginEnd: 0 } as any,
      },
      yAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: false } as any,
        tickLine: { show: false } as any,
        tickText: { size: 10, color: c.tickColor, family: 'var(--font-sans)' } as any,
      },
      separator: { size: 1, color: c.separatorColor, fill: false, activeBackgroundColor: 'rgba(255,255,255,0.02)' },
      crosshair: {
        show: true,
        horizontal: { show: true, line: { show: true, color: c.lineColor, size: 1 }, text: { show: true, size: 10, color: c.crosshairText, family: 'var(--font-mono)', backgroundColor: c.crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
        vertical: { show: true, line: { show: true, color: c.lineColor, size: 1 }, text: { show: true, size: 10, color: c.crosshairText, family: 'var(--font-mono)', backgroundColor: c.crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
      },
    });
  }

  function applyCandlestickStyles() {
    if (!chart.value) return;
    const c = themeColors();
    const dateLabel = isMinuteK(currentPeriod.value) ? '时间' : '日期';

    chart.value.setStyles({
      candle: {
        type: 'candle_solid',
        bar: { upColor: '#f85149', downColor: '#3fb950', upBorderColor: '#f85149', downBorderColor: '#3fb950', upWickColor: '#f85149', downWickColor: '#3fb950', noChangeColor: '#8b949e', noChangeBorderColor: '#8b949e', noChangeWickColor: '#8b949e', compareRule: 'previous_close' as any },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          labels: [dateLabel, '开', '高', '低', '收', '量', '额'],
          title: { show: false } as any,
          rect: { position: 'pointer' as any, paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: c.tooltipBg } as any,
          text: { size: 11, color: c.tooltipText, family: 'var(--font-sans)' } as any,
        } as any,
        priceMark: {
          high: { show: false } as any,
          low: { show: false } as any,
          last: { show: false, extendTexts: [] } as any,
        },
      },
    });
  }

  function reapplyStyles() {
    applyChartStyles();
    if (currentPeriod.value !== 'minute') {
      applyCandlestickStyles();
    }
  }

  function periodToKlinecharts(period: PeriodType): { type: string; span: number } {
    const minuteSpan = parseInt(period);
    if (!isNaN(minuteSpan)) return { type: 'minute', span: minuteSpan };
    switch (period) {
      case 'minute': return { type: 'minute', span: 5 };
      case 'weekly': return { type: 'week', span: 1 };
      case 'monthly': return { type: 'month', span: 1 };
      default: return { type: 'day', span: 1 };
    }
  }

  async function initChart(period: PeriodType) {
    if (!options.chartRef.value) return;

    const isNew = !chart.value;
    if (isNew) {
      chart.value = init(options.chartRef.value, {
        locale: 'zh-CN',
        layout: { basicParams: { yAxisInside: true } },
      });
      if (!chart.value) {
        error.value = '图表初始化失败';
        return;
      }

      chart.value.overrideIndicator({
        name: 'VOL',
        shortName: '成交量',
        series: 'volume',
        calcParams: [5, 10, 20],
        precision: 0,
        shouldFormatBigNumber: true,
        minValue: 0,
        figures: [
          { key: 'ma1', title: 'MA5: ', type: 'line' },
          { key: 'ma2', title: 'MA10: ', type: 'line' },
          { key: 'ma3', title: 'MA20: ', type: 'line' },
          { key: 'volume', title: 'VOLUME: ', type: 'bar', baseValue: 0, styles: { upColor: 'rgba(248,81,73,0.4)', downColor: 'rgba(63,185,80,0.4)' } } as any,
        ],
      } as any);
    }

    // setSymbol + setPeriod are deferred to loadData so data arrives before
    // klinecharts triggers getBars('init'), avoiding stale-data rendering.
    if (!chart.value) return;

    currentPeriod.value = period;
    applyChartStyles();
    if (period !== 'minute') {
      applyCandlestickStyles();
      // 叠加价格均线 MA5/MA10/MA20 到主图（仅 K 线图，分时图不叠加）
      const existingMA = chart.value.getIndicators({ name: 'MA' });
      if (existingMA.length === 0) {
        chart.value.createIndicator({
          name: 'MA',
          calcParams: [5, 10, 20, 60],
        }, { pane: { id: 'candle_pane' } });
      }
    }
  }

  /**
   * 根据 K 线数据自适应价格精度。
   * 扫描最近 10 根 K 线的 OHLC 值（共 40 个价格点）来检测价格的小数位数，
   * 避免单点采样（仅收盘价）在第三位小数为零时（如 0.950 → JSON 传 0.95）
   * 误判为 2 位小数，导致 ETF/可转债等 3 位精度品种的图表价格标注错误。
   */
  function syncPrecision() {
    if (!chart.value || klineData.value.length === 0) return;
    let precision = 2;
    const barsToCheck = klineData.value.slice(-10);
    for (const bar of barsToCheck) {
      for (const val of [bar.open, bar.high, bar.low, bar.close]) {
        if (val != null && !isNaN(val) && val !== 0 && getPricePrecision(val) === 3) {
          precision = 3;
          break;
        }
      }
      if (precision === 3) break;
    }
    const last = klineData.value[klineData.value.length - 1];
    if (last.close != null && !isNaN(last.close) && last.close !== 0) {
      chart.value.setSymbol({
        ticker: unref(options.code),
        name: unref(options.name) || unref(options.code),
        pricePrecision: precision,
        volumePrecision: 0,
      });
    }
  }

  function getRefreshInterval(period: PeriodType): number {
    const minuteSpan = parseInt(period);
    if (!isNaN(minuteSpan)) return minuteSpan <= 5 ? 8000 : 20000; // 短分钟 K 8s / 长分钟 K 20s
    switch (period) {
      case 'minute': return 5000;    // 分时图：5 秒
      case 'daily':  return 30000;   // 日K：30 秒
      case 'weekly': return 60000;   // 周K：60 秒
      case 'monthly':return 60000;   // 月K：60 秒
      default:       return 30000;
    }
  }

  function startAutoRefresh(period: PeriodType) {
    stopAutoRefresh();

    if (period === 'minute') {
      // Minute chart: use old full-reload behavior (calls get_intraday)
      const interval = getRefreshInterval(period);
      refreshTimer = setInterval(() => {
        if (!loading.value) {
          loadData(period);
        }
      }, interval);
      return;
    }

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
        // 增量刷新：只取最新 10 条，更新末尾
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
          count: 10,
        });
        const newBars = mapKLineToChart(data);
        if (newBars.length > 0) {
          // 合并到 allData：按 timestamp 去重、更新
          const map = new Map(allData.value.map((d) => [d.timestamp, d]));
          for (const bar of newBars) {
            map.set(bar.timestamp, bar);
          }
          allData.value = [...map.values()].sort((a, b) => a.timestamp - b.timestamp);
          klineData.value = allData.value;
          // 优先 subscribeBar 增量推送（无抖动），未注册时回退到 setDataLoader
          if (barSubscriber) {
            for (const bar of newBars) {
              barSubscriber(bar);
            }
          } else if (chart.value) {
            chart.value.setDataLoader(dataLoader);
          }
        }
      } catch (e) {
        // 静默失败，不影响用户浏览
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
      if (period === 'minute') {
        const data = await invoke<MinuteData[]>('get_intraday', {
          code: unref(options.code),
          market: unref(options.market),
        });
        if (signal.aborted) return;

        if (data.length) {
          const today = new Date();
          klineData.value = data.map((d) => {
            let h = 0, m = 0;
            if (d.time.includes(':')) {
              [h, m] = d.time.split(':').map(Number);
            } else if (d.time.length >= 4) {
              h = Number(d.time.slice(0, 2));
              m = Number(d.time.slice(2, 4));
            }
            const ts = new Date(today.getFullYear(), today.getMonth(), today.getDate(), h || 0, m || 0).getTime();
            return {
              timestamp: ts,
              open: d.open ?? d.price,
              high: d.high ?? d.price,
              low: d.low ?? d.price,
              close: d.price,
              volume: d.volume,
            };
          });
        }
      } else {
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
        });
        if (signal.aborted) return;

        if (data.length) {
          const mapped = mapKLineToChart(data);
          // 存入 allData（按时间升序）
          allData.value = mapped.sort((a, b) => a.timestamp - b.timestamp);
          // 分钟 K 一次性加载，不做左滑懒加载；新浪日K 600 条到底；腾讯日/周/月支持懒加载
          hasMoreForward.value = settings.activeDatasource !== 'sina';
          // 保持 klineData 兼容性（对外暴露）
          klineData.value = mapped;
        }
      }

      if (signal.aborted) return;
      if (chart.value) {
        chart.value.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
        chart.value.setPeriod(periodToKlinecharts(period) as any);
        chart.value.setDataLoader(dataLoader);
        syncPrecision();
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

  function disposeChart() {
    stopAutoRefresh();
    barSubscriber = null;
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
    if (chart.value) {
      dispose(chart.value);
      chart.value = null;
    }
  }

  // Theme change: reapply the correct style set for the current period
  watch(() => settings.theme, () => {
    reapplyStyles();
  });

  onUnmounted(() => {
    disposeChart();
  });

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
