import { describe, expect, it } from 'vitest';
import {
  SESSION_MINUTES,
  SESSION_TICKS,
  inferBarMinutes,
  minuteAxisLayout,
  percentText,
  percentTicks,
  sessionTicks,
  symmetricRange,
  tradingMinutesOfDay,
} from './minuteAxis';

/**
 * 分时图横轴排版的契约。
 *
 * 背景：横轴固定一个交易日（09:30–15:00），曲线自左端起、未走到的时段留白。klinecharts
 * 默认按数据根数排版（不够铺满时贴右边），所以格子数、柱宽、右侧留白都得自己算 ——
 * 这几个数算错的表现是「曲线挤在右边」或「柱子与刻度对不上」，都是肉眼看不出原因的那类。
 */

function bars(times: string[]): { timestamp: number }[] {
  return times.map((t) => {
    const [h, m] = t.split(':').map(Number);
    return { timestamp: new Date(2026, 8, 24, h, m).getTime() };
  });
}

/** 09:30 起、每根 1 分钟的 n 根 bar */
function minuteBars(n: number): { timestamp: number }[] {
  return Array.from({ length: n }, (_, i) => {
    const t = 9 * 60 + 30 + i;
    return { timestamp: new Date(2026, 8, 24, Math.floor(t / 60), t % 60).getTime() };
  });
}

/** 09:35 起、每根 5 分钟（新浪分时）的 n 根 bar */
function fiveMinuteBars(n: number): { timestamp: number }[] {
  return Array.from({ length: n }, (_, i) => {
    const t = 9 * 60 + 35 + i * 5;
    return { timestamp: new Date(2026, 8, 24, Math.floor(t / 60), t % 60).getTime() };
  });
}

describe('tradingMinutesOfDay', () => {
  it('按交易时段计数，午休不计', () => {
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 9, 0))).toBe(0); // 开盘前
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 9, 30))).toBe(0);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 10, 30))).toBe(60);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 11, 30))).toBe(120);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 12, 30))).toBe(120); // 午休
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 13, 0))).toBe(120);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 14, 0))).toBe(180);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 15, 0))).toBe(SESSION_MINUTES);
    expect(tradingMinutesOfDay(new Date(2026, 8, 24, 16, 0))).toBe(SESSION_MINUTES);
  });
});

describe('inferBarMinutes', () => {
  it('1 分钟线推 1，5 分钟线推 5', () => {
    expect(inferBarMinutes(minuteBars(30))).toBe(1);
    expect(inferBarMinutes(fiveMinuteBars(12))).toBe(5);
  });

  it('午休不算断层：11:30 与 13:01 的相邻差仍是 1 分钟', () => {
    const acrossLunch = bars(['11:29', '11:30', '13:01', '13:02']);
    expect(inferBarMinutes(acrossLunch)).toBe(1);
  });

  it('停牌造成的空洞不拉宽整条横轴（取中位数而非均值）', () => {
    // 10:00 之后停了 60 分钟，均值会被拉到 ~20 分钟
    const halted = bars(['09:58', '09:59', '10:00', '11:00', '11:01', '11:02']);
    expect(inferBarMinutes(halted)).toBe(1);
  });

  it('数据不足时退回 1 分钟', () => {
    expect(inferBarMinutes([])).toBe(1);
    expect(inferBarMinutes(minuteBars(1))).toBe(1);
    expect(inferBarMinutes(bars(['09:30', '09:30']))).toBe(1);
  });
});

