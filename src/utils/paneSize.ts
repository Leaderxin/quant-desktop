// src/utils/paneSize.ts

/**
 * 图表副图窗格（pane）的目标高度。
 *
 * klinecharts 的 pane 高度是**固定像素**，主图吃掉剩余空间。默认 100px 在小视口下会占到
 * 图区三分之一以上，喧宾夺主；这里按图区高度的 28% 计算并夹在 48–80px：大窗口不会无限
 * 长高，小窗口也保得住主图的面积。
 *
 * K 线的指标副图与分时图的成交量副图共用这一份口径 —— 两张图在详情面板里占同一块
 * 区域，切换周期时副图高度不该跳。
 */
const SUB_PANE_RATIO = 0.28;
const SUB_PANE_MIN = 48;
const SUB_PANE_MAX = 80;
/** 图区还没量到高度时的兜底值（约等于中等窗口下的一档） */
const SUB_PANE_FALLBACK = 72;

export function subPaneHeight(totalHeight: number): number {
  if (!(totalHeight > 0)) return SUB_PANE_FALLBACK;
  return Math.round(Math.min(SUB_PANE_MAX, Math.max(SUB_PANE_MIN, totalHeight * SUB_PANE_RATIO)));
}
