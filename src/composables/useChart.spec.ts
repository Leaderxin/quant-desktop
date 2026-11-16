// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createApp, ref } from 'vue';
import { createPinia, setActivePinia } from 'pinia';

/**
 * K 线切股时的旧请求丢弃。
 *
 * 图表实例与 `allData` 跨标的复用；上一只股票的 forward 请求回来时若还往
 * 图表里 prepend，就会让新标的的图上多出一段旧历史（新标的 2010 年上市，
 * 图上却从 2000 年开始）。这里用假图表 + 挂起的 forward 请求复现交错。
 */
const mocks = vi.hoisted(() => {
  const state = {
    loader: null as {
      getBars: (params: any) => Promise<void>;
    } | null,
  };
  const chart = new Proxy(
    {
      setDataLoader: vi.fn((loader: typeof state.loader) => {
        state.loader = loader;
      }),
      getIndicators: vi.fn(() => []),
    },
    {
      get(target, prop) {
        if (!(prop in target)) {
          (target as Record<string, unknown>)[prop as string] = vi.fn();
        }
        return (target as Record<string, unknown>)[prop as string];
      },
    },
  );
  return { state, chart };
});

vi.mock('klinecharts', () => ({
  init: vi.fn(() => mocks.chart),
  dispose: vi.fn(),
}));

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { useChart } from './useChart';
import type { MainOverlayType, SubIndicatorType } from '@/types';

function bar(date: string, close: number) {
  return { date, open: close, high: close, low: close, close, volume: 100 };
}

const A_MAIN = [bar('2020-01-02', 10), bar('2026-01-02', 11)];
const A_HISTORY = [bar('2000-01-04', 5)];
const B_MAIN = [bar('2010-01-04', 20), bar('2026-01-02', 21)];

let teardown: (() => void) | null = null;

function mountChart() {
  const code = ref('sh600000');
  const market = ref('cn');
  const name = ref('A');
  const chartRef = ref<HTMLElement | null>(document.createElement('div'));
  let api!: ReturnType<typeof useChart>;
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = createApp({
    setup() {
      api = useChart({
        chartRef,
        code,
        market,
        name,
        subIndicator: ref<SubIndicatorType>('VOL'),
        mainOverlay: ref<MainOverlayType>('MA'),
      });
      return () => null;
    },
  });
  app.mount(host);
  teardown = () => {
    api?.disposeChart();
    app.unmount();
    host.remove();
  };
  return { api, code };
}

describe('useChart 切股丢弃旧请求', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mocks.state.loader = null;
    vi.clearAllMocks();
  });

  afterEach(() => {
    teardown?.();
    teardown = null;
  });

  it('切股后旧 forward 返回：不回调图表，也不污染 allData', async () => {
    let resolveForward!: (bars: unknown[]) => void;
    const forwardPromise = new Promise<unknown[]>((resolve) => {
      resolveForward = resolve;
    });
    invokeMock.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd !== 'get_kline') return undefined;
      if (args?.count === 200) return forwardPromise;
      if (args?.code === 'sh600000') return A_MAIN;
      return B_MAIN;
    });

    const { api, code } = mountChart();
    await api.initChart('daily');
    await api.loadData('daily');

    let forwarded: unknown[] | null = null;
    const loader = mocks.state.loader!;
    const pending = loader.getBars({
      type: 'forward',
      symbol: { ticker: 'sh600000' },
      period: { type: 'day', span: 1 },
      timestamp: null,
      callback: (data: unknown[]) => {
        forwarded = data;
      },
    });

    // 切换到 2010 年才上市的另一只
    code.value = 'sh600001';
    await api.loadData('daily');

    // 上一只的 forward 这时才回来
    resolveForward(A_HISTORY);
    await pending;

    expect(forwarded).toBeNull();
  });

  it('同一只股票的 forward 正常回调（闸门不误伤）', async () => {
    invokeMock.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd !== 'get_kline') return undefined;
      if (args?.count === 200) return A_HISTORY;
      return A_MAIN;
    });

    const { api } = mountChart();
    await api.initChart('daily');
    await api.loadData('daily');

    let forwarded: unknown[] | null = null;
    await mocks.state.loader!.getBars({
      type: 'forward',
      symbol: { ticker: 'sh600000' },
      period: { type: 'day', span: 1 },
      timestamp: null,
      callback: (data: unknown[]) => {
        forwarded = data;
      },
    });

    expect(forwarded).not.toBeNull();
    expect(forwarded![0]).toMatchObject({
      close: 5,
      timestamp: new Date('2000-01-04').getTime(),
    });
  });

  it('新标的加载失败时，旧标的的自动刷新定时器必须停掉', async () => {
    vi.useFakeTimers();
    try {
      invokeMock.mockImplementation(async (cmd: string, args?: any) => {
        if (cmd !== 'get_kline') return undefined;
        if (args?.count === 10) return [];
        if (args?.code === 'sh600000') return A_MAIN;
        throw new Error('B failed');
      });

      const { api, code } = mountChart();
      await api.initChart('daily');
      await api.loadData('daily'); // A 成功，启动 30s 定时器

      code.value = 'sh600001';
      await api.loadData('daily'); // B 失败，不应留下 A 的定时器

      vi.mocked(invokeMock).mockClear();
      await vi.advanceTimersByTimeAsync(31_000);

      const incremental = invokeMock.mock.calls.filter(
        ([, args]) => (args as { count?: number } | undefined)?.count === 10,
      );
      expect(incremental).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
