// src/utils/watchGroups.ts — 分组删除的影响面计算

/** 计算所需的最小分组形状。WatchGroup 结构上满足它。 */
export interface GroupLike {
  id: number;
  name: string;
  watch_ids: number[];
}

export interface GroupRemovalImpact {
  /** 只属于这个分组的股票数 —— 删组后会被并入默认分组 */
  orphans: number;
  /** 还留在其它分组里的股票数 —— 不受影响 */
  kept: number;
  /** 该分组当前的去重自选数 */
  total: number;
}

/**
 * 算删掉某个分组会影响到哪些自选。
 *
 * 这个数字直接写进确认框文案（"其中 N 只仅在此分组，将移入「自选股」"），而用户
 * 对「删分组」的默认预期往往是「数据没了」—— 说错一个数就会让人以为要丢东西。
 * 数字全部来自快照里已有的 `groups[].watch_ids`，不需要额外的预览 IPC。
 *
 * 两处按 `Set` 去重：同组内重复的 watch_id，以及同一只股票在多个分组里重复出现。
 * 分组归属表的 `PRIMARY KEY (group_id, watch_id)` 保证这两者都不会发生，但这段
 * 逻辑要经得起坏数据 —— 靠它得出的数字是要展示给用户做删除决策的。
 */
export function computeGroupRemovalImpact(
  groups: GroupLike[],
  targetGroupId: number,
): GroupRemovalImpact {
  const target = groups.find((g) => g.id === targetGroupId);
  if (!target) return { orphans: 0, kept: 0, total: 0 };

  const membership = new Map<number, number>();
  for (const g of groups) {
    for (const id of new Set(g.watch_ids)) {
      membership.set(id, (membership.get(id) ?? 0) + 1);
    }
  }

  let orphans = 0;
  let kept = 0;
  for (const id of new Set(target.watch_ids)) {
    if ((membership.get(id) ?? 0) <= 1) orphans += 1;
    else kept += 1;
  }
  return { orphans, kept, total: new Set(target.watch_ids).size };
}
