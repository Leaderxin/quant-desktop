<script setup lang="ts">
// 自选列表：表格列、默认排序、涨跌配色。
// 分组本身在自选表表头的标签栏里维护，这里不重复一套入口。
import { computed } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import {
  ALL_COLUMNS,
  REQUIRED_COLUMNS,
  type ColumnKey,
} from '@/utils/prefs';
import { moveByStep } from '@/utils/dragSort';
import SettingsRow from './SettingsRow.vue';
import SegmentedControl from './SegmentedControl.vue';
import DragSortList from '@/components/common/DragSortList.vue';

const settings = useSettingsStore();

/** 列在 ALL_COLUMNS 里的下标，作为拖拽列表的稳定 key（列集合是编译期常量）。 */
const colIndex = new Map<string, number>(ALL_COLUMNS.map((c, i) => [c.key, i]));

const columnItems = computed(() =>
  settings.watchlistColumns.map((key) => ({
    id: colIndex.get(key) ?? -1,
    key,
    label: ALL_COLUMNS.find((c) => c.key === key)?.label ?? key,
    required: REQUIRED_COLUMNS.includes(key),
  })),
);

/** 未显示的列。它们只是「可以加回来」的候选，混进上面的排序列表会和
 *  「当前列序」混淆，所以拆成独立一段（与指数区同一套做法）。 */
const hiddenColumns = computed(() =>
  ALL_COLUMNS.filter((c) => !settings.watchlistColumns.includes(c.key)),
);

function toggleColumn(key: ColumnKey, visible: boolean) {
  if (visible) {
    // 追加到末尾：新开一列通常是想看它，放末尾不会把已有布局整体挤位
    void settings.setWatchlistColumns([...settings.watchlistColumns, key]);
  } else {
    if (REQUIRED_COLUMNS.includes(key)) return;
    void settings.setWatchlistColumns(settings.watchlistColumns.filter((k) => k !== key));
  }
}

function onColumnReorder(ids: number[]) {
  const keys = ids
    .map((id) => ALL_COLUMNS[id]?.key)
    .filter((k): k is ColumnKey => k !== undefined);
  void settings.setWatchlistColumns(keys);
}

function onColumnMove(index: number, direction: -1 | 1) {
  const next = moveByStep(settings.watchlistColumns, index, direction);
  if (!next) return; // 队首上移 / 队尾下移：跳过 IPC
  void settings.setWatchlistColumns(next);
}

// ── 默认排序 ──

const sortKey = computed(() => settings.watchlistDefaultSort?.key ?? '');
const sortOrder = computed(() => settings.watchlistDefaultSort?.order ?? 'descend');

function onSortKeyChange(key: string) {
  if (key === '') {
    void settings.setWatchlistDefaultSort(null);
    return;
  }
  void settings.setWatchlistDefaultSort({ key: key as ColumnKey, order: sortOrder.value });
}

function onSortOrderChange(order: 'ascend' | 'descend') {
  const key = sortKey.value;
  if (!key) return;
  void settings.setWatchlistDefaultSort({ key: key as ColumnKey, order });
}

const orderOptions = [
  { value: 'descend' as const, label: '降序' },
  { value: 'ascend' as const, label: '升序' },
];
</script>

