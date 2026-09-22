<script setup lang="ts">
// 开关。尺寸与状态栏现有的「开机自启」开关一致（30×17 / 滑块 13），
// 两处视觉语言统一。
const props = defineProps<{
  modelValue: boolean;
  /** 无障碍名称。开关自身没有文字，必须显式提供。 */
  label: string;
}>();
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>();

function toggle() {
  emit('update:modelValue', !props.modelValue);
}
</script>

<template>
  <button
    class="toggle"
    type="button"
    role="switch"
    :class="{ on: modelValue }"
    :aria-checked="modelValue"
    :aria-label="label"
    @click="toggle"
  >
    <span class="knob" aria-hidden="true"></span>
  </button>
</template>

<style scoped>
.toggle {
  position: relative;
  width: 30px;
  height: 17px;
  flex-shrink: 0;
  padding: 0;
  border: none;
  border-radius: var(--radius-full);
  background: var(--color-border-1);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.toggle.on {
  background: var(--color-accent);
}
.toggle:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
  transition: transform var(--transition-fast);
}
.toggle.on .knob {
  transform: translateX(13px);
}
@media (prefers-reduced-motion: reduce) {
  .toggle,
  .knob {
    transition: none;
  }
}
</style>
