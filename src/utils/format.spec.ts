import { describe, expect, it } from 'vitest';
import { formatIndexCode, formatTurnoverDelta } from './format';

// 「较昨同期」的增量是唯一一个只有符号能表达方向的地方 —— 它不套涨跌色
// （放量/缩量不是涨跌，且切到 us 配色后红绿会反过来），所以正负号一旦丢了，
// 界面就只剩一个看不出方向的数量。这里把「符号 + 取整到亿 + 持平阈值」钉住。
describe('formatTurnoverDelta', () => {
  it('符号留在文字里', () => {
    expect(formatTurnoverDelta(1234e8)).toBe('+1234亿');
    expect(formatTurnoverDelta(-987.4e8)).toBe('-987亿');
    expect(formatTurnoverDelta(987.6e8)).toBe('+988亿');
  });

  it('不足一亿按持平，不出现 +0亿', () => {
    expect(formatTurnoverDelta(0)).toBe('持平');
    expect(formatTurnoverDelta(0.3e8)).toBe('持平');
    expect(formatTurnoverDelta(-0.4e8)).toBe('持平');
  });

  it('拿不到增量时给空串，调用方据此整块隐藏', () => {
    expect(formatTurnoverDelta(null)).toBe('');
    expect(formatTurnoverDelta(undefined)).toBe('');
    expect(formatTurnoverDelta(NaN)).toBe('');
  });
});

// 指数代码在详情页与设置页各显示一处，两处都得是同一个写法 —— 顺带防止
// 有人把 `s_` 当成交所前缀一起删掉（`s_sz399001` → `399001` 会丢掉深市标记）。
describe('formatIndexCode', () => {
  it('只去数据源的 s_ 前缀，交易所前缀保留', () => {
    expect(formatIndexCode('s_sh000001')).toBe('sh000001');
    expect(formatIndexCode('s_sz399001')).toBe('sz399001');
    expect(formatIndexCode('sh000001')).toBe('sh000001');
    expect(formatIndexCode('')).toBe('');
  });
});
