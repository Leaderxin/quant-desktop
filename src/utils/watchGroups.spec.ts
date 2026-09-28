// src/utils/watchGroups.spec.ts
//
// 删分组确认框里的那两个数字。它们直接决定用户敢不敢按下删除 —— 文案是
// 「其中 N 只仅在此分组，将移入『自选股』」，说错了会让人以为要丢数据。
import { describe, expect, it } from 'vitest';
import { computeGroupRemovalImpact, type GroupLike } from './watchGroups';

function g(id: number, name: string, watch_ids: number[]): GroupLike {
  return { id, name, watch_ids };
}

describe('computeGroupRemovalImpact', () => {
  it('区分「只在此组（会被并入默认分组）」与「还属于其它分组」', () => {
    const groups = [g(1, '自选股', [1, 2, 3]), g(2, '科技', [3, 4])];
    // 删「科技」：4 只仅在此组 → 孤儿；3 还在自选股里 → 不受影响
    expect(computeGroupRemovalImpact(groups, 2)).toEqual({ orphans: 1, kept: 1, total: 2 });
    // 删「自选股」：1、2 是孤儿；3 还在科技里
    expect(computeGroupRemovalImpact(groups, 1)).toEqual({ orphans: 2, kept: 1, total: 3 });
  });

  it('一只股票属于三个分组时，删掉其中一个算「不受影响」', () => {
    const groups = [g(1, 'A', [7]), g(2, 'B', [7]), g(3, 'C', [7])];
    expect(computeGroupRemovalImpact(groups, 2)).toEqual({ orphans: 0, kept: 1, total: 1 });
  });

  it('空分组', () => {
    expect(computeGroupRemovalImpact([g(1, 'A', []), g(2, 'B', [])], 1))
      .toEqual({ orphans: 0, kept: 0, total: 0 });
  });

  it('目标分组不存在时返回全零，而不是抛错', () => {
    // 菜单打开期间分组被删是可能的，这时确认框会立刻关掉，但计算不能炸
    expect(computeGroupRemovalImpact([g(1, 'A', [1])], 99))
      .toEqual({ orphans: 0, kept: 0, total: 0 });
    expect(computeGroupRemovalImpact([], 1)).toEqual({ orphans: 0, kept: 0, total: 0 });
  });

  it('对重复 id 去重，不把数字算大', () => {
    // 归属表的 PRIMARY KEY (group_id, watch_id) 保证不会出现重复，
    // 但这个数字是给用户做删除决策用的，得经得起坏数据
    const groups = [g(1, 'A', [5, 5, 5, 6]), g(2, 'B', [5])];
    expect(computeGroupRemovalImpact(groups, 1)).toEqual({ orphans: 1, kept: 1, total: 2 });
  });

  it('总数与「孤儿 + 保留」一致', () => {
    const groups = [g(1, 'A', [1, 2]), g(2, 'B', [2, 3]), g(3, 'C', [3, 4])];
    for (const target of [1, 2, 3]) {
      const r = computeGroupRemovalImpact(groups, target);
      expect(r.orphans + r.kept).toBe(r.total);
    }
  });
});
