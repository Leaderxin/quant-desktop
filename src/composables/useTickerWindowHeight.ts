// useTickerWindowHeight.ts — 让行情条窗口高度跟着内容走，适配系统缩放。
//
// 背景：行情条窗口尺寸写在 tauri.conf.json 里（230×40 逻辑像素，只能取一个
// 固定值）。逻辑像素不随 DPI 变化，但内容的 CSS 像素高度在变：两行文本
// line-height:1.4 + 上下 padding 本身就有 ~38.8px，再叠上不同缩放下的子像素
// 取整，固定的那个数字总会在某些缩放下把内容裁掉一截 —— 100% 和 125% 裁掉的
// 还不一样多。所以高度不能靠猜，得量。
//
// 做法：量出内容真实需要多少逻辑像素，再把窗口高度设成同一个值。逻辑像素 =
// webview 的 CSS 像素，所以量出来的数字可以直接喂给 setSize(LogicalSize)。
// 这套逻辑不依赖平台：Windows 的系统缩放、macOS 的 Retina、Linux 的分数缩放
// 都由 winit 折算成同一个 scale_factor，只要窗口和内容用同一把尺子（逻辑
// 像素），就不会错位。
import { onMounted, onUnmounted, watch, type Ref } from 'vue';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { LogicalSize, PhysicalPosition, getCurrentWindow, type Window } from '@tauri-apps/api/window';

/** 兜底下限：内容还没渲染出来时别把窗口压成 0。 */
const MIN_HEIGHT = 24;

/** 高度变化超过这个值才真的改窗口，避免亚像素抖动引发 setSize 循环。 */
const EPSILON = 0.5;

/**
 * 行情条宽度（逻辑像素），与 tauri.conf.json 的 ticker 窗口保持一致
 * （Rust 侧 datasource::TICKER_WIDTH 是同一数值的另一份拷贝）。
 *
 * 这里**用常量而不是去读 innerSize()**：缩放切换的那一瞬间，OS 会先把
 * scale_factor 翻新、再改窗口矩形，两次独立的异步 IPC 读到的 innerSize 和
 * scaleFactor 可能不是同一时刻的，据此换算出的宽度会失真。宽度本来就不该被
 * 我们改，读它只会引入这种竞态，不如直接写死。
 *
 * 代价：这个值改了 tauri.conf.json（和 Rust 常量）就必须同步改这里，没有
 * 编译期约束。
 */
const WIDTH = 230;

/** 缩放变化后的重试节奏：间隔毫秒数 × 次数（见 scheduleRetries 注释）。 */
const RETRY_INTERVAL_MS = 150;
const RETRY_COUNT = 4;

export interface TickerWindowHeightOptions {
  /**
   * 内容就绪开关：false 期间完全不动窗口尺寸。
   *
   * TickerBar 初始加载完成前渲染的是「暂无自选」占位行，量它会把窗口先
   * 压到占位高度、真内容渲染后再弹回去（启动时 40→24→39 的跳变）。调用方
   * 在自选列表首次加载完成或初始化失败后翻成 true。
   */
  ready?: Ref<boolean>;
}

/**
 * 观察 `content` 的自然高度，并把当前窗口的高度同步成它。
 *
 * `content` 必须是**内容高度不受窗口高度影响**的元素（自身不写 height:100%），
 * 否则「量高度 → 改窗口 → 再量」会自我循环。
 *
 * 需要 `core:window:allow-set-size` 等权限，见 capabilities/default.json。
 */
