<script setup lang="ts">
/**
 * 设置页异步 chunk 的加载/失败占位。
 *
 * 设置页把整块看盘界面替换掉（AppLayout 的 v-if），所以加载中或加载失败时
 * 不能什么都不画 —— 那样用户既看不到反馈，也没有「返回」按钮可点。
 * 失败态给一个整窗重载入口：chunk 加载失败通常意味着资源/环境异常，重启窗口
 * 是最直接且一定能退出的恢复路径。
 */
defineProps<{ error?: Error | null }>();

function reload() {
  location.reload();
}
</script>

<template>
  <div class="settings-fallback" role="status" aria-live="polite">
    <template v-if="error">
      <p class="fallback-title fallback-title--error">设置页加载失败</p>
      <p class="fallback-detail">{{ error.message || String(error) }}</p>
      <button class="btn" @click="reload">重新加载</button>
    </template>
    <p v-else class="fallback-title">加载设置...</p>
  </div>
</template>

<style scoped>
.settings-fallback {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-6);
  background: var(--color-surface-0);
}

.fallback-title {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
}
.fallback-title--error {
  color: var(--color-warning);
}

.fallback-detail {
  max-width: 420px;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
  text-align: center;
  word-break: break-word;
}
</style>
