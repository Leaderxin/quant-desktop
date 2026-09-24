// src/utils/minuteBars.ts

import type { KLineData as KCLineData } from 'klinecharts';
import type { MinuteData } from '@/types';

/**
 * 分时图数据整形：MinuteData → 图表 bar。
 *
 * 两个约束决定了这里不能直接用接口返回的原始序列：
 *
 * 1. **接口返回的是滚动窗口，不是一个交易日。** 腾讯 mkline 的 `,,242`、新浪的
 *    `datalen=240&scale=5` 都是「最近 N 根」，盘中任意时刻都跨着上一个交易日
 *    （实测 2026-09-24 09:56 拉腾讯 242 根：9-23 有 213 根、9-24 只有 29 根）。
 *    分时图只画一个交易日，所以按日期切出最后一根所在的那个交易日；不切的话图上
 *    会画出「今天的下午」——那其实是昨日下午，日期自然也就跟着错。
 *
 * 2. **日期必须来自数据本身。** 早先的实现把每根 bar 都补上本机当天，于是周末查到
 *    的周五行情、盘前查到的昨日行情，统统被标成了今天。数据源已经给出带日期的
 *    时间戳（腾讯 `202609230958`、新浪 `2026-09-23 09:35:00`，适配器统一成
 *    `YYYY-MM-DD HH:mm`），照用即可。
 */

/** 数据源时间戳：`2026-09-23 09:58`（适配器统一后的格式） */
const WITH_DATE = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/;
/** 只有时刻的旧格式：`09:58`（后端旧版本） */
const TIME_ONLY = /^(\d{1,2}):(\d{2})$/;
/** 只有时刻的旧格式：`0958`（后端旧版本） */
const HHMM = /^(\d{2})(\d{2})$/;

interface Stamped {
  bar: KCLineData;
  /** 所属交易日 `YYYY-MM-DD`，用于切分交易日 */
  day: string;
}

function pad(n: number): string {
  return String(n).padStart(2, '0');
}

/**
 * 单根 bar 的映射。时间戳缺日期时（旧后端）退回 `base` 当天，此时全部 bar 归入同一天，
 * 交易日切分退化为「原样返回」，与旧行为一致。时刻认不出来则丢弃该根 —— 宁可少画一根，
 * 也不能把它放到错误的横坐标上。
 */
function stamp(d: MinuteData, base: Date): Stamped | null {
  let y: number, mo: number, day: number, h: number, mi: number;

  const m = WITH_DATE.exec(d.time);
  if (m) {
    y = Number(m[1]); mo = Number(m[2]); day = Number(m[3]);
    h = Number(m[4]); mi = Number(m[5]);
  } else {
    const t = TIME_ONLY.exec(d.time) ?? HHMM.exec(d.time);
    if (!t) return null;
    y = base.getFullYear(); mo = base.getMonth() + 1; day = base.getDate();
    h = Number(t[1]); mi = Number(t[2]);
  }

  const price = d.price;
  return {
    day: `${y}-${pad(mo)}-${pad(day)}`,
    bar: {
      timestamp: new Date(y, mo - 1, day, h, mi).getTime(),
      open: d.open ?? price,
      high: d.high ?? price,
      low: d.low ?? price,
      close: price,
      volume: d.volume,
    },
  };
}

/** 数据里最后一个交易日 —— 分时图要画的那一天 */
function lastSessionDay(stamped: Stamped[]): string {
  return stamped[stamped.length - 1].day;
}

/** 时间戳所属的交易日 `YYYY-MM-DD`（与 stamp 同口径：按本机时区还原出的那一天） */
export function sessionKeyOf(timestamp: number): string {
  const d = new Date(timestamp);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * 分时图 bar：只保留数据里最后一个交易日，按时间升序、时间戳去重。
 * @param base 时间戳缺日期时的基准日（默认当前时间），仅为兼容旧后端存在。
 */
export function mapMinuteBars(data: MinuteData[], base: Date = new Date()): KCLineData[] {
  const stamped: Stamped[] = [];
  for (const d of data) {
    const s = stamp(d, base);
    if (s) stamped.push(s);
  }
  if (stamped.length === 0) return [];

  // 接口按时间升序返回，最后一根所属的交易日即当前交易日
  const session = lastSessionDay(stamped);

  // 同一分钟在两日窗口重叠时可能出现重复，按时间戳去重（保后者）
  const byTimestamp = new Map<number, KCLineData>();
  for (const s of stamped) {
    if (s.day === session) byTimestamp.set(s.bar.timestamp, s.bar);
  }

  return [...byTimestamp.values()].sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * 所画交易日的昨收 —— 窗口里紧邻它前面那根 bar 的收盘价，也就是上一个交易日的收盘。
 *
 * 纵轴以昨收为中心对称，而「昨收」是相对**画出来那一天**说的：盘前窗口里画的是昨日行情，
 * 这时实时行情的 `price - change` 说的是今日昨收，两者差着昨日一整个涨跌幅，曲线会被推到
 * 中心线一侧。所以基准优先从数据里取。
 *
 * 窗口里没有上一交易日时（新股上市首日）返回 null，调用方再退回实时行情。
 */
export function sessionPrevClose(data: MinuteData[], base: Date = new Date()): number | null {
  const stamped: Stamped[] = [];
  for (const d of data) {
    const s = stamp(d, base);
    if (s) stamped.push(s);
  }
  if (stamped.length === 0) return null;

  const session = lastSessionDay(stamped);
  let prevClose: number | null = null;
  for (const s of stamped) {
    if (s.day === session) break;
    prevClose = s.bar.close;
  }
  return prevClose;
}

/** 增量刷新要怎么落到图表上 */
export type SessionUpdate = 'reset' | 'append';

/**
 * 判断本次刷新该整份重挂（reset）还是增量推送（append）。
 *
 * 交易日变了必须重挂：图表的增量回调（subscribeBar → `_addData(data, 'update')`）只会
 * 「时间戳比最后一根大就追加」，上一个交易日的 240 根会留在图里 —— 画出来正是「昨天的下午」
 * 接着今天的早盘，也就是分时图最初那个日期错乱。图里装的是哪一天记不住时也重挂。
 */
export function sessionUpdate(
  renderedSession: string | null,
  bars: readonly { timestamp: number }[],
): SessionUpdate {
  if (bars.length === 0) return 'append';
  if (renderedSession === null) return 'reset';
  return sessionKeyOf(bars[bars.length - 1].timestamp) === renderedSession ? 'append' : 'reset';
}

/** 数据源时区（北京时间 UTC+8，见 src-tauri/src/datasource/market_clock.rs）相对 UTC 的偏移 */
const SOURCE_OFFSET_MINUTES = 8 * 60;

/**
 * 与 bar 时间戳同一口径的「现在」。
 *
 * bar 的时间戳是「数据源给的钟点按**本机**时区拼出来的」（见 stamp），拿它直接和 Date.now()
 * 比就掺进了时差：本机在 UTC+8 以西时，标着 09:35 的那根会被当成还没到的一点，整段会话都
 * 被判成未来、被过期过滤整批丢掉。这里把当下换算到数据源钟点上再按同一口径拼时间戳。
 */
export function beijingNow(now: number = Date.now()): number {
  const local = new Date(now);
  const shifted = new Date(now + (SOURCE_OFFSET_MINUTES + local.getTimezoneOffset()) * 60_000);
  return new Date(
    shifted.getFullYear(), shifted.getMonth(), shifted.getDate(),
    shifted.getHours(), shifted.getMinutes(),
  ).getTime();
}
