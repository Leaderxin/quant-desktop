<script setup lang="ts">
// 自选分组标签栏（同花顺式）：点击切换 / 双击就地重命名 / 右键菜单 / ＋新建。
//
// 标签本身是按钮而不是链接：这里没有路由，切换分组只是换一份本地过滤结果，
// 用 <a> 会让人以为会跳页。
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { NDropdown } from 'naive-ui';
import { useWatchlistStore } from '@/stores/watchlist';
import { GROUP_NAME_MAX_LEN } from '@/utils/prefs';
import { computeGroupRemovalImpact, type GroupRemovalImpact } from '@/utils/watchGroups';
import { moveByStep } from '@/utils/dragSort';
import { ChevronLeft, ChevronRight, Plus } from '@lucide/vue';
import DeleteGroupDialog from './DeleteGroupDialog.vue';
import type { WatchGroup, WatchItem } from '@/types';

const watchlist = useWatchlistStore();

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
  { label: '左移', key: 'left', disabled: ctxIndex.value <= 0 },
  { label: '右移', key: 'right', disabled: ctxIndex.value >= watchlist.groups.length - 1 },
]);

const ctxIndex = computed(() =>
  ctxGroup.value ? watchlist.groups.findIndex((g) => g.id === ctxGroup.value!.id) : -1,
);

function moveGroup(index: number, direction: -1 | 1) {
  const next = moveByStep(watchlist.groups.map((g) => g.id), index, direction);
  if (!next) return; // 已在最左 / 最右：跳过 IPC
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
    case 'left':
      moveGroup(ctxIndex.value, -1);
      break;
    case 'right':
      moveGroup(ctxIndex.value, 1);
      break;
  }
}

/** 正在确认删除的分组；null 表示确认框关闭。 */
const deleteTarget = ref<WatchGroup | null>(null);

/**
 * 删除分组前如实说明影响面。
 *
 * 名单在本地算得出来（快照里已有每组的有序成员 id），不需要额外的预览 IPC：
 * `orphanIds` 是「只属于这一个分组」的股票，它们会被并入 `deleteFallbackName`；
 * `keptIds` 还留在别的分组里，不受影响。确认框要把两边都逐只列出来 —— 只给
 * 一个数字的话，「删了会不会丢东西」还是得靠猜。
 */
const deleteImpact = computed<GroupRemovalImpact>(() =>
  deleteTarget.value
    ? computeGroupRemovalImpact(watchlist.groups, deleteTarget.value.id)
    : { orphanIds: [], keptIds: [] },
);

/**
 * 把影响面里的 id 换成可展示的标的。
 * 池里找不到的（关联表残留的已删 watch_id）直接跳过 —— 与 `visibleItems`
 * 同一条兜底，一条坏数据不该让整份名单消失。
 */
function resolveStocks(ids: number[]): WatchItem[] {
  const byId = watchlist.itemsById;
  return ids.map((id) => byId.get(id)).filter((it): it is WatchItem => !!it);
}

const deleteOrphans = computed(() => resolveStocks(deleteImpact.value.orphanIds));
const deleteKept = computed(() => resolveStocks(deleteImpact.value.keptIds));

/**
 * 孤儿会并入哪个分组 —— 展示顺序里第一个**别的**分组，与后端
 * `delete_watch_group` 的 fallback 查询同一条规则（`id != ?1 ORDER BY
 * sort_order, id LIMIT 1`）。
 *
 * 不图省事取 `defaultGroupId`：被删的恰好是默认分组（展示顺序第一个）时，
 * 那个 id 指向的正是正在删的分组自己，文案会变成「移入「（它自己）」」。
 */
const deleteFallbackName = computed(() => {
  const target = deleteTarget.value;
  if (!target) return '';
  return watchlist.groups.find((g) => g.id !== target.id)?.name ?? '';
});

function requestDelete(g: WatchGroup) {
  deleteTarget.value = g;
}

function closeDeleteDialog() {
  deleteTarget.value = null;
}

async function confirmDelete() {
  const target = deleteTarget.value;
  // 先关框再落库：删除是一次 IPC + 重拉快照，等它回来才关会让按钮看起来发木。
  deleteTarget.value = null;
  if (target) await watchlist.deleteGroup(target.id);
}

// ── 横向溢出 ──
// 标签栏隐藏滚动条（保持头部干净），但必须给出可滚动的提示，否则分组一多，
// 鼠标用户根本不知道右边还有内容：纵向滚轮转成横向滚动 + 两侧渐隐 + 箭头按钮。

