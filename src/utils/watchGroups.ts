// src/utils/watchGroups.ts — 分组删除的影响面计算

/** 计算所需的最小分组形状。WatchGroup 结构上满足它。 */
export interface GroupLike {
  id: number;
  name: string;
  watch_ids: number[];
}

export interface GroupRemovalImpact {
  /**
   * 只属于这个分组的自选 id，按组内顺序 —— 删组后会被并入兜底分组。
   * 给出 id 而不只是数量：确认框要把「哪几只」列出来，一个光秃秃的数字
   * 对「删了会不会丢东西」这个判断帮不上忙。
   */
  orphanIds: number[];
  /** 还留在其它分组里的自选 id，按组内顺序 —— 不受影响 */
  keptIds: number[];
}

/**
 * 算删掉某个分组会影响到哪些自选。
 *
 * 结果直接决定确认框里列出的名单，而用户对「删分组」的默认预期往往是
 * 「数据没了」—— 分错一只就会让人以为要丢东西。名单全部来自快照里已有的
 * `groups[].watch_ids`，不需要额外的预览 IPC。
 *
 * 两处按 `Set` 去重：同组内重复的 watch_id，以及同一只股票在多个分组里重复出现。
 * 分组归属表的 `PRIMARY KEY (group_id, watch_id)` 保证这两者都不会发生，但这段
 * 逻辑要经得起坏数据 —— 靠它得出的名单是要展示给用户做删除决策的。
 */
export function computeGroupRemovalImpact(
  groups: GroupLike[],
  targetGroupId: number,
): GroupRemovalImpact {
  const target = groups.find((g) => g.id === targetGroupId);
  if (!target) return { orphanIds: [], keptIds: [] };

  const membership = new Map<number, number>();
  for (const g of groups) {
    for (const id of new Set(g.watch_ids)) {
      membership.set(id, (membership.get(id) ?? 0) + 1);
    }
  }

  const orphanIds: number[] = [];
  const keptIds: number[] = [];
  const seen = new Set<number>();
  for (const id of target.watch_ids) {
    if (seen.has(id)) continue;
    seen.add(id);
    if ((membership.get(id) ?? 0) <= 1) orphanIds.push(id);
    else keptIds.push(id);
  }
  return { orphanIds, keptIds };
}
