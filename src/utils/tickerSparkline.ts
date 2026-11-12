// src/utils/tickerSparkline.ts — 行情条迷你分时线的纯几何计算
//
// 两种纵轴共用同一套 x/y 映射，区别只在「半幅」怎么来：
// - price：以昨收为中心，半幅取数据最大偏离与昨收 1% 的较大值（与主图一致）
// - limit：半幅由涨停价/跌停价决定；数据超出边界（新股、规则判错）时回退 price
//
// `both` 把两者画进同一块小画布：price 那条满不透明当主线，limit 那条 50% 透明
// 叠在下面 —— 两套纵轴都以昨收为中心，所以 0% 那条参考线对两条线同时成立。
//
// 不依赖 canvas，方便单测；绘制在 useTickerSparkline 里。

import { SESSION_MINUTES, tradingMinutesOfDay } from './minuteAxis';
import { limitPrices } from './priceLimit';
import type { TickerChartMode, TickerChartStyle } from './prefs';

export type SparklineMode = 'price' | 'limit';

/** 行情条一格里要画的轴组合（`off` 由调用方决定不挂载组件）。 */
export type SparklineChartKind = TickerChartMode;

export interface SparklineRange {
  from: number;
  to: number;
  /** limit 模式下真实的涨停价；回退 price 时为 null */
  limitUp: number | null;
  /** limit 模式下真实的跌停价；回退 price 时为 null */
  limitDown: number | null;
  /** limit 模式因规则未知或数据越界而退回 price */
  fellBack: boolean;
}

/** 与主图 `minuteAxis.ts` 的 MIN_RANGE_RATE 保持一致：涨跌幅很小时纵轴不被放大。 */
const MIN_RANGE_RATE = 0.01;

/** limit 模式允许数据超出涨跌停区间的容差，超过即回退 price。 */
const LIMIT_TOLERANCE = 1.02;

function firstValidClose(bars: readonly { close: number }[]): number {
  for (const bar of bars) {
    if (Number.isFinite(bar.close) && bar.close > 0) return bar.close;
  }
  return 0;
}

function maxDeviation(bars: readonly { close: number }[], base: number): number {
  let half = 0;
  for (const bar of bars) {
    if (!Number.isFinite(bar.close)) continue;
    const deviation = Math.abs(bar.close - base);
    if (deviation > half) half = deviation;
  }
  return half;
}

function priceRange(base: number, bars: readonly { close: number }[], minRate: number) {
  const half = Math.max(maxDeviation(bars, base), base * minRate);
  return { from: base - half, to: base + half };
}

/**
 * 解析纵轴区间。
 *
 * `prevClose <= 0`（行情还没到、新股首日拿不到昨收）时用第一根有效 close 兜底，
 * 但 limit 模式一律回退 price —— 拿一个猜出来的基准去算涨跌停价没有意义。
 */
export function resolveSparklineRange(
  bars: readonly { close: number }[],
  prevClose: number,
  mode: SparklineMode,
  limitRate: number | null,
  minRate = MIN_RANGE_RATE,
): SparklineRange {
  const base = prevClose > 0 ? prevClose : firstValidClose(bars);
  if (!(base > 0)) {
    return { from: 0, to: 0, limitUp: null, limitDown: null, fellBack: mode === 'limit' };
  }

  if (mode === 'limit' && prevClose > 0 && limitRate !== null && limitRate > 0) {
    const { up, down } = limitPrices(prevClose, limitRate);
    const half = Math.max(up - prevClose, prevClose - down);
    if (maxDeviation(bars, prevClose) <= half * LIMIT_TOLERANCE) {
      return {
        from: prevClose - half,
        to: prevClose + half,
        limitUp: up,
        limitDown: down,
        fellBack: false,
      };
    }
    // 数据越界：新股首日、退市整理期、或规则判错。退回价格轴，别画假的边界。
    const fallback = priceRange(prevClose, bars, minRate);
    return { ...fallback, limitUp: null, limitDown: null, fellBack: true };
  }

  const range = priceRange(base, bars, minRate);
  return {
    ...range,
    limitUp: null,
    limitDown: null,
    fellBack: mode === 'limit',
  };
}

export interface SparklinePoint {
  x: number;
  y: number;
}

/**
 * 把 bar 映射到画布坐标。
 *
 * x 按交易日分钟数（09:30–15:00）铺满整宽，曲线从左向右生长；午休不计入。
 * y 在 `from..to` 之间线性映射，并夹在 padding 内，防止越界数据画出画布。
 */
