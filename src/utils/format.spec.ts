import { describe, expect, it } from 'vitest';
import { formatIndexCode, formatTurnoverDelta, formatPrice, getQuotePrecision } from './format';

// 「较昨同期」的增量界面会按应用配色上色（放量=涨色、缩量=跌色，随 cn/us
// 翻转），但符号仍必须留在文字里 —— 颜色不是唯一信号。这里把「符号 + 取整到亿
// + 持平阈值」钉住。
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

// ETF/LOF 的最小变动价位是 0.001，精度必须按**类别**定：按数值判断会让 0.740
// 这类整分价格掉回 2 位，同一只 ETF 盘中精度来回跳。
describe('getQuotePrecision / formatPrice', () => {
  it('ETF/LOF 固定 3 位，整分价格也不降级', () => {
    expect(getQuotePrecision('sh588760', 0.74)).toBe(3);
    expect(getQuotePrecision('sz159929', 1.23)).toBe(3);
    expect(getQuotePrecision('sh510300', 4)).toBe(3);
    expect(formatPrice(0.74, '--', getQuotePrecision('sh588760', 0.74))).toBe('0.740');
    expect(formatPrice(0.01, '--', getQuotePrecision('sh588760', 0.01))).toBe('0.010');
    expect(formatPrice(-0.013, '--', getQuotePrecision('sh588760', -0.013))).toBe('-0.013');
  });

  it('股票 2 位，可转债仍按值自适配到 3 位', () => {
    expect(getQuotePrecision('sh600519', 1257.12)).toBe(2);
    expect(getQuotePrecision('sz000001', 0.01)).toBe(2);
    expect(getQuotePrecision('sh113001', 123.456)).toBe(3);
    expect(formatPrice(123.456, '--', getQuotePrecision('sh113001', 123.456))).toBe('123.456');
  });
});
