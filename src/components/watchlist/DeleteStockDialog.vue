<script setup lang="ts">
// 删除自选的确认框内容。
//
// 两条菜单路径都落到这里：显式的「删除自选」；以及「从本组移除」但这是它
// 唯一的分组（后端会把股票一并删掉）。两者最终都从自选池里删除，所以文案
// 说明的是「从哪些分组里消失」，而不是把两条路径分开讲。
import { computed } from 'vue';
import ConfirmDialog from '@/components/common/ConfirmDialog.vue';
import type { WatchGroup } from '@/types';

const props = defineProps<{
  /** 待删除的自选名；null 表示关闭 */
  stockName: string | null;
  /** 它当前所属的分组，用于说明会从哪些分组里消失 */
  groups: WatchGroup[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'confirm'): void;
}>();

/**
 * 名称逐个列出，而不是只说「所有分组」：多归属下用户往往正看着某一个分组，
 * 需要知道删除会连带影响其它分组。
 */
const description = computed(() => {
  const names = props.groups.map((g) => `「${g.name}」`).join('、');
  if (!names) return '删除后不可撤销。';
  if (props.groups.length === 1) {
    return `删除后不可撤销。它只属于 ${names}，删除后将不再出现在自选里。`;
  }
  return `删除后不可撤销。它同时属于 ${names}，这些分组里都会被移除。`;
});
</script>

<template>
  <ConfirmDialog
    :show="stockName !== null"
    :title="`删除自选「${stockName ?? ''}」？`"
    :description="description"
    confirm-text="删除自选"
    @close="emit('close')"
    @confirm="emit('confirm')"
  />
</template>