export function sparklinePoints(
  bars: readonly { timestamp: number; close: number }[],
  range: { from: number; to: number },
  width: number,
  height: number,
  paddingY = 3,
): SparklinePoint[] {
  if (bars.length === 0 || !(width > 0) || !(height > 0) || !(range.to > range.from)) {
    return [];
  }
  const innerHeight = Math.max(1, height - paddingY * 2);
  const span = range.to - range.from;
  return bars.map((bar) => {
    const minutes = tradingMinutesOfDay(new Date(bar.timestamp));
    const x = (minutes / SESSION_MINUTES) * width;
    const ratio = Math.min(1, Math.max(0, (range.to - bar.close) / span));
    return { x, y: paddingY + ratio * innerHeight };
  });
}

// ── 单元格几何 ──
//
// 左右结构：左边「名称 / 价格+涨跌」两行文字，右边一块图。图的高度约等于
// 两行字（11px × 1.4 × 2 ≈ 31px），取 30。**两种样式共用同一个高度** —— 否则
// 切换样式会让窗口长高变矮，行情条在屏幕上跳一下（见 ticker_chart_style）。
//
// 画什么由 `TickerChartStyle`（utils/prefs）决定：`axis` 额外画一条 x 轴线与三个刻度（起点 /
// 午间 / 终点），`plain` 只有 0% 虚线与曲线。涨跌停边界一律不画 —— 那一维由
// 半透明的涨跌停曲线自己表达，两条边界线在平静日只是噪音。

/**
 * 画布宽（CSS 像素）。固定值，不跟着剩余空间长 —— 每行都按内容定宽，
 * 图跟着 flex 走会让同一屏里各行的图宽不一样，竖着看就成了锯齿。
 *
 * 横轴固定是 240 个交易日分钟，75px 下上午半天约 37px：密集震荡会被压平
 * 一点，但形状仍读得出来。窗口宽度按「文字列 + 间距 + 图 + 内边距」配平
 * （见 datasource::TICKER_WIDTH），不留多余空白。
 */
export const SPARKLINE_CANVAS_WIDTH = 75;
/** 曲线区上边缘（自顶向下） */
export const SPARKLINE_PAD_TOP = 2;
/** 曲线区高度 */
export const SPARKLINE_PLOT_HEIGHT = 22;
/** 曲线区内边距：越界数据不画出这条带 */
export const SPARKLINE_CURVE_PADDING_Y = 2;
/** x 轴线所在的 y（`plain` 样式不画它，这块位置留白） */
export const SPARKLINE_AXIS_Y = SPARKLINE_PAD_TOP + SPARKLINE_PLOT_HEIGHT + 1;
/** 刻度线长度（自轴线向下） */
export const SPARKLINE_TICK_HEIGHT = 2;
/** 曲线末端圆点半径 */
export const SPARKLINE_DOT_RADIUS = 1.5;
/** 画布总高 —— 两种样式同一个值，切换不动窗口 */
export const SPARKLINE_CANVAS_HEIGHT = 30;

/**
 * 0% 线（昨收）在画布上的 y。
 *
 * price 与 limit 两套纵轴都以昨收为中心（涨跌停价对称，半幅取较宽的一侧），
 * 所以这条线在两个坐标系里是同一个位置 —— 只需画一次。
 */
export function sparklineBaselineY(): number {
  return SPARKLINE_PAD_TOP + SPARKLINE_PLOT_HEIGHT / 2;
}

/**
 * 把 bar 映射到这一格的画布坐标（y 已经带上顶部留白）。
 *
 * `null` 表示这个区间没有可画的线（无数据或区间塌缩）。参数只用到 from/to，
 * 与 `sparklinePoints` 一样接受裸区间。
 */
export function sparklineCanvasPoints(
  bars: readonly { timestamp: number; close: number }[],
  range: { from: number; to: number } | null,
  width: number,
): SparklinePoint[] {
  if (!range) return [];
  return sparklinePoints(
    bars,
    range,
    width,
    SPARKLINE_PLOT_HEIGHT,
    SPARKLINE_CURVE_PADDING_Y,
  ).map((p) => ({ x: p.x, y: p.y + SPARKLINE_PAD_TOP }));
}

/** 一格要画的两条线。 */
export interface SparklineLines {
  /** 满不透明的主线 */
  main: SparklineRange | null;
  /** 叠在下面、半透明的涨跌停投影；null 表示这条模式不画 */
  limit: SparklineRange | null;
}

/**
 * 按模式算出要画哪些线。
 *
 * - `price`：只有价格轴主线。
 * - `limit`：涨跌停投影就是主线，满不透明。昨收不可用或规则未知时
 *   `resolveSparklineRange` 已经把它回退成价格轴区间（`fellBack`），照原样画。
 * - `both`：price 当主线，limit 半透明叠在下面。涨跌停轴回退时两条线会是同一条，
 *   叠出来毫无信息量，所以那种情况只画主线。
 */
