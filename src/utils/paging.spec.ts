// src/utils/paging.spec.ts
//
// 行情条轮播的分屏计算。它同时管两件事：取哪一屏，以及下一屏从哪开始。
// 后者有一处容易漏的边界 —— 列表短到一屏装得下时必须归零，否则一套只有 2 只
// 自选、每屏显示 4 条的配置会每隔 3 秒把两行数据上下颠倒一次。
import { describe, expect, it } from 'vitest';
import { nextPageStart, windowItems } from './paging';

const ABCDE = ['A', 'B', 'C', 'D', 'E'];

describe('windowItems', () => {
  it('取连续的一屏', () => {
    expect(windowItems(ABCDE, 0, 2)).toEqual(['A', 'B']);
    expect(windowItems(ABCDE, 2, 2)).toEqual(['C', 'D']);
  });

  it('越过后按模回绕', () => {
    expect(windowItems(ABCDE, 4, 2)).toEqual(['E', 'A']);
  });

  it('起始下标超出长度也不取空（每屏条数刚调大时会出现）', () => {
    // 原来 5 只、每屏 2 条，page 停在 4；改成每屏 3 条后仍应取到完整一屏
    expect(windowItems(ABCDE, 4, 3)).toEqual(['E', 'A', 'B']);
    expect(windowItems(ABCDE, 99, 2)).toHaveLength(2);
  });

  it('每屏条数超过列表长度时按列表长度截断，不重复展示', () => {
    expect(windowItems(['A', 'B'], 0, 4)).toEqual(['A', 'B']);
    expect(windowItems(['A'], 0, 4)).toEqual(['A']);
  });

  it('空列表返回空', () => {
    expect(windowItems([], 0, 2)).toEqual([]);
    expect(windowItems([], 7, 2)).toEqual([]);
  });

  it('每屏条数下限为 1（0 或负数不应取到空屏）', () => {
    expect(windowItems(ABCDE, 0, 0)).toEqual(['A']);
    expect(windowItems(ABCDE, 0, -3)).toEqual(['A']);
  });
});

describe('nextPageStart', () => {
  it('按每屏条数前进', () => {
    expect(nextPageStart(0, 5, 2)).toBe(2);
    expect(nextPageStart(2, 5, 2)).toBe(4);
  });

  it('到末尾回绕', () => {
    expect(nextPageStart(4, 5, 2)).toBe(1);
  });

  it('一屏装得下时恒归零 —— 否则短列表会每 3 秒自我重排一次', () => {
    expect(nextPageStart(0, 2, 2)).toBe(0);
    expect(nextPageStart(0, 1, 2)).toBe(0);
    // 关键：page 停在非零值、随后列表缩短到一屏装得下
    expect(nextPageStart(3, 2, 4)).toBe(0);
  });
});
