<script setup lang="ts">
// 自选分组标签栏（同花顺式）：点击切换 / 双击就地重命名 / 右键菜单 / ＋新建。
//
// 标签本身是按钮而不是链接：这里没有路由，切换分组只是换一份本地过滤结果，
// 用 <a> 会让人以为会跳页。
import { computed, h, nextTick, ref } from 'vue';
import { NDropdown, useDialog } from 'naive-ui';
import { useWatchlistStore } from '@/stores/watchlist';
import { GROUP_NAME_MAX_LEN } from '@/utils/prefs';
import { computeGroupRemovalImpact } from '@/utils/watchGroups';
import { moveByStep } from '@/utils/dragSort';
import { Plus } from '@lucide/vue';
import type { WatchGroup } from '@/types';

const watchlist = useWatchlistStore();
const dialog = useDialog();

/** 正在编辑的标签：重命名某个已有分组，或新建一个。 */
const editing = ref<{ mode: 'rename'; id: number } | { mode: 'create' } | null>(null);
const draftName = ref('');
const inputEl = ref<HTMLInputElement | null>(null);
const editError = ref('');

/**
 * 用函数 ref 而不是 `ref="inputEl"`：重命名输入框位于 `<template v-for>` 内部，
 * 对象式 ref 在那里会被收集成数组，两处复用同一个 ref 名时类型和取值都会变形。
 */
function setInputRef(el: Element | { $el?: Element } | null) {
  inputEl.value = (el instanceof HTMLInputElement ? el : null);
}

async function focusInput() {
  await nextTick();
  inputEl.value?.focus();
  inputEl.value?.select();
}

function startRename(g: WatchGroup) {
  editing.value = { mode: 'rename', id: g.id };
  draftName.value = g.name;
  editError.value = '';
  void focusInput();
}

function startCreate() {
  editing.value = { mode: 'create' };
  draftName.value = `分组 ${watchlist.groups.length + 1}`;
  editError.value = '';
  void focusInput();
}

function cancelEdit() {
  editing.value = null;
  draftName.value = '';
  editError.value = '';
}

async function commitEdit() {
  const current = editing.value;
  if (!current) return;
  const name = draftName.value.trim();

  if (!name) {
    editError.value = '分组名不能为空';
    return;
  }
  if (name.length > GROUP_NAME_MAX_LEN) {
    editError.value = `不能超过 ${GROUP_NAME_MAX_LEN} 个字符`;
    return;
  }
  // 新建时名称重复是常见误操作（默认名「分组 N」），本地先拦一道，
  // 省掉一次必然失败的 IPC 往返。重命名时后端同样会校验。
  const duplicated = watchlist.groups.some(
    (g) => g.name === name && !(current.mode === 'rename' && g.id === current.id),
  );
  if (duplicated) {
    editError.value = `已存在名为「${name}」的分组`;
    return;
  }

  if (current.mode === 'create') {
    const id = await watchlist.addGroup(name);
    if (id === null) {
      editError.value = watchlist.error ?? '新建失败';
      return;
    }
  } else {
    await watchlist.renameGroup(current.id, name);
    if (watchlist.error) {
      editError.value = watchlist.error;
      return;
    }
  }
  cancelEdit();
}

// ── 右键菜单 ──

const ctxX = ref(0);
const ctxY = ref(0);
const ctxGroup = ref<WatchGroup | null>(null);
const showCtx = ref(false);

function handleContextMenu(e: MouseEvent, g: WatchGroup) {
  e.preventDefault();
  // 夹到视口内，靠右/靠下的标签右键时菜单不会跑到屏幕外
  ctxX.value = Math.min(e.clientX, window.innerWidth - 150);
  ctxY.value = Math.min(e.clientY, window.innerHeight - 160);
  ctxGroup.value = g;
  showCtx.value = true;
}

const canDelete = computed(() => watchlist.groups.length > 1);

const ctxOptions = computed(() => [
  { label: '重命名', key: 'rename' },
  { label: '删除分组', key: 'delete', disabled: !canDelete.value },
  { type: 'divider' as const, key: 'd1' },
  { label: '上移', key: 'up', disabled: ctxIndex.value <= 0 },
  { label: '下移', key: 'down', disabled: ctxIndex.value >= watchlist.groups.length - 1 },
]);

const ctxIndex = computed(() =>
  ctxGroup.value ? watchlist.groups.findIndex((g) => g.id === ctxGroup.value!.id) : -1,
);

function moveGroup(index: number, direction: -1 | 1) {
  const next = moveByStep(watchlist.groups.map((g) => g.id), index, direction);
  if (!next) return; // 队首上移 / 队尾下移：跳过 IPC
  void watchlist.reorderGroups(next);
}

function handleCtxSelect(key: string) {
  const g = ctxGroup.value;
  showCtx.value = false;
  if (!g) return;
  switch (key) {
    case 'rename':
      startRename(g);
      break;
    case 'delete':
      requestDelete(g);
      break;
    case 'up':
      moveGroup(ctxIndex.value, -1);
      break;
    case 'down':
      moveGroup(ctxIndex.value, 1);
      break;
  }
}

/**
 * 删除分组前如实说明影响面。
 *
 * 两个数字都在本地算得出来（快照里已有每组的有序成员 id），不需要额外的
 * 预览 IPC：`orphans` 是「只属于这一个分组」的股票，它们会被并入默认分组；
 * 其余还留在别的分组里，不受影响。删除本身不删自选，这一点必须写进文案 ——
 * 用户对「删分组」的默认预期往往是「数据没了」。
 */
