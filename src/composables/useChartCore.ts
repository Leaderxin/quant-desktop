import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { init, dispose } from 'klinecharts';
import type { CandleTooltipLegendsCustomCallback, Chart, KLineData as KCLineData, Period } from 'klinecharts';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { getPricePrecision } from '@/utils/format';
import { hexToRgba } from '@/utils/color';
import { isMinuteK, minuteKSpan } from './minutePeriod';

/**
 * tooltip 里的价格文本：固定精度 + 千分位。
 * klinecharts 自带的 `{open}` 等占位符会按 chart 的 thousandSeparator（`,`）格式化，
 * 均价是我们自己算好塞进图例的，得用同样的口径才不会在同一行里出现 `1438.03` 与
 * `1,438.03` 两种写法。
 */
function tooltipPriceText(value: number, precision: number): string {
  return value.toFixed(precision).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

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
    // 涨跌语义色读 CSS 令牌：variables.css 是唯一事实源 —— 主题（dark/light）
    // × 涨跌配色（cn/us）四种组合全在级联里定好，这里不用自己维护组合矩阵。
    // 图表画在 canvas 上读不了 var()，只能解析成字面值；读不到时回落 cn
    // 红涨绿跌，别让整块图表没颜色。
    const rootStyle = getComputedStyle(document.documentElement);
    const up = rootStyle.getPropertyValue('--color-up').trim() || '#f85149';
    const down = rootStyle.getPropertyValue('--color-down').trim() || '#3fb950';
    const noChange = '#8b949e';
    // 多条均线配色按配置顺序使用；分时均价线固定取第 2 个，保持黄色口径。
    const lineColors = isDark
      ? ['#F1F1F1', '#FFD302', '#E454CE', '#32CD32', '#01C5C4', '#FF7F50', '#6495ED', '#DAA520', '#7FFFD4', '#FF69B4']
      : ['#333333', '#CC8800', '#B8308F', '#1E8C4A', '#0A8A8A', '#C84B28', '#4169B1', '#996515', '#188A75', '#C33A79'];
    return {
      up,
      down,
      noChange,
      lineColor: isDark ? '#58a6ff' : '#0969da',
      avgLineColor: lineColors[1],
      gridHColor: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)',
      gridVColor: isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.04)',
      axisColor: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)',
      tickColor: isDark ? '#8b949e' : '#656d76',
      tooltipBg: isDark ? 'rgba(22,27,34,0.95)' : 'rgba(255,255,255,0.95)',
      tooltipText: isDark ? '#c9d1d9' : '#24292f',
      separatorColor: isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)',
      crosshairBg: isDark ? 'rgba(22,27,34,0.9)' : 'rgba(31,35,40,0.85)',
      crosshairText: isDark ? '#c9d1d9' : '#e6edf3',
      // 副图指标配色 — 柱子跟随涨跌令牌（含配色方案），透明度随深/浅主题微调
      indicatorBarUp: hexToRgba(up, isDark ? 0.7 : 0.72),
      indicatorBarDown: hexToRgba(down, isDark ? 0.7 : 0.72),
      indicatorBarNoChange: hexToRgba(noChange, 0.6),
      // 量比柱低透明度
      volumeBarUp: hexToRgba(up, isDark ? 0.5 : 0.55),
      volumeBarDown: hexToRgba(down, isDark ? 0.5 : 0.55),
      volumeBarNoChange: hexToRgba(noChange, isDark ? 0.45 : 0.5),
      lineColors,
    };
  }

  // ---- 样式应用 ----

  /**
   * 成交量指标（VOL）的实例配置。K 线与分时图的成交量副图共用同一套外观。
   * 注意：不覆盖内置 `figures` —— v10 内置 VOL 的 volume 柱用 styles 回调从
   * `indicator.styles.bars[0]` 取色，静态 figure 会丢掉取色逻辑导致柱子不显示；
   * 只通过 `styles.bars[0]` 传柱子配色，保留内置 figures/calc。
   * 分时图传 `calcParams: []`（只要量柱，不要均量线），K 线用默认的 [5, 10, 20]。
   */
  function volIndicatorConfig(paneId: string, calcParams: number[] = [5, 10, 20]) {
    const colors = themeColors();
    return {
      name: 'VOL',
      paneId,
      shortName: '成交量',
      calcParams,
      styles: {
        bars: [
          { upColor: colors.volumeBarUp, downColor: colors.volumeBarDown, noChangeColor: colors.volumeBarNoChange },
        ],
      },
    };
  }

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
        bar: { upColor: c.up, downColor: c.down, upBorderColor: c.up, downBorderColor: c.down, upWickColor: c.up, downWickColor: c.down, noChangeColor: c.noChange, noChangeBorderColor: c.noChange, noChangeWickColor: c.noChange, compareRule: 'current_open' },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          // 均价并入蜡烛 tooltip 的同一行（放在「量」后面），不再让均价指标单独占一行。
          // 指标自身的那行图例在 useMinuteChart 里通过「空短名 + 无 title 的 figure」关掉。
          legend: {
            template: ((data) => {
              const current = data.current;
              const precision = chart.value?.getSymbol()?.pricePrecision ?? 2;
              const avgValue = current?.avgPrice;
              const avg = typeof avgValue === 'number' ? tooltipPriceText(avgValue, precision) : '--';
              return [
                { title: '时间', value: '{time}' },
                { title: '开', value: '{open}' },
                { title: '高', value: '{high}' },
                { title: '低', value: '{low}' },
                { title: '收', value: '{close}' },
                { title: '量', value: '{volume}' },
                {
                  title: { text: '均价', color: c.avgLineColor },
                  value: { text: avg, color: c.avgLineColor },
                },
              ];
            }) as CandleTooltipLegendsCustomCallback,
          },
          title: { show: false },
          rect: { position: 'pointer', paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: c.tooltipBg },
          text: { size: 11, color: c.tooltipText, family: 'var(--font-sans)' },
        } as any, // text 为遗留字段，不在 v10 CandleTooltipStyle 类型中
        priceMark: {
          high: { show: false },
          low: { show: false },
          last: { show: false, upColor: c.up, downColor: c.down, noChangeColor: c.noChange, extendTexts: [] },
        },
      },
      indicator: {
        ohlc: { upColor: c.up, downColor: c.down, noChangeColor: c.noChange, compareRule: 'current_open' },
        bars: [
          { upColor: c.indicatorBarUp, downColor: c.indicatorBarDown, noChangeColor: c.indicatorBarNoChange },
        ],
        lines: c.lineColors.map(color => ({ style: 'solid', smooth: false, size: 1, color })),
        lastValueMark: { show: false },
        tooltip: { show: true, labels: ['', '', '', '', '', '量', '额'], text: { size: 11, color: c.tooltipText } } as any, // labels/text 为遗留字段
      },
      xAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: true, color: c.axisColor, size: 1 },
        tickLine: { show: false },
        tickText: { size: 10, color: c.tickColor, family: 'var(--font-sans)', marginStart: 0, marginEnd: 0 },
      },
      yAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: false },
        tickLine: { show: false },
        tickText: { size: 10, color: c.tickColor, family: 'var(--font-sans)' },
      },
      separator: { size: 1, color: c.separatorColor, fill: false, activeBackgroundColor: 'rgba(255,255,255,0.02)' },
      crosshair: {
        show: true,
        horizontal: { show: true, line: { show: true, color: c.lineColor, size: 1 }, text: { show: true, size: 10, color: c.crosshairText, family: 'var(--font-mono)', backgroundColor: c.crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any }, // backgroundColor 不在 StateTextStyle 中
        vertical: { show: true, line: { show: true, color: c.lineColor, size: 1 }, text: { show: true, size: 10, color: c.crosshairText, family: 'var(--font-mono)', backgroundColor: c.crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any },
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
        bar: { upColor: c.up, downColor: c.down, upBorderColor: c.up, downBorderColor: c.down, upWickColor: c.up, downWickColor: c.down, noChangeColor: c.noChange, noChangeBorderColor: c.noChange, noChangeWickColor: c.noChange, compareRule: 'current_open' },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          legend: {
            template: [
              { title: dateLabel, value: '{time}' },
              { title: '开', value: '{open}' },
              { title: '高', value: '{high}' },
              { title: '低', value: '{low}' },
              { title: '收', value: '{close}' },
              { title: '涨跌幅', value: '{change}' },
              { title: '量', value: '{volume}' },
            ],
          },
          title: { show: false },
          rect: { position: 'pointer', paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: c.tooltipBg },
          text: { size: 11, color: c.tooltipText, family: 'var(--font-sans)' },
        } as any, // text 为遗留字段
        priceMark: {
          high: { show: false },
          low: { show: false },
          last: { show: false, upColor: c.up, downColor: c.down, noChangeColor: c.noChange, extendTexts: [] },
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

  /** 将自定义 PeriodType 映射为 klinecharts Period 对象 */
  function periodToKlinecharts(period: PeriodType): Period {
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
        layout: { yAxis: { inside: true } },
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

  // 主题或涨跌配色变化时重设样式：图表画在 canvas 上，令牌在 CSS 里翻转后
  // 不会自动生效，必须重新 setStyles
  watch(() => [settings.theme, settings.colorScheme], () => {
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
    volIndicatorConfig,
    applyChartStyles,
    applyCandlestickStyles,
    reapplyStyles,
    periodToKlinecharts,
    syncPrecision,
    initChartCore,
    disposeChart,
  };
}
