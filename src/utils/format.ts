// src/utils/format.ts

/**
 * 根据价格的实际小数位数返回合适的显示精度。
 * 规则：如果 price 的小数点后第3位有值（price * 100 的余数 > 0.001），
 *       则用 3 位小数；否则用 2 位。
 * 覆盖：普通股票（2位）、ETF < 1（3位）、可转债 ≥ 1 但精度为 3 位
 */
export function getPricePrecision(price: number): number {
  if (price == null || isNaN(price) || price === 0) return 2;
  const absPrice = Math.abs(price);
  const remainder = Math.abs(absPrice * 100 - Math.round(absPrice * 100));
  return remainder > 0.001 ? 3 : 2;
}

/**
 * 格式化价格字符串
 */
export function formatPrice(price: number | null | undefined, fallback = '--'): string {
  if (price == null || isNaN(price)) return fallback;
  return price.toFixed(getPricePrecision(price));
}
