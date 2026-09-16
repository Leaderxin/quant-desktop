export const TICKER_ROW_HEIGHT = 15;
export function tickerRows(height: number): number {
  return Math.max(2, Math.min(30, Math.round((height - 8) / TICKER_ROW_HEIGHT)));
}

export function tickerPage<T>(items: T[], start: number, rows: number): T[] {
  return Array.from({ length: Math.min(rows, items.length) }, (_, i) => items[(start + i) % items.length]);
}

export function tickerGroups<T extends { ticker_enabled: boolean; ticker_pinned: boolean }>(items: T[], rows: number) {
  const enabled = items.filter((item) => item.ticker_enabled);
  const preferred = enabled.filter((item) => item.ticker_pinned);
  // A smaller monitor may temporarily lack room for all saved pins. Retain the
  // preference, but keep overflow reachable through the reserved rotation slot.
  const pinned = preferred.slice(0, Math.max(0, rows - 1));
  const fixed = new Set(pinned);
  return { pinned, rotating: enabled.filter((item) => !fixed.has(item)), slots: Math.max(1, rows - pinned.length) };
}
