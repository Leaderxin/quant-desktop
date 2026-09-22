<script setup lang="ts">
// 设置页的表单行：左侧是标题 + 常驻说明，右侧是控件。
//
// 说明文字一律常驻（不用 placeholder / tooltip 承载）—— 设置项一旦保存就没法
// 从界面上反推它的含义，说明必须一直看得见。
defineProps<{
  title: string;
  description?: string;
  /** 只读/不可用项：降低对比度并禁用指针交互，与「可点但没反应」区分开。 */
  disabled?: boolean;
}>();
</script>

<template>
  <div class="settings-row" :class="{ disabled }">
    <div class="row-label">
      <div class="row-title">{{ title }}</div>
      <div v-if="description" class="row-desc">{{ description }}</div>
    </div>
    <div class="row-control">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: 40px;
  padding: var(--space-2) 0;
}
/* 行之间才画分割线；同一分区由卡片分组区隔，卡片内部连续的行不画线会糊成一片 */
.settings-row + .settings-row {
  border-top: 1px solid var(--color-border-0);
}
.row-label {
  min-width: 0;
}
.row-title {
  font-size: var(--text-sm);
  color: var(--color-text-primary);
}
.row-desc {
  margin-top: 1px;
  font-size: 11px;
  line-height: 1.6;
  color: var(--color-text-tertiary);
}
.row-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}
.settings-row.disabled .row-title {
  color: var(--color-text-tertiary);
}
.settings-row.disabled .row-control {
  opacity: 0.4;
  pointer-events: none;
}
</style>
