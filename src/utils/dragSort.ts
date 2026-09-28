// src/utils/dragSort.ts — 列表排序的下标运算
//
// 这几个函数原本内联在 `DragSortList.vue` 和三个设置分区的拖拽/上移/下移处理里。
// 抽出来的唯一理由是**可测**：它们是列表排序里唯一有真实逻辑的部分，而"差一位"
// 这类错误在界面上表现为「每次拖动都偏一格」，很容易被当成手感问题而不是 bug。
// 组件里的内联算式没有测试能抵达。

/**
 * 计算拖拽落点对应的目标下标。
 *
 * 两处坐标转换：
 * 1. `after` 为真表示指针落在悬停行的**下半部分** → 插到它后面（下标 +1）。
 *    没有这一支就只能往上拖，「移到列表末尾」永远表达不出来。
 * 2. 返回的是「把源项移除之后」的坐标系。源项原本在目标之前时，移除会让目标前移
 *    一位，不减掉这一位就是经典的 off-by-one。
 *
 * @param from  拖起的行下标
 * @param over  指针悬停的行下标（调用方已排除 from === over 的情况）
 * @param after 落点是否在悬停行的下半部分
 */
export function computeDropTarget(from: number, over: number, after: boolean): number {
  let to = over + (after ? 1 : 0);
  if (from < to) to -= 1;
  return to;
}

/** 把 `from` 处元素移到 `to` 处，返回新数组（不改原数组）。 */
export function moveItem<T>(items: T[], from: number, to: number): T[] {
  const next = items.slice();
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

/**
 * 单步移动（↑/↓ 按钮与 Alt+↑/↓ 走的路径）。
 *
 * 越界返回 `null` 而不是原数组：调用方据此**跳过** IPC。发一次注定无变化的写请求
 * 除了浪费往返，还会让后端重排一遍 sort_order，把本来稳定的组内序号搅动一次。
 */
export function moveByStep<T>(items: T[], index: number, direction: -1 | 1): T[] | null {
  const target = index + direction;
  if (target < 0 || target >= items.length) return null;
  const next = items.slice();
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}
