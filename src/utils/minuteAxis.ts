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

/**
 * klinecharts 的 `barSpaceLimit.min`（v10 默认 1）：柱宽小于它时 `setBarSpace` 会被**静默丢掉**，
 * 图表沿用原柱宽 —— 自算的留白和刻度就全对不上真实柱宽了。
 */
export const MIN_BAR_SPACE = 1;

/**
 * 一整天能不能按这个柱宽铺进图宽。铺不下时（图区窄于 240 × MIN_BAR_SPACE，窗口被拖得很窄）
 * 整条横轴都得让给 klinecharts 自己排：先把柱宽缩到 MIN_BAR_SPACE 以下是不可能的，
 * 硬按 `图宽 / 格子数` 算刻度只会得到一串被钳在右缘、叠在一起的时间。
 */
export function sessionFits(plotWidth: number, barMinutes: number): boolean {
  if (!(plotWidth > 0) || !(barMinutes > 0)) return false;
  return plotWidth / (SESSION_MINUTES / barMinutes) >= MIN_BAR_SPACE;
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

// ---- 纵轴：自适应区间 ----

/**
 * 0% 两侧的保底幅度（昨收的 1%）。开盘头一分钟涨跌往往不到 0.05%，
 * 不兜底的话纵轴会被放大到看不出任何幅度；单边行情下也靠它把 0% 线留在图内。
 */
const MIN_RANGE_RATE = 0.01;

/**
 * 分时图纵轴区间：覆盖价格线与均价线，但不强制以昨收为中心对称。
 *
 * 早先是上下完全均分（0.00% 在正中），单边行情（尤其接近涨跌停）下曲线会整段压在
 * 上半区或下半区，浪费一半画布。这里改成自适应：
 * - 上沿取数据相对昨收的最大正偏离，下沿取最大负偏离；
 * - 两侧各保底 MIN_RANGE_RATE（昨收的 1%）：0% 线仍有可见空间，全天走平时也不会被放大。
 *
 * 收盘价与均价一起参与取幅：均价由典型价累计，可能略微超出收盘价范围，不一起算会被
 * 裁到图外。
 */
export function adaptiveRange(
  bars: readonly { close: number; avgPrice?: unknown }[],
  prevClose: number,
  minSideRate = MIN_RANGE_RATE,
): { from: number; to: number } {
  let up = 0;
  let down = 0;
  const consider = (value: unknown) => {
    if (typeof value !== 'number' || !Number.isFinite(value)) return;
    const diff = value - prevClose;
    if (diff > up) up = diff;
    if (-diff > down) down = -diff;
  };
  for (const bar of bars) {
    consider(bar.close);
    consider(bar.avgPrice);
  }
  const floor = Math.abs(prevClose) * minSideRate;
  up = Math.max(up, floor);
  down = Math.max(down, floor);
  return { from: prevClose - down, to: prevClose + up };
}

/** 价格相对昨收的涨跌幅文本，如 `+1.23%` / `-0.45%` */
export function percentText(price: number, prevClose: number, digits = 2): string {
  if (!(prevClose > 0)) return '';
  const rounded = (((price - prevClose) / prevClose) * 100).toFixed(digits);
  return `${Number(rounded) > 0 ? '+' : ''}${rounded}%`;
}
