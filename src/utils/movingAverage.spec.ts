import { describe, expect, it, vi } from 'vitest';
import type { Chart } from 'klinecharts';
import {
  DEFAULT_MOVING_AVERAGE_PERIODS,
  MAX_MOVING_AVERAGE_COUNT,
  MAX_MOVING_AVERAGE_PERIOD,
  MIN_MOVING_AVERAGE_PERIOD,
  nextMovingAveragePeriod,
  parseMovingAveragePeriods,
  normalizeMovingAveragePeriods,
  syncMovingAverage,
} from './movingAverage';

describe('moving average settings', () => {
  it('uses the existing four-line configuration for missing or invalid values', () => {
    expect(parseMovingAveragePeriods(undefined)).toEqual([5, 10, 20, 60]);
    expect(parseMovingAveragePeriods('{')).toEqual([5, 10, 20, 60]);
    expect(parseMovingAveragePeriods('{"periods":"5"}')).toEqual([5, 10, 20, 60]);
    expect(parseMovingAveragePeriods('{"periods":[5]}')).toEqual([5]);
    expect(parseMovingAveragePeriods('[7,14]')).toEqual([7, 14]);
  });

  it('keeps valid positive integer periods in order and limits the count to ten', () => {
    expect(normalizeMovingAveragePeriods([20, 5, 10.7, 0, -3, '30', 60])).toEqual([20, 5, 60]);
    expect(normalizeMovingAveragePeriods(Array.from({ length: 12 }, (_, i) => i + 1)))
      .toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(normalizeMovingAveragePeriods([MAX_MOVING_AVERAGE_PERIOD + 1]))
      .toEqual([MAX_MOVING_AVERAGE_PERIOD]);
    expect(normalizeMovingAveragePeriods([])).toEqual(DEFAULT_MOVING_AVERAGE_PERIODS);
    expect(normalizeMovingAveragePeriods([MIN_MOVING_AVERAGE_PERIOD - 1]))
      .toEqual(DEFAULT_MOVING_AVERAGE_PERIODS);
    expect(MAX_MOVING_AVERAGE_COUNT).toBe(10);
  });

  it('chooses the next unused preset, then finds an unused period', () => {
    expect(nextMovingAveragePeriod([5, 10, 20, 60])).toBe(120);
    expect(nextMovingAveragePeriod([5, 10, 20, 60, 120, 250, 500, 750, 1000, 1500]))
      .toBe(1501);
  });
});

describe('syncMovingAverage', () => {
  it('creates an MA overlay with the configured periods and line colors', () => {
    const chart = {
      getIndicators: vi.fn(() => []),
      createIndicator: vi.fn(),
      overrideIndicator: vi.fn(),
    } as unknown as Chart;

    syncMovingAverage(chart, [5, 12, 26], 'candle_pane', ['#111', '#222', '#333']);

    expect(chart.createIndicator).toHaveBeenCalledWith({
      name: 'MA',
      calcParams: [5, 12, 26],
      paneId: 'candle_pane',
      styles: {
        lines: [
          { style: 'solid', smooth: false, size: 1, color: '#111' },
          { style: 'solid', smooth: false, size: 1, color: '#222' },
          { style: 'solid', smooth: false, size: 1, color: '#333' },
        ],
      },
    }, true);
  });

  it('overrides the existing indicator when periods change', () => {
    const chart = {
      getIndicators: vi.fn(() => [{ name: 'MA' }]),
      createIndicator: vi.fn(),
      overrideIndicator: vi.fn(),
    } as unknown as Chart;

    syncMovingAverage(chart, [7, 14], 'candle_pane', ['#111', '#222']);

    expect(chart.overrideIndicator).toHaveBeenCalledWith(expect.objectContaining({
      name: 'MA',
      calcParams: [7, 14],
      paneId: 'candle_pane',
    }));
    expect(chart.createIndicator).not.toHaveBeenCalled();
  });
});