const tabsViewport = ref<HTMLElement | null>(null);
const canScrollLeft = ref(false);
const canScrollRight = ref(false);
let hintObserver: ResizeObserver | null = null;

function updateScrollHints() {
  const el = tabsViewport.value;
  if (!el) return;
  // 平滑滚动/缩放时 scrollLeft 可能是小数，留 1px 容差，箭头才不会在边缘抖动
  canScrollLeft.value = el.scrollLeft > 1;
  canScrollRight.value = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
}

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === 'function'
    && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

// 滚轮滑行：滚轮事件频率高、每次 delta 不等，直接赋值 scrollLeft 会一格一格地跳。
// 改为时间基准的 ease-out（约 220ms），每次事件从**当前位置**重新起算：
// 连续滚动会自然接续，反向滚动也能立刻响应，不会被上一个目标拖住。
const WHEEL_GLIDE_MS = 220;
let wheelRaf = 0;
let wheelFrom = 0;
let wheelTo = 0;
let wheelStart = 0;

function stopWheelGlide() {
  if (wheelRaf) cancelAnimationFrame(wheelRaf);
  wheelRaf = 0;
}

function startWheelGlide(el: HTMLElement, delta: number) {
  const max = el.scrollWidth - el.clientWidth;
  const to = Math.max(0, Math.min(max, el.scrollLeft + delta));
  if (Math.abs(to - el.scrollLeft) < 0.5) return;
  if (prefersReducedMotion()) {
    stopWheelGlide();
    el.scrollLeft = to;
    return;
  }

  stopWheelGlide();
  wheelFrom = el.scrollLeft;
  wheelTo = to;
  wheelStart = performance.now();
  const tick = (now: number) => {
    const t = Math.min(1, (now - wheelStart) / WHEEL_GLIDE_MS);
    const eased = 1 - Math.pow(1 - t, 3); // easeOutCubic
    el.scrollLeft = wheelFrom + (wheelTo - wheelFrom) * eased;
    if (t < 1) {
      wheelRaf = requestAnimationFrame(tick);
    } else {
      el.scrollLeft = wheelTo;
      wheelRaf = 0;
    }
  };
  wheelRaf = requestAnimationFrame(tick);
}

/** 「新建分组」已固定在滚动区外，这里只负责把当前分组滚进视野，不碰其它布局 */
function scrollActiveIntoView(behavior: ScrollBehavior = 'smooth') {
  const viewport = tabsViewport.value;
  if (!viewport) return;
  const active = viewport.querySelector<HTMLElement>('.group-tab.active');
  if (!active) return;

  stopWheelGlide();
  const motion = prefersReducedMotion() ? 'auto' : behavior;
  // 用 bounding rect 而不是 offsetLeft：活动标签的 offsetParent 不一定是滚动容器。
  // pad 要盖过边缘渐隐（24px）和箭头按钮的叠压，否则当前标签的计数会被挡住
  const pad = 28;
  const view = viewport.getBoundingClientRect();
  const tab = active.getBoundingClientRect();
  if (tab.left < view.left + pad) {
    viewport.scrollBy({ left: tab.left - view.left - pad, behavior: motion });
  } else if (tab.right > view.right - pad) {
    viewport.scrollBy({ left: tab.right - view.right + pad, behavior: motion });
  }
}

function scrollTabs(direction: -1 | 1) {
  const el = tabsViewport.value;
  if (!el) return;
  stopWheelGlide();
  const step = Math.max(80, Math.round(el.clientWidth * 0.6));
  el.scrollBy({
    left: direction * step,
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
  });
}

function handleTabsWheel(e: WheelEvent) {
  const el = tabsViewport.value;
  if (!el) return;
  // 没溢出、或已经滚到这一侧的尽头时，把滚轮交还给外层，不要吞掉滚动
  if (el.scrollWidth <= el.clientWidth) return;
  const dominant = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
  if (dominant === 0) return;
  // deltaMode：像素（0）/ 行（1）/ 页（2）。鼠标滚轮常报「行」，
  // 直接当像素用会几乎滚不动
  const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientWidth : 1;
  const delta = dominant * unit;
  const max = el.scrollWidth - el.clientWidth;
  const target = Math.max(0, Math.min(max, el.scrollLeft + delta));
  if (target === el.scrollLeft) return;
  e.preventDefault();
  startWheelGlide(el, delta);
}

