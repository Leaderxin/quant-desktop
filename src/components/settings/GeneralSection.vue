<script setup lang="ts">
// 通用：主题、开机自启、数据源、快捷键与 K 线均线。
import { computed } from 'vue';
import { NSwitch, NSelect, NInputNumber, NButton, NForm, NFormItem } from 'naive-ui';
import { CircleAlert, Monitor, Moon, Sun, Plus, Trash2 } from '@lucide/vue';
import { useSettingsStore } from '@/stores/settings';
import {
  MAX_MOVING_AVERAGE_COUNT,
  MAX_MOVING_AVERAGE_PERIOD,
  MIN_MOVING_AVERAGE_PERIOD,
  nextMovingAveragePeriod,
} from '@/utils/movingAverage';
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

function setMovingAveragePeriod(index: number, period: number | null) {
  if (period === null) return;
  const next = [...settings.movingAveragePeriods];
  next[index] = period;
  void settings.setMovingAveragePeriods(next);
}

function addMovingAverage() {
  if (settings.movingAveragePeriods.length >= MAX_MOVING_AVERAGE_COUNT) return;
  void settings.setMovingAveragePeriods([
    ...settings.movingAveragePeriods,
    nextMovingAveragePeriod(settings.movingAveragePeriods),
  ]);
}

function removeMovingAverage(index: number) {
  if (settings.movingAveragePeriods.length <= 1) return;
  void settings.setMovingAveragePeriods(settings.movingAveragePeriods.filter((_, i) => i !== index));
}
</script>

<template>
  <section class="panel">
    <p class="panel-hint">界面主题、K 线均线、全局快捷键、开机自启与行情数据来源。</p>

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

        <SettingsRow title="数据源" description="主数据源与备用源互为备份，和顶部栏的下拉是同一个设置；行情异常时可换一个试试">
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
          class="ma-settings-row"
          title="主图均线"
          description="周期按 K 线根数计算；调整后图表立即更新"
        >
          <div class="ma-settings-control">
            <NForm
              class="ma-form"
              size="small"
              label-placement="left"
              label-width="42px"
              :show-feedback="false"
              @submit.prevent
            >
              <div class="ma-period-grid">
                <NFormItem
                  v-for="(period, index) in settings.movingAveragePeriods"
                  :key="index"
                  class="ma-period-field"
                  :label="'MA' + (index + 1)"
                >
                  <NInputNumber
                    style="width: 88px"
                    :value="period"
                    :min="MIN_MOVING_AVERAGE_PERIOD"
                    :max="MAX_MOVING_AVERAGE_PERIOD"
                    :precision="0"
                    :show-button="false"
                    :input-props="{ 'aria-label': 'MA' + (index + 1) + ' 周期' }"
                    @update:value="setMovingAveragePeriod(index, $event)"
                  />
                  <NButton
                    quaternary
                    circle
                    :disabled="settings.movingAveragePeriods.length <= 1"
                    :aria-label="'删除 MA' + (index + 1)"
                    :title="'删除 MA' + (index + 1)"
                    @click="removeMovingAverage(index)"
                  >
                    <template #icon><Trash2 :size="14" /></template>
                  </NButton>
                </NFormItem>
              </div>
              <div class="ma-form-actions">
                <NButton
                  quaternary
                  circle
                  :disabled="settings.movingAveragePeriods.length >= MAX_MOVING_AVERAGE_COUNT"
                  aria-label="添加均线"
                  title="添加均线"
                  @click="addMovingAverage"
                >
                  <template #icon><Plus :size="14" /></template>
                </NButton>
              </div>
            </NForm>
            <span class="ma-period-count">{{ settings.movingAveragePeriods.length }} / {{ MAX_MOVING_AVERAGE_COUNT }}</span>
          </div>
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
/* 均线控件需要在桌面和窄窗口下都能容纳最多十个周期输入框。 */
.ma-settings-control {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 4px;
  min-width: 0;
  width: 100%;
}

.ma-form {
  width: 100%;
}

.ma-period-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 4px 12px;
  width: 100%;
}

.ma-period-count {
  align-self: flex-end;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
}

.ma-form-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 2px;
}

:deep(.ma-period-field.n-form-item) {
  margin-bottom: 0;
}

:deep(.ma-period-field .n-form-item-blank) {
  display: flex;
  align-items: center;
  gap: 4px;
}

:deep(.settings-row.ma-settings-row) {
  align-items: flex-start;
}

:deep(.ma-settings-row .row-control) {
  flex: 1;
  justify-content: flex-end;
}

@media (max-width: 680px) {
  :deep(.settings-row.ma-settings-row) {
    flex-direction: column;
    align-items: stretch;
  }

  .ma-settings-control {
    align-items: flex-start;
  }

  .ma-form {
    width: 100%;
  }
}
</style>
