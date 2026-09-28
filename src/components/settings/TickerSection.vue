<script setup lang="ts">
// 行情条：显示开关、透明背景、单次轮播条数、轮播范围。
import { computed, ref, watch } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import { useWatchlistStore } from '@/stores/watchlist';
import { formatCode } from '@/utils/format';
import { clampTickerItems, TICKER_ITEMS_MAX, TICKER_ITEMS_MIN, TICKER_ITEMS_PRESET_MAX } from '@/utils/prefs';
import { moveByStep } from '@/utils/dragSort';
import SettingsRow from './SettingsRow.vue';
import SegmentedControl from './SegmentedControl.vue';
import DragSortList from '@/components/common/DragSortList.vue';
import { NSwitch } from 'naive-ui';

const settings = useSettingsStore();
const watchlist = useWatchlistStore();

/** 全部自选，按轮播位次排序。列表顺序即轮播顺序，所以这里必须用 ticker_order
 *  而不是分组顺序 —— 播报范围是跨分组的扁平列表。 */
const orderedItems = computed(() =>
  [...watchlist.items].sort(
    (a, b) => a.ticker_order - b.ticker_order || a.id - b.id,
  ),
);

// ── 分组筛选 ──
// 0 表示「全部」。分组 id 从 1 起（SQLite AUTOINCREMENT），不会撞上。
const ALL = 0;
const filterGroup = ref<number>(ALL);

const filterOptions = computed(() => [
  { value: ALL, label: '全部' },
  ...watchlist.groups.map((g) => ({ value: g.id, label: g.name })),
]);

// 分组被删后筛选值会悬空，回落「全部」
watch(
  () => watchlist.groups.map((g) => g.id).join(','),
  () => {
    if (filterGroup.value !== ALL && !watchlist.groups.some((g) => g.id === filterGroup.value)) {
      filterGroup.value = ALL;
    }
  },
);

const filteredItems = computed(() =>
  filterGroup.value === ALL
    ? orderedItems.value
    : orderedItems.value.filter((it) => watchlist.groupsOf(it.id).some((g) => g.id === filterGroup.value)),
);

const enabledCount = computed(() => watchlist.items.filter((i) => i.ticker_enabled).length);

/**
 * 排序只在「全部」视图下可用，拖拽和 ↑/↓ 按钮一起禁用。
 *
 * 筛选视图是全局顺序的**子集**，「把筛选结果里的第 3 项拖到第 1 项」映射不回
 * 全局顺序 —— 子集内换位会写出一组与其余项冲突的 `ticker_order`，把全局顺序
 * 搅乱（不是"顺序不理想"，是真正的数据损坏）。所以这里不是"禁用拖拽、保留键盘
 * 通路"，而是两条路径一起关掉、并给出回「全部」的指引。
 *
 * 勾选在筛选下照常可用 —— 那正是分组筛选的主要用途（按分组批量开关播报）。
 */
const canDrag = computed(() => filterGroup.value === ALL);

function onReorder(ids: number[]) {
  void watchlist.reorderTicker(ids);
}

function onMove(index: number, direction: -1 | 1) {
  if (!canDrag.value) return;
  const next = moveByStep(orderedItems.value.map((i) => i.id), index, direction);
  if (!next) return; // 队首上移 / 队尾下移：跳过 IPC
  void watchlist.reorderTicker(next);
}

function toggleOne(id: number, enabled: boolean) {
  void watchlist.setTickerEnabled(id, enabled);
}

function selectAll(enabled: boolean) {
  void watchlist.setTickerEnabledBulk(filteredItems.value.map((i) => i.id), enabled);
}

// ── 单次轮播条数 ──
// 预设只列到 TICKER_ITEMS_PRESET_MAX，更大的走「自定义」输入框（上限 TICKER_ITEMS_MAX）。
const itemPresets = Array.from(
  { length: TICKER_ITEMS_PRESET_MAX - TICKER_ITEMS_MIN + 1 },
  (_, i) => {
    const n = TICKER_ITEMS_MIN + i;
    return { value: n, label: `${n} 条` };
  },
);

type ItemsMode = number | 'custom';

function deriveItemsMode(n: number): ItemsMode {
  return itemPresets.some((o) => o.value === n) ? n : 'custom';
}

// mode 是本地状态而不是从设置推导的 computed：刚点「自定义」时还没输入任何数字，
// 此时推不推导都得停在自定义态等用户填，用本地值表达最直接。
const itemsMode = ref<ItemsMode>(deriveItemsMode(settings.tickerItemsPerPage));
const customItemsText = ref(String(settings.tickerItemsPerPage));

