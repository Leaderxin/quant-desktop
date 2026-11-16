// src/utils/tickerSparkline.ts — 行情条迷你分时线的纯几何计算
//
// 只用价格轴：以昨收为中心，半幅取数据最大偏离与昨收 1% 的较大值（与主图一致）。
// 小波动不会被放大成噪声，接近涨跌停的日子自然撑满。0% 虚线就是昨收参考线。
//
// 不依赖 canvas，方便单测；绘制在 useTickerSparkline 里。

import { SESSION_MINUTES, tradingMinutesOfDay } from './minuteAxis';
import { beijingNow } from './minuteBars';
import type { TickerChartStyle } from './prefs';

export interface SparklineRange {
  from: number;
  to: number;
}

/** 与主图 `minuteAxis.ts` 的 MIN_RANGE_RATE 保持一致：涨跌幅很小时纵轴不被放大。 */
const MIN_RANGE_RATE = 0.01;

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

/**
 * 解析纵轴区间：以昨收为中心，半幅取最大偏离与昨收 1% 的较大值。
 *
 * `prevClose <= 0`（行情还没到、新股首日拿不到昨收）时用第一根有效 close 兜底。
 */
export function resolveSparklineRange(
  bars: readonly { close: number }[],
  prevClose: number,
  minRate = MIN_RANGE_RATE,
): SparklineRange {
  const base = prevClose > 0 ? prevClose : firstValidClose(bars);
  if (!(base > 0)) return { from: 0, to: 0 };
  const half = Math.max(maxDeviation(bars, base), base * minRate);
  return { from: base - half, to: base + half };
}

export interface SparklinePoint {
  x: number;
  y: number;
}

/**
 * 把 bar 映射到画布坐标。
 *
 * x 按交易日分钟数铺满整宽，曲线从左向右生长；午休不计入。`xSpanMinutes`
 * 决定横轴铺多少分钟：默认全天 240，早盘传 120 时只铺 09:30–11:30，午后的
 * bar 会被丢掉（此时它们本来也不该存在）。y 在 `from..to` 之间线性映射，并夹
 * 在 padding 内，防止越界数据画出画布。
 */
export function sparklinePoints(
  bars: readonly { timestamp: number; close: number }[],
  range: { from: number; to: number },
  width: number,
  height: number,
  paddingY = 3,
  xSpanMinutes = SESSION_MINUTES,
): SparklinePoint[] {
  if (bars.length === 0 || !(width > 0) || !(height > 0) || !(range.to > range.from)) {
    return [];
  }
  const innerHeight = Math.max(1, height - paddingY * 2);
  const priceSpan = range.to - range.from;
  const xSpan = Math.max(1, xSpanMinutes);
  const points: SparklinePoint[] = [];
  for (const bar of bars) {
    const minutes = tradingMinutesOfDay(new Date(bar.timestamp));
    if (minutes > xSpan) continue;
    const ratio = Math.min(1, Math.max(0, (range.to - bar.close) / priceSpan));
    points.push({
      x: (minutes / xSpan) * width,
      y: paddingY + ratio * innerHeight,
    });
  }
  return points;
}

/**
 * 迷你分时的横轴跨度（交易日分钟）。
 *
 * 早盘（当前时刻早于 11:30，且画的是今天的行情）只铺 09:30–11:30：此时线只走
 * 了不到一半，按全天铺会把它挤在左半格。11:30 之后切回全天，下午才有地方长。
 * 盘前/休市拿到的是上一交易日的完整数据时按全天，避免把昨天的一整天塞进
 * 半天窗口。
 */
export function sparklineSpanMinutes(
  bars: readonly { timestamp: number }[],
  now: number = beijingNow(),
): number {
  const last = bars[bars.length - 1];
  if (!last) return SESSION_MINUTES;
  const nowDate = new Date(now);
  const lastDate = new Date(last.timestamp);
  const sameDay =
    nowDate.getFullYear() === lastDate.getFullYear()
    && nowDate.getMonth() === lastDate.getMonth()
    && nowDate.getDate() === lastDate.getDate();
  if (!sameDay) return SESSION_MINUTES;
  const minutes = nowDate.getHours() * 60 + nowDate.getMinutes();
  return minutes < 11 * 60 + 30 ? SESSION_MINUTES / 2 : SESSION_MINUTES;
}

// ── 单元格几何 ──
//
// 左右结构：左边「名称 / 价格+涨跌」两行文字，右边一块图。图的高度约等于
// 两行字（11px × 1.4 × 2 ≈ 31px），取 30。**两种样式共用同一个高度** —— 否则
// 切换样式会让窗口长高变矮，行情条在屏幕上跳一下（见 ticker_chart_style）。
//
// 画什么由 `TickerChartStyle`（utils/prefs）决定：`axis` 额外画一条 x 轴线与三个刻度（起点 /
// 午间 / 终点），`plain` 只有 0% 虚线与曲线。

/**
 * 画布宽（CSS 像素）。固定值，不跟着剩余空间长 —— 每行都按内容定宽，
 * 图跟着 flex 走会让同一屏里各行的图宽不一样，竖着看就成了锯齿。
 *
 * 横轴固定是 240 个交易日分钟，88px 下上午半天约 44px。窗口宽度按
 * 「内边距 16 + 文字列 108 + 间距 8 + 图 88 = 220」配平
 * （见 datasource::TICKER_WIDTH），不留多余空白。
 */
