<script setup lang="ts">
// 通用：主题、开机自启、数据源，以及刷新策略的只读说明。
import { computed } from 'vue';
import { NSwitch } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import SettingsRow from './SettingsRow.vue';
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
    <p class="panel-hint">界面主题、开机自启与行情数据来源。</p>

    <div class="card">
      <div class="card-body">
        <SettingsRow title="主题" description="和底部状态栏里的主题按钮是同一个设置">
          <SegmentedControl
            :model-value="settings.theme"
            :options="themeOptions"
            label="主题"
            @update:model-value="onThemeChange"
          />
        </SettingsRow>

        <SettingsRow
          title="开机自启"
          description="开机进入 Windows 后自动打开本应用"
        >
          <NSwitch
            size="small"
            :value="settings.autoLaunch"
            aria-label="开机自启"
            @update:value="settings.toggleAutoLaunch()"
          />
        </SettingsRow>

        <SettingsRow title="数据源" description="行情数据来自哪家服务商，和顶部栏的下拉是同一个设置；行情异常时可换一家试试">
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
