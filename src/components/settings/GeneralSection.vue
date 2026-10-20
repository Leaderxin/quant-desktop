<script setup lang="ts">
// 通用：主题、开机自启、数据源、快捷键。
import { computed } from 'vue';
import { NSwitch, NSelect } from 'naive-ui';
import { CircleAlert, Monitor, Moon, Sun } from '@lucide/vue';
import { useSettingsStore } from '@/stores/settings';
import SettingsRow from './SettingsRow.vue';
import SegmentedControl from './SegmentedControl.vue';
import HotkeyInput from './HotkeyInput.vue';

const settings = useSettingsStore();

// 三态。顺序把「跟随系统」放最前：它是默认值，也是多数人该选的那个 ——
// 系统深色下选亮色，透明行情条上的股票名会落在深色桌面上看不清（见 issue #27）。
// 图标是显示器 / 月亮 / 太阳（lucide，与 CC Switch 的三选一同款）：一列三段扫一眼
// 就分得清「交给系统」和「钉死成哪一个」。
const themeOptions = [
  { value: 'system' as const, label: '跟随系统', icon: Monitor },
  { value: 'dark' as const, label: '暗色', icon: Moon },
  { value: 'light' as const, label: '亮色', icon: Sun },
];

const dsOptions = computed(() =>
  settings.datasources.map(([id, name]) => ({ value: id, label: name })),
);
</script>

<template>
  <section class="panel">
    <p class="panel-hint">界面主题、全局快捷键、开机自启与行情数据来源。</p>

    <div class="card">
      <div class="card-body">
        <!-- 主题模式的写入只此一处：状态栏那颗快切按钮已下线 —— 跟随系统下它
             表达不了「切到哪儿」（点一下是固定亮色还是固定暗色？），而三态在
             一颗只有日/月两个图标的按钮上放不下。 -->
        <SettingsRow
          title="主题"
          description="「跟随系统」按 Windows 的「默认应用模式」实时切换，主窗口、标题栏与行情条一起跟随"
        >
          <SegmentedControl
            :model-value="settings.themeMode"
            :options="themeOptions"
            label="主题"
            @update:model-value="settings.setThemeMode"
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
          <NSelect
            size="small"
            style="width: 120px"
            :value="settings.activeDatasource"
            :options="dsOptions"
            :input-props="{ 'aria-label': '数据源' }"
            @update:value="settings.switchDatasource"
          />
        </SettingsRow>

        <SettingsRow
          title="老板键"
          description="按下这个组合键立刻把主窗口藏起来（行情条不受影响，它由自己的开关管）。恢复用托盘图标，或点一下行情条。"
        >
          <HotkeyInput
            :model-value="settings.bossKey"
            label="老板键"
            @update:model-value="settings.setBossKey($event)"
          />
        </SettingsRow>
      </div>
    </div>

    <div v-if="settings.bossKeyError" class="note">
      <CircleAlert :size="12" aria-hidden="true" />
      <span>{{ settings.bossKeyError }}，原快捷键保持不变。</span>
    </div>
  </section>
</template>

<style scoped>
/* 无分区独有样式 —— 卡片、下拉、行布局都在 assets/styles/settings.css。
   「通用」只放真正可配置的项：刷新间隔由交易时段自动决定，放在这里当只读信息
   纯属噪音，已移除。 */
</style>
