// src/utils/watchGroups.spec.ts
//
// 删分组确认框里的两个名单：会被挪到兜底分组的，和留在别的分组里不受影响的。
// 它们直接决定用户敢不敢按下删除 —— 名单是「删了会丢东西吗」的唯一依据，
// 分错一只就会让人误判。
import { describe, expect, it } from 'vitest';
import { computeGroupRemovalImpact, type GroupLike } from './watchGroups';

function g(id: number, name: string, watch_ids: number[]): GroupLike {
  return { id, name, watch_ids };
}

describe('computeGroupRemovalImpact', () => {
  it('区分「只在此组（会被并入兜底分组）」与「还属于其它分组」', () => {
    const groups = [g(1, '自选股', [1, 2, 3]), g(2, '科技', [3, 4])];
    // 删「科技」：4 只仅在此组 → 孤儿；3 还在自选股里 → 不受影响
    expect(computeGroupRemovalImpact(groups, 2)).toEqual({
      orphanIds: [4],
      keptIds: [3],
    });
    // 删「自选股」：1、2 是孤儿；3 还在科技里
    expect(computeGroupRemovalImpact(groups, 1)).toEqual({
      orphanIds: [1, 2],
      keptIds: [3],
    });
  });

  it('一只股票属于三个分组时，删掉其中一个算「不受影响」', () => {
    const groups = [g(1, 'A', [7]), g(2, 'B', [7]), g(3, 'C', [7])];
    expect(computeGroupRemovalImpact(groups, 2)).toEqual({
      orphanIds: [],
      keptIds: [7],
    });
  });

  it('空分组', () => {
    expect(computeGroupRemovalImpact([g(1, 'A', []), g(2, 'B', [])], 1))
      .toEqual({ orphanIds: [], keptIds: [] });
  });

  it('目标分组不存在时返回空名单，而不是抛错', () => {
    // 菜单打开期间分组被删是可能的，这时确认框会立刻关掉，但计算不能炸
    expect(computeGroupRemovalImpact([g(1, 'A', [1])], 99))
      .toEqual({ orphanIds: [], keptIds: [] });
    expect(computeGroupRemovalImpact([], 1)).toEqual({ orphanIds: [], keptIds: [] });
  });

  it('对重复 id 去重，不把同一只股票列两次', () => {
    // 归属表的 PRIMARY KEY (group_id, watch_id) 保证不会出现重复，
    // 但这份名单是给用户做删除决策用的，得经得起坏数据
    expect(computeGroupRemovalImpact([g(1, 'A', [5, 5, 5, 6]), g(2, 'B', [5])], 1))
      .toEqual({ orphanIds: [6], keptIds: [5] });
  });

  it('名单按组内顺序排列，且每只股票只出现在一边', () => {
    const groups = [g(1, 'A', [1, 5, 2]), g(2, 'B', [2, 3]), g(3, 'C', [3, 4])];
    // 孤儿与保留各自保持组内相对顺序
    expect(computeGroupRemovalImpact(groups, 1)).toEqual({
      orphanIds: [1, 5],
      keptIds: [2],
    });

    for (const target of [1, 2, 3]) {
      const r = computeGroupRemovalImpact(groups, target);
      const all = [...r.orphanIds, ...r.keptIds];
      expect(new Set(all).size).toBe(all.length);
      expect(all.length).toBe(
        new Set(groups.find((x) => x.id === target)!.watch_ids).size,
      );
    }
  });
});
