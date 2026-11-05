<script setup lang="ts">
/**
 * 详情面板懒加载时的占位。
 *
 * 高度与真实面板一致（StockDetail 用 clamp(240px, 52vh, 560px)、IndexDetail 用
 * min(560px, 100vh - 160px)），否则异步 chunk 到位前后自选表会先撑开再缩回去，
 * 出现一次明显的跳动。
 */
defineProps<{ variant: 'stock' | 'index' }>();
</script>

<template>
  <div
    class="detail-loading"
    :class="`detail-loading--${variant}`"
    role="status"
    aria-live="polite"
  >
    <span class="detail-loading-text">加载详情...</span>
  </div>
</template>

<style scoped>
.detail-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-surface-1);
  padding: 12px 16px;
}

.detail-loading-text {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

/* 与 StockDetail 的高度/占位方式一致：夹在自选表下面的普通流内元素 */
.detail-loading--stock {
  flex: 0 1 auto;
  height: clamp(240px, 52vh, 560px);
}

/* 与 IndexDetail 一致：挂在指数条下方的绝对定位浮层 */
.detail-loading--index {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  z-index: 10;
  height: min(560px, calc(100vh - 160px));
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

@media (max-height: 820px) {
  .detail-loading {
    padding: 8px 16px 10px;
  }
}
</style>
