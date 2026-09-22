/** '#f85149' → 'rgba(248,81,73,0.5)'。
 *
 * K 线图等画在 canvas 上，var()/CSS 令牌在 canvas 里解析不了；量柱这类
 * 低透明度派生色只能从令牌的 hex 字面值现场换算。输入必须是 #rrggbb。 */
export function hexToRgba(hex: string, alpha: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
}
