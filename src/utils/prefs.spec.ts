// src/utils/prefs.spec.ts
//
// 设置值存在 SQLite 的字符串表里，可能被手工改坏、也可能来自旧版本。
// 这些解析函数的契约是「任何坏输入都退回一个可用值，绝不抛出」—— 一个坏键
// 不该让整个界面白屏，所以坏输入的路径必须逐个覆盖。
import { describe, it, expect } from 'vitest';
import {
  ALL_COLUMNS,
  REQUIRED_COLUMNS,
  clampTickerItems,
  clampTopN,
  parseBool,
  parseColumns,
  parseCount,
  parseDefaultSort,
  parseJsonArray,
} from './prefs';

describe('parseJsonArray', () => {
  it('解析合法数组', () => {
    expect(parseJsonArray<string>('["a","b"]', [])).toEqual(['a', 'b']);
  });

  it('缺键、非法 JSON、类型不符都退回 fallback', () => {
    expect(parseJsonArray(undefined, ['x'])).toEqual(['x']);
    expect(parseJsonArray('', ['x'])).toEqual(['x']);
    expect(parseJsonArray('not json', ['x'])).toEqual(['x']);
    // JSON 合法但不是数组 —— 直接返回它会让调用方在后面对字符串做 .map 而崩
    expect(parseJsonArray('"just a string"', ['x'])).toEqual(['x']);
    expect(parseJsonArray('{"a":1}', ['x'])).toEqual(['x']);
  });
});

describe('parseColumns', () => {
  it('缺键时返回全部列（含必选列，顺序即 ALL_COLUMNS）', () => {
    expect(parseColumns(undefined)).toEqual(ALL_COLUMNS.map((c) => c.key));
  });

  it('过滤未知列并去重', () => {
    const parsed = parseColumns('["code","price","price","bogus","name","volume"]');
    expect(parsed).toEqual(['code', 'price', 'name', 'volume']);
    expect(parsed).not.toContain('bogus');
  });

  it('必选列被坏配置抹掉时补回队首，但不强制排在已有列之前', () => {
    // 只剩 price/volume 的坏配置：代码与名称是表格可读性下限，必须补回来
    expect(parseColumns('["price","volume"]')).toEqual(['code', 'name', 'price', 'volume']);
  });

  it('尊重用户把名称拖到代码之前的顺序', () => {
    // 与上一条的区别：两列都在，此时用户顺序是要尊重的，不该被"必选列排最前"打乱
    expect(parseColumns('["name","code","price"]')).toEqual(['name', 'code', 'price']);
  });

  it('全是未知列时退回全部列', () => {
    expect(parseColumns('["bogus","nope"]')).toEqual(ALL_COLUMNS.map((c) => c.key));
  });

  it('必选列是代码与名称', () => {
    expect(REQUIRED_COLUMNS).toEqual(['code', 'name']);
  });
});

describe('parseDefaultSort', () => {
  it('解析合法配置', () => {
    expect(parseDefaultSort('{"key":"change_pct","order":"ascend"}'))
      .toEqual({ key: 'change_pct', order: 'ascend' });
  });

  it('空串 / 非法 JSON / 未知列都表示「不排序」', () => {
    expect(parseDefaultSort(undefined)).toBeNull();
    expect(parseDefaultSort('')).toBeNull();
    expect(parseDefaultSort('{')).toBeNull();
    expect(parseDefaultSort('{"key":"bogus"}')).toBeNull();
    expect(parseDefaultSort('null')).toBeNull();
  });

  it('order 非法时退回降序，而不是原样透传给 naive-ui', () => {
    expect(parseDefaultSort('{"key":"price","order":"sideways"}'))
      .toEqual({ key: 'price', order: 'descend' });
  });
});

describe('clampTopN / clampTickerItems', () => {
  it('夹到各自区间', () => {
    expect(clampTopN(0)).toBe(1);
    expect(clampTopN(5)).toBe(5);
    expect(clampTopN(999)).toBe(50);
    expect(clampTickerItems(0)).toBe(1);
    expect(clampTickerItems(2)).toBe(2);
    expect(clampTickerItems(99)).toBe(4);
  });

  it('小数取整、NaN 退回下限', () => {
    expect(clampTopN(7.6)).toBe(8);
    expect(clampTopN(Number.NaN)).toBe(1);
    expect(clampTickerItems(Number.NaN)).toBe(1);
  });
});

describe('parseBool / parseCount', () => {
  it('parseBool 只认 "1"/"0"，其余退回 fallback', () => {
    expect(parseBool('1', false)).toBe(true);
    expect(parseBool('0', true)).toBe(false);
    expect(parseBool(undefined, true)).toBe(true);
    expect(parseBool('true', false)).toBe(false);
  });

  it('parseCount 解析失败时走 clamp 后的 fallback', () => {
    expect(parseCount('10', 5, clampTopN)).toBe(10);
    expect(parseCount(undefined, 5, clampTopN)).toBe(5);
    expect(parseCount('abc', 5, clampTopN)).toBe(5);
    expect(parseCount('999', 5, clampTopN)).toBe(50);
  });
});
