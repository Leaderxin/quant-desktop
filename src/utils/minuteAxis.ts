// src/utils/minuteAxis.ts

/**
 * 分时图横轴 —— 固定一个交易日。
 *
 * 分时图回答的是「今天走了多少」：横轴永远是 09:30–15:00，曲线自左端向右生长，还没走到的
 * 时段留白。klinecharts 默认按「最近若干根」排版（数据不够铺满时贴右边），所以 bar 宽度和
 * 右侧留白得自己算：整天 240 个格子铺满图宽，数据占前 N 个格子。
 *
 * 格子数按 bar 的分钟跨度换算：腾讯分时是 1 分钟线（240 格），新浪分时是 5 分钟线（48 格），
 * 两者曲线占的横向比例才会一致。
 */

/** 一个交易日的分钟数：09:30–11:30 + 13:00–15:00 = 240 */
export const SESSION_MINUTES = 240;

/**
 * 首尾刻度内缩的像素。x 轴刻度文字在 klinecharts 里是居中对齐、不做贴边避让的
 * （十字光标文字才有避让），不内缩的话 09:30 / 15:00 会被画布裁掉半个字。
 */
const EDGE_INSET = 16;

/** 交易时刻 → 当日已开盘分钟数（午休 11:30–13:00 不计），钳到 [0, 240]。 */
export function tradingMinutesOfDay(date: Date): number {
  const minutes = date.getHours() * 60 + date.getMinutes();
  const open = 9 * 60 + 30;
  const morningMinutes = 120; // 09:30–11:30
  if (minutes <= open) return 0;
  if (minutes <= open + morningMinutes) return minutes - open;
  if (minutes <= 13 * 60) return morningMinutes;
  if (minutes <= 15 * 60) return morningMinutes + (minutes - 13 * 60);
  return SESSION_MINUTES;
}

/**
 * 单根 bar 的分钟跨度。
 * 取相邻 bar 交易日分钟差的中位数，而不是首尾均值：停牌、缺数据造成的空洞不该把整条横轴拉宽。
 * 交易日分钟差里午休本来就被抹平，正常的 11:30→13:01 相邻差仍是 1 分钟。
 */
export function inferBarMinutes(bars: readonly { timestamp: number }[]): number {
  if (bars.length < 2) return 1;
  const diffs: number[] = [];
  for (let i = 1; i < bars.length; i++) {
    const diff =
      tradingMinutesOfDay(new Date(bars[i].timestamp)) -
      tradingMinutesOfDay(new Date(bars[i - 1].timestamp));
    if (diff > 0) diffs.push(diff);
  }
  if (diffs.length === 0) return 1;
  diffs.sort((a, b) => a - b);
  return diffs[diffs.length >> 1];
}

export interface MinuteAxisLayout {
  /** 一个交易日的格子数（240 / barMinutes） */
  slots: number;
  /** 单根 bar 的分钟跨度 */
  barMinutes: number;
  /** 每根 bar 的像素宽 —— 让一整天恰好铺满图宽 */
  barSpace: number;
  /** 曲线左侧对齐后，右侧该留的空白（像素）。为 0 说明当天已走完 */
  offsetRightDistance: number;
}

/**
 * 分时图横轴排版。第 i 根 bar 的中心落在 (i + 0.5) 个 bar 宽处 —— 与 klinecharts
 * `dataIndexToCoordinate` 在这组参数下的结果一致（`barSpace * (i + 0.5)`），
 * 刻度才能和柱子对齐。
 */
export function minuteAxisLayout(
  plotWidth: number,
  bars: readonly { timestamp: number }[],
): MinuteAxisLayout {
  const barMinutes = inferBarMinutes(bars);
  const slots = Math.max(1, Math.round(SESSION_MINUTES / barMinutes));
  const barSpace = plotWidth > 0 ? plotWidth / slots : 0;
  return {
    slots,
    barMinutes,
    barSpace,
    offsetRightDistance: Math.max(0, plotWidth - bars.length * barSpace),
  };
}

/** 刻度：`value` 是交易日分钟数（klinecharts 只画 coord/text，value 仅作标记） */
export interface MinuteTick {
  coord: number;
  value: number;
  text: string;
}

/** 分时图横轴的固定刻度，与常见行情软件一致 */
export const SESSION_TICKS: readonly { minute: number; text: string }[] = [
  { minute: 0, text: '09:30' },
  { minute: 60, text: '10:30' },
  { minute: 120, text: '11:30/13:00' },
  { minute: 180, text: '14:00' },
  { minute: SESSION_MINUTES, text: '15:00' },
];

/**
 * 把固定刻度映射成像素坐标。用图表**当前**的 bar 宽而非理论值 —— setBarSpace 可能被
 * barSpaceLimit 夹住，那时刻度得跟着实际柱宽走。
 */
export function sessionTicks(plotWidth: number, barSpace: number, barMinutes: number): MinuteTick[] {
  if (!(plotWidth > 0) || !(barSpace > 0) || !(barMinutes > 0)) return [];
  const inset = Math.min(EDGE_INSET, plotWidth / 4);
  return SESSION_TICKS.map(({ minute, text }) => ({
    coord: Math.min(Math.max(barSpace * (minute / barMinutes + 0.5), inset), plotWidth - inset),
    value: minute,
    text,
  }));
}

// ---- 纵轴：以昨收为中心的对称区间 ----

/**
 * 涨跌幅很小时的半幅下限（昨收的 1%）。开盘头一分钟涨跌往往不到 0.05%，
 * 不兜底的话纵轴会被放大到看不出任何幅度。
 */
const MIN_RANGE_RATE = 0.01;

/**
 * 以昨收为中心的对称价格区间 —— 分时图的标准画法：0.00% 落在正中，涨跌一眼看出对称。
 * 半幅取收盘价相对昨收的最大偏离（分时图画的是一条收盘价连线，所以只看 close），
 * 再兜一个最小半幅。
 */
export function symmetricRange(
  bars: readonly { close: number }[],
  prevClose: number,
  minRate = MIN_RANGE_RATE,
): { from: number; to: number } {
  let half = 0;
  for (const bar of bars) {
    const deviation = Math.abs(bar.close - prevClose);
    if (deviation > half) half = deviation;
  }
  half = Math.max(half, Math.abs(prevClose) * minRate);
  return { from: prevClose - half, to: prevClose + half };
}

/** 价格相对昨收的涨跌幅文本，如 `+1.23%` / `-0.45%` */
export function percentText(price: number, prevClose: number, digits = 2): string {
  if (!(prevClose > 0)) return '';
  const rounded = (((price - prevClose) / prevClose) * 100).toFixed(digits);
  return `${Number(rounded) > 0 ? '+' : ''}${rounded}%`;
}

/**
 * 把一组价格刻度改写成涨跌幅文本 —— 坐标一律不动。
 * 涨跌幅轴和价格轴共用同一个区间，刻度也就逐行对应。
 */
export function percentTicks<T extends { value: number | string; text: string }>(
  ticks: readonly T[],
  prevClose: number,
): T[] {
  return ticks.map((tick) => ({ ...tick, text: percentText(Number(tick.value), prevClose) }));
}