describe('minuteAxisLayout', () => {
  it('一个交易日恰好铺满图宽，曲线自左端起', () => {
    const layout = minuteAxisLayout(720, minuteBars(41));

    expect(layout.slots).toBe(240);
    expect(layout.barSpace).toBe(3); // 720 / 240
    expect(layout.offsetRightDistance).toBe(720 - 41 * 3); // 右侧留白 = 没走到的时段
  });

  it('柱宽按 bar 的分钟跨度换算：一根 bar 占的宽度等于它那几分钟在全天里的占比', () => {
    const minute = minuteAxisLayout(720, minuteBars(41)); // 1 分钟一根
    const five = minuteAxisLayout(720, fiveMinuteBars(9)); // 5 分钟一根

    expect(minute.slots).toBe(240);
    expect(five.slots).toBe(48);
    // 同为 720px 图宽：1 分钟一根时一根 3px，5 分钟一根时一根 15px —— 横向时间尺度一致
    expect(minute.barSpace / 720).toBeCloseTo(1 / SESSION_MINUTES, 6);
    expect(five.barSpace / 720).toBeCloseTo(5 / SESSION_MINUTES, 6);
  });

  it('当天走完（曲线已顶到右边缘）时右侧留白为 0，不出现负留白', () => {
    const full = minuteAxisLayout(720, minuteBars(SESSION_MINUTES + 1));
    expect(full.offsetRightDistance).toBe(0);
  });

  it('宽度未知时不产生 NaN 排版', () => {
    const layout = minuteAxisLayout(0, minuteBars(10));
    expect(layout.barSpace).toBe(0);
    expect(layout.offsetRightDistance).toBe(0);
  });
});

describe('sessionTicks', () => {
  it('固定五个刻度，收盘贴着右缘但内缩半个标签宽', () => {
    const ticks = sessionTicks(720, 3, 1);

    expect(ticks.map((t) => t.text)).toEqual(SESSION_TICKS.map((t) => t.text));
    expect(ticks[0].coord).toBeGreaterThanOrEqual(16); // 09:30 内缩，否则半个字被裁掉
    expect(ticks[ticks.length - 1].coord).toBeLessThanOrEqual(720 - 16); // 15:00 同理
  });

  it('刻度位置随时间单调递增，中间刻度落在对应时刻的格子上', () => {
    const ticks = sessionTicks(720, 3, 1);
    const coords = ticks.map((t) => t.coord);
    expect([...coords].sort((a, b) => a - b)).toEqual(coords);
    expect(coords[1]).toBeCloseTo(3 * (60 + 0.5), 1); // 10:30 在第 60 格中心
  });

  it('5 分钟线下按格子换算，而不是按分钟数', () => {
    const ticks = sessionTicks(720, 15, 5);
    expect(ticks[1].coord).toBeCloseTo(15 * (60 / 5 + 0.5), 1); // 10:30 = 第 12 格
  });

  it('宽度或柱宽无效时给空列表（调用方据此退回默认刻度）', () => {
    expect(sessionTicks(0, 3, 1)).toEqual([]);
    expect(sessionTicks(720, 0, 1)).toEqual([]);
    expect(sessionTicks(720, 3, 0)).toEqual([]);
  });
});

describe('symmetricRange', () => {
  it('半幅取收盘价相对昨收的最大偏离，两侧对称', () => {
    const range = symmetricRange([{ close: 103 }, { close: 97 }, { close: 100 }], 100);
    expect(range).toEqual({ from: 97, to: 103 });
  });

  it('涨跌不到 1% 时兜底到昨收的 1%，免得开盘头一分钟被放大到满屏', () => {
    const range = symmetricRange([{ close: 100.1 }, { close: 100.05 }], 100);
    expect(range.from).toBeCloseTo(99, 6);
    expect(range.to).toBeCloseTo(101, 6);
  });

  it('已超过 1% 时按实际幅度走，不额外留白', () => {
    expect(symmetricRange([{ close: 106 }], 100)).toEqual({ from: 94, to: 106 });
  });
});

describe('percentText / percentTicks', () => {
  it('按昨收换算，正数带 + 号', () => {
    expect(percentText(110, 100)).toBe('+10.00%');
    expect(percentText(90, 100)).toBe('-10.00%');
    expect(percentText(100, 100)).toBe('0.00%');
  });

  it('昨收无效时给空串，不写出 Infinity%', () => {
    expect(percentText(110, 0)).toBe('');
    expect(percentText(110, NaN)).toBe('');
  });

  it('percentTicks 只换文字，坐标与 value 原样保留', () => {
    const ticks = [{ coord: 12, value: '110', text: '110' }];
    expect(percentTicks(ticks, 100)).toEqual([{ coord: 12, value: '110', text: '+10.00%' }]);
  });
});
