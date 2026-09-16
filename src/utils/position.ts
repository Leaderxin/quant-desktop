export function positionProfit(price: number | null | undefined, cost: number | null, quantity: number | null): number | null {
  if (price == null || cost == null || quantity == null || !Number.isFinite(price)
    || !Number.isFinite(cost) || !Number.isFinite(quantity) || price <= 0 || cost < 0 || quantity < 0) return null;
  const amount = (price - cost) * quantity;
  return Number.isFinite(amount) ? (Math.round((amount + Math.sign(amount) * Number.EPSILON) * 100) / 100 || 0) : null;
}

export function formatProfit(amount: number | null, compact = false): string {
  if (amount === null || !Number.isFinite(amount)) return '--';
  if (amount === 0) return '0.00';
  const sign = amount > 0 ? '+' : '';
  if (!compact) return `${sign}${amount.toLocaleString('en-US', {
    useGrouping: false, minimumFractionDigits: 2, maximumFractionDigits: 2,
  })}`;
  const magnitude = Math.abs(amount);
  const units = ['', '万', '亿', '万亿'];
  let index = 0;
  // 升级单位时考虑两位小数的进位，避免出现“10000.00万”。
  while (Number((magnitude / 1e4 ** index).toFixed(2)) >= 1e4) {
    if (index === units.length - 1) return `${sign}${amount.toExponential(2)}`;
    index++;
  }
  return `${sign}${(amount / 1e4 ** index).toFixed(2)}${units[index]}`;
}
