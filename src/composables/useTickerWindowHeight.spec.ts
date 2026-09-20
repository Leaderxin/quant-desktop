// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createApp, nextTick, ref } from 'vue';

/**
 * useTickerWindowHeight 的窗口同步行为测试。
 *
 * 与 market.spec.ts 同一思路：mock 掉 Tauri 边界（这里是 window 模块），
 * composable 与 Vue 本体都是真的。fake window 带一份会跟着 setSize /
 * setPosition 走的物理像素状态，用来验证「下边缘钉在原地」这条不变量 ——
 * setSize 以左上角为锚伸缩，而行情条贴在屏幕右下角，这两个事实冲突时
 * 必须由 composable 补偿。
 */

const mocks = vi.hoisted(() => {
  const state = {
    // 物理像素。初始高度 50 = 40 逻辑 × 1.25 缩放；位置选一组
    // 「屏幕高 1080、窗口贴底」的右下角坐标。
    x: 1690,
    y: 1030,
    height: 50,
    scale: 1.25,
  };
  const api = {
    setSize: vi.fn(),
    outerSize: vi.fn(),
    scaleFactor: vi.fn(),
    outerPosition: vi.fn(),
    setPosition: vi.fn(),
    onScaleChanged: vi.fn(),
  };
  return { state, api, getCurrentWindowThrows: false };
});

vi.mock('@tauri-apps/api/window', () => {
  class LogicalSize {
    constructor(public width: number, public height: number) {}
  }
  class PhysicalPosition {
    constructor(public x: number, public y: number) {}
  }
  return {
    LogicalSize,
    PhysicalPosition,
    getCurrentWindow: () => {
      // 模拟纯浏览器 / node 环境：__TAURI_INTERNALS__ 不存在时
      // getCurrentWindow() 在这一步同步抛 TypeError。
      if (mocks.getCurrentWindowThrows) {
        throw new TypeError("Cannot read properties of undefined (reading 'metadata')");
      }
      return mocks.api;
    },
  };
});

import { useTickerWindowHeight } from './useTickerWindowHeight';

/** 模拟 ResizeObserver：手动触发回调，代表「内容高度变了」。 */
class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];
  constructor(public callback: () => void) {
    FakeResizeObserver.instances.push(this);
  }
  observe() {}
  disconnect() {}
  unobserve() {}
}

/** rAF 改成同步队列：flush 一次 = 跑一帧。 */
let rafQueue: FrameRequestCallback[] = [];
function flushRaf() {
  const queue = rafQueue;
  rafQueue = [];
  queue.forEach((cb) => cb(0));
}

/** 跑完 apply() 的 await 链（IPC mock 都立即 resolve）。 */
async function flushAsync(rounds = 12) {
  for (let i = 0; i < rounds; i++) await Promise.resolve();
}

/** 把 composable 挂进真实 Vue 组件（onMounted/onUnmounted 需要实例上下文）。 */
let teardown: (() => void) | null = null;
function mountComposable(setup: () => void) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = createApp({ setup, render: () => null });
  app.mount(host);
  teardown = () => {
    app.unmount();
    host.remove();
  };
}

/** 内容元素替身：measure() 只读 scrollHeight。 */
function fakeEl(scrollHeight: number) {
  return { scrollHeight } as unknown as HTMLElement;
}

const BOTTOM_EDGE = 1080; // 初始 y + height = 1030 + 50

describe('useTickerWindowHeight', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getCurrentWindowThrows = false;
    Object.assign(mocks.state, { x: 1690, y: 1030, height: 50, scale: 1.25 });
    FakeResizeObserver.instances = [];
    rafQueue = [];

    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      rafQueue.push(cb);
      return rafQueue.length;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());

    // fake window：物理状态跟着 setSize / setPosition 走，读到的永远是「现在」。
    mocks.api.setSize.mockImplementation(async (size: { height: number }) => {
      mocks.state.height = Math.round(size.height * mocks.state.scale);
    });
    mocks.api.outerSize.mockImplementation(async () => ({ width: 288, height: mocks.state.height }));
    mocks.api.scaleFactor.mockImplementation(async () => mocks.state.scale);
    mocks.api.outerPosition.mockImplementation(async () => ({ x: mocks.state.x, y: mocks.state.y }));
    mocks.api.setPosition.mockImplementation(async (p: { x: number; y: number }) => {
      mocks.state.x = p.x;
      mocks.state.y = p.y;
    });
    mocks.api.onScaleChanged.mockResolvedValue(() => {});
  });

  afterEach(() => {
    teardown?.();
    teardown = null;
    vi.unstubAllGlobals();
  });

  it('非 Tauri 环境（getCurrentWindow 抛错）下不让 setup 炸掉，整体停用高度同步', () => {
    mocks.getCurrentWindowThrows = true;
    const content = ref(fakeEl(24));
    expect(() => useTickerWindowHeight(content)).not.toThrow();
  });

  it('ready=false 期间不动窗口（初始占位行不压扁窗口），ready 翻真后按内容设高度', async () => {
    const el = fakeEl(24);
    const content = ref(el);
    const ready = ref(false);
    mountComposable(() => useTickerWindowHeight(content, { ready }));

    // mount 时的 apply(true) 与内容变化（ResizeObserver）都应被 ready 拦下
    FakeResizeObserver.instances[0].callback();
    flushRaf();
    await flushAsync();
    expect(mocks.api.setSize).not.toHaveBeenCalled();

    ready.value = true;
    await nextTick();
    flushRaf();
    await flushAsync();
    expect(mocks.api.setSize).toHaveBeenCalledTimes(1);
    expect(mocks.api.setSize.mock.calls[0][0]).toMatchObject({ width: 230, height: 24 });
  });

  it('高度变化后按物理高度差回移 y，下边缘始终钉在原地', async () => {
    // 留着原始对象引用：scrollHeight 在 HTMLElement 类型上是只读的
    const raw = { scrollHeight: 24 };
    const content = ref(raw as unknown as HTMLElement);
    const ready = ref(true);
    mountComposable(() => useTickerWindowHeight(content, { ready }));
    await flushAsync();

    // 初始内容 24 逻辑（=30 物理）：高度 50→30，y 下移 20 补偿，下边缘不动
    expect(mocks.api.setSize).toHaveBeenCalledWith(
      expect.objectContaining({ width: 230, height: 24 })
    );
    expect(mocks.state.y + mocks.state.height).toBe(BOTTOM_EDGE);

    // 内容长到两行 39 逻辑（=49 物理）：y 上移 19，下边缘仍不动
    raw.scrollHeight = 39;
    FakeResizeObserver.instances[0].callback();
    flushRaf();
    await flushAsync();

    expect(mocks.api.setSize).toHaveBeenLastCalledWith(
      expect.objectContaining({ width: 230, height: 39 })
    );
    expect(mocks.api.setPosition).toHaveBeenLastCalledWith(
      expect.objectContaining({ x: 1690, y: 1031 })
    );
    expect(mocks.state.y + mocks.state.height).toBe(BOTTOM_EDGE);
  });
});
