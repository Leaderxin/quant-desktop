import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData as KCLineData } from 'klinecharts';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { getPricePrecision } from '@/utils/format';
import { isMinuteK, minuteKSpan } from './minutePeriod';

/**
 * 图表核心 — 创建/销毁、样式主题、精度自适应等公共逻辑。
 * K 线图和分时图共用，各自在上层 composable 中补充数据加载和自动刷新。
 */
export function useChartCore(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
}) {
  const settings = useSettingsStore();

  const chart = ref<Chart | null>(null);
  const loading = ref(false);
  const error = ref('');
  const currentPeriod = ref<PeriodType>('minute');

  // ---- 主题颜色 ----

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
      // 副图指标配色 — 柱子与主图蜡烛一致，深/浅主题自适应
      indicatorBarUp: isDark ? 'rgba(248,81,73,0.7)' : 'rgba(248,81,73,0.72)',
      indicatorBarDown: isDark ? 'rgba(63,185,80,0.7)' : 'rgba(63,185,80,0.72)',
      indicatorBarNoChange: isDark ? 'rgba(139,148,158,0.6)' : 'rgba(139,148,158,0.6)',
      // 量比柱低透明度
      volumeBarUp: isDark ? 'rgba(248,81,73,0.5)' : 'rgba(248,81,73,0.55)',
      volumeBarDown: isDark ? 'rgba(63,185,80,0.5)' : 'rgba(63,185,80,0.55)',
      volumeBarNoChange: isDark ? 'rgba(139,148,158,0.45)' : 'rgba(139,148,158,0.5)',
      // 多条均线配色
      lineColors: isDark
        ? ['#F1F1F1', '#FFD302', '#E454CE', '#32CD32', '#01C5C4']
        : ['#333333', '#CC8800', '#B8308F', '#1E8C4A', '#0A8A8A'],
    };
  }

  // ---- 样式应用 ----

  function applyChartStyles() {
    if (!chart.value) return;
    const c = themeColors();

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
        bars: [
          { upColor: c.indicatorBarUp, downColor: c.indicatorBarDown, noChangeColor: c.indicatorBarNoChange },
        ],
        lines: c.lineColors.map(color => ({ style: 'solid', smooth: false, size: 1, color })),
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

  // ---- 工具 ----

  function periodToKlinecharts(period: PeriodType): { type: string; span: number } {
    const span = minuteKSpan(period);
    if (span !== null) return { type: 'minute', span };
    switch (period) {
      case 'minute': return { type: 'minute', span: 1 };
      case 'weekly': return { type: 'week', span: 1 };
      case 'monthly': return { type: 'month', span: 1 };
      default: return { type: 'day', span: 1 };
    }
  }

  /**
   * 根据 K 线数据自适应价格精度。
   * 扫描最近 10 根 K 线的 OHLC 值（共 40 个价格点）来检测价格的小数位数，
   * 避免单点采样（仅收盘价）在第三位小数为零时误判精度，导致 ETF/可转债等显示错误。
   */
  function syncPrecision(klineData: KCLineData[]) {
    if (!chart.value || klineData.length === 0) return;
    let precision = 2;
    const barsToCheck = klineData.slice(-10);
    for (const bar of barsToCheck) {
      for (const val of [bar.open, bar.high, bar.low, bar.close]) {
        if (val != null && !isNaN(val) && val !== 0 && getPricePrecision(val) === 3) {
          precision = 3;
          break;
        }
      }
      if (precision === 3) break;
    }
    const last = klineData[klineData.length - 1];
    if (last.close != null && !isNaN(last.close) && last.close !== 0) {
      chart.value.setSymbol({
        ticker: unref(options.code),
        name: unref(options.name) || unref(options.code),
        pricePrecision: precision,
        volumePrecision: 0,
      });
    }
  }

  // ---- 生命周期 ----

  function initChartCore(period: PeriodType): boolean {
    if (!options.chartRef.value) return false;

    const isNew = !chart.value;
    if (isNew) {
      chart.value = init(options.chartRef.value, {
        locale: 'zh-CN',
        layout: { basicParams: { yAxisInside: true } },
      });
      if (!chart.value) {
        error.value = '图表初始化失败';
        return false;
      }
    }

    if (!chart.value) return false;

    currentPeriod.value = period;
    applyChartStyles();
    if (period !== 'minute') {
      applyCandlestickStyles();
    }
    return isNew;
  }

  function disposeChart() {
    if (chart.value) {
      dispose(chart.value);
      chart.value = null;
    }
  }

  // Theme change: reapply styles
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
    currentPeriod,
    themeColors,
    applyChartStyles,
    applyCandlestickStyles,
    reapplyStyles,
    periodToKlinecharts,
    syncPrecision,
    initChartCore,
    disposeChart,
  };
}
