import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { MinuteData, KLineData, PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { getPricePrecision } from '@/utils/format';

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

  const klineData = ref<KCLineData[]>([]);

  const dataLoader: DataLoader = {
    getBars: (params) => {
      if (params.type === 'init') {
        params.callback(klineData.value, false);
      } else {
        // No more historical data available beyond initial load
        params.callback([], true);
      }
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

    chart.value.setStyles({
      candle: {
        type: 'candle_solid',
        bar: { upColor: '#f85149', downColor: '#3fb950', upBorderColor: '#f85149', downBorderColor: '#3fb950', upWickColor: '#f85149', downWickColor: '#3fb950', noChangeColor: '#8b949e', noChangeBorderColor: '#8b949e', noChangeWickColor: '#8b949e', compareRule: 'previous_close' as any },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          labels: ['日期', '开', '高', '低', '收', '量', '额'],
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

    // Always update symbol and period on stock/period change (even for reused chart)
    if (!chart.value) return;
    chart.value.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });

    currentPeriod.value = period;
    chart.value.setPeriod(periodToKlinecharts(period) as any);
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

  function syncPrecision() {
    if (!chart.value || klineData.value.length === 0) return;
    const last = klineData.value[klineData.value.length - 1];
    if (last.close != null && !isNaN(last.close) && last.close !== 0) {
      chart.value.setSymbol({
        ticker: unref(options.code),
        name: unref(options.name) || unref(options.code),
        pricePrecision: getPricePrecision(last.close),
        volumePrecision: 0,
      });
    }
  }

  function getRefreshInterval(period: PeriodType): number {
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
    const interval = getRefreshInterval(period);
    refreshTimer = setInterval(() => {
      if (!loading.value) {
        loadData(period);
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
          klineData.value = data.map((d) => {
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
      }

      if (signal.aborted) return;
      if (chart.value) {
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
