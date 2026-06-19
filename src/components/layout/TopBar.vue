<script setup lang="ts">
import { computed } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import { NButton, NIcon, NDropdown, NTooltip } from 'naive-ui';

const settings = useSettingsStore();

const dsDisplayName = computed(() => {
  const found = settings.datasources.find(([id]) => id === settings.activeDatasource);
  return found ? found[1] : settings.activeDatasource;
});

const dsOptions = computed(() =>
  settings.datasources.map(([id, name]) => ({
    label: name,
    key: id,
  }))
);

function handleDsSelect(key: string) {
  settings.switchDatasource(key);
}
</script>

<template>
  <header class="top-bar">
    <div class="top-left">
      <n-dropdown
        trigger="click"
        :options="dsOptions"
        @select="handleDsSelect"
      >
        <n-tooltip>
          <template #trigger>
            <span class="ds-tag">
              <span class="ds-label">{{ dsDisplayName }}</span>
              <n-icon :size="12" class="ds-swap-icon">
                <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M7 3 3 7l4 4" />
                  <path d="M17 11v1a4 4 0 0 1-4 4H3" />
                  <path d="M13 17 17 13l-4-4" />
                  <path d="M3 9V8a4 4 0 0 1 4-4h10" />
                </svg>
              </n-icon>
            </span>
          </template>
          点击切换数据源
        </n-tooltip>
      </n-dropdown>
    </div>
    <div class="top-right">
      <n-tooltip>
        <template #trigger>
          <n-button
            text
            size="small"
            @click="settings.toggleTheme()"
            class="theme-toggle"
            :aria-label="settings.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'"
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
        </template>
        点击切换主题
      </n-tooltip>
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

.ds-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-size: var(--text-xs);
  cursor: pointer;
  user-select: none;
  transition: background var(--transition-fast), color var(--transition-fast);
  -webkit-app-region: no-drag;
}
.ds-tag:hover {
  filter: brightness(1.4);
}
.ds-label {
  line-height: 1;
}
.ds-swap-icon {
  color: inherit;
  flex-shrink: 0;
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
