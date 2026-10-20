<script setup lang="ts">
import { computed, nextTick, ref, watch, onUnmounted } from 'vue';
import { NModal, NCard, NInput, NSpace, useMessage } from 'naive-ui';
import type { InputInst } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import type { StockBrief, WatchGroup } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';
import { formatCode } from '@/utils/format';
import { Check, CircleAlert, Search } from '@lucide/vue';
import MarketTag from './MarketTag.vue';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: 'update:show', val: boolean): void }>();
const message = useMessage();
const watchlist = useWatchlistStore();

const keyword = ref('');
const results = ref<StockBrief[]>([]);
const searching = ref(false);
const searchError = ref('');
const searchInputRef = ref<InputInst | null>(null);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

/** 这次添加落到哪些分组。默认是打开对话框时所在的分组，用户可勾选多个。 */
const targetGroupIds = ref<number[]>([watchlist.activeGroupId]);

/** 已勾选的分组，按分组栏的顺序排列（提示文案与提交都用这个顺序）。 */
const selectedGroups = computed<WatchGroup[]>(() =>
  watchlist.groups.filter((g) => targetGroupIds.value.includes(g.id)),
);

function isGroupSelected(id: number): boolean {
  return targetGroupIds.value.includes(id);
}

/**
 * 勾选/取消一个分组。至少保留一个 —— 自选必须属于某个分组，全不选意味着
 * 一只看不见的股票；后端在这种输入下会兜底到默认分组，但那与用户勾的动作不符，
 * 所以在这里就拦住并说明。
 */
function toggleGroup(id: number) {
  if (isGroupSelected(id)) {
    if (selectedGroups.value.length <= 1) {
      message.warning('至少要保留一个分组');
      return;
    }
    targetGroupIds.value = targetGroupIds.value.filter((x) => x !== id);
  } else {
    targetGroupIds.value = [...targetGroupIds.value, id];
  }
}

/**
 * 打开时重置一次：NModal 的内容在关闭后并不卸载，不重置的话上次的关键字、
 * 结果和勾过的分组会残留到下一次。目标分组回到当前标签页 —— 打开对话框那一刻
 * 所在的分组才是用户的上下文，上一次的选择不该压过它。
 */
watch(() => props.show, async (visible) => {
  if (!visible) return;
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  keyword.value = '';
  results.value = [];
  searchError.value = '';
  searching.value = false;
  targetGroupIds.value = [
    watchlist.groups.some((g) => g.id === watchlist.activeGroupId)
      ? watchlist.activeGroupId
      : watchlist.defaultGroupId,
  ];
  // 打开后直接能打字。Modal 的 auto-focus 会落到第一个可聚焦元素（现在是分组勾选），
  // 所以那边关掉了，焦点由这里交给搜索框 —— 搜索才是这个对话框的主流程。
  await nextTick();
  searchInputRef.value?.focus();
});

onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
});

watch(() => keyword.value, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!val || val.trim().length === 0) {
    results.value = [];
    searchError.value = '';
    return;
  }
  debounceTimer = setTimeout(async () => {
    searching.value = true;
    searchError.value = '';
    try {
      results.value = await invoke<StockBrief[]>('search_stocks', { keyword: val.trim() });
    } catch (e) {
      results.value = [];
      searchError.value = `搜索失败: ${String(e).slice(0, 60)}`;
    } finally {
      searching.value = false;
    }
  }, 300);
});

/**
 * 这只股票还没加入的已勾选分组。
 *
 * 一只股票可以同时属于多个分组，勾选里通常一部分已经归属 —— 后端 add_watch 对
 * 已有的关联是 INSERT OR IGNORE（保留原组内位置），所以只需把缺的那些补上。
 * 返回空数组即「勾选的分组它全在」，此时添加是个空操作，界面上直接说明。
 */
function missingGroupsFor(stock: StockBrief): WatchGroup[] {
  const existing = watchlist.items.find(
    (i) => i.code === stock.code && i.market === stock.market,
  );
  const memberOf = new Set(
    existing ? watchlist.groupsOf(existing.id).map((g) => g.id) : [],
  );
  return selectedGroups.value.filter((g) => !memberOf.has(g.id));
}

async function handleAdd(stock: StockBrief) {
  if (selectedGroups.value.length === 0) {
    message.warning('请先选择要加入的分组');
    return;
  }
  const missing = missingGroupsFor(stock);
  if (missing.length === 0) {
    message.info(`「${stock.name}」已在所选分组中`);
    return;
  }
  try {
    // 只提交缺的那些分组；已经在的组不动，组内位置也不会被挪到队尾。
    await watchlist.addStock(
      stock.code,
      stock.market,
      stock.name,
      missing.map((g) => g.id),
    );
    if (watchlist.error) {
      message.error(watchlist.error);
      return;
    }
    message.success(`已添加到${missing.map((g) => `「${g.name}」`).join('、')}`);
    keyword.value = '';
    results.value = [];
    // 连续加几只时不用再点一次输入框
    await nextTick();
    searchInputRef.value?.focus();
  } catch (e) {
    message.error(`添加失败: ${e}`);
  }
}
</script>