<template>
  <section class="panel">
    <header class="panel-head">
      <h2>自选列表</h2>
      <p>自选表显示哪些列、打开时的默认排序与涨跌配色。分组的管理在自选表上方的分组标签栏。</p>
    </header>

    <div class="card">
      <div class="card-head">
        <h3>显示列</h3>
        <span class="meta">{{ columnItems.length }} / {{ ALL_COLUMNS.length }}</span>
      </div>
      <div class="card-body flush">
        <DragSortList
          :items="columnItems"
          :label-of="(item) => item.label"
          @reorder="onColumnReorder"
          @move="onColumnMove"
        >
          <template #row="{ item }">
            <input
              class="cbx"
              type="checkbox"
              :checked="true"
              :disabled="item.required"
              :aria-label="`显示 ${item.label} 列`"
              @change="toggleColumn(item.key, false)"
            />
            <span class="dname">{{ item.label }}</span>
            <span v-if="item.required" class="dmeta">必有</span>
          </template>
        </DragSortList>
      </div>
      <p class="card-foot">
        列表顺序就是列的显示顺序。代码和名称两列不能隐藏，但可以拖动换位。
      </p>

      <!-- 未显示的列以芯片列出：它们是「可加回来」的候选，混在排序列表里
           会和「当前列序」混淆，所以拆成独立一段（与指数区同一套做法）。 -->
      <template v-if="hiddenColumns.length > 0">
        <div class="card-head sub">
          <h3>未显示</h3>
          <span class="meta">{{ hiddenColumns.length }} 项</span>
        </div>
        <div class="chips">
          <button
            v-for="c in hiddenColumns"
            :key="c.key"
            class="chip"
            type="button"
            @click="toggleColumn(c.key, true)"
          >＋ {{ c.label }}</button>
        </div>
      </template>
    </div>

    <div class="card">
      <div class="card-body">
        <SettingsRow title="默认排序字段" description="打开应用时自选表默认的排列方式">
          <select
            class="select"
            :value="sortKey"
            aria-label="默认排序字段"
            @change="onSortKeyChange(($event.target as HTMLSelectElement).value)"
          >
            <option value="">不排序（自选顺序）</option>
            <option v-for="c in ALL_COLUMNS" :key="c.key" :value="c.key">{{ c.label }}</option>
          </select>
          <SegmentedControl
            :model-value="sortOrder"
            :options="orderOptions"
            label="排序方向"
            small
            :class="{ inactive: !sortKey }"
            @update:model-value="onSortOrderChange"
          />
        </SettingsRow>
      </div>
    </div>

    <div class="card">
      <div class="card-head"><h3>涨跌配色</h3></div>
      <div class="card-body">
        <p class="row-hint">A 股是红涨绿跌，欧美市场相反，按自己的看盘习惯选。切换后表格、图表、指数卡会一起更换。</p>
        <div class="color-cards">
          <button
            v-for="opt in [
              { key: 'cn' as const, label: '红涨绿跌（A 股）' },
              { key: 'us' as const, label: '绿涨红跌（欧美）' },
            ]"
            :key="opt.key"
            class="color-card"
            type="button"
            role="radio"
            :aria-checked="settings.colorScheme === opt.key"
            :class="{ on: settings.colorScheme === opt.key }"
            @click="settings.setColorScheme(opt.key)"
          >
            <span class="cc-top">
              <span class="cc-radio" aria-hidden="true"></span>
              {{ opt.label }}
            </span>
            <span class="cc-preview">
              <span class="up">+1.24%</span>
              <span class="down">-0.87%</span>
            </span>
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.card-head.sub {
  border-top: 1px solid var(--color-border-0);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
}
.chip {
  display: inline-flex;
  align-items: center;
  height: 26px;
  padding: 0 10px;
  border: 1px dashed var(--color-border-1);
  border-radius: var(--radius-full);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: border-color var(--transition-fast), color var(--transition-fast);
}
.chip:hover {
  border-style: solid;
  border-color: var(--color-accent);
  color: var(--color-accent);
}
.chip:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}
.row-hint {
  margin: 0 0 var(--space-3);
  font-size: var(--text-xs);
  line-height: 1.6;
  color: var(--color-text-tertiary);
}
/* 「不排序」时方向选择没有意义，降对比度而非禁用 —— 它仍会在选了字段后立刻可用 */
.inactive {
  opacity: 0.4;
  pointer-events: none;
}

.color-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}
.color-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  background: var(--color-surface-0);
  cursor: pointer;
  text-align: left;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.color-card:hover {
  border-color: var(--color-border-1);
}
.color-card.on {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 1px var(--color-accent);
}
.color-card:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}
.cc-top {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-sm);
  color: var(--color-text-primary);
}
.cc-radio {
  position: relative;
  width: 13px;
  height: 13px;
  flex-shrink: 0;
  border: 1.5px solid var(--color-border-1);
  border-radius: 50%;
}
.color-card.on .cc-radio {
  border-color: var(--color-accent);
}
.color-card.on .cc-radio::after {
  content: '';
  position: absolute;
  inset: 2px;
  border-radius: 50%;
  background: var(--color-accent);
}
.cc-preview {
  display: flex;
  gap: var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
</style>
