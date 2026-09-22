<script setup lang="ts" generic="T extends { id: number }">
// src/components/common/DragSortList.vue
//
// 可拖拽排序的列表壳。设置页里指数区、显示列、轮播范围三处共用 —— 它们的行内容
// 差别很大（勾选框 / 名称 / 代码 / 所属分组 / 删除按钮），但排序交互完全一样，
// 所以把拖动、落点指示、上下移按钮收在这里，行内容交给 slot。
//
// 键盘可达性：拖拽本身对键盘用户不可用，所以每行都带 ↑/↓ 按钮（首行禁用上移、
// 末行禁用下移），并支持 Alt+↑/↓。拖拽是快捷方式，不是唯一路径。
import { ref } from 'vue';
import { computeDropTarget, moveItem } from '@/utils/dragSort';

const props = withDefaults(defineProps<{
  items: T[];
  /** 是否允许拖动。筛选视图下为 false —— 见 TickerSection 的说明。 */
  draggable?: boolean;
  /** 是否显示 ↑/↓ 按钮（始终可用的排序路径）。 */
  sortable?: boolean;
  /** 拖拽手柄的无障碍说明前缀，如「上证指数」。 */
  labelOf?: (item: T, index: number) => string;
}>(), {
  draggable: true,
  sortable: true,
});

const emit = defineEmits<{
  (e: 'reorder', ids: number[]): void;
  /** 单步移动请求（键盘/按钮路径），由调用方决定调哪个 IPC。 */
  (e: 'move', index: number, direction: -1 | 1): void;
}>();

const dragIndex = ref<number | null>(null);
const overIndex = ref<number | null>(null);
const dropAfter = ref(false);

function reorder(from: number, to: number) {
  emit('reorder', moveItem(props.items.map((it) => it.id), from, to));
}

function onDragStart(index: number, e: DragEvent) {
  if (!props.draggable) return;
  dragIndex.value = index;
  // 不设 dataTransfer 的话 Firefox/部分 webview 不会启动拖拽
  e.dataTransfer?.setData('text/plain', String(props.items[index].id));
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
}

function onDragOver(index: number, e: DragEvent) {
  if (!props.draggable || dragIndex.value === null) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
  if (index === dragIndex.value) {
    overIndex.value = null;
    return;
  }
  overIndex.value = index;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  // 落在行的下半部分 → 插到它后面。没有这个判断的话只能往上拖，
  // 因为「插到目标之前」永远无法表达「移到列表末尾」。
  dropAfter.value = e.clientY > rect.top + rect.height / 2;
}

function onDrop() {
  if (dragIndex.value === null || overIndex.value === null) return;
  const from = dragIndex.value;
  const to = computeDropTarget(from, overIndex.value, dropAfter.value);
  if (from !== to) reorder(from, to);
  onDragEnd();
}

function onDragEnd() {
  dragIndex.value = null;
  overIndex.value = null;
  dropAfter.value = false;
}

function onKeydown(index: number, e: KeyboardEvent) {
  if (!e.altKey) return;
  if (e.key === 'ArrowUp' && index > 0) {
    e.preventDefault();
    emit('move', index, -1);
  } else if (e.key === 'ArrowDown' && index < props.items.length - 1) {
    e.preventDefault();
    emit('move', index, 1);
  }
}

defineExpose({ onDragEnd });
</script>

<template>
  <div class="drag-list" role="list">
    <div
      v-for="(item, i) in items"
      :key="item.id"
      class="drag-row"
      :class="{
        dragging: dragIndex === i,
        'drop-before': overIndex === i && !dropAfter,
        'drop-after': overIndex === i && dropAfter,
      }"
      role="listitem"
      :draggable="draggable"
      @dragstart="onDragStart(i, $event)"
      @dragover="onDragOver(i, $event)"
      @drop.prevent="onDrop"
      @dragend="onDragEnd"
      @keydown="onKeydown(i, $event)"
      tabindex="0"
    >
      <span
        v-if="draggable"
        class="drag-handle"
        :title="draggable ? '拖动排序' : '当前视图下不支持拖动排序'"
        aria-hidden="true"
      >
        <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor">
          <circle cx="5.5" cy="4" r="1.2"/><circle cx="10.5" cy="4" r="1.2"/>
          <circle cx="5.5" cy="8" r="1.2"/><circle cx="10.5" cy="8" r="1.2"/>
          <circle cx="5.5" cy="12" r="1.2"/><circle cx="10.5" cy="12" r="1.2"/>
        </svg>
      </span>

      <slot name="row" :item="item" :index="i" />

      <template v-if="sortable">
        <button
          class="drag-move"
          type="button"
          :disabled="i === 0"
          :aria-label="`上移${labelOf ? labelOf(item, i) : ''}`"
          title="上移（Alt+↑）"
          @click="emit('move', i, -1)"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 9 8 5 12 9"/></svg>
        </button>
        <button
          class="drag-move"
          type="button"
          :disabled="i === items.length - 1"
          :aria-label="`下移${labelOf ? labelOf(item, i) : ''}`"
          title="下移（Alt+↓）"
          @click="emit('move', i, 1)"
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 5 8 9 12 5"/></svg>
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.drag-list {
  display: flex;
  flex-direction: column;
}
.drag-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 32px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
}
.drag-row:hover {
  background: var(--color-bg-elevated);
}
.drag-row:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}
.drag-row.dragging {
  opacity: 0.4;
}
/* 落点指示：用 inset 阴影而不是 border —— border 会改变行高，
   拖动过程中整列内容跟着抖。 */
.drag-row.drop-before {
  box-shadow: inset 0 2px 0 var(--color-accent);
}
.drag-row.drop-after {
  box-shadow: inset 0 -2px 0 var(--color-accent);
}

.drag-handle {
  display: flex;
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  cursor: grab;
}
.drag-handle:active {
  cursor: grabbing;
}
.drag-row[draggable='false'] .drag-handle {
  cursor: not-allowed;
  opacity: 0.35;
}

.drag-move {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  flex-shrink: 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.drag-move:hover:not(:disabled) {
  background: var(--color-surface-2);
  color: var(--color-text-primary);
}
/* 段末按钮必须看起来就不可点：opacity + not-allowed，不只靠变淡 */
.drag-move:disabled {
  opacity: 0.25;
  cursor: not-allowed;
}
</style>
