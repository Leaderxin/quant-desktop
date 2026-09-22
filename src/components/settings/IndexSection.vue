<script setup lang="ts">
// 指数区配置：从候选池里勾选要显示在顶栏的指数，并拖拽决定左右顺序。
//
// 交互拆成「已显示 / 可添加」两段而不是一个 14 项长列表：长列表里勾选后项还在
// 原位不动，用户看不出顶栏会按什么顺序排。拆开后上半段的物理顺序 = 顶栏顺序，
// 一一对应，不需要额外的序号说明。
import { computed } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import { moveByStep } from '@/utils/dragSort';
import DragSortList from '@/components/common/DragSortList.vue';

const settings = useSettingsStore();

interface PoolEntry {
  /** 候选池里的下标。作为列表 key 与拖拽标识，跨增删保持稳定。 */
  id: number;
  code: string;
  name: string;
}

/** 代码 → 候选池条目。池外的代码（改动过候选池的老配置）兜底用代码当名字。 */
const poolByCode = computed(() => {
  const m = new Map<string, PoolEntry>();
  settings.indexPool.forEach(([code, name], i) => m.set(code, { id: i, code, name }));
  return m;
});

const displayed = computed<PoolEntry[]>(() =>
  settings.indexCodes.map((code) => poolByCode.value.get(code) ?? { id: -1, code, name: code }),
);

const available = computed<PoolEntry[]>(() =>
  settings.indexPool
    .map(([code, name], i) => ({ id: i, code, name }))
    .filter((e) => !settings.indexCodes.includes(e.code)),
);

function add(code: string) {
  void settings.setIndexCodes([...settings.indexCodes, code]);
}

function remove(code: string) {
  // 至少留一个：全清空后顶栏会退回「等待指数数据...」占位，看起来像故障
  if (settings.indexCodes.length <= 1) return;
  void settings.setIndexCodes(settings.indexCodes.filter((c) => c !== code));
}

function codeOf(ids: number[]): string[] {
  return ids
    .map((id) => settings.indexPool[id]?.[0])
    .filter((c): c is string => typeof c === 'string');
}

function onReorder(ids: number[]) {
  void settings.setIndexCodes(codeOf(ids));
}

function onMove(index: number, direction: -1 | 1) {
  const next = moveByStep(settings.indexCodes, index, direction);
  if (!next) return; // 队首上移 / 队尾下移：跳过 IPC
  void settings.setIndexCodes(next);
}
</script>

<template>
  <section class="panel">
    <header class="panel-head">
      <h2>指数区</h2>
      <p>挑选显示在顶栏的指数，拖动决定先后顺序。可选 {{ settings.indexPool.length }} 个主流指数。</p>
    </header>

    <div class="card">
      <div class="card-head">
        <h3>已显示</h3>
        <span class="meta">{{ displayed.length }} / {{ settings.indexPool.length }}</span>
      </div>
      <div class="card-body flush">
        <DragSortList
          :items="displayed"
          :label-of="(item) => item.name"
          @reorder="onReorder"
          @move="onMove"
        >
          <template #row="{ item }">
            <input
              class="cbx"
              type="checkbox"
              checked
              :aria-label="`隐藏 ${item.name}`"
              :disabled="displayed.length <= 1"
              @change="remove(item.code)"
            />
            <span class="dname">{{ item.name }}</span>
            <span class="dcode">{{ item.code.replace(/^s_/, '') }}</span>
          </template>
        </DragSortList>
      </div>
      <p class="card-foot">
        列表顺序就是顶栏从左到右的顺序，也可用键盘 Alt + ↑ / ↓ 调整。
        <span v-if="displayed.length <= 1" class="warn">至少保留一个指数。</span>
      </p>
    </div>

    <div class="card">
      <div class="card-head">
        <h3>可添加</h3>
        <span class="meta">{{ available.length }} 项</span>
      </div>
      <div v-if="available.length > 0" class="chips">
        <button
          v-for="e in available"
          :key="e.code"
          class="chip"
          type="button"
          @click="add(e.code)"
        >＋ {{ e.name }}</button>
      </div>
      <p v-else class="empty-note">所有指数都已显示在顶栏。</p>
    </div>
  </section>
</template>

<style scoped>
/* 卡片、列表行、复选框等共用样式在 assets/styles/settings.css（以 .settings-page
   为祖先选择器），这里只放本分区独有的「可添加」芯片。 */
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
.empty-note {
  margin: 0;
  padding: var(--space-3) var(--space-4);
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
</style>
