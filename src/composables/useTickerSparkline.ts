// src/composables/useTickerSparkline.ts — 行情条行内迷你分时线
//
// 行情条是启动即创建、常驻置顶的小窗口，不能把 klinecharts（约 235KB）带进来；
// 这里给每一行配一块 88×18 的 canvas 自绘折线。区间与坐标换算全在
// utils/tickerSparkline 里（含单测），本文件只负责取数、缓存与绘制。
//
// `both` 把两套纵轴画进同一块画布：价格轴那条满不透明当主线，涨跌停轴那条 50%
// 透明叠在下面。两者都以昨收为中心，所以一条 0% 虚线对两条线同时成立 ——
// 一眼读出「今天怎么走的」和「离涨跌停还有多远」。
//
// 画法可切换（`ticker_chart_style`）：`axis` 多一条 x 轴线与三个刻度，`plain`
// 只有 0% 虚线 + 两条曲线。涨跌停边界两种画法都不画 —— 那一维由半透明的涨跌停
// 曲线自己表达。绘制本体在 utils/tickerSparkline 的 `paintTickerSparkline`，
// 与设置页预览共用同一份，免得「设置里看着对、实际不是那样」。
//
// 缓存、在途请求、重试间隔都放在**模块作用域**：行情条每 3 秒翻一屏，行随 key
// 变化整套重建，实例级的东西留不住任何东西，翻页就会变成对数据源的持续轰炸。
import { nextTick, onMounted, onUnmounted, ref, unref, watch, type MaybeRef, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { KLineData as KCLineData } from 'klinecharts';
import type { MinuteData } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { beijingNow, mapMinuteBars, sessionPrevClose } from '@/utils/minuteBars';
import { priceLimitRate } from '@/utils/priceLimit';
import type { TickerChartMode, TickerChartStyle } from '@/utils/prefs';
import {
  paintTickerSparkline,
  resolveSparklineLines,
  sparklineBaselineY,
  sparklineCanvasPoints,
  type SparklineRange,
} from '@/utils/tickerSparkline';

/**
 * 盘中刷新节奏。
 *
 * 一屏最多 10 行、每行一个实例，15s 会变成持续对数据源的小轰炸；而分时是
 * 1 分钟粒度 —— 30s 已经比数据本身变得快一倍。报价与涨跌幅由主调度器每 2s
 * 推过来，不依赖这条线。
 */
const POLL_TRADING_MS = 30_000;
/** 午休 / 收盘 / 周末：降频，避免常驻窗口整夜空拉接口。 */
const POLL_IDLE_MS = 60_000;
/** 数据缓存有效期：这之内回到同一只（悬停、轮播）直接画，不重新请求。 */
const CACHE_TTL_MS = 30_000;
/**
 * 同一标的两次**网络请求**的最小间隔，失败也算一次。
 *
 * 必须短于 `CACHE_TTL_MS`：翻页时行被销毁重建，重建的实例只看缓存新鲜度 ——
 * TTL 一过就重发，3 秒一轮的轮播会把这里变成对数据源的持续轰炸。它同时是
 * 「某只股票接口挂了」时的重试下限：挂了不写缓存，但记一次尝试时间。
 */
const REQUEST_MIN_INTERVAL_MS = 10_000;
const CACHE_MAX = 32;

interface CacheEntry {
  bars: KCLineData[];
  /** 数据自身算出的昨收；拿不到为 0，绘制时再用实时行情的兜底值 */
  dataPrevClose: number;
  fetchedAt: number;
}

/** 跨实例共享的分时数据缓存（理由见文件头）。 */
const cache = new Map<string, CacheEntry>();
/** 每个标的最近一次请求的时间（成功或失败都记），用于 `REQUEST_MIN_INTERVAL_MS`。 */
const lastAttempt = new Map<string, number>();
/** 同一标的的在途请求：多个实例同时挂载只发一次。 */
const inflight = new Map<string, Promise<boolean>>();
/** 数据落地后要唤醒的实例（按标的 key 分组）。 */
const subscribers = new Map<string, Set<() => void>>();

function notifySubscribers(key: string) {
  const set = subscribers.get(key);
  if (!set) return;
  for (const fn of set) fn();
}

/** 是否处于 A 股连续竞价时段（含集合竞价前后的缓冲）。 */
function isTradingSession(now = new Date()): boolean {
  const day = now.getDay();
  if (day === 0 || day === 6) return false;
  const minutes = now.getHours() * 60 + now.getMinutes();
  return (
    (minutes >= 9 * 60 + 15 && minutes <= 11 * 60 + 35) ||
    (minutes >= 12 * 60 + 55 && minutes <= 15 * 60 + 5)
  );
}

/** CSS 令牌可能是空串或被改坏；这里只接受 #rrggbb。 */
function safeHex(value: string, fallback: string): string {
  return /^#[0-9a-f]{6}$/i.test(value) ? value : fallback;
}

export interface TickerSparklineOptions {
  canvas: Ref<HTMLCanvasElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name: MaybeRef<string>;
  /** `off` 由调用方决定不挂载，这里只会收到 price / limit / both */
  mode: MaybeRef<TickerChartMode>;
  /** 画法：带 x 轴线与刻度 / 只有虚线与曲线 */
  style: MaybeRef<TickerChartStyle>;
  /** 实时行情给的兜底昨收（`price - change`），数据里没有上一交易日时才用 */
  prevClose?: MaybeRef<number | undefined>;
}

export function useTickerSparkline(options: TickerSparklineOptions) {
  const settings = useSettingsStore();

  const loading = ref(false);
  const error = ref('');

  let myKey = '';
  /** 指向 `cache` 里那条记录的同一份数组，不复制 */
  let bars: KCLineData[] = [];
  /** 数据自身算出的昨收；0 表示数据里没有上一交易日 */
  let dataPrevClose = 0;

  let disposed = false;
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  let drawRaf = 0;
  let tauriScale = 1;

  let appWindow: ReturnType<typeof getCurrentWindow> | null = null;
  try {
    appWindow = getCurrentWindow();
  } catch {
    // 纯浏览器 dev / 单测：没有 Tauri 窗口 API，可见性判断退化为 document
  }

  // ---- 标的与可见性 ----

  function keyOf(): string {
    return `${unref(options.market)}:${unref(options.code)}`;
  }

  function effectivePrevClose(): number {
    if (dataPrevClose > 0) return dataPrevClose;
    const fallback = Number(unref(options.prevClose ?? 0));
    return Number.isFinite(fallback) && fallback > 0 ? fallback : 0;
  }

  /**
   * 窗口当前是否真的可见。
   *
   * `document.visibilityState` 在不同平台/不同隐藏方式下并不可靠（Windows 的
   * WebView2 通常会给 hidden，部分 Linux WM 不会），再用 Tauri 的 isVisible()
   * 兜一层。拿不到窗口时按可见处理，别把 dev 环境整个禁掉。
   */
  async function windowVisible(): Promise<boolean> {
    if (document.visibilityState !== 'visible') return false;
    if (!appWindow) return true;
    try {
      return await appWindow.isVisible();
    } catch {
      return true;
    }
  }

  // ---- 取数 ----

  function trimCache() {
    while (cache.size > CACHE_MAX) {
      const oldest = cache.keys().next().value;
      if (oldest === undefined) break;
      cache.delete(oldest);
      // 被挤出去的标的不能再留着请求时间：自选换血时它会回来，
      // 留着只会让它白等一次 REQUEST_MIN_INTERVAL_MS。
      lastAttempt.delete(oldest);
    }
  }

  /** 把共享缓存里属于当前标的的数据接到本实例上（没有就清空）。 */
  function adoptCache() {
    const entry = cache.get(myKey);
    bars = entry ? entry.bars : [];
    dataPrevClose = entry ? entry.dataPrevClose : 0;
  }

  /**
   * 该不该为这个标的发一次网络请求。
   *
   * `force`（轮询、切数据源）绕过判断 —— 那是唯一会主动要新数据的路径。
   * 其余情况（挂载、翻页重建、切回同一只）只在「没有可用数据」或「数据过期」
   * 时发，且受 `REQUEST_MIN_INTERVAL_MS` 限制。
   */
  function shouldFetch(key: string, force: boolean, now = Date.now()): boolean {
    if (force) return true;
    const recent = (lastAttempt.get(key) ?? 0) + REQUEST_MIN_INTERVAL_MS > now;
    if (recent) return false;
    const entry = cache.get(key);
    if (!entry) return true;
    return now - entry.fetchedAt >= CACHE_TTL_MS;
  }

  /**
   * 取当前标的的分时数据；返回是否真的发过一次请求。
   *
   * 失败不写缓存（写了坏数据会把重试挡整整一个 TTL），但记一次尝试时间，
   * 免得一只接口挂掉的票被每 3 秒的翻页反复捶。
   */
  async function fetchIntraday(force: boolean): Promise<boolean> {
    const code = unref(options.code);
    const market = unref(options.market);
    if (!code || !market) return false;

    const key = `${market}:${code}`;
    if (!shouldFetch(key, force)) {
      // 不发请求也要把已有数据接上：可能是缓存还新鲜，也可能是刚试过失败。
      adoptCache();
      scheduleDraw();
      return false;
    }

    // 同一标的的并发请求（几个实例同时挂载同一只）合并成一次。
    const pending = inflight.get(key);
    if (pending) {
      loading.value = bars.length === 0;
      const fetched = await pending;
      if (disposed || myKey !== key) return fetched;
      adoptCache();
      if (fetched) scheduleDraw();
      return fetched;
    }

    loading.value = bars.length === 0;
    error.value = '';
    lastAttempt.set(key, Date.now());

    const task = (async (): Promise<boolean> => {
      try {
        const data = await invoke<MinuteData[]>('get_intraday', { code, market });
        const all = mapMinuteBars(data);
        // 数据源会把「还没走完的那一分钟」也推下来；主图也是这么过滤的。
        const boundary = beijingNow();
        const valid = all.filter((bar) => bar.timestamp <= boundary);
        const prev = sessionPrevClose(data) ?? 0;
        cache.set(key, { bars: valid, dataPrevClose: prev, fetchedAt: Date.now() });
        trimCache();
        return true;
      } catch (e) {
        console.error('[ticker-sparkline] get_intraday failed:', e);
        return false;
      } finally {
        inflight.delete(key);
        notifySubscribers(key);
      }
    })();

    inflight.set(key, task);
    const fetched = await task;
    if (!disposed) loading.value = false;
    if (disposed || myKey !== key) return fetched;
    if (fetched) {
      adoptCache();
      scheduleDraw();
    } else {
      error.value = '分时数据加载失败';
    }
    return fetched;
  }

  async function fetchIfVisible(force = false): Promise<void> {
    if (!(await windowVisible())) return;
    await fetchIntraday(force);
  }

  // ---- 轮询 ----

  function schedulePoll() {
    if (pollTimer) clearTimeout(pollTimer);
    pollTimer = setTimeout(async () => {
      pollTimer = null;
      if (disposed) return;
      await fetchIfVisible(true);
      schedulePoll();
    }, isTradingSession() ? POLL_TRADING_MS : POLL_IDLE_MS);
  }

  // ---- 绘制 ----

  function scheduleDraw() {
    if (disposed || drawRaf) return;
    drawRaf = requestAnimationFrame(() => {
      drawRaf = 0;
      if (!disposed) draw();
    });
  }

  function readColors() {
    const style = getComputedStyle(document.documentElement);
    return {
      accent: safeHex(style.getPropertyValue('--color-accent').trim(), '#58a6ff'),
      up: safeHex(style.getPropertyValue('--color-up').trim(), '#f85149'),
      down: safeHex(style.getPropertyValue('--color-down').trim(), '#3fb950'),
      baseline: style.getPropertyValue('--color-text-tertiary').trim() || '#8b949e',
    };
  }

  function draw() {
    const canvas = options.canvas.value;
    if (!canvas) return;

    // 隐藏窗口里 clientWidth 可能是 0；等可见 / resize 再画。
    const cssWidth = canvas.clientWidth;
    const cssHeight = canvas.clientHeight;
    if (cssWidth <= 0 || cssHeight <= 0) return;

    // WebKitGTK 的分数缩放下 devicePixelRatio 可能只报 1，用 Tauri 的 scaleFactor
    // 兜底；两者取大，避免 1.25 屏上画出一张糊图。
    const dpr = Math.max(window.devicePixelRatio || 1, tauriScale);
    const bitmapWidth = Math.round(cssWidth * dpr);
    const bitmapHeight = Math.round(cssHeight * dpr);
    if (canvas.width !== bitmapWidth) canvas.width = bitmapWidth;
    if (canvas.height !== bitmapHeight) canvas.height = bitmapHeight;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const mode = unref(options.mode);
    const rate = priceLimitRate(unref(options.code), unref(options.name));
    const prevClose = effectivePrevClose();
    const { main, limit } = resolveSparklineLines(bars, prevClose, mode, rate);
    const lastBar = bars[bars.length - 1];

    paintTickerSparkline(
      ctx,
      {
        style: unref(options.style),
        // 昨收是猜出来的（新股首日）时给 null：那条线本身就没有意义。
        baselineY: prevClose > 0 ? sparklineBaselineY() : null,
        under: sparklineCanvasPoints(bars, limit, cssWidth),
        over: sparklineCanvasPoints(bars, main, cssWidth),
        dot:
          prevClose > 0 && lastBar
            ? sparklineDot(bars, main ?? limit, cssWidth, prevClose)
            : null,
        width: cssWidth,
        height: cssHeight,
      },
      readColors(),
      dpr,
    );
  }

  /**
   * 末端圆点：位置跟着主线（没有主线时是涨跌停线），颜色跟随相对昨收的涨跌。
   *
   * 昨收未知时不画 —— 「涨还是跌」这个问题本身没有答案。
   */
  function sparklineDot(
    barList: readonly { timestamp: number; close: number }[],
    range: SparklineRange | null,
    width: number,
    prevClose: number,
  ) {
    const points = sparklineCanvasPoints(barList, range, width);
    const last = points[points.length - 1];
    if (!last) return null;
    const lastBar = barList[barList.length - 1];
    return { x: last.x, y: last.y, rising: lastBar.close >= prevClose };
  }

  // ---- 生命周期 ----

  async function refreshScale() {
    if (!appWindow) return;
    try {
      tauriScale = await appWindow.scaleFactor();
    } catch {
      // 读不到就继续用 devicePixelRatio
    }
    scheduleDraw();
  }

  const onVisibilityChange = () => {
    if (document.visibilityState !== 'visible') return;
    // 显示时立刻拉一次并重绘，不用等下一个轮询 tick。
    void fetchIfVisible(false);
    scheduleDraw();
  };

  const onResize = () => {
    void refreshScale();
  };

  let unlistenScale: (() => void) | null = null;

  /** 数据落地后唤醒本实例（订阅按标的 key 分组）。 */
  const wakeUp = () => {
    if (disposed) return;
    adoptCache();
    scheduleDraw();
  };

  function subscribe(key: string) {
    const set = subscribers.get(key);
    if (set) set.add(wakeUp);
    else subscribers.set(key, new Set([wakeUp]));
  }

  function unsubscribe(key: string) {
    const set = subscribers.get(key);
    if (!set) return;
    set.delete(wakeUp);
    if (set.size === 0) subscribers.delete(key);
  }

  /** 切到新的标的：换订阅、接缓存。 */
  function bindKey() {
    const next = keyOf();
    if (next === myKey) return;
    if (myKey) unsubscribe(myKey);
    myKey = next;
    if (!next) return;
    subscribe(next);
    adoptCache();
    scheduleDraw();
  }

  watch(
    [() => unref(options.code), () => unref(options.market)],
    () => {
      bindKey();
      void fetchIfVisible(false);
    },
  );

  // 名称/模式/画法/兜底昨收变化只影响怎么画，不重新请求。
  // 画法必须在这里：它只改绘制分支，别处没人读 options.style，漏了这条就是
  // 「设置里改了、行情条不动」。
  watch(
    [
      () => unref(options.name),
      () => unref(options.mode),
      () => unref(options.style),
      () => unref(options.prevClose),
    ],
    () => scheduleDraw(),
  );

  // canvas 读不了 var()：主题/配色/透明切换后必须重绘。等一帧让
  // <html data-theme> 与 data-color-scheme 先落定。
  watch(
    [() => settings.theme, () => settings.colorScheme, () => settings.tickerTransparent],
    () => {
      void nextTick(() => scheduleDraw());
    },
  );

  // 切数据源后旧缓存可能来自另一个源的 1 分钟/5 分钟口径，整份作废。
  watch(
    () => settings.activeDatasource,
    () => {
      cache.clear();
      lastAttempt.clear();
      adoptCache();
      void fetchIfVisible(true);
    },
  );

  onMounted(() => {
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('resize', onResize);
    bindKey();
    void refreshScale();
    void fetchIfVisible(false);
    schedulePoll();

    if (appWindow) {
      appWindow
        .onScaleChanged(() => {
          void refreshScale();
        })
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlistenScale = unlisten;
        })
        .catch(() => {
          // 没有缩放事件时靠 window.resize 兜底
        });
    }
  });

  onUnmounted(() => {
    disposed = true;
    document.removeEventListener('visibilitychange', onVisibilityChange);
    window.removeEventListener('resize', onResize);
    if (pollTimer) clearTimeout(pollTimer);
    if (drawRaf) cancelAnimationFrame(drawRaf);
    unlistenScale?.();
    unlistenScale = null;
    if (myKey) unsubscribe(myKey);
    // 缓存与请求节流是模块级的，留给下一个实例（翻页）用
  });

  return { loading, error, redraw: scheduleDraw };
}
