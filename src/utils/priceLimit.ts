// src/utils/priceLimit.ts — A 股涨跌停幅度
//
// 只对「规则确定」的个股给出比例。基金、指数、可转债、逆回购一律返回 null，
// 让调用方回退到价格轴 —— 创业板/科创板 ETF 是 20%、跨境与债券 ETF 又各有
// 规则，仅凭 6 位代码无法可靠区分；错画一条涨跌停线比不画更糟。

/** 去掉交易所前缀与指数代码的 `s_` 前缀，统一成 6 位数字。 */
function normalizeCode(code: string): { market: string; digits: string } {
  const raw = code.trim().toLowerCase().replace(/^s_/, '');
  const market = raw.slice(0, 2);
  if (market === 'sh' || market === 'sz' || market === 'bj') {
    return { market, digits: raw.slice(2) };
  }
  return { market: '', digits: raw };
}

/** 名称是否带 ST/*ST 前缀（退市风险警示）。 */
export function isStName(name: string): boolean {
  return /^\*?ST/i.test(name.trim());
}

/**
 * 名称是否属于「无固定涨跌停」的新股阶段。
 *
 * 注册制新股上市前 5 个交易日不设涨跌幅，数据源用 `N`（首日）/`C`（次日起）
 * 前缀标识。这类标的没有可画的涨跌停边界，返回 null 走价格轴。
 */
function isNewListingName(name: string): boolean {
  return /^[NC]/.test(name.trim());
}

/**
 * 涨跌停比例（0.05 / 0.10 / 0.20 / 0.30）。
 *
 * `null` 表示「没有可靠的固定涨跌停」，调用方应回退价格轴：
 * - 指数、ETF/LOF、可转债、逆回购
 * - 无交易所前缀或无法识别的代码
 * - 名称处于 N/C 新股阶段
 *
 * ST 只影响沪深主板（5%）；创业板/科创板/北交所的 ST 仍按各自板块比例。
 */
export function priceLimitRate(code: string, name = ''): number | null {
  const { market, digits } = normalizeCode(code);
  if (!/^\d{6}$/.test(digits)) return null;
  if (isNewListingName(name)) return null;

  if (market === 'bj') {
    // 899xxx 是北证指数，没有涨跌停
    return digits.startsWith('899') ? null : 0.30;
  }

  if (market === 'sh') {
    if (digits.startsWith('000')) return null;          // 上证指数
    if (digits.startsWith('688') || digits.startsWith('689')) return 0.20; // 科创板
    if (digits.startsWith('5')) return null;            // ETF / LOF
    if (digits.startsWith('11') || digits.startsWith('12')) return null;   // 可转债
    if (digits.startsWith('204')) return null;          // 国债逆回购
    if (digits.startsWith('9')) return 0.10;            // B 股
    // 600/601/603/605 主板
    return isStName(name) ? 0.05 : 0.10;
  }

  if (market === 'sz') {
    if (digits.startsWith('39')) return null;           // 深证指数
    if (digits.startsWith('30')) return 0.20;           // 创业板（300/301/302）
    if (digits.startsWith('15') || digits.startsWith('16') || digits.startsWith('18')) {
      return null;                                      // ETF / LOF
    }
    if (digits.startsWith('12')) return null;           // 可转债
    if (digits.startsWith('131')) return null;          // 国债逆回购
    if (digits.startsWith('2')) return 0.10;            // B 股
    // 000/001/002/003 主板
    return isStName(name) ? 0.05 : 0.10;
  }

  return null;
}

/** 四舍五入到分。浮点误差（10.05 × 1.1 = 11.055000000000001）会算错边界。 */
function roundToCent(value: number): number {
  return Math.round((value + 1e-9) * 100) / 100;
}

/**
 * 实际涨停价/跌停价。交易所按昨收 × 比例四舍五入到分，所以两条边界到昨收的
 * 距离可能差 0.01；调用方取两者较大值当半幅，0% 仍严格居中。
 */
export function limitPrices(prevClose: number, rate: number): { up: number; down: number } {
  return {
    up: roundToCent(prevClose * (1 + rate)),
    down: roundToCent(prevClose * (1 - rate)),
  };
}
