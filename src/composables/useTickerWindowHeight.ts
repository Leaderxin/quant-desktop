// useTickerWindowHeight.ts — 让行情条窗口高度跟着内容走，适配系统缩放。
//
// 背景：行情条窗口尺寸写在 tauri.conf.json 里（210×40 逻辑像素，只能取一个
// 固定值）。逻辑像素不随 DPI 变化，但内容的 CSS 像素高度在变：两行文本
// line-height:1.4 + 上下 padding 本身就有 ~38.8px，再叠上不同缩放下的子像素
// 取整，固定的那个数字总会在某些缩放下把内容裁掉一截 —— 100% 和 125% 裁掉的
// 还不一样多。所以高度不能靠猜，得量。
//
// 做法：量出内容真实需要多少逻辑像素，再把窗口高度设成同一个值。逻辑像素 =
// webview 的 CSS 像素，所以量出来的数字可以直接喂给 `set_ticker_size`（后端按
// 逻辑像素处理，并顺手把 min/max 尺寸提示钉成同一个值 —— 窗口因此不可被缩放）。
// 这套逻辑不依赖平台：Windows 的系统缩放、macOS 的 Retina、Linux 的分数缩放
// 都由 winit 折算成同一个 scale_factor，只要窗口和内容用同一把尺子（逻辑
// 像素），就不会错位。
import { onMounted, onUnmounted, watch, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { PhysicalPosition, getCurrentWindow, type Window } from '@tauri-apps/api/window';

/** 兜底下限：内容还没渲染出来时别把窗口压成 0。 */
const MIN_HEIGHT = 24;

/** 高度变化超过这个值才真的改窗口，避免亚像素抖动引发 setSize 循环。 */
const EPSILON = 0.5;

// 宽度不在这里出现：改尺寸走 `set_ticker_size` 命令，宽度由 Rust 侧
// `datasource::TICKER_WIDTH` 单点决定。前端只报高度，就既没有「三处常量要同步」
// 的隐患，也不会在缩放切换的瞬间读到失真的 innerSize。

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
  /** 已写回 `ticker_height` 的高度，避免强制同步时重复写库。 */
  let lastPersisted = 0;
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
      // 只调高度，宽度由后端按常量给（理由见 WIDTH 注释）。走 IPC 而不是
      // `setSize`：后端那一步会把 min/max 尺寸提示一起钉成同一个值，窗口因此
      // 不可被用户/WM 缩放 —— 直接在 webview 里 setSize 就绕过了它。
      await invoke('set_ticker_size', { height: Math.round(target) });
      lastApplied = target;
    } catch (e) {
      // IPC 层面失败（比如窗口正在销毁），静默跳过
      console.debug('[ticker] height sync skipped:', e);
      return;
    }
    // 高度写回库：下次启动/显示前先按它设尺寸，位置才不会因为
    // 「先按配置高度建窗、再按内容长高」而每次重启向上漂移。
    if (Math.abs(target - lastPersisted) >= EPSILON) {
      try {
        await invoke('set_setting', { key: 'ticker_height', value: String(target) });
        lastPersisted = target;
      } catch (e) {
        console.debug('[ticker] height persist skipped:', e);
      }
    }
    if (prevHeight === null) return;
    try {
      // 用 setSize 之后的真实高度差，而不是 target × scale 的估算：
      // 125%/150% 下取整方向未必一致，估算值会累积出 1px 漂移。
      let afterHeight = (await appWindow.outerSize()).height;
      if (afterHeight === prevHeight) {
        // 个别平台读回来的仍是旧值，退回按缩放估算（与旧实现一致）。
        const scale = await appWindow.scaleFactor();
        afterHeight = Math.round(target * scale);
      }
      const growth = afterHeight - prevHeight;
      if (growth !== 0) {
        const pos = await appWindow.outerPosition();
        await appWindow.setPosition(new PhysicalPosition(pos.x, pos.y - growth));
      }
      // 后端把 min/max 钉成了同一个值（禁止用户缩放）。如果哪一天某个平台
      // 连**我们的**改尺寸也一起夹住，症状是窗口底部永久多出一截空白 ——
      // 那是一年前在 Linux 上踩过的坑，这里留一条能查的记录，而不是靠肉眼发现。
      const scale = await appWindow.scaleFactor().catch(() => 1);
      const wanted = Math.round(target * scale);
      if (Math.abs(afterHeight - wanted) > 1) {
        console.warn(
          `[ticker] 窗口高度没跟上内容: 目标 ${wanted}px / 实际 ${afterHeight}px（内容 ${target} 逻辑像素 × ${scale}）`,
        );
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

  /**
   * 窗口从隐藏变回可见时强制同步一次。
   *
   * 隐藏期间 ResizeObserver/rAF 可能被暂停，而 apply() 已经把 lastApplied
   * 记成目标高度；不强制重来一次的话，重新显示时窗口可能仍是旧尺寸。
   */
  const onVisibilityChange = () => {
    if (document.visibilityState === 'visible') void apply(true);
  };

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
    document.addEventListener('visibilitychange', onVisibilityChange);

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
    document.removeEventListener('visibilitychange', onVisibilityChange);
    unlistenScale?.();
    unlistenScale = null;
    stopRetries();
    if (syncRaf) cancelAnimationFrame(syncRaf);
    lastApplied = 0;
  });
}
