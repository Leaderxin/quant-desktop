<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NModal, NCard, NFormItem, NInputNumber, NButton, NSpace, useMessage } from 'naive-ui';
import type { WatchItem } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';

const props = defineProps<{ item: WatchItem | null }>();
const emit = defineEmits<{ close: [] }>();
const watchlist = useWatchlistStore();
const message = useMessage();
const cost = ref<number | null>(null);
const quantity = ref<number | null>(null);
const saving = ref(false);
watch(() => props.item, (item) => {
  cost.value = item?.cost_price ?? null;
  quantity.value = item?.quantity ?? null;
});
const valid = computed(() => cost.value !== null && Number.isFinite(cost.value) && cost.value >= 0 && cost.value <= 1e9
  && quantity.value !== null && Number.isInteger(quantity.value) && quantity.value >= 0 && quantity.value <= 1e12);

async function save(clear = false) {
  if (!props.item || saving.value || (!clear && !valid.value)) return;
  saving.value = true;
  try {
    await watchlist.setPosition(props.item.id, clear ? null : cost.value, clear ? null : quantity.value);
    message.success(clear ? '已清除成本和持仓' : '成本和持仓已保存');
    emit('close');
  } catch (e) {
    message.error(`保存失败：${e}`);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <NModal :show="item !== null" :mask-closable="!saving" :close-on-esc="!saving" @update:show="!$event && !saving && emit('close')">
    <NCard :title="`${item?.name ?? ''} · 设置成本`" style="width: 380px; max-width: calc(100vw - 32px)" :closable="!saving" :bordered="false" @close="emit('close')">
      <NFormItem label="每股成本价">
        <NInputNumber v-model:value="cost" :min="0" :max="1e9" :disabled="saving" placeholder="输入平均成本价" style="width: 100%" />
      </NFormItem>
      <NFormItem label="持仓数量（股）">
        <NInputNumber v-model:value="quantity" :min="0" :max="1e12" :disabled="saving" placeholder="输入股数，不是手数" style="width: 100%" />
      </NFormItem>
      <p class="position-hint">持仓盈亏 =（最新价 − 每股成本价）× 股数。按报价币种计算，不另计交易费用。</p>
      <NSpace justify="end">
        <NButton :disabled="saving" @click="save(true)">清除</NButton>
        <NButton type="primary" :loading="saving" :disabled="!valid" @click="save()">保存</NButton>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
.position-hint { margin: 0 0 20px; font-size: 12px; color: var(--color-text-secondary); }
</style>
