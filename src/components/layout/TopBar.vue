<script setup lang="ts">
import { computed } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import { NIcon, NDropdown, NTooltip } from 'naive-ui';

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
    <!-- Slogan -->
    <span class="brand-slogan">实时行情 · 多源切换 · 免费高效</span>

    <!-- Right: Data source switcher -->
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
  </header>
</template>

<style scoped>
.top-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--header-height);
  padding: 0 var(--space-4);
  background: var(--color-surface-1);
  border-bottom: 1px solid var(--color-border-0);
  flex-shrink: 0;
  -webkit-app-region: drag;
}

/* ── Slogan ── */
.brand-slogan {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  white-space: nowrap;
  letter-spacing: 0.02em;
}

/* ── Data source tag ── */
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
</style>
