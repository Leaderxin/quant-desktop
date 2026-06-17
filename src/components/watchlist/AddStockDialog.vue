<script setup lang="ts">
import { ref, watch } from 'vue';
import { NModal, NCard, NInput, NList, NListItem, NButton, NSpace, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import type { StockBrief } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{
  (e: 'update:show', val: boolean): void;
}>();
const message = useMessage();
const watchlist = useWatchlistStore();

const keyword = ref('');
const results = ref<StockBrief[]>([]);
const searching = ref(false);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(() => keyword.value, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!val || val.trim().length === 0) {
    results.value = [];
    return;
  }
  debounceTimer = setTimeout(async () => {
    searching.value = true;
    try {
      results.value = await invoke<StockBrief[]>('search_stocks', { keyword: val.trim() });
    } catch {
      results.value = [];
    } finally {
      searching.value = false;
    }
  }, 300);
});

function onUpdateShow(val: boolean) {
  emit('update:show', val);
}

function onClose() {
  emit('update:show', false);
}

async function handleAdd(stock: StockBrief) {
  await watchlist.addStock(stock.code, stock.market, stock.name);
  message.success(`已添加 ${stock.name}`);
  keyword.value = '';
  results.value = [];
}
</script>

<template>
  <NModal :show="props.show" @update:show="onUpdateShow">
    <NCard title="添加自选" style="width: 420px;" closable @close="onClose">
      <NSpace vertical>
        <NInput
          v-model:value="keyword"
          placeholder="输入代码或名称搜索..."
          :loading="searching"
          clearable
        />
        <NList v-if="results.length > 0" hoverable style="max-height: 300px; overflow-y: auto;">
          <NListItem v-for="s in results" :key="s.code">
            <div style="display:flex;justify-content:space-between;align-items:center;width:100%;">
              <div>
                <span style="font-weight:500;margin-right:8px;">{{ s.name }}</span>
                <span style="color:var(--color-text-secondary);font-size:var(--font-size-sm);">{{ s.code }}</span>
              </div>
              <NButton size="tiny" type="primary" @click="handleAdd(s)">+ 添加</NButton>
            </div>
          </NListItem>
        </NList>
      </NSpace>
    </NCard>
  </NModal>
</template>
