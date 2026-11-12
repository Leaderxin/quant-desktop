// src/utils/priceLimit.spec.ts
//
// 涨跌停比例直接决定行情条上两条边界线画在哪。判错的代价不是「少一条线」，
// 而是把 +5% 的 ST 股画在 ±10% 的区间里、看起来离涨停还很远 —— 所以这里把
// 各板块、ST、基金/指数/可转债的返回值逐个钉住。
import { describe, expect, it } from 'vitest';
import { isStName, limitPrices, priceLimitRate } from './priceLimit';

describe('priceLimitRate', () => {
  it('沪深主板 10%', () => {
    expect(priceLimitRate('sh600000', '浦发银行')).toBe(0.10);
    expect(priceLimitRate('sh601318', '中国平安')).toBe(0.10);
    expect(priceLimitRate('sh603259', '药明康德')).toBe(0.10);
    expect(priceLimitRate('sz000001', '平安银行')).toBe(0.10);
    expect(priceLimitRate('sz002594', '比亚迪')).toBe(0.10);
    expect(priceLimitRate('sz003816', '中国广核')).toBe(0.10);
  });

  it('创业板 / 科创板 20%', () => {
    expect(priceLimitRate('sz300750', '宁德时代')).toBe(0.20);
    expect(priceLimitRate('sz301029', '怡合达')).toBe(0.20);
    expect(priceLimitRate('sz302132', '中航成飞')).toBe(0.20);
    expect(priceLimitRate('sh688981', '中芯国际')).toBe(0.20);
    expect(priceLimitRate('sh689009', '九号公司')).toBe(0.20);
  });

  it('北交所 30%', () => {
    expect(priceLimitRate('bj430047', '诺思兰德')).toBe(0.30);
    expect(priceLimitRate('bj920001', '万达轴承')).toBe(0.30);
  });

  it('ST 只影响沪深主板（5%）；创业板/科创板/北交所 ST 仍按板块比例', () => {
    expect(priceLimitRate('sh600000', 'ST易购')).toBe(0.05);
    expect(priceLimitRate('sh600000', '*ST美谷')).toBe(0.05);
    expect(priceLimitRate('sz000001', 'st平安')).toBe(0.05);
    expect(priceLimitRate('sz300750', 'ST宁德')).toBe(0.20);
    expect(priceLimitRate('sh688981', 'ST中芯')).toBe(0.20);
    expect(priceLimitRate('bj430047', 'ST诺思')).toBe(0.30);
  });

  it('B 股 10%', () => {
    expect(priceLimitRate('sh900901', '云赛B股')).toBe(0.10);
    expect(priceLimitRate('sz200002', '万科B')).toBe(0.10);
  });

  it('指数、ETF/LOF、可转债、逆回购返回 null（回退价格轴）', () => {
    expect(priceLimitRate('sh000001', '上证指数')).toBeNull();
    expect(priceLimitRate('sz399001', '深证成指')).toBeNull();
    expect(priceLimitRate('bj899050', '北证50')).toBeNull();
    expect(priceLimitRate('sh510300', '沪深300ETF')).toBeNull();
    expect(priceLimitRate('sh588000', '科创50ETF')).toBeNull();
    expect(priceLimitRate('sz159915', '创业板ETF')).toBeNull();
    expect(priceLimitRate('sz160632', '白酒LOF')).toBeNull();
    expect(priceLimitRate('sh113001', '转债')).toBeNull();
    expect(priceLimitRate('sz123001', '转债')).toBeNull();
    expect(priceLimitRate('sh204001', 'GC001')).toBeNull();
    expect(priceLimitRate('sz131810', 'R-001')).toBeNull();
  });

  it('N/C 新股阶段与无法识别的代码返回 null', () => {
    expect(priceLimitRate('sh600000', 'N某某')).toBeNull();
    expect(priceLimitRate('sz300750', 'C宁德')).toBeNull();
    expect(priceLimitRate('600000', '浦发银行')).toBeNull();
    expect(priceLimitRate('sh60000', '浦发银行')).toBeNull();
    expect(priceLimitRate('hk00700', '腾讯')).toBeNull();
  });

  it('容忍 s_ 前缀与大小写', () => {
    expect(priceLimitRate('s_sh000001', '上证指数')).toBeNull();
    expect(priceLimitRate('SH600000', '浦发银行')).toBe(0.10);
  });
});

describe('isStName', () => {
  it('识别 ST / *ST 前缀，不误伤普通名称', () => {
    expect(isStName('ST易购')).toBe(true);
    expect(isStName('*ST美谷')).toBe(true);
    expect(isStName(' st 某某')).toBe(true);
    expect(isStName('浦发银行')).toBe(false);
    expect(isStName('BEST股份')).toBe(false);
  });
});

describe('limitPrices', () => {
  it('四舍五入到分，并消掉浮点误差', () => {
    expect(limitPrices(10.05, 0.10)).toEqual({ up: 11.06, down: 9.05 });
    expect(limitPrices(3.33, 0.10)).toEqual({ up: 3.66, down: 3.00 });
    expect(limitPrices(1.05, 0.05)).toEqual({ up: 1.10, down: 1.00 });
    expect(limitPrices(100, 0.20)).toEqual({ up: 120, down: 80 });
    expect(limitPrices(100, 0.30)).toEqual({ up: 130, down: 70 });
  });
});
