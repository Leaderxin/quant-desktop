<script setup lang="ts">
import { computed } from 'vue';
import { useMessage } from 'naive-ui';
import type { WatchGroup } from '@/types';
import { Check } from '@lucide/vue';

/**
 * 分组多选（药丸按钮）。「添加自选」和「导入自选」两处都要选落点，
 * 共用一份而不是各写一套 —— 尤其「至少保留一个」这条规则，写两遍迟早
 * 只有一处记得改。
 */
const props = withDefaults(defineProps<{
  groups: WatchGroup[];
  modelValue: number[];
  /** 无障碍名称。视觉上的标题由调用方渲染，这里只管 role="group" 念什么 */
  label?: string;
  /** 常驻说明。空串表示不显示 */
  hint?: string;
}>(), {
  label: '目标分组',
  hint: '',
});

const emit = defineEmits<{ (e: 'update:modelValue', ids: number[]): void }>();
const message = useMessage();

const selected = computed(() => props.groups.filter((g) => props.modelValue.includes(g.id)));

/**
 * 勾选/取消一个分组。至少保留一个 —— 自选必须属于某个分组，全不选意味着
 * 一只看不见的股票；后端在这种输入下会兜底到默认分组，但那与用户勾的动作不符。
 */
function toggle(id: number) {
  if (props.modelValue.includes(id)) {
    if (selected.value.length <= 1) {
      message.warning('至少要保留一个分组');
      return;
    }
    emit('update:modelValue', props.modelValue.filter((x) => x !== id));
  } else {
    emit('update:modelValue', [...props.modelValue, id]);
  }
}
</script>

<template>
  <div class="group-picker">
    <div class="group-chips" role="group" :aria-label="label">
      <button
        v-for="g in groups"
        :key="g.id"
        type="button"
        class="group-chip"
        :class="{ active: modelValue.includes(g.id) }"
        :aria-pressed="modelValue.includes(g.id)"
        @click="toggle(g.id)"
      >
        <Check v-if="modelValue.includes(g.id)" :size="12" aria-hidden="true" />
        <span class="group-chip-name">{{ g.name }}</span>
      </button>
    </div>
    <p v-if="hint" class="group-hint">{{ hint }}</p>
  </div>
</template>

<style scoped>
.group-picker {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.group-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  /* 分组多时封顶并滚动：对话框不能因为分组长到把下面的内容挤出屏幕 */
  max-height: 92px;
  overflow-y: auto;
  padding: 1px;
}
.group-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-full);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast),
    color var(--transition-fast);
}
.group-chip:hover {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}
/* 选中态不只靠颜色：勾号图标同时出现 */
.group-chip.active {
  border-color: var(--color-accent);
  background: var(--color-accent-dim);
  color: var(--color-accent);
}
.group-chip:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
.group-chip-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-hint {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-tertiary);
}
</style>
