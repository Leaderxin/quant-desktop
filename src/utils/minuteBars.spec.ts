import { describe, expect, it } from 'vitest';
import type { MinuteData } from '@/types';
import { mapMinuteBars } from './minuteBars';

/**
 * 分时图数据整形的契约。
 *
 * 背景：分时接口给的是「最近 N 根」滚动窗口，不是一个交易日 —— 实测 2026-09-24 09:56
 * 拉腾讯 242 根，9-23 占 213 根、9-24 只有 29 根。所以这里锁两件事：只留最后一根所在
 * 的那个交易日；时间戳用数据源给出的真实日期，而不是本机当天（否则周末看到的周五行情、
 * 盘前看到的昨日行情，日期都会被标成今天）。
 */

function bar(time: string, price = 10): MinuteData {
  return { time, price, open: price, high: price, low: price, volume: 100, avg_price: price };
}

/** 便于断言：把时间戳还原成 `YYYY-MM-DD HH:mm`（本地时区，与构造时一致）。 */
function label(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

describe('mapMinuteBars', () => {
  it('跨交易日的窗口只保留最后一根所在的交易日', () => {
    const bars = mapMinuteBars([
      bar('2026-09-23 14:59'),
      bar('2026-09-23 15:00'),
      bar('2026-09-24 09:30'),
      bar('2026-09-24 09:31'),
    ]);

    expect(bars).toHaveLength(2);
    expect(bars.map((b) => label(b.timestamp))).toEqual([
      '2026-09-24 09:30',
      '2026-09-24 09:31',
    ]);
  });

  it('只有上一交易日的行情时保留整日，日期仍是上一交易日', () => {
    // 周末（2026-09-26 周六）打开分时图：数据是周五的，不能被标成周六
    const bars = mapMinuteBars(
      [bar('2026-09-25 09:30'), bar('2026-09-25 14:59'), bar('2026-09-25 15:00')],
      new Date(2026, 8, 26, 11, 0),
    );

    expect(bars.map((b) => label(b.timestamp))).toEqual([
      '2026-09-25 09:30',
      '2026-09-25 14:59',
      '2026-09-25 15:00',
    ]);
  });

  it('旧后端的时间戳（只有时刻）退回基准日，不丢数据', () => {
    const bars = mapMinuteBars(
      [bar('09:30'), bar('0931')],
      new Date(2026, 8, 24, 10, 0),
    );

    expect(bars.map((b) => label(b.timestamp))).toEqual([
      '2026-09-24 09:30',
      '2026-09-24 09:31',
    ]);
  });

  it('同一交易日内乱序与重复时间戳被归一并去重（后者生效）', () => {
    const bars = mapMinuteBars([
      bar('2026-09-24 09:32', 12),
      bar('2026-09-24 09:30', 10),
      bar('2026-09-24 09:31', 11),
      bar('2026-09-24 09:30', 10.5),
    ]);

    expect(bars.map((b) => label(b.timestamp))).toEqual([
      '2026-09-24 09:30',
      '2026-09-24 09:31',
      '2026-09-24 09:32',
    ]);
    expect(bars[0].close).toBe(10.5);
  });

  it('认不出时刻的条目被丢弃，且不影响交易日判定', () => {
    const bars = mapMinuteBars([bar('2026-09-24 09:30'), bar('--')]);

    expect(bars).toHaveLength(1);
    expect(label(bars[0].timestamp)).toBe('2026-09-24 09:30');
  });

  it('OHLC 与成交量按原值透传', () => {
    const bars = mapMinuteBars([
      {
        time: '2026-09-24 09:30',
        price: 10.2,
        open: 10,
        high: 10.5,
        low: 9.9,
        volume: 12345,
        avg_price: 10.1,
      },
    ]);

    expect(bars[0]).toMatchObject({
      open: 10,
      high: 10.5,
      low: 9.9,
      close: 10.2,
      volume: 12345,
    });
  });

  it('空数据与全坏数据都返回空数组而不抛错', () => {
    expect(mapMinuteBars([])).toEqual([]);
    expect(mapMinuteBars([bar(''), bar('n/a')])).toEqual([]);
  });
});