export function useTickerWindowHeight(
  content: Ref<HTMLElement | null>,
  options?: TickerWindowHeightOptions
) {
  // 非 Tauri 环境（纯浏览器里跑 vite dev、node 单测）没有 __TAURI_INTERNALS__，
  // getCurrentWindow() 在这一步就会同步抛错。高度同步完全建立在窗口 API
  // 之上，拿不到窗口就整体停用，绝不能把组件的 setup 炸掉。
  let current: Window | null = null;
  try {
    current = getCurrentWindow();
  } catch {
    // 落到下面判空统一退出
  }
  if (!current) return;
  const appWindow = current;

  const ready = options?.ready;
  const isReady = () => ready === undefined || ready.value;

  let unlistenScale: UnlistenFn | null = null;
  let observerRef: ResizeObserver | null = null;
  let retryTimer: ReturnType<typeof setInterval> | null = null;
  let retryTicks = 0;
  let syncRaf = 0;
  let lastApplied = 0;
  let disposed = false;

  function measure(): number {
    const el = content.value;
    if (!el) return 0;
    // scrollHeight 取的是内容撑出来的高度，即使元素被压扁也拿得到真实值。
    return Math.max(MIN_HEIGHT, Math.ceil(el.scrollHeight));
  }

  /**
   * 把窗口高度设成当前内容高度。
   *
   * 行情条贴在屏幕右下角，但 setSize 以左上角为锚伸缩：长高会把下边缘
   * 推出屏幕，压矮则让窗口悬在半空。所以改完尺寸后按「物理高度差」把
   * y 反向挪回去，把下边缘钉在原地（新位置会被 lib.rs 的 Moved 处理器
   * 照常存进 SQLite）。行情条无窗口装饰（inner == outer），outerSize 量
   * 到的就是 setSize 设的那个高度。
   */
  async function apply(force = false) {
    const target = measure();
    if (target <= 0) return;
    if (!isReady()) return;
    if (!force && Math.abs(target - lastApplied) < EPSILON) return;
    // 预读当前物理高度：setSize 一旦生效窗口矩形就变了，位移量得拿改之前的算。
    let prevHeight: number | null = null;
    try {
      prevHeight = (await appWindow.outerSize()).height;
    } catch {
      // 读不到就只改高度、放弃锚定
    }
    try {
      // 只调高度，宽度用常量（理由见 WIDTH 注释）。
      await appWindow.setSize(new LogicalSize(WIDTH, target));
      lastApplied = target;
    } catch (e) {
      // IPC 层面失败（比如窗口正在销毁），静默跳过
      console.debug('[ticker] height sync skipped:', e);
      return;
    }
    if (prevHeight === null) return;
    try {
      const scale = await appWindow.scaleFactor();
      const growth = Math.round(target * scale) - prevHeight;
      if (growth !== 0) {
        const pos = await appWindow.outerPosition();
        await appWindow.setPosition(new PhysicalPosition(pos.x, pos.y - growth));
      }
    } catch (e) {
      console.debug('[ticker] bottom-edge anchor skipped:', e);
    }
  }

  function scheduleSync() {
    if (disposed || syncRaf) return;
    syncRaf = requestAnimationFrame(() => {
      syncRaf = 0;
      if (disposed) return;
      void apply();
    });
  }

  /**
   * 缩放变化后重复下发几次窗口尺寸。
   *
   * 实测（Windows 10，125%/150% 切换）：setSize 之后系统还会在几百毫秒内二次
   * 调整窗口矩形，只调一次会被覆盖掉，所以按固定节奏补几次。每次都带 force，
   * 因为目标高度没变、缓存的 lastApplied 会把重试当成无变化而跳过。
   */
  function scheduleRetries() {
    if (retryTimer) clearInterval(retryTimer);
    retryTicks = 0;
    retryTimer = setInterval(() => {
      retryTicks += 1;
      void apply(true);
      if (retryTicks >= RETRY_COUNT) stopRetries();
    }, RETRY_INTERVAL_MS);
  }

  function stopRetries() {
    if (!retryTimer) return;
    clearInterval(retryTimer);
    retryTimer = null;
  }

  const onWindowResize = () => scheduleSync();
  const onViewportResize = () => scheduleSync();

  if (ready) {
    // 就绪翻转那一刻内容高度未必变化（不会触发 ResizeObserver），主动补一次。
    watch(ready, (v) => {
      if (v) scheduleSync();
    });
  }

  onMounted(() => {
    // 内容自身尺寸变化（字体加载完成、从"暂无自选"切到两行行情等）
    observerRef = new ResizeObserver(() => scheduleSync());
    const el = content.value;
    if (el) observerRef.observe(el);

    // 窗口/viewport 尺寸变化，含缩放切换后 webview 重排带来的变化
    window.addEventListener('resize', onWindowResize);
    window.visualViewport?.addEventListener('resize', onViewportResize);

    // 缩放因子变化是最直接的信号，收到就先重试一轮，再按常规路径同步一次
    appWindow
      .onScaleChanged(() => {
        scheduleRetries();
        scheduleSync();
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenScale = unlisten;
      })
      .catch((e) => {
        console.debug('[ticker] scale-change listener unavailable:', e);
      });

    void apply(true);
  });

  onUnmounted(() => {
    disposed = true;
    observerRef?.disconnect();
    observerRef = null;
    window.removeEventListener('resize', onWindowResize);
    window.visualViewport?.removeEventListener('resize', onViewportResize);
    unlistenScale?.();
    unlistenScale = null;
    stopRetries();
    if (syncRaf) cancelAnimationFrame(syncRaf);
    lastApplied = 0;
  });
}
