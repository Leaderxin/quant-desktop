import { describe, expect, it } from 'vitest';
import { hexToRgba } from './color';

// K 线图量柱的涨跌色是 CSS 令牌 hex 的低透明度派生（canvas 画不了
// var()，只能现场算字面值）。派生值直接决定量柱观感，而图表视觉是
// eyeball-only 测不到的 —— 这里用 variables.css 的真实令牌值把
// 「令牌 → 量柱色」的换算钉死，改令牌或改透明度时这里有测试兜着。
describe('hexToRgba', () => {
  it('四种涨跌令牌（主题 × 配色组合）× 常用量柱透明度', () => {
    expect(hexToRgba('#f85149', 0.5)).toBe('rgba(248,81,73,0.5)');   // dark cn 涨
    expect(hexToRgba('#3fb950', 0.55)).toBe('rgba(63,185,80,0.55)'); // dark us 涨
    expect(hexToRgba('#d1242f', 0.7)).toBe('rgba(209,36,47,0.7)');    // light cn 涨
    expect(hexToRgba('#1a7f64', 0.72)).toBe('rgba(26,127,100,0.72)'); // light us 涨
  });

  it('平盘灰与副图指标透明度', () => {
    expect(hexToRgba('#8b949e', 0.6)).toBe('rgba(139,148,158,0.6)');
    expect(hexToRgba('#8b949e', 0.45)).toBe('rgba(139,148,158,0.45)');
  });
});
