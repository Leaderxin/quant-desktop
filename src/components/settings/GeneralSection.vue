<script setup lang="ts">
// 通用：主题、开机自启、数据源，以及刷新策略的只读说明。
import { computed } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import SettingsRow from './SettingsRow.vue';
import ToggleSwitch from './ToggleSwitch.vue';
import SegmentedControl from './SegmentedControl.vue';

const settings = useSettingsStore();

const themeOptions = [
  { value: 'dark' as const, label: '暗色' },
  { value: 'light' as const, label: '亮色' },
];

// 主题切换走 store 的 toggleTheme（它会广播 theme-changed 给行情条窗口）。
// 这里先判断当前值，避免「点已选中的那一项」把主题翻过去。
function onThemeChange(t: 'dark' | 'light') {
  if (t !== settings.theme) void settings.toggleTheme();
}

const dsOptions = computed(() =>
  settings.datasources.map(([id, name]) => ({ value: id, label: name })),
);
</script>

<template>
  <section class="panel">
    <header class="panel-head">
      <h2>通用</h2>
      <p>主题、启动方式与数据源。</p>
    </header>

    <div class="card">
      <div class="card-body">
        <SettingsRow title="主题" description="与状态栏的主题按钮双向同步">
          <SegmentedControl
            :model-value="settings.theme"
            :options="themeOptions"
            label="主题"
            @update:model-value="onThemeChange"
          />
        </SettingsRow>

        <SettingsRow
          title="开机自启"
          description="Windows 商店版走 StartupTask，其余安装方式走注册表 Run 键"
        >
          <ToggleSwitch
            :model-value="settings.autoLaunch"
            label="开机自启"
            @update:model-value="settings.toggleAutoLaunch()"
          />
        </SettingsRow>

        <SettingsRow title="数据源" description="与顶栏的数据源下拉双向同步">
          <select
            class="select"
            style="min-width: 110px"
            :value="settings.activeDatasource"
            aria-label="数据源"
            @change="settings.switchDatasource(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="o in dsOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
        </SettingsRow>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* 无分区独有样式 —— 卡片、下拉、行布局都在 assets/styles/settings.css。
   「通用」只放真正可配置的项：刷新间隔由交易时段自动决定，放在这里当只读信息
   纯属噪音，已移除。 */
</style>
