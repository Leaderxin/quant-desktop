<script setup lang="ts">
// 老板键的录制控件：点一下进入录制态，此后按下的组合键即为热键。
//
// 为什么是「录制」而不是让用户手打字符串：加速键用的是 global-hotkey 那套
// Code 名（KeyH / Digit1 / F5 / ArrowUp），要求用户记住并拼对不现实。词表与
// 校验都在 utils/hotkey 里，与后端解析端共用同一份约定。
import { onDeactivated, onUnmounted, ref } from 'vue';
import { X } from '@lucide/vue';
import { acceleratorFromEvent, formatAccelerator } from '@/utils/hotkey';

defineProps<{
  modelValue: string;
  /** 无障碍名称，如「老板键」。 */
  label: string;
}>();
const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>();

const recording = ref(false);
/** 录制期间按了不合法的组合时的说明（如只按了字母键）。 */
const hint = ref('');

function start() {
  recording.value = true;
  hint.value = '';
  // 捕获阶段挂在 window 上：录制期间这一串按键不能被页面里任何其它监听
  // 抢先消费掉，否则用户按下的组合会被别处「吃掉」一半。
  window.addEventListener('keydown', onKeydown, true);
}

function stop() {
  recording.value = false;
  hint.value = '';
  window.removeEventListener('keydown', onKeydown, true);
}

function onKeydown(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();

  if (e.key === 'Escape') {
    stop();
    return;
  }

  // 单按退格/删除 = 取消设置。带着修饰键时仍按普通主键处理（Shift+Delete 之类）。
  const bare = !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey;
  if (bare && (e.key === 'Backspace' || e.key === 'Delete')) {
    emit('update:modelValue', '');
    stop();
    return;
  }

  const accelerator = acceleratorFromEvent(e);
  if (!accelerator) {
    hint.value = '修饰键需含 Ctrl / Alt / Win 之一（只按 Shift 会挡住打字），或单按功能键';
    return;
  }
  emit('update:modelValue', accelerator);
  stop();
}

// 设置页各分区是 KeepAlive 缓存的（切走是 deactivate，不是 unmount），所以
// 两个钩子都要挂：只挂 onUnmounted 的话，录制中途切到别的分区，那个挂在 window
// 捕获阶段的监听会留在原地，把整个设置页的按键全部吞掉。
onDeactivated(stop);
onUnmounted(stop);
</script>

<template>
  <div class="hotkey">
    <div class="hotkey-line">
      <button
        type="button"
        class="hk-btn"
        :class="{ recording }"
        :aria-label="label"
        @click="recording ? stop() : start()"
        @blur="stop"
      >
        {{ recording ? '请按下组合键…' : (modelValue ? formatAccelerator(modelValue) : '未设置') }}
      </button>
      <button
        v-if="modelValue && !recording"
        type="button"
        class="hk-clear"
        :aria-label="`清除${label}`"
        :title="`清除${label}`"
        @click="emit('update:modelValue', '')"
      >
        <X :size="12" aria-hidden="true" />
      </button>
    </div>
    <!-- 录制中按住不动时会一直停在这条提示上，所以写全「怎么办」而不是只说「不行」 -->
    <p v-if="hint" class="hk-hint">{{ hint }}</p>
  </div>
</template>

<style scoped>
.hotkey {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
}
.hotkey-line {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}
.hk-btn {
  /* 定宽：文字在「未设置 / 请按下组合键… / Ctrl + Shift + F5」之间切换时，
     按钮宽度不变，右侧那一列才不会跟着抽动。 */
  min-width: 132px;
  padding: 3px 10px;
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-sm);
  background: var(--color-surface-2);
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.6;
  cursor: pointer;
  transition: border-color var(--transition-fast), color var(--transition-fast);
}
.hk-btn:hover {
  border-color: var(--color-accent);
}
.hk-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
/* 录制态用强调色 + 虚线边框：此刻这个控件在等一次全应用的键盘输入，
   长得和「一个安静的只读框」一样会让人不知道该按什么。 */
.hk-btn.recording {
  border-style: dashed;
  border-color: var(--color-accent);
  color: var(--color-accent);
  font-family: var(--font-sans);
}
.hk-clear {
  display: inline-flex;
  align-items: center;
  padding: 3px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  transition: color var(--transition-fast);
}
.hk-clear:hover {
  color: var(--color-text-primary);
}
.hk-hint {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--color-warning);
}
</style>