// 反向同步：值从别处变了（另一窗口广播的设置，或提交后被夹取）时模式跟着走 ——
// itemsMode 是本地状态，不跟就会与真实值脱节。
watch(
  () => settings.tickerItemsPerPage,
  (n) => {
    itemsMode.value = deriveItemsMode(n);
    customItemsText.value = String(n);
  },
);

const itemOptions = computed(() => [
  ...itemPresets,
  { value: 'custom' as const, label: '自定义' },
]);

function onItemsModeChange(m: ItemsMode) {
  itemsMode.value = m;
  if (m === 'custom') return; // 等用户在输入框里填，不在这里改设置
  void settings.setTickerItemsPerPage(m);
}

/** 失焦/回车时提交。解析失败就回滚到当前值，不写库里一个 NaN。 */
function commitCustomItems() {
  const n = Number.parseInt(customItemsText.value, 10);
  if (Number.isNaN(n)) {
    customItemsText.value = String(settings.tickerItemsPerPage);
    return;
  }
  const clamped = clampTickerItems(n);
  customItemsText.value = String(clamped);
  if (clamped !== settings.tickerItemsPerPage) void settings.setTickerItemsPerPage(clamped);
}

/** 预览用的样例行。条数上限是 TICKER_ITEMS_MAX，所以这里必须有那么多条 ——
 *  数组短了第 5 行往后就是空白。 */
const PREVIEW_ROWS = [
  { name: '贵州茅台', price: '1486.20', pct: '+1.24%' },
  { name: '宁德时代', price: '268.44', pct: '+0.31%' },
  { name: '比亚迪', price: '312.60', pct: '+2.31%' },
  { name: '招商银行', price: '42.18', pct: '-0.52%' },
  { name: '中国平安', price: '56.72', pct: '+0.88%' },
  { name: '五粮液', price: '138.90', pct: '-1.07%' },
  { name: '隆基绿能', price: '18.63', pct: '+3.42%' },
  { name: '长江电力', price: '29.15', pct: '+0.14%' },
  { name: '万华化学', price: '78.06', pct: '-0.76%' },
  { name: '紫金矿业', price: '19.84', pct: '+1.95%' },
];

const previewRows = computed(() => PREVIEW_ROWS.slice(0, settings.tickerItemsPerPage));
</script>

