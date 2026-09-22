// src/stores/market.ts
import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { MarketOverview } from '@/types';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useSettingsStore } from '@/stores/settings';

export type MarketDirection = 'up' | 'down';

/** 兜底刷新间隔(ms) —— 与盘中一致。真正的节奏由后端 `market_clock` 按时段给出:
 *  盘中/盘前 60s、午休 120s、休市 300s。休市时段这几块数据完全不动,退避以免整夜空转。 */
const DEFAULT_INTERVAL_MS = 60_000;

export const useMarketStore = defineStore('market', () => {
  const settings = useSettingsStore();
  const overview = ref<MarketOverview | null>(null);
  const direction = ref<MarketDirection>('up');
  /** 板块榜单默认折叠 —— 展开态约占 280px 且 flex-shrink: 0,会一直挤压自选表。
   *  注意:折叠时标题栏仍显示成交额/涨跌家数,所以数据刷新与展开状态无关。 */
  const expanded = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);

  let timer: ReturnType<typeof setTimeout> | null = null;
  let unlistenSession: UnlistenFn | null = null;
  let intervalMs = DEFAULT_INTERVAL_MS;
  let running = false;
  /** 是否有请求在途,以及在途期间是否又有人要数据(见 fetchOverview)。 */
  let inFlight = false;
  let refetchQueued = false;
  /** 递增的轮次号。tick 是 async,期间可能被事件触发的下一轮或 stopRefresh 打断,
   *  用它保证只有最新一轮有权排期,避免残留多个 setTimeout 造成重复轮询。 */
  let generation = 0;

  /**
   * 拉一次概览。
   *
   * 关于「取消在途请求」:tauri 的 `invoke` 不支持中止(InvokeOptions 只有 headers),
   * 后端那 4 个 HTTP 请求一旦发出就会跑完,前端无法真正取消。能做的是两件事:
   *   1. 过期响应不落地 —— 期间方向变了,就把这次结果丢掉,避免旧榜单覆盖新榜单;
   *   2. 记下这次诉求,在途请求一回来立刻补一次 —— 于是切换榜单最多等一个在途请求
   *      的时间(~1s),而不是干等到下一个轮询周期(休市时段可达 5 分钟)。
   */
  async function fetchOverview() {
    if (inFlight) {
      refetchQueued = true;
      return;
    }
    inFlight = true;
    loading.value = true;
    const requested = direction.value;
    try {
      const data = await invoke<MarketOverview>('get_market_overview', {
        direction: requested,
        // 榜单条数由设置页配置。后端会再夹一次区间 —— IPC 参数不能假设已被校验。
        topN: settings.sectorTopN,
      });
      // 只有请求方向仍是当前方向时才落地,否则丢弃
      if (requested === direction.value) {
        overview.value = data;
        error.value = null;
      }
    } catch (e) {
      if (requested === direction.value) error.value = `市场概览加载失败: ${e}`;
      console.error('[market store] fetchOverview failed:', e);
    } finally {
      inFlight = false;
      loading.value = false;
      if (refetchQueued) {
        refetchQueued = false;
        void fetchOverview();
      }
    }
  }

  /** 切换到指定方向。已是当前方向时不做任何事 —— 点已选中的 tab 不应把榜单翻过来。 */
  function setDirection(v: MarketDirection) {
    if (direction.value === v) return;
    direction.value = v;
    // 立即丢弃旧方向的榜单:补拉回来前(失败则直到下次成功)面板会渲染的
    // 是反方向的旧数据 + 新的「领涨/领跌」标签。成交额/涨跌家数与方向无关,保留。
    if (overview.value) {
      overview.value = { ...overview.value, industry: [], concept: [] };
    }
    fetchOverview();
  }

  /** 展开/折叠板块榜单。
   *  轮询由组件的挂载/卸载驱动(startRefresh/stopRefresh),不随展开状态启停 ——
   *  折叠时标题栏的成交额与涨跌家数仍需保持最新。 */
  function setExpanded(v: boolean) {
    if (expanded.value === v) return;
    expanded.value = v;
    // 展开时立刻拉一次,避免展示最多 60s 前的榜单
    if (v) fetchOverview();
  }

  /** 拉一次数据,再按当前时段间隔排下一次。
   *  时段切换时也会调用它 —— 先取消在途排期再重排,所以 9:30 开盘不必等休市的 5 分钟。 */
  async function tick() {
    const gen = ++generation;
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    await fetchOverview();
    if (!running || gen !== generation) return;
    timer = setTimeout(tick, intervalMs);
  }

  async function startRefresh() {
    stopRefresh();
    running = true;

    // 初始节奏种子 —— market-session-changed 只在时段切换时推送,启动时不补发
    try {
      intervalMs = (await invoke<number>('get_overview_interval')) * 1000;
    } catch (e) {
      console.error('[market store] get_overview_interval 失败,回退 60s:', e);
    }

    try {
      unlistenSession = await listen<{ overview_interval_secs?: number }>(
        'market-session-changed',
        (event) => {
          const secs = event.payload?.overview_interval_secs;
          if (typeof secs === 'number' && secs > 0) intervalMs = secs * 1000;
          if (running) void tick();
        }
      );
    } catch (e) {
      console.error('[market store] 监听 market-session-changed 失败:', e);
    }

    if (running) await tick();
  }

  function stopRefresh() {
    running = false;
    generation++;
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (unlistenSession) {
      unlistenSession();
      unlistenSession = null;
    }
  }

  /**
   * 榜单条数改了要立刻按新条数拉一次，而不是干等到下一个轮询周期 ——
   * 休市时段那可能是 5 分钟，用户会以为设置没生效。
   *
   * 只在轮询运行中触发：面板被隐藏时 startRefresh 不会被调用，
   * 这里也就不会发出任何请求（隐藏即静默，是「关闭市场概览」的承诺）。
   */
  watch(
    () => settings.sectorTopN,
    () => {
      if (running) void fetchOverview();
    },
  );

  return {
    overview,
    direction,
    expanded,
    loading,
    error,
    fetchOverview,
    setDirection,
    setExpanded,
    startRefresh,
    stopRefresh,
  };
});
