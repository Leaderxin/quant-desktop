import type { Chart } from 'klinecharts';

export const DEFAULT_MOVING_AVERAGE_PERIODS = [5, 10, 20, 60] as const;
export const MIN_MOVING_AVERAGE_PERIOD = 1;
export const MAX_MOVING_AVERAGE_PERIOD = 9999;
export const MAX_MOVING_AVERAGE_COUNT = 10;

const PERIOD_PRESETS = [120, 250, 500, 750, 1000, 1500];

export function normalizeMovingAveragePeriods(value: unknown): number[] {
  if (!Array.isArray(value)) return [...DEFAULT_MOVING_AVERAGE_PERIODS];

  const periods = value
    .filter((period): period is number => typeof period === 'number' && Number.isInteger(period))
    .filter((period) => period >= MIN_MOVING_AVERAGE_PERIOD)
    .map((period) => Math.min(period, MAX_MOVING_AVERAGE_PERIOD))
    .slice(0, MAX_MOVING_AVERAGE_COUNT);

  return periods.length > 0 ? periods : [...DEFAULT_MOVING_AVERAGE_PERIODS];
}

export function parseMovingAveragePeriods(raw: string | undefined): number[] {
  if (!raw) return [...DEFAULT_MOVING_AVERAGE_PERIODS];

  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) return normalizeMovingAveragePeriods(parsed);
    if (!parsed || typeof parsed !== 'object' || !('periods' in parsed)) {
      return [...DEFAULT_MOVING_AVERAGE_PERIODS];
    }
    return normalizeMovingAveragePeriods(parsed.periods);
  } catch {
    return [...DEFAULT_MOVING_AVERAGE_PERIODS];
  }
}

export function nextMovingAveragePeriod(periods: readonly number[]): number {
  const used = new Set(periods);
  const preset = PERIOD_PRESETS.find((period) => !used.has(period));
  if (preset !== undefined) return preset;

  const maxPeriod = Math.max(0, ...periods);
  if (maxPeriod < MAX_MOVING_AVERAGE_PERIOD) return maxPeriod + 1;

  for (let period = MIN_MOVING_AVERAGE_PERIOD; period <= MAX_MOVING_AVERAGE_PERIOD; period += 1) {
    if (!used.has(period)) return period;
  }

  return MAX_MOVING_AVERAGE_PERIOD;
}

export function syncMovingAverage(
  chart: Chart,
  periods: readonly number[],
  paneId: string,
  colors: readonly string[],
): void {
  const calcParams = normalizeMovingAveragePeriods(periods);
  const lines = calcParams.map((_, index) => ({
    style: 'solid' as const,
    smooth: false,
    size: 1,
    color: colors[index % colors.length] ?? '#8b949e',
  }));
  const indicator = {
    name: 'MA',
    calcParams,
    paneId,
    styles: { lines },
  };

  if (chart.getIndicators({ paneId, name: 'MA' }).length > 0) {
    chart.overrideIndicator(indicator);
  } else {
    chart.createIndicator(indicator, true);
  }
}