export function resolveSparklineLines(
  bars: readonly { timestamp: number; close: number }[],
  prevClose: number,
  kind: SparklineChartKind,
  limitRate: number | null,
): SparklineLines {
  if (kind === 'off' || bars.length === 0) return { main: null, limit: null };

  if (kind === 'limit') {
    const only = resolveSparklineRange(bars, prevClose, 'limit', limitRate);
    return { main: only.to > only.from ? only : null, limit: null };
  }

  const price = resolveSparklineRange(bars, prevClose, 'price', limitRate);
  const main = price.to > price.from ? price : null;
  if (kind === 'price') return { main, limit: null };

  const limit = resolveSparklineRange(bars, prevClose, 'limit', limitRate);
  return { main, limit: !limit.fellBack && limit.to > limit.from ? limit : null };
}

/**
 * x 轴刻度位置：09:30（起点）、11:30/13:00（午间分界）、15:00（终点）。
 *
 * 一个交易日在横轴上均匀铺开（`tradingMinutesOfDay` 已抹平午休），中点正好是
 * 上午收盘与下午开盘的分界。不放文字：宽度里挤下「09:30」既占地方又必然压线。
 */
export function axisTickPositions(width: number): number[] {
  if (!(width > 0)) return [];
  return [0, width / 2, Math.max(0, width - 1)];
}

/** 一次绘制需要的全部输入；数值都已经是画布坐标。 */
export interface SparklinePaintInput {
  style: TickerChartStyle;
  /** 0% 线（昨收）的 y；<= 0 表示昨收不可用，不画这条线 */
  baselineY: number | null;
  /** 先画的线（涨跌停投影），`both` 时半透明 */
  under: SparklinePoint[];
  /** 压在上面的主线（价格轴） */
  over: SparklinePoint[];
  /** 末端圆点：跟随相对昨收的涨跌上色 */
  dot: { x: number; y: number; rising: boolean } | null;
  width: number;
  height: number;
}

export interface SparklinePalette {
  accent: string;
  baseline: string;
  up: string;
  down: string;
}

/**
 * 画一格分时线。纯过程：输入 → canvas 状态，不读 DOM、不碰响应式。
 *
 * 单点实现的理由和窗口 helper 一样 —— 行情条与设置页的预览共用它，各画一份
 * 迟早会漂，而漂的症状是「设置里看着对、实际不是那样」，比没预览更糟。
 * 线与轴都落在物理像素上：1px 线在 125%/150% 缩放下按 CSS 坐标会发虚。
 */
export function paintTickerSparkline(
  ctx: CanvasRenderingContext2D,
  input: SparklinePaintInput,
  palette: SparklinePalette,
  dpr: number,
) {
  const { width, height } = input;
  // 线宽/坐标都用 CSS 像素：物理像素的部分交给各处 setTransform(1,…) 的参考线。
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);

  if (input.style === 'axis') {
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.strokeStyle = palette.baseline;
    ctx.lineWidth = Math.max(1, Math.round(dpr));
    const y = Math.round(SPARKLINE_AXIS_Y * dpr) + 0.5;
    ctx.globalAlpha = 0.35;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(Math.round(width * dpr), y);
    ctx.stroke();
    // 刻度自轴线向下：向上会去抢曲线区的位置
    ctx.globalAlpha = 0.3;
    ctx.beginPath();
    for (const tick of axisTickPositions(width)) {
      const x = Math.round(tick * dpr) + 0.5;
      ctx.moveTo(x, y);
      ctx.lineTo(x, Math.round((SPARKLINE_AXIS_Y + SPARKLINE_TICK_HEIGHT) * dpr) + 0.5);
    }
    ctx.stroke();
    ctx.restore();
  }

  if (input.baselineY !== null) {
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = 0.3;
    ctx.setLineDash([2 * dpr, 3 * dpr]);
    ctx.lineWidth = Math.max(1, Math.round(dpr));
    ctx.strokeStyle = palette.baseline;
    const y = Math.round(input.baselineY * dpr) + 0.5;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(Math.round(width * dpr), y);
    ctx.stroke();
    ctx.restore();
  }

  const stroke = (points: SparklinePoint[], alpha: number, lineWidth: number) => {
    if (points.length === 0) return;
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    points.forEach((point, i) => {
      if (i === 0) ctx.moveTo(point.x, point.y);
      else ctx.lineTo(point.x, point.y);
    });
    ctx.strokeStyle = palette.accent;
    ctx.lineWidth = lineWidth;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
    ctx.restore();
  };

  const hasOver = input.over.length > 0;
  stroke(input.under, hasOver ? 0.5 : 1, 1);
  stroke(input.over, 1, 1.25);

  if (input.dot) {
    ctx.beginPath();
    ctx.arc(
      Math.min(input.dot.x, width - SPARKLINE_DOT_RADIUS),
      input.dot.y,
      SPARKLINE_DOT_RADIUS,
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = input.dot.rising ? palette.up : palette.down;
    ctx.fill();
  }
}
