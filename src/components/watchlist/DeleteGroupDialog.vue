<script setup lang="ts">
// 删除分组的确认框内容：在共用外壳（ConfirmDialog）里摆出影响面。
//
// 影响面是两组「一行结论 + 一份名单」：哪几只被挪到兜底分组、哪几只还留在
// 别的分组里不受影响。只给数量的话，「删了会不会丢东西」还得靠猜，所以名单
// 逐只列出名称 + 代码。
import { ArrowRight } from '@lucide/vue';
import { formatCode } from '@/utils/format';
import ConfirmDialog from '@/components/common/ConfirmDialog.vue';

/** 名单里的一行，只用到名称/代码/市场；WatchItem 结构上满足它。 */
interface StockRef {
  code: string;
  name: string;
  market: string;
}

defineProps<{
  /** 待删除的分组名；null 表示关闭 */
  groupName: string | null;
  /** 孤儿自选会被并入的分组名（展示顺序里第一个别的分组） */
  fallbackName: string;
  /** 删除后会被移入 `fallbackName` 的自选 */
  orphans: StockRef[];
  /** 仍属于其它分组、不受影响的自选 */
  kept: StockRef[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'confirm'): void;
}>();
</script>

<template>
  <ConfirmDialog
    :show="groupName !== null"
    :title="`删除分组「${groupName ?? ''}」？`"
    description="删除后不可撤销。自选本身不会被删除。"
    confirm-text="删除分组"
    @close="emit('close')"
    @confirm="emit('confirm')"
  >
    <!-- 有自选时才值得摆出账目面板；空分组一句话说明就够，
         摆一个只剩表头的框反而像渲染坏了。 -->
    <div v-if="orphans.length + kept.length > 0" class="impact">
      <div class="impact-head">
        <span class="impact-label">影响范围</span>
        <span class="impact-total tabular-nums">{{ orphans.length + kept.length }} 只自选</span>
      </div>

      <div v-if="orphans.length > 0" class="impact-group">
        <div class="impact-row">
          <span class="count tabular-nums">{{ orphans.length }}</span>
          <span class="row-text">仅在此分组</span>
          <ArrowRight :size="12" class="row-arrow" aria-hidden="true" />
          <span class="row-tail">移入「{{ fallbackName }}」</span>
        </div>
        <ul class="stock-list">
          <li
            v-for="s in orphans"
            :key="`${s.market}:${s.code}`"
            class="stock-row"
          >
            <span class="stock-name">{{ s.name }}</span>
            <span class="stock-code tabular-nums">{{ formatCode(s.code) }}</span>
          </li>
        </ul>
      </div>

      <div v-if="kept.length > 0" class="impact-group">
        <div class="impact-row">
          <span class="count tabular-nums">{{ kept.length }}</span>
          <span class="row-text">还属于其它分组，不受影响</span>
        </div>
        <ul class="stock-list">
          <li
            v-for="s in kept"
            :key="`${s.market}:${s.code}`"
            class="stock-row"
          >
            <span class="stock-name">{{ s.name }}</span>
            <span class="stock-code tabular-nums">{{ formatCode(s.code) }}</span>
          </li>
        </ul>
      </div>
    </div>
    <p v-else class="impact-empty">该分组下没有自选。</p>
  </ConfirmDialog>
</template>

<style scoped>
.impact {
  margin-top: var(--space-4);
  padding: 10px var(--space-3);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
}
.impact-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
  padding-bottom: 8px;
  border-bottom: 1px solid var(--color-border-0);
}
.impact-label {
  font-size: var(--text-xs);
  /* 用 secondary 而不是 tertiary：这两行是「影响面」的标题与总数，属于要读的
     内容；tertiary 在浅色主题下只有约 2.7:1，够不上 12px 正文的门槛。 */
  color: var(--color-text-secondary);
}
.impact-total {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
}
.impact-group {
  padding-top: 8px;
}
/* 两组之间的分隔线：名单一长，两段就糊成一片 */
.impact-group + .impact-group {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border-0);
}
.impact-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  line-height: 1.4;
}
.count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  min-width: 24px;
  height: 20px;
  padding: 0 6px;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-sm);
  background: var(--color-surface-0);
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}
.row-text {
  color: var(--color-text-secondary);
}
.row-arrow {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
}
.row-tail {
  color: var(--color-text-primary);
  overflow-wrap: anywhere;
}

/* ── 名单 ──
   左缩进 32px = 计数徽章 24 + 行间距 8，与上面那行文字对齐。
   封顶后滚动：分组里几十只股票时，确认框不能长到把按钮顶出屏幕。 */
.stock-list {
  list-style: none;
  margin: 4px 0 0;
  padding: 0 6px 0 32px;
  /* 6 行整（22px × 6）。行高写死是为了让截断落在行边界上，
     半行文字看着像渲染坏了；滚动条本身已经说明还有内容。 */
  max-height: 132px;
  overflow-y: auto;
}
.stock-row {
  display: flex;
  /* 居中对齐而不是基线对齐：名称 12px、代码 11px，按基线对齐时两条下降线
     会各撑一点，行高变成 23.5 而不是 22，封顶的 6 行就落在半行上 */
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: 2px 0;
  font-size: var(--text-sm);
  line-height: 1.5;
}
.stock-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--color-text-primary);
}
.stock-code {
  flex-shrink: 0;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
}
.impact-empty {
  margin: var(--space-4) 0 0;
  font-size: var(--text-sm);
  line-height: 1.6;
  color: var(--color-text-secondary);
}
</style>
