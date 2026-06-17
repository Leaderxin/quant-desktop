<script setup lang="ts">
import { useSettingsStore } from '@/stores/settings';
import { NButton, NTag } from 'naive-ui';

const settings = useSettingsStore();
</script>

<template>
  <header class="top-bar">
    <div class="top-left">
      <div class="brand">
        <svg class="brand-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/>
          <polyline points="16 7 22 7 22 13"/>
        </svg>
        <span class="brand-name">QuantDesktop</span>
      </div>
      <n-tag size="tiny" :bordered="false" type="info" class="ds-tag">
        {{ settings.activeDatasource }}
      </n-tag>
    </div>
    <div class="top-right">
      <n-button
        text
        size="small"
        @click="settings.toggleTheme()"
        class="theme-toggle"
      >
        <template #icon>
          <svg v-if="settings.theme === 'dark'" viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
            <path fill-rule="evenodd" d="M7.455 2.004a.75.75 0 01.26.77 7 7 0 009.958 7.967.75.75 0 011.067.853A8.5 8.5 0 116.647 1.921a.75.75 0 01.808.083z" clip-rule="evenodd"/>
          </svg>
          <svg v-else viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
            <path d="M10 2a.75.75 0 01.75.75v1.5a.75.75 0 01-1.5 0v-1.5A.75.75 0 0110 2zM10 15a.75.75 0 01.75.75v1.5a.75.75 0 01-1.5 0v-1.5A.75.75 0 0110 15zM10 7a3 3 0 100 6 3 3 0 000-6zM15.657 5.404a.75.75 0 10-1.06-1.06l-1.061 1.06a.75.75 0 001.06 1.06l1.06-1.06zM6.464 14.596a.75.75 0 10-1.06-1.06l-1.061 1.06a.75.75 0 001.06 1.06l1.06-1.06zM18 10a.75.75 0 01-.75.75h-1.5a.75.75 0 010-1.5h1.5A.75.75 0 0118 10zM5 10a.75.75 0 01-.75.75h-1.5a.75.75 0 010-1.5h1.5A.75.75 0 015 10zM14.596 15.657a.75.75 0 001.06-1.06l-1.06-1.061a.75.75 0 10-1.06 1.06l1.06 1.06zM5.404 6.464a.75.75 0 001.06-1.06l-1.06-1.06a.75.75 0 10-1.061 1.06l1.06 1.06z"/>
          </svg>
        </template>
      </n-button>
    </div>
  </header>
</template>

<style scoped>
.top-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: var(--header-height);
  padding: 0 var(--space-4);
  background: var(--color-surface-1);
  border-bottom: 1px solid var(--color-border-0);
  flex-shrink: 0;
  -webkit-app-region: drag;
}
.top-left {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.brand-icon {
  width: 18px;
  height: 18px;
  color: var(--color-accent);
}
.brand-name {
  font-weight: var(--font-weight-semibold);
  font-size: var(--text-md);
  color: var(--color-text-primary);
  letter-spacing: -0.01em;
}
.ds-tag {
  font-size: var(--text-xs) !important;
  text-transform: uppercase;
  opacity: 0.8;
}
.top-right {
  display: flex;
  align-items: center;
  -webkit-app-region: no-drag;
}
.theme-toggle {
  color: var(--color-text-secondary);
}
.theme-toggle:hover {
  color: var(--color-text-primary);
}
</style>
