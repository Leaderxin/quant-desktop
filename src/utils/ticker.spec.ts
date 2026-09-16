import { describe, expect, it } from 'vitest';
import { tickerGroups, tickerPage, tickerRows } from './ticker';

describe('固定关注与轮播', () => {
  const items = Array.from({ length: 5 }, (_, id) => ({ id, ticker_enabled: true, ticker_pinned: id < 2 }));
  it('三行固定两只，剩下一行遍历其余三只，不重复固定股票', () => {
    const { pinned, rotating, slots } = tickerGroups(items, 3);
    expect(pinned.map(i => i.id)).toEqual([0, 1]);
    expect(slots).toBe(1);
    expect([0, 1, 2, 3].map(start => tickerPage(rotating, start, slots)[0].id)).toEqual([2, 3, 4, 2]);
  });
  it('关闭播报不丢失固定偏好，重新开启仍为固定项', () => {
    const disabled = items.map(i => ({ ...i, ticker_enabled: i.id !== 0 }));
    expect(tickerGroups(disabled, 3).pinned.map(i => i.id)).toEqual([1]);
    expect(disabled[0].ticker_pinned).toBe(true);
    disabled[0].ticker_enabled = true;
    expect(tickerGroups(disabled, 3).pinned.map(i => i.id)).toEqual([0, 1]);
  });
  it('取消固定恢复轮播；没有候选不重复播放固定项', () => {
    expect(tickerGroups(items.map(i => ({ ...i, ticker_pinned: false })), 3).rotating).toHaveLength(5);
    const group = tickerGroups(items.slice(0, 2), 3);
    expect(group.rotating).toEqual([]);
    expect(group.slots).toBe(1);
  });
  it('小屏临时空间不足时保留一行轮播，且不丢失任何股票', () => {
    const group = tickerGroups(items, 2);
    expect(group.pinned).toHaveLength(1);
    expect(group.slots).toBe(1);
    expect([...group.pinned, ...group.rotating]).toHaveLength(5);
    expect(items[1].ticker_pinned).toBe(true);
  });
});

describe('悬浮窗行数及翻页', () => {
  it('保持原有 38px 两行高度，每 15px 增加一行', () => {
    expect(tickerRows(0)).toBe(2);
    expect(tickerRows(38)).toBe(2);
    expect(tickerRows(53)).toBe(3);
    expect(tickerRows(68)).toBe(4);
  });
  it('不足一屏不重复，最后一屏循环补齐', () => {
    expect(tickerPage([], 0, 3)).toEqual([]);
    expect(tickerPage(['a'], 0, 3)).toEqual(['a']);
    expect(tickerPage(['a', 'b', 'c', 'd', 'e'], 3, 3)).toEqual(['d', 'e', 'a']);
    expect(tickerPage(['a', 'b', 'c'], 0, 4)).toEqual(['a', 'b', 'c']);
  });
});
