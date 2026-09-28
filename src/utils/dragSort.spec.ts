// src/utils/dragSort.spec.ts
//
// 拖拽落点的下标换算。这是列表排序里唯一有真实逻辑的一步，也是最容易差一位的
// 一步 —— 在界面上表现为「每次拖动都偏一格」，很容易被当成手感问题而不是 bug。
// 下面把四种落点方向 × 源项在目标前后都钉住。
import { describe, expect, it } from 'vitest';
import { computeDropTarget, moveByStep, moveItem } from './dragSort';

/** 按 computeDropTarget 的结果真的搬一次，便于断言"用户看到的结果"。 */
function applyDrag<T>(items: T[], from: number, over: number, after: boolean): T[] {
  return moveItem(items, from, computeDropTarget(from, over, after));
}

const ABC = ['A', 'B', 'C'];

describe('computeDropTarget', () => {
  it('落在悬停行上半部分 → 插到它前面', () => {
    // C 拖到 A 的上半部分：C 排到最前
    expect(applyDrag(ABC, 2, 0, false)).toEqual(['C', 'A', 'B']);
    // A 拖到 B 的上半部分：位置不变（它本来就在 B 前面）
    expect(applyDrag(ABC, 0, 1, false)).toEqual(['A', 'B', 'C']);
  });

  it('落在悬停行下半部分 → 插到它后面', () => {
    // A 拖到 B 的下半部分 → 排到 B 后面
    expect(applyDrag(ABC, 0, 1, true)).toEqual(['B', 'A', 'C']);
    // C 拖到 A 的下半部分 → 排在 A 后面、B 前面
    expect(applyDrag(ABC, 2, 0, true)).toEqual(['A', 'C', 'B']);
  });

  it('这是唯一能表达「移到列表末尾」的路径', () => {
    // 只有"落在最后一项的下半部分"这一种落点能表达"放最后"，
    // 若没有 after 分支，A 永远拖不到 C 后面
    expect(applyDrag(ABC, 0, 2, true)).toEqual(['B', 'C', 'A']);
  });

  it('源项在目标之前时，移除源项会让目标前移一位', () => {
    // from=0 → over=2 且 after=true：先 +1 得 3，再因 from<to 减 1 得 2。
    // 漏掉这次修正就会得到 3（越界）或把 A 插到错误位置
    expect(computeDropTarget(0, 2, true)).toBe(2);
    expect(computeDropTarget(0, 1, true)).toBe(1);
    // 源项在目标之后时不做修正，否则会反向偏移
    expect(computeDropTarget(2, 0, false)).toBe(0);
    expect(computeDropTarget(2, 0, true)).toBe(1);
  });

  it('相邻换位两个方向都自洽', () => {
    expect(applyDrag(['A', 'B'], 0, 1, true)).toEqual(['B', 'A']);
    expect(applyDrag(['A', 'B'], 1, 0, false)).toEqual(['B', 'A']);
  });
});

describe('moveItem', () => {
  it('按 from → to 重排', () => {
    expect(moveItem(ABC, 0, 2)).toEqual(['B', 'C', 'A']);
    expect(moveItem(ABC, 2, 0)).toEqual(['C', 'A', 'B']);
  });

  it('不改原数组（调用方常在 computed 的值上操作）', () => {
    const original = [...ABC];
    moveItem(original, 0, 2);
    expect(original).toEqual(ABC);
  });
});

describe('moveByStep', () => {
  it('单步上移/下移', () => {
    expect(moveByStep(ABC, 1, -1)).toEqual(['B', 'A', 'C']);
    expect(moveByStep(ABC, 1, 1)).toEqual(['A', 'C', 'B']);
  });

  it('越界返回 null —— 调用方据此跳过 IPC', () => {
    expect(moveByStep(ABC, 0, -1)).toBeNull();
    expect(moveByStep(ABC, 2, 1)).toBeNull();
    expect(moveByStep([], 0, 1)).toBeNull();
  });

  it('不改原数组', () => {
    const original = [...ABC];
    moveByStep(original, 1, 1);
    expect(original).toEqual(ABC);
  });
});