export const SPARKLINE_CANVAS_WIDTH = 88;
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
 * 纵轴以昨收为中心，所以这条线永远在曲线区的正中间。
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
  xSpanMinutes = SESSION_MINUTES,
): SparklinePoint[] {
  if (!range) return [];
  return sparklinePoints(
    bars,
    range,
    width,
    SPARKLINE_PLOT_HEIGHT,
    SPARKLINE_CURVE_PADDING_Y,
    xSpanMinutes,
  ).map((p) => ({ x: p.x, y: p.y + SPARKLINE_PAD_TOP }));
}

/**
 * x 轴刻度位置：全天三个（起点 / 午间 / 右端），早盘两个（起点 / 右端）。
 *
 * 早盘横轴只覆盖 09:30–11:30，此时中点不是午休，画出来会被误读成「已经到
 * 11:30/13:00 了」；所以只在全天模式下加中间那个刻度。右端内缩一像素避免
 * 与边线重合。不放文字：宽度里挤下「09:30」既占地方又必然压线。
 */
export function axisTickPositions(width: number, xSpanMinutes = SESSION_MINUTES): number[] {
  if (!(width > 0)) return [];
  const end = Math.max(0, width - 1);
  return xSpanMinutes < SESSION_MINUTES ? [0, end] : [0, width / 2, end];
}

/** 一次绘制需要的全部输入；数值都已经是画布坐标。 */
export interface SparklinePaintInput {
  style: TickerChartStyle;
  /** 0% 线（昨收）的 y；<= 0 表示昨收不可用，不画这条线 */
  baselineY: number | null;
  /** 价格曲线（画布坐标） */
  points: SparklinePoint[];
  /** 末端圆点：跟随相对昨收的涨跌上色 */
  dot: { x: number; y: number; rising: boolean } | null;
  width: number;
  height: number;
  /** 横轴跨度（交易日分钟）：早盘 120 只画两个端点刻度，全天 240 画三个。 */
  xSpanMinutes?: number;
}

export interface SparklinePalette {
  /** 没有方向信息（无圆点）时的兜底线色 */
  accent: string;
  baseline: string;
  up: string;
  down: string;
}

/** `#rrggbb` → `rgba(...)`，只给同色渐变加 alpha；无法解析时原样返回。 */
function withAlpha(color: string, alpha: number): string {
  const match = /^#([0-9a-f]{6})$/i.exec(color.trim());
  if (!match) return color;
  const n = Number.parseInt(match[1], 16);
  return `rgba(${(n >> 16) & 0xff}, ${(n >> 8) & 0xff}, ${n & 0xff}, ${alpha})`;
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
    // 轴线与刻度是背景网格，压到刚好看得见：再亮就会和曲线抢注意力。
    ctx.globalAlpha = 0.28;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(Math.round(width * dpr), y);
    ctx.stroke();
    // 刻度自轴线向下：向上会去抢曲线区的位置
    ctx.globalAlpha = 0.22;
    ctx.beginPath();
    for (const tick of axisTickPositions(width, input.xSpanMinutes)) {
      const x = Math.round(tick * dpr) + 0.5;
      ctx.moveTo(x, y);
      ctx.lineTo(x, Math.round((SPARKLINE_AXIS_Y + SPARKLINE_TICK_HEIGHT) * dpr) + 0.5);
    }
    ctx.stroke();
    ctx.restore();
  }

  // 线与末端圆点同色，取当日方向（相对昨收）：图与左边红/绿的数字才像同一行
  // 的数据。没有圆点时退回 accent（纯色兜底，不参与真实渲染）。
  const lineColor = input.dot
    ? input.dot.rising
      ? palette.up
      : palette.down
    : palette.accent;

  // 曲线与轴线之间的同色渐变只做「体量」，不做装饰：16% → 1%。
  // 用渐变而不是整块半透明色，靠近轴线处自然收掉，不会在底部留一条硬边。
  const points = input.points;
  if (points.length >= 2) {
    const gradient = ctx.createLinearGradient(0, SPARKLINE_PAD_TOP, 0, SPARKLINE_AXIS_Y);
    gradient.addColorStop(0, withAlpha(lineColor, 0.16));
    gradient.addColorStop(1, withAlpha(lineColor, 0.01));
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(points[0].x, SPARKLINE_AXIS_Y);
    ctx.lineTo(points[0].x, points[0].y);
    for (let i = 1; i < points.length; i++) {
      ctx.lineTo(points[i].x, points[i].y);
    }
    ctx.lineTo(points[points.length - 1].x, SPARKLINE_AXIS_Y);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();
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
    ctx.strokeStyle = lineColor;
    ctx.lineWidth = lineWidth;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
    ctx.restore();
  };

  // 主线 1.5px：30px 高的格子里再细就发虚，再粗会盖住 0% 参考线。
  stroke(input.points, 1, 1.5);

  if (input.dot) {
    ctx.beginPath();
    ctx.arc(
      Math.min(input.dot.x, width - SPARKLINE_DOT_RADIUS),
      input.dot.y,
      SPARKLINE_DOT_RADIUS,
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = lineColor;
    ctx.fill();
  }
}