function requestDelete(g: WatchGroup) {
  const defaultName = watchlist.groupName(watchlist.defaultGroupId);
  const { orphans, kept } = computeGroupRemovalImpact(watchlist.groups, g.id);

  const lines: string[] = [];
  if (orphans > 0) lines.push(`其中 ${orphans} 只仅在此分组，将移入「${defaultName}」；`);
  if (kept > 0) lines.push(`其余 ${kept} 只还属于其它分组，不受影响。`);
  if (orphans === 0 && kept === 0) lines.push('该分组下没有自选。');

  // 用 error 而不是 warning：删除是不可逆操作，确认按钮应当是危险色。
  // warning 会把按钮渲染成琥珀色 —— 本应用的琥珀只用于「警告/注意」，
  // 而且删除按钮和警告图标同色会削弱"这一步会删东西"的信号。
  dialog.error({
    title: `删除分组「${g.name}」？`,
    content: () => [
      ...lines.map((t) => h('p', { style: 'margin:0 0 4px;line-height:1.7;' }, t)),
      h('p', { style: 'margin:8px 0 0;line-height:1.7;' }, '自选本身不会被删除。'),
    ],
    positiveText: '删除分组',
    negativeText: '取消',
    onPositiveClick: () => {
      void watchlist.deleteGroup(g.id);
    },
  });
}
</script>

<template>
  <div class="group-tabs-wrap">
    <div class="group-tabs" role="tablist" aria-label="自选分组">
      <template v-for="g in watchlist.groups" :key="g.id">
        <!-- 编辑态：输入框取代标签，Enter 提交 / Esc 取消 / 失焦提交 -->
        <div
          v-if="editing?.mode === 'rename' && editing.id === g.id"
          class="group-tab editing"
        >
          <input
            :ref="setInputRef"
            v-model="draftName"
            class="group-input"
            spellcheck="false"
            :maxlength="GROUP_NAME_MAX_LEN"
            :aria-label="`重命名分组 ${g.name}`"
            @keydown.enter.prevent="commitEdit"
            @keydown.esc.prevent="cancelEdit"
            @blur="commitEdit"
          />
        </div>
        <button
          v-else
          class="group-tab"
          type="button"
          role="tab"
          :aria-selected="watchlist.activeGroupId === g.id"
          :class="{ active: watchlist.activeGroupId === g.id }"
          :title="`${g.name}（双击重命名，右键更多操作）`"
          @click="watchlist.activeGroupId = g.id"
          @dblclick="startRename(g)"
          @contextmenu="handleContextMenu($event, g)"
        >
          {{ g.name }}
          <span class="cnt">{{ g.watch_ids.length }}</span>
        </button>
      </template>

      <div v-if="editing?.mode === 'create'" class="group-tab editing">
        <input
          :ref="setInputRef"
          v-model="draftName"
          class="group-input"
          spellcheck="false"
          :maxlength="GROUP_NAME_MAX_LEN"
          aria-label="新分组名称"
          @keydown.enter.prevent="commitEdit"
          @keydown.esc.prevent="cancelEdit"
          @blur="commitEdit"
        />
      </div>
      <button v-else class="group-add" type="button" @click="startCreate">
        <Plus :size="11" aria-hidden="true" />
        新建分组
      </button>
    </div>

    <p v-if="editError" class="edit-error" role="alert">{{ editError }}</p>

    <NDropdown
      :show="showCtx"
      :x="ctxX"
      :y="ctxY"
      :options="ctxOptions"
      placement="bottom-start"
      trigger="manual"
      @select="handleCtxSelect"
      @clickoutside="showCtx = false"
    />
  </div>
</template>

<style scoped>
.group-tabs-wrap {
  min-width: 0;
  flex: 1;
}
.group-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  scrollbar-width: none;
}
.group-tabs::-webkit-scrollbar {
  display: none;
}

.group-tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 10px;
  flex-shrink: 0;
  border: none;
  border-bottom: 2px solid transparent;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  background: transparent;
  color: var(--color-text-tertiary);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  white-space: nowrap;
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.group-tab:hover {
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
}
/* 当前分组用下划线 + 文字色，不只靠颜色 */
.group-tab.active {
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
  border-bottom-color: var(--color-accent);
}
.group-tab:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}
.group-tab .cnt {
  min-width: 18px;
  height: 15px;
  padding: 0 5px;
  border-radius: var(--radius-full);
  background: var(--color-surface-2);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 15px;
  text-align: center;
}
.group-tab.active .cnt {
  background: var(--color-accent-dim);
  color: var(--color-accent);
}

.group-tab.editing {
  padding: 0;
  border-bottom-color: var(--color-accent);
}
.group-input {
  width: 92px;
  height: 28px;
  padding: 0 6px;
  border: 1px solid var(--color-accent);
  border-radius: var(--radius-sm);
  background: var(--color-surface-0);
  color: var(--color-text-primary);
  font-family: var(--font-sans);
  font-size: var(--text-sm);
  outline: none;
}

.group-add {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 26px;
  padding: 0 8px;
  margin-left: 2px;
  flex-shrink: 0;
  border: 1px dashed var(--color-border-1);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: color var(--transition-fast), border-color var(--transition-fast);
}
.group-add:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
}
.group-add:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

.edit-error {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--color-warning);
}
</style>
