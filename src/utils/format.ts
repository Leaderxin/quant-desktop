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

/**
 * 格式化成交量（输入为股，输出为手/万手/亿手）
 * Stock and index volume are normalized to shares (股) by data source adapters.
 * Display: < 1万手 → "1234手"; ≥ 1万手 → "12.34万手";
 *          ≥ 100万手 → "1234万手"; ≥ 1亿手 → "12.34亿手"
 */
export function formatVolume(volume: number | null | undefined, fallback = '--'): string {
  if (volume == null || isNaN(volume)) return fallback;
  const shou = volume / 100; // 股 → 手
  if (shou >= 10000) {
    const wan = shou / 10000;
    if (wan >= 10000) return (wan / 10000).toFixed(2) + '亿手';
    if (wan >= 100) return wan.toFixed(0) + '万手';
    return wan.toFixed(2) + '万手';
  }
  if (shou > 0) return shou.toFixed(0) + '手';
  return '0手';
}