onMounted(async () => {
  await nextTick();
  updateScrollHints();
  scrollActiveIntoView('auto');
  if (typeof ResizeObserver !== 'undefined' && tabsViewport.value) {
    hintObserver = new ResizeObserver(() => updateScrollHints());
    hintObserver.observe(tabsViewport.value);
  }
});

onUnmounted(() => {
  hintObserver?.disconnect();
  hintObserver = null;
  stopWheelGlide();
});

// 分组增删/改名/排序、编辑态切换都会改变内容宽度；当前分组变化时要保证它可见。
// 深度监听是必要的：组内股票数变化会改变标签宽度。
watch(
  [() => watchlist.groups, () => watchlist.activeGroupId, editing],
  async () => {
    await nextTick();
    updateScrollHints();
    scrollActiveIntoView();
  },
  { deep: true },
);
</script>

<template>
  <div class="group-tabs-wrap">
    <div class="group-tabs-row">
      <div class="group-tabs-scroller">
        <div
          ref="tabsViewport"
          class="group-tabs-viewport"
          :class="{ 'fade-left': canScrollLeft, 'fade-right': canScrollRight }"
          @scroll.passive="updateScrollHints"
          @wheel="handleTabsWheel"
        >
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
          </div>
        </div>

        <button
          v-if="canScrollLeft"
          class="tabs-scroll-btn left"
          type="button"
          title="向左滚动"
          aria-label="向左滚动分组"
          @click="scrollTabs(-1)"
        >
          <ChevronLeft :size="14" aria-hidden="true" />
        </button>
        <button
          v-if="canScrollRight"
          class="tabs-scroll-btn right"
          type="button"
          title="向右滚动"
          aria-label="向右滚动分组"
          @click="scrollTabs(1)"
        >
          <ChevronRight :size="14" aria-hidden="true" />
        </button>
      </div>

      <!-- 新建入口固定在滚动区外：分组再多也不会被滚出视野 -->
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

    <DeleteGroupDialog
      :group-name="deleteTarget?.name ?? null"
      :fallback-name="deleteFallbackName"
      :orphans="deleteOrphans"
      :kept="deleteKept"
      @close="closeDeleteDialog"
      @confirm="confirmDelete"
    />
  </div>
</template>

<style scoped>
.group-tabs-wrap {
  min-width: 0;
  flex: 1;
}
.group-tabs-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.group-tabs-scroller {
  position: relative;
  flex: 1;
  min-width: 0;
}
.group-tabs-viewport {
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
  /* 横向滚到尽头时不要让滚动链传导给外层 */
  overscroll-behavior-x: contain;
}
.group-tabs-viewport::-webkit-scrollbar {
  display: none;
}

/* 可滚动一侧做渐隐。mask 不依赖背景色，深浅主题、透明行情条上都能用 */
.group-tabs-viewport.fade-left {
  -webkit-mask-image: linear-gradient(to right, transparent 0, #000 24px, #000 100%);
  mask-image: linear-gradient(to right, transparent 0, #000 24px, #000 100%);
}
.group-tabs-viewport.fade-right {
  -webkit-mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 24px), transparent 100%);
  mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 24px), transparent 100%);
}
.group-tabs-viewport.fade-left.fade-right {
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0,
    #000 24px,
    #000 calc(100% - 24px),
    transparent 100%
  );
  mask-image: linear-gradient(
    to right,
    transparent 0,
    #000 24px,
    #000 calc(100% - 24px),
    transparent 100%
  );
}

/* 悬在滚动区两端的圆形箭头：鼠标用户的可点击提示，键盘同样可达 */
.tabs-scroll-btn {
  position: absolute;
  top: 50%;
  z-index: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--color-border-1);
  border-radius: var(--radius-full);
  background: var(--color-surface-2);
  color: var(--color-text-secondary);
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transform: translateY(-50%);
  transition: color var(--transition-fast), border-color var(--transition-fast);
}
.tabs-scroll-btn.left {
  left: 0;
}
.tabs-scroll-btn.right {
  right: 0;
}
.tabs-scroll-btn:hover {
  color: var(--color-accent);
  border-color: var(--color-accent);
}
.tabs-scroll-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

.group-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  /* 明确让行宽等于内容宽：scrollWidth 才稳定包含溢出的标签 */
  width: max-content;
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
