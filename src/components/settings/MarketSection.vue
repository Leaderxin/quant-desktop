<script setup lang="ts">
// 市场概览：面板显示开关 + 板块榜单条数。
import { ref, watch } from 'vue';
import { NSwitch } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { clampTopN, SECTOR_TOP_N_MAX, SECTOR_TOP_N_MIN } from '@/utils/prefs';
import SettingsRow from './SettingsRow.vue';
import SegmentedControl from './SegmentedControl.vue';
import { CircleAlert } from '@lucide/vue';

const settings = useSettingsStore();

type TopNMode = '5' | '10' | 'custom';

function deriveMode(n: number): TopNMode {
  return n === 5 ? '5' : n === 10 ? '10' : 'custom';
}

// mode 是本地状态而不是从 sectorTopN 推导的 computed:点「自定义」时还没输入
// 任何数字，此时推不推导都得停在自定义态等用户填，用本地值表达最直接。
const mode = ref<TopNMode>(deriveMode(settings.sectorTopN));
const customText = ref(String(settings.sectorTopN));

watch(
  () => settings.sectorTopN,
  (n) => {
    mode.value = deriveMode(n);
    customText.value = String(n);
  },
);

const modeOptions = [
  { value: '5' as const, label: '5 条' },
  { value: '10' as const, label: '10 条' },
  { value: 'custom' as const, label: '自定义' },
];

function onModeChange(m: TopNMode) {
  mode.value = m;
  if (m === 'custom') return; // 等用户在输入框里填，不在这里改设置
  void settings.setSectorTopN(Number(m));
}

/** 失焦/回车时提交。解析失败就回滚到当前值，不写库里一个 NaN。 */
function commitCustom() {
  const n = Number.parseInt(customText.value, 10);
  if (Number.isNaN(n)) {
    customText.value = String(settings.sectorTopN);
    return;
  }
  const clamped = clampTopN(n);
  customText.value = String(clamped);
  if (clamped !== settings.sectorTopN) void settings.setSectorTopN(clamped);
}
</script>

<template>
  <section class="panel">
    <p class="panel-hint">设置主界面是否显示市场概览，以及板块榜单显示多少条。</p>

    <div class="card">
      <div class="card-body">
        <SettingsRow
          title="在顶部显示市场概览面板"
          description="关闭后主界面更简洁，成交额、涨跌家数与板块行情也不再刷新"
        >
          <NSwitch
            size="small"
            :value="settings.marketOverviewVisible"
            aria-label="显示市场概览面板"
            @update:value="settings.setMarketOverviewVisible($event)"
          />
        </SettingsRow>

        <SettingsRow
          title="板块涨跌排名条数"
          :description="`行业与概念板块的涨跌榜各显示前 N 条，可选 ${SECTOR_TOP_N_MIN}–${SECTOR_TOP_N_MAX} 条`"
          :disabled="!settings.marketOverviewVisible"
        >
          <SegmentedControl
            :model-value="mode"
            :options="modeOptions"
            label="板块涨跌排名条数"
            @update:model-value="onModeChange"
          />
          <input
            v-if="mode === 'custom'"
            v-model="customText"
            class="num-input"
            type="text"
            inputmode="numeric"
            maxlength="2"
            aria-label="自定义板块条数"
            :placeholder="String(settings.sectorTopN)"
            @blur="commitCustom"
            @keydown.enter="commitCustom"
          />
        </SettingsRow>
      </div>
    </div>

    <div v-if="!settings.marketOverviewVisible" class="note">
      <CircleAlert :size="12" aria-hidden="true" />
      <span>面板当前隐藏中，条数设置会在重新显示面板后生效。</span>
    </div>
  </section>
</template>
