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
 * 去掉代码的交易所前缀用于展示（sz000852 → 000852）。
 * 无前缀的代码（HK/US 等）原样返回。
 */
export function formatCode(code: string): string {
  if (code.startsWith('sh') || code.startsWith('sz') || code.startsWith('bj')) {
    return code.slice(2);
  }
  return code;
}

/**
 * 根据完整代码和证券类别生成市场标签（「沪A」「深A」「沪指」「深指」「ETF」等）。
 * category 取值：GP-A / GP-B / ETF / LOF / ZS（指数）。
 */
export function marketTag(code: string, category: string): string {
  const isSh = code.startsWith('sh');
  const isSz = code.startsWith('sz');
  const isBj = code.startsWith('bj');
  switch (category) {
    case 'ZS': return isSh ? '沪指' : isSz ? '深指' : isBj ? '北指' : '指数';
    case 'ETF': return 'ETF';
    case 'LOF': return 'LOF';
    case 'GP-B': return isSh ? '沪B' : isSz ? '深B' : 'B股';
    case 'GP-A': return isSh ? '沪A' : isSz ? '深A' : isBj ? '北A' : 'A股';
    default: return '';
  }
}

/**
 * 从完整代码推导证券类别（与 Rust 端 `cn_category` 保持一致）：
 * sh 0xxxxx → 指数；sh 5xxxxx → ETF；sh 6xxxxx → A股；sh 9xxxxx → B股
 * sz 399xxx → 指数；sz 159xxx → ETF；sz 16xxxx → LOF；sz 2xxxxx → B股；其余 → A股
 * bj 899xxx → 指数；其余 bj → 北交所 A股
 */
export function cnCategory(code: string): string {
  if (code.startsWith('sh')) {
    const n = code.slice(2);
    if (n.startsWith('0')) return 'ZS';
    if (n.startsWith('5')) return 'ETF';
    if (n.startsWith('9')) return 'GP-B';
    return 'GP-A';
  }
  if (code.startsWith('sz')) {
    const n = code.slice(2);
    if (n.startsWith('39')) return 'ZS';
    if (n.startsWith('159')) return 'ETF';
    if (n.startsWith('16')) return 'LOF';
    if (n.startsWith('2')) return 'GP-B';
    return 'GP-A';
  }
  if (code.startsWith('bj')) {
    const n = code.slice(2);
    if (n.startsWith('899')) return 'ZS';
    return 'GP-A';
  }
  return '';
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
