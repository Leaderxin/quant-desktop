<script setup lang="ts">
// 市场概览：面板显示开关 + 板块榜单条数。
import { watch } from 'vue';
import { NSwitch } from 'naive-ui';
import { useSettingsStore } from '@/stores/settings';
import { nearestTopNPreset, SECTOR_TOP_N_PRESETS } from '@/utils/prefs';
import SettingsRow from './SettingsRow.vue';
import SegmentedControl from './SegmentedControl.vue';
import { CircleAlert } from '@lucide/vue';

const settings = useSettingsStore();

const modeOptions = SECTOR_TOP_N_PRESETS.map((n) => ({ value: n, label: `${n} 条` }));

/**
 * 「自定义」档去掉后取值域收窄到预设，但库里可能还留着老版本存的自定义值（8、20
 * 之类）—— 分段控件拿它一个都匹配不上，会显示成「全都不选中」，看着像坏了。所以
 * 进来就归到最近的一档并写回。
 *
 * 对齐放在这里而不是启动时的 store：这个值只有市场概览和这个设置页在读，而设置页
 * 是唯一能改它的地方。塞进 `fetchSettings` 会给那条路径加一个写库副作用。
 *
 * 必须等 `loaded`：fetchSettings 落地前 `sectorTopN` 是兜底值，那时写回等于拿兜底值
 * 覆盖掉库里真正的设置。
 */
watch(
  [() => settings.loaded, () => settings.sectorTopN],
  ([loaded, n]) => {
    if (!loaded) return;
    if (!SECTOR_TOP_N_PRESETS.includes(n)) void settings.setSectorTopN(nearestTopNPreset(n));
  },
  { immediate: true },
);
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
          description="行业与概念板块的涨跌榜各显示前 N 条"
          :disabled="!settings.marketOverviewVisible"
        >
          <SegmentedControl
            :model-value="settings.sectorTopN"
            :options="modeOptions"
            label="板块涨跌排名条数"
            @update:model-value="settings.setSectorTopN($event)"
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