<template>
  <section class="panel">
    <p class="panel-hint">桌面右下角悬浮的行情小窗：显示开关、外观与轮播范围。</p>

    <div class="card">
      <div class="card-body">
        <SettingsRow title="显示行情条" description="与系统托盘菜单里的「显示/隐藏行情条」是同一个开关">
          <NSwitch
            size="small"
            :value="settings.tickerVisible"
            aria-label="显示行情条"
            @update:value="settings.setTickerVisible($event)"
          />
        </SettingsRow>

        <SettingsRow
          title="透明背景"
          description="开启后去掉圆角底板与阴影，只留文字浮在桌面上"
        >
          <NSwitch
            size="small"
            :value="settings.tickerTransparent"
            aria-label="行情条透明背景"
            @update:value="settings.setTickerTransparent($event)"
          />
        </SettingsRow>

        <SettingsRow
          title="单次轮播条数"
          :description="`每屏同时展示几只自选的实时行情，可选 ${TICKER_ITEMS_MIN}–${TICKER_ITEMS_MAX} 条`"
        >
          <SegmentedControl
            :model-value="itemsMode"
            :options="itemOptions"
            label="单次轮播条数"
            @update:model-value="onItemsModeChange"
          />
          <input
            v-if="itemsMode === 'custom'"
            v-model="customItemsText"
            class="num-input"
            type="text"
            inputmode="numeric"
            maxlength="2"
            aria-label="自定义单次轮播条数"
            :placeholder="String(settings.tickerItemsPerPage)"
            @blur="commitCustomItems"
            @keydown.enter="commitCustomItems"
          />
        </SettingsRow>
      </div>

      <!-- 实景观感预览：透明背景是个「开了才知道什么样」的选项，
           尤其透明模式下圆角与阴影必须一起去掉，否则会剩一圈脏边。 -->
      <div class="preview-wrap">
        <div class="preview" :class="{ transparent: settings.tickerTransparent }">
          <div class="preview-pill" :class="{ bare: settings.tickerTransparent }">
            <div v-for="row in previewRows" :key="row.name" class="preview-row">
              <span class="preview-name">{{ row.name }}</span>
              <span class="preview-price up">{{ row.price }}</span>
              <span class="preview-pct up">{{ row.pct }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <h3>轮播范围</h3>
        <span class="meta">{{ enabledCount }} / {{ watchlist.items.length }} 只播报</span>
      </div>

      <div class="range-toolbar">
        <span class="toolbar-label">分组</span>
        <SegmentedControl
          :model-value="filterGroup"
          :options="filterOptions"
          label="按分组筛选"
          small
          @update:model-value="filterGroup = $event"
        />
        <div class="toolbar-actions">
          <button class="link-btn" type="button" @click="selectAll(true)">全选</button>
          <button class="link-btn" type="button" @click="selectAll(false)">全不选</button>
        </div>
      </div>

      <div v-if="watchlist.items.length === 0" class="empty-note">
        自选列表还是空的，先在主界面添加股票。
      </div>
      <div v-else-if="filteredItems.length === 0" class="empty-note">
        「{{ watchlist.groupName(filterGroup) }}」分组下暂时没有自选。
      </div>
      <div v-else class="card-body flush">
        <DragSortList
          :items="filteredItems"
          :draggable="canDrag"
          :sortable="canDrag"
          :label-of="(item) => item.name"
          @reorder="onReorder"
          @move="onMove"
        >
          <template #row="{ item }">
            <input
              class="cbx"
              type="checkbox"
              :checked="item.ticker_enabled"
              :aria-label="`${item.name} 参与轮播`"
              @change="toggleOne(item.id, ($event.target as HTMLInputElement).checked)"
            />
            <span class="dname">{{ item.name }}</span>
            <span class="dcode">{{ formatCode(item.code) }}</span>
            <span class="dmeta">{{ watchlist.groupsOf(item.id).map((g) => g.name).join(' · ') }}</span>
          </template>
        </DragSortList>
      </div>

      <p class="card-foot">
        不勾选的股票不会被播报；列表顺序就是播报顺序。
        <span v-if="!canDrag" class="warn">正在按分组筛选，此时不能拖动排序；切回「全部」即可调整播报顺序。</span>
      </p>
    </div>
  </section>
</template>

<style scoped>
/* 棋盘格底：透明与否只有在「有东西可以透过去」时才看得出来 */
.preview-wrap {
  padding: 0 var(--space-4) var(--space-3);
}
.preview {
  display: flex;
  align-items: center;
  justify-content: center;
  /* 高度跟条数走：固定 62px 只装得下 2 行，选 4 条时药丸会被
     overflow: hidden 裁掉；min-height + 上下内边距让棋盘格画布
     在药丸四周始终留一圈，条数多时按内容撑开 */
  min-height: 62px;
  padding: var(--space-2) 0;
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  overflow: hidden;
  background-color: var(--color-surface-2);
  background-image:
    linear-gradient(45deg, var(--color-surface-1) 25%, transparent 25%),
    linear-gradient(-45deg, var(--color-surface-1) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, var(--color-surface-1) 75%),
    linear-gradient(-45deg, transparent 75%, var(--color-surface-1) 75%);
  background-size: 14px 14px;
  background-position: 0 0, 0 7px, 7px -7px, -7px 0;
}
.preview-pill {
  padding: 5px 10px;
  border-radius: 7px;
  background: var(--color-surface-1);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.28), 0 0 0 0.5px rgba(255, 255, 255, 0.06);
}
/* 透明模式下底板、圆角、阴影必须一起消失 —— 只去 background 会留一圈阴影脏边 */
.preview-pill.bare {
  background: transparent;
  border-radius: 0;
  box-shadow: none;
}
.preview-row {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  line-height: 1.4;
  font-size: var(--text-xs);
}
.preview-name {
  width: 62px;
  color: var(--color-text-primary);
}
.preview-price {
  width: 46px;
  text-align: right;
  font-family: var(--font-mono);
}
.preview-pct {
  width: 48px;
  text-align: right;
  font-family: var(--font-mono);
}
.up { color: var(--color-up); }

.range-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
  padding: var(--space-3) var(--space-4) var(--space-2);
}
.toolbar-label {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.toolbar-actions {
  display: flex;
  gap: var(--space-2);
  margin-left: auto;
}
.link-btn {
  height: 24px;
  padding: 0 10px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
}
.link-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
}
.link-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
.empty-note {
  margin: 0;
  padding: var(--space-3) var(--space-4);
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
</style>
