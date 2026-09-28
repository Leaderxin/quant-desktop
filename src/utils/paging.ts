// src/utils/paging.ts — 行情条轮播的分屏计算

/**
 * 取一屏要展示的项。
 *
 * `start` 是**起始下标**而不是页号：行情条每 3 秒把下标往前推 `perPage` 个，
 * 用下标表达就不需要「页号 × 每屏条数」这一次换算，列表长度变化时也不用修正页号。
 *
 * 下标按模回绕，所以 `start` 超出长度（例如每屏条数刚从 2 调到 4）不会取到空数组；
 * `count` 超过列表长度时按列表长度截断，短列表不会重复展示同一只股票。
 */
export function windowItems<T>(items: T[], start: number, count: number): T[] {
  const len = items.length;
  if (len === 0) return [];
  const size = Math.min(Math.max(count, 1), len);
  const begin = ((start % len) + len) % len;
  const result: T[] = [];
  for (let i = 0; i < size; i++) {
    result.push(items[(begin + i) % len]);
  }
  return result;
}

/**
 * 下一屏的起始下标。
 *
 * 列表短到一屏装得下时恒返回 0 —— 否则会每隔 3 秒把同一批数据重排一次，
 * 视觉上是无意义的抖动。
 */
export function nextPageStart(start: number, length: number, perPage: number): number {
  if (length <= perPage) return 0;
  return (start + perPage) % length;
}
