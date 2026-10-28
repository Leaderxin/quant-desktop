<script setup lang="ts">
// 破坏性操作的确认框外壳：危险标记 + 标题 + 常驻说明 + 内容插槽 + 取消/确认。
//
// 「删除分组」与「删除自选」共用这一份，不再各自搭一张卡片。卡片底色、字号、
// 按钮高度与设计令牌的对应关系只写一遍 —— 两处各写一套，迟早只剩一处被改。
//
// 不用 naive-ui 的 `useDialog().error()`：那个弹框的卡片、字号、内边距全是
// naive 自己的一套，套上 themeOverrides 也只是把按钮换成主色，正文仍是一片
// 没有层级的段落 —— 放在这个 GitHub 风格的界面里像另一个应用的窗口。
import { nextTick, ref, useId, watch } from 'vue';
import { NModal } from 'naive-ui';
import { Trash2 } from '@lucide/vue';

const props = withDefaults(defineProps<{
  show: boolean;
  title: string;
  /** 标题下的常驻说明。空串表示不显示 */
  description?: string;
  /** 确认按钮文案 */
  confirmText?: string;
}>(), {
  description: '',
  confirmText: '删除',
});

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'confirm'): void;
}>();

// 两个实例可能先后打开（分组 / 自选），id 不能写死，否则 aria-labelledby
// 会指向另一个弹框的标题。
const titleId = useId();
const descId = useId();

const cancelRef = ref<HTMLButtonElement | null>(null);

/**
 * 默认焦点给「取消」，而不是回车能直接触发的第一个按钮。
 * 这是一步不可撤销的删除，误按回车的代价比多点一下高得多。
 */
watch(() => props.show, async (visible) => {
  if (!visible) return;
  await nextTick();
  cancelRef.value?.focus();
}, { immediate: true });
</script>

<template>
  <NModal
    :show="show"
    :auto-focus="false"
    @update:show="(v: boolean) => { if (!v) emit('close'); }"
  >
    <div
      class="confirm-card"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      :aria-describedby="description ? descId : undefined"
    >
      <div class="confirm-head">
        <span class="danger-mark" aria-hidden="true">
          <Trash2 :size="15" />
        </span>
        <div class="head-text">
          <h2 :id="titleId" class="confirm-title">{{ title }}</h2>
          <p v-if="description" :id="descId" class="confirm-desc">
            {{ description }}
          </p>
        </div>
      </div>

      <!-- 调用方补充的影响面 / 名单；没有就只留标题与说明 -->
      <slot />

      <div class="confirm-foot">
        <button
          ref="cancelRef"
          type="button"
          class="btn btn-ghost"
          @click="emit('close')"
        >
          取消
        </button>
        <button type="button" class="btn btn-danger" @click="emit('confirm')">
          {{ confirmText }}
        </button>
      </div>
    </div>
  </NModal>
</template>

<style scoped>
.confirm-card {
  width: 400px;
  max-width: calc(100vw - 48px);
  padding: var(--space-5) var(--space-5) var(--space-4);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  font-family: var(--font-sans);
}

/* ── 头部：危险标记 + 标题 + 常驻说明 ── */
.confirm-head {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}
.danger-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border: 1px solid var(--color-error-border);
  border-radius: var(--radius-md);
  background: var(--color-error-bg);
  color: var(--color-error);
}
.head-text {
  min-width: 0;
}
.confirm-title {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-weight-semibold);
  line-height: 1.4;
  color: var(--color-text-primary);
  /* 分组名/股票名过长时换行而不是截断：确认框里必须看清删的是哪一个 */
  overflow-wrap: anywhere;
}
.confirm-desc {
  margin: 3px 0 0;
  font-size: var(--text-sm);
  line-height: 1.6;
  color: var(--color-text-secondary);
}

/* ── 底部操作 ── */
.confirm-foot {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  margin-top: var(--space-5);
}
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 28px;
  padding: 0 14px;
  border-radius: var(--radius-md);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast),
    color var(--transition-fast);
}
.btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}
.btn-ghost {
  border: 1px solid var(--color-border-1);
  background: transparent;
  color: var(--color-text-secondary);
}
.btn-ghost:hover {
  background: var(--color-surface-2);
  color: var(--color-text-primary);
}
.btn-danger {
  border: 1px solid transparent;
  background: var(--color-danger-solid);
  color: var(--color-on-danger);
}
.btn-danger:hover {
  background: var(--color-danger-solid-hover);
}
.btn-danger:focus-visible {
  outline-color: var(--color-error);
}
</style>