<template>
  <NModal :show="props.show" :auto-focus="false" @update:show="emit('update:show', $event)">
    <NCard
      title="添加自选"
      style="width: 400px;"
      closable
      @close="emit('update:show', false)"
      :bordered="false"
    >
      <NSpace vertical :size="12">
        <!-- 目标分组。放在搜索框之前：先定「加到哪儿」，再找「加什么」，
             用户不用在选中股票之后回头确认落点。多归属下可以一次勾多个。 -->
        <div class="group-block">
          <span id="add-groups-label" class="group-label">添加到分组</span>
          <div class="group-chips" role="group" aria-labelledby="add-groups-label">
            <button
              v-for="g in watchlist.groups"
              :key="g.id"
              type="button"
              class="group-chip"
              :class="{ active: isGroupSelected(g.id) }"
              :aria-pressed="isGroupSelected(g.id)"
              @click="toggleGroup(g.id)"
            >
              <Check v-if="isGroupSelected(g.id)" :size="12" aria-hidden="true" />
              <span class="group-chip-name">{{ g.name }}</span>
            </button>
          </div>
          <p v-if="watchlist.groups.length > 1" class="group-hint">
            可同时勾选多个分组；加入这里不会移出它已有的分组。
          </p>
        </div>

        <NInput
          ref="searchInputRef"
          v-model:value="keyword"
          placeholder="输入代码或名称..."
          :loading="searching"
          clearable
          size="medium"
        >
          <template #prefix>
            <Search :size="14" aria-hidden="true" style="color:var(--color-text-tertiary)" />
          </template>
        </NInput>

        <div class="results-wrapper">
          <div v-if="results.length > 0" class="results-list">
            <div
              v-for="s in results"
              :key="s.code"
              class="result-item"
              @click="handleAdd(s)"
            >
              <div class="result-info">
                <MarketTag :code="s.code" :category="s.category" />
                <span class="result-name">{{ s.name }}</span>
                <span class="result-code tabular-nums">{{ formatCode(s.code) }}</span>
              </div>
              <span v-if="missingGroupsFor(s).length === 0" class="result-state">
                已在所选分组
              </span>
              <button v-else class="result-add-btn" @click.stop="handleAdd(s)">
                添加
              </button>
            </div>
          </div>

          <div v-else-if="searchError" class="search-error" role="alert">
            <CircleAlert :size="14" aria-hidden="true" class="search-error-icon" />
            <span>{{ searchError }}</span>
          </div>
          <div v-else-if="keyword && !searching" class="no-results">
            未找到匹配标的
          </div>
        </div>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
.group-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.group-label {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
}
.group-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  /* 分组多时封顶并滚动：对话框不能因为分组长到把搜索结果挤出屏幕 */
  max-height: 92px;
  overflow-y: auto;
  padding: 1px;
}
.group-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-full);
  background: transparent;
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast),
    color var(--transition-fast);
}
.group-chip:hover {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}
/* 选中态不只靠颜色：勾号图标同时出现 */
.group-chip.active {
  border-color: var(--color-accent);
  background: var(--color-accent-dim);
  color: var(--color-accent);
}
.group-chip:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
.group-chip-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-hint {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-tertiary);
}
.results-wrapper {
  min-height: 260px;
}
.results-list {
  max-height: 260px;
  overflow-y: auto;
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
}
.result-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  transition: background var(--transition-fast);
  border-bottom: 1px solid var(--color-border-0);
}
.result-item:last-child { border-bottom: none; }
.result-item:hover { background: var(--color-surface-2); }
.result-add-btn {
  padding: 2px 10px;
  border: 1px solid var(--color-accent);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-accent);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--transition-fast), filter var(--transition-fast);
}
.result-add-btn:hover {
  background: var(--color-accent-dim);
}
.result-state {
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  white-space: nowrap;
}

.result-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  overflow: hidden;
  min-width: 0;
  max-width: 280px;
}
.result-name {
  font-size: var(--text-md);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  line-height: 1.4;
  word-break: break-all;
}
.result-code {
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  color: var(--color-text-tertiary);
  line-height: 1.4;
  white-space: nowrap;
  flex-shrink: 0;
}
.no-results {
  text-align: center;
  padding: var(--space-6);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
.search-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: var(--space-4) var(--space-3);
  color: var(--color-warning);
  font-size: var(--text-xs);
  text-align: center;
}
.search-error-icon {
  flex-shrink: 0;
  color: var(--color-warning);
}
</style>
