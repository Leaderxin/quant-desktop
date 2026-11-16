import { describe, expect, it } from 'vitest';
import { subPaneHeight } from './paneSize';

describe('subPaneHeight', () => {
  it('按图区高度的 28% 取，夹在 48–80px', () => {
    expect(subPaneHeight(200)).toBe(56);
    expect(subPaneHeight(1000)).toBe(80);
    expect(subPaneHeight(100)).toBe(48);
  });

  it('图区高度未知时给兜底值，不产生 NaN', () => {
    expect(subPaneHeight(0)).toBe(72);
    expect(subPaneHeight(NaN)).toBe(72);
  });
});
