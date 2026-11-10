import { describe, expect, it } from 'vitest';
import type { MinuteData } from '@/types';
import {
  beijingNow,
  cumulativeAvgPrices,
  mapMinuteBars,
  sessionKeyOf,
  sessionPrevClose,
  sessionUpdate,
} from './minuteBars';

/**
 * 分时图数据整形的契约。
 *
 * 背景：分时接口给的是「最近 N 根」滚动窗口，不是一个交易日 —— 实测 2026-09-24 09:56
 * 拉腾讯 242 根，9-23 占 213 根、9-24 只有 29 根。所以这里锁两件事：只留最后一根所在
 * 的那个交易日；时间戳用数据源给出的真实日期，而不是本机当天（否则周末看到的周五行情、
 * 盘前看到的昨日行情，日期都会被标成今天）。
 *
 * 另外锁住两个跟着窗口来的判断：昨收从窗口里取（画的是哪一天，基准就该是哪一天的），
 * 以及刷新时该整份重挂还是增量追加（交易日一变，增量追加就会把昨天留在图上）。
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

describe('cumulativeAvgPrices', () => {
  it('按成交量加权累计，均价从开盘单调地跟着成交走', () => {
    // 典型价 = (high + low + close) / 3；这里 high = low = close，退化为收盘价
    const avg = cumulativeAvgPrices([
      { close: 10, high: 10, low: 10, volume: 100 },
      { close: 12, high: 12, low: 12, volume: 100 },
      { close: 11, high: 11, low: 11, volume: 200 },
    ]);
    expect(avg).toEqual([10, 11, 11]);
  });

  it('用典型价而不是收盘价估算成交额', () => {
    // 一根振幅很大的 bar：(10 + 14 + 12) / 3 = 12
    expect(cumulativeAvgPrices([{ close: 12, high: 14, low: 10, volume: 100 }])).toEqual([12]);
  });

  it('开盘第一根就没有成交时退回该根收盘价，不产出 0', () => {
    const avg = cumulativeAvgPrices([
      { close: 10, high: 10, low: 10, volume: 0 },
      { close: 12, high: 12, low: 12, volume: 100 },
    ]);
    expect(avg).toEqual([10, 12]);
  });

  it('盘中停牌（已有累计成交量后出现零量 bar）均价保持上一根的值', () => {
    // 11:00 停牌：该 bar 不贡献成交额，均价停在 10，而不是跳到该根收盘价 12
    const avg = cumulativeAvgPrices([
      { close: 10, high: 10, low: 10, volume: 100 },
      { close: 12, high: 12, low: 12, volume: 0 },
    ]);
    expect(avg).toEqual([10, 10]);
  });

  it('缺 high/low/volume 时退回收盘价与 0，不产出 NaN', () => {
    expect(cumulativeAvgPrices([{ close: 8 }])).toEqual([8]);
    expect(cumulativeAvgPrices([])).toEqual([]);
  });
});

describe('mapMinuteBars 的均价字段', () => {
  it('只按当前交易日的成交累计 —— 上一交易日的量不参与', () => {
    const bars = mapMinuteBars([
      { time: '2026-09-23 15:00', price: 99, open: 99, high: 99, low: 99, volume: 1_000_000, avg_price: 99 },
      { time: '2026-09-24 09:30', price: 10, open: 10, high: 10, low: 10, volume: 100, avg_price: 10 },
      { time: '2026-09-24 09:31', price: 12, open: 12, high: 12, low: 12, volume: 100, avg_price: 12 },
    ]);

    expect(bars.map((b) => b.avgPrice)).toEqual([10, 11]);
  });
});

describe('sessionKeyOf', () => {
  it('给出的交易日与 mapMinuteBars 的切分口径一致', () => {
    const [first, last] = mapMinuteBars([bar('2026-09-24 09:30'), bar('2026-09-24 09:31')]);
    expect(sessionKeyOf(first.timestamp)).toBe('2026-09-24');
    expect(sessionKeyOf(last.timestamp)).toBe('2026-09-24');
  });
});

describe('sessionPrevClose', () => {
  it('取所画交易日之前那根 bar 的收盘价 —— 也就是上一交易日的收盘', () => {
    // 09:31 拉到的窗口：昨日收尾 + 今日开盘
    expect(
      sessionPrevClose([
        bar('2026-09-23 14:59', 10.1),
        bar('2026-09-23 15:00', 10.5),
        bar('2026-09-24 09:30', 10.8),
        bar('2026-09-24 09:31', 10.9),
      ]),
    ).toBe(10.5);
  });

  it('盘前只画得出昨日行情时，给的是昨日的昨收（不是今日的）', () => {
    // 盘前窗口里画的是 9-23 那一整天，基准就该是 9-22 的收盘
    expect(
      sessionPrevClose([
        bar('2026-09-22 15:00', 9.8),
        bar('2026-09-23 09:30', 9.9),
        bar('2026-09-23 15:00', 10.5),
      ]),
    ).toBe(9.8);
  });

  it('窗口里没有上一交易日时返回 null（新股上市首日），由调用方退回实时行情', () => {
    expect(sessionPrevClose([bar('2026-09-24 09:30'), bar('2026-09-24 09:31')])).toBeNull();
  });

  it('空数据与全坏数据返回 null', () => {
    expect(sessionPrevClose([])).toBeNull();
    expect(sessionPrevClose([bar('n/a')])).toBeNull();
  });
});

describe('sessionUpdate', () => {
  it('同一交易日继续追加', () => {
    const bars = mapMinuteBars([bar('2026-09-24 09:30'), bar('2026-09-24 09:31')]);
    expect(sessionUpdate('2026-09-24', bars)).toBe('append');
  });

  it('交易日变了要整份重挂 —— 增量追加只会往后接，昨天的 240 根会留在图里', () => {
    // 盘前开着面板，09:31 拿到的第一根落在新交易日
    const bars = mapMinuteBars([bar('2026-09-23 15:00'), bar('2026-09-24 09:30')]);
    expect(sessionUpdate('2026-09-23', bars)).toBe('reset');
  });

  it('图里还没装过数据时也要重挂（图里装的是哪一天记不住）', () => {
    expect(sessionUpdate(null, mapMinuteBars([bar('2026-09-24 09:30')]))).toBe('reset');
  });

  it('这一轮没有新数据时什么都不用做', () => {
    expect(sessionUpdate('2026-09-24', [])).toBe('append');
  });
});

describe('beijingNow', () => {
  it('把本机时刻换算成数据源钟点（UTC+8），与 bar 时间戳同一口径', () => {
    // 2026-09-24T01:30Z = 北京时间 09:30
    const bj = new Date(beijingNow(Date.UTC(2026, 8, 24, 1, 30)));
    expect([bj.getFullYear(), bj.getMonth(), bj.getDate(), bj.getHours(), bj.getMinutes()]).toEqual([
      2026, 8, 24, 9, 30,
    ]);
  });

  it('盘中拉到的 bar 不会被判成未来 —— 本机时区不参与这个比较', () => {
    const [open] = mapMinuteBars([bar('2026-09-24 09:30')]);
    // 不管本机在哪个时区：北京 09:31 时，标着 09:30 的那根是过去的
    expect(open.timestamp).toBeLessThanOrEqual(beijingNow(Date.UTC(2026, 8, 24, 1, 31)));
  });
});
