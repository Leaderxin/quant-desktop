import { watch, type Ref } from 'vue';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';

/** 分钟周期 → 分钟跨度。非分钟K（分时 'minute' 与日/周/月）返回 null。
 * 单一来源，与后端 datasource::minute_span 保持一致。 */
export function minuteKSpan(period: PeriodType): number | null {
  switch (period) {
    case '1min': return 1;
    case '5min': return 5;
    case '15min': return 15;
    case '30min': return 30;
    case '60min': return 60;
    default: return null;
  }
}

/** 判断某周期是否为分钟 K（区别于分时 'minute' 与日/周/月 K）。*/
export function isMinuteK(period: PeriodType): boolean {
  return minuteKSpan(period) !== null;
}

/** 新浪数据源不支持 1 分钟 K 线：若 activePeriod 为 1min 且切到新浪，自动回落到 5min。
 * 复用方传入自己持有的 activePeriod ref，本函数挂载 watcher 统一回退。 */
export function useMinuteKUnavailable(activePeriod: Ref<PeriodType>): void {
  const settings = useSettingsStore();
  watch(
    () => settings.activeDatasource,
    (ds) => {
      if (ds === 'sina' && activePeriod.value === '1min') {
        activePeriod.value = '5min';
      }
    },
  );
}
