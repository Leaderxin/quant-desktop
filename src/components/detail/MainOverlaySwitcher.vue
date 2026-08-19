<script setup lang="ts">
import type { MainOverlayType } from '@/types';

defineProps<{
  modelValue: MainOverlayType;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: MainOverlayType];
}>();

const overlays: { key: MainOverlayType; label: string }[] = [
  { key: 'MA', label: '均线' },
  { key: 'BOLL', label: 'BOLL' },
];
</script>

<template>
  <div class="main-overlay-switcher" role="tablist" aria-label="主图叠加指标切换">
    <button
      v-for="tab in overlays"
      :key="tab.key"
      role="tab"
      :aria-selected="modelValue === tab.key"
      class="switcher-tab"
      :class="{ active: modelValue === tab.key }"
      @click="emit('update:modelValue', tab.key)"
    >
      {{ tab.label }}
    </button>
  </div>
</template>

<style scoped>
.main-overlay-switcher {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
}

.switcher-tab {
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: all var(--transition-fast);
  line-height: 1.4;
}

.switcher-tab:hover {
  color: var(--color-text-secondary);
}

.switcher-tab.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
</style>
