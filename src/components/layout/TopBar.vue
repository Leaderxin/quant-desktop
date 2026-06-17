<script setup lang="ts">
import { useSettingsStore } from '@/stores/settings';
import { NButton, NTag } from 'naive-ui';

const settings = useSettingsStore();

function handleToggleTheme() {
  settings.toggleTheme();
}
</script>

<template>
  <div class="top-bar">
    <div class="top-left">
      <span class="app-title">QuantDesktop</span>
      <n-tag size="small" :bordered="false" type="info">
        {{ settings.activeDatasource }}
      </n-tag>
    </div>
    <div class="top-right">
      <n-button text @click="handleToggleTheme" class="theme-btn">
        {{ settings.theme === 'dark' ? '☀' : '☾' }}
      </n-button>
    </div>
  </div>
</template>

<style scoped>
.top-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  background-color: var(--color-header-bg);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.top-left {
  display: flex;
  align-items: center;
  gap: 12px;
}
.app-title {
  font-weight: 700;
  font-size: 16px;
  color: var(--color-text-primary);
}
.top-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.theme-btn {
  font-size: 18px;
}
</style>
