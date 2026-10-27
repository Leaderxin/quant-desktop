<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { NModal, NCard, NButton, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import type { ImportEntry, ImportRowResult } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';
import { decodeCsv, mapWatchlistRows, parseWatchlistCsv, type CsvImportRow } from '@/utils/csv';
import { formatCode } from '@/utils/format';
import { Check, CircleAlert, Download, FileUp, Minus } from '@lucide/vue';
import GroupPicker from './GroupPicker.vue';

/** 与后端 `MAX_IMPORT_ROWS` 一致：前端先拦一道，省得传上去再被打回来 */
const MAX_ROWS = 500;

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: 'update:show', val: boolean): void }>();
const message = useMessage();
const watchlist = useWatchlistStore();

const targetGroupIds = ref<number[]>([watchlist.activeGroupId]);
const fileInput = ref<HTMLInputElement | null>(null);
const dropzone = ref<HTMLButtonElement | null>(null);

const fileName = ref('');
const rows = ref<CsvImportRow[]>([]);
const skipped = ref(0);
const parseError = ref('');
const dragging = ref(false);
const importing = ref(false);
const results = ref<ImportRowResult[]>([]);
const templatePath = ref('');

const counts = computed(() => {
  const c = { added: 0, exists: 0, failed: 0 };
  for (const r of results.value) c[r.status] += 1;
  return c;
});

const canImport = computed(() => rows.value.length > 0 && !importing.value);
/** 导入完成后同一个文件不该再点一次「开始导入」—— 换文件会清空结果。 */
const done = computed(() => results.value.length > 0);

function reset() {
  fileName.value = '';
  rows.value = [];
  skipped.value = 0;
  parseError.value = '';
  dragging.value = false;
  importing.value = false;
  results.value = [];
  templatePath.value = '';
  targetGroupIds.value = [
    watchlist.groups.some((g) => g.id === watchlist.activeGroupId)
      ? watchlist.activeGroupId
      : watchlist.defaultGroupId,
  ];
}

/**
 * 每次打开都重置：NModal 关闭后并不卸载内容，不重置的话上次的文件名、
 * 结果列表会残留到下一次。
 */
watch(() => props.show, async (visible) => {
  if (!visible) return;
  reset();
  await nextTick();
  dropzone.value?.focus();
});

function close() {
  emit('update:show', false);
}

async function downloadTemplate() {
  try {
    // 原生「另存为」：目录由用户选，取消时返回 null
    const path = await save({
      title: '保存自选导入模板',
      defaultPath: 'QuantDesktop-自选导入模板.xlsx',
      filters: [{ name: 'Excel 工作簿', extensions: ['xlsx'] }],
    });
    if (!path) return;
    templatePath.value = await invoke<string>('save_import_template', { path });
    message.success('模板已保存');
  } catch (e) {
    message.error(`保存模板失败: ${e}`);
  }
}

function pickFile() {
  fileInput.value?.click();
}

async function readFile(file: File) {
  parseError.value = '';
  results.value = [];
  fileName.value = file.name;
  try {
    const buffer = await file.arrayBuffer();
    // .xlsx / .xls 由后端拆成单元格网格，CSV 在前端解（编码/引号都在那儿）。
    // 两者的表头识别与列映射走同一段 mapWatchlistRows。
    const isExcel = /\.(xlsx|xls)$/i.test(file.name);
    const parsed = isExcel
      ? mapWatchlistRows(
          await invoke<string[][]>('read_spreadsheet', {
            bytes: Array.from(new Uint8Array(buffer)),
          }),
        )
      : parseWatchlistCsv(decodeCsv(buffer));

    if (parsed.rows.length === 0) {
      rows.value = [];
      parseError.value = '这个文件里没有可导入的行 —— 需要「代码」或「名称」列';
      return;
    }
    if (parsed.rows.length > MAX_ROWS) {
      rows.value = [];
      parseError.value = `一次最多导入 ${MAX_ROWS} 条，当前 ${parsed.rows.length} 条，请拆分文件后再试`;
      return;
    }
    rows.value = parsed.rows;
    skipped.value = parsed.skipped;
  } catch (e) {
    rows.value = [];
    parseError.value = `读取文件失败: ${e}`;
  }
}

function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) void readFile(file);
  // 清空 value：同一个文件改完再选一次，也要能触发 change
  input.value = '';
}

function onDrop(e: DragEvent) {
  dragging.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (file) void readFile(file);
}

async function doImport() {
  if (!canImport.value) return;
  importing.value = true;
  parseError.value = '';
  try {
    const entries: ImportEntry[] = rows.value.map((r) => ({ code: r.code, name: r.name }));
    results.value = await invoke<ImportRowResult[]>('import_watchlist', {
      entries,
      groupIds: targetGroupIds.value,
    });
    // 后端只发一次 watchlist-changed；主窗口这边重拉一次快照即可
    await watchlist.fetchWatchlist();
  } catch (e) {
    parseError.value = `导入失败: ${e}`;
  } finally {
    importing.value = false;
  }
}

/** 后端只知道提交顺序，展示时换回文件里的物理行号 */
function fileLineOf(result: ImportRowResult): number {
  return rows.value[result.line - 1]?.line ?? result.line;
}

function statusText(r: ImportRowResult): string {
  if (r.status === 'added') return '已加入';
  if (r.status === 'exists') return '已在所选分组';
  return r.message ?? '导入失败';
}

function rowTitle(r: ImportRowResult): string {
  return r.name || r.input || '（空行）';
}
</script>

<template>
  <NModal
    :show="props.show"
    :auto-focus="false"
    :mask-closable="!importing"
    :close-on-esc="!importing"
    @update:show="emit('update:show', $event)"
  >
    <NCard
      title="导入自选"
      style="width: 560px;"
      :closable="!importing"
      :bordered="false"
      @close="close"
    >
      <div class="import-body">
        <!-- 1 分组 -->
        <section class="block">
          <h3 class="block-title"><span class="step">1</span>导入到分组</h3>
          <GroupPicker
            v-model="targetGroupIds"
            :groups="watchlist.groups"
            label="导入到分组"
            :hint="watchlist.groups.length > 1
              ? '可同时勾选多个分组；已在其它分组的标的不会被移出。'
              : ''"
          />
        </section>

        <!-- 2 文件 -->
        <section class="block">
          <h3 class="block-title"><span class="step">2</span>选择 CSV 文件</h3>

          <div class="template-row">
            <button type="button" class="link-btn" @click="downloadTemplate">
              <Download :size="14" aria-hidden="true" />
              下载模板
            </button>
            <span class="template-hint">
              两列：代码、名称；填一个即可。代码带不带 sh / sz 前缀都行
            </span>
          </div>

          <button
            ref="dropzone"
            type="button"
            class="dropzone"
            :class="{ 'is-dragging': dragging, 'has-file': rows.length > 0 }"
            @click="pickFile"
            @dragover.prevent="dragging = true"
            @dragleave="dragging = false"
            @drop.prevent="onDrop"
          >
            <FileUp :size="18" aria-hidden="true" class="dropzone-icon" />
            <span class="dropzone-main">{{ fileName || '点击选择或拖入表格文件' }}</span>
            <span class="dropzone-sub">
              <template v-if="rows.length > 0">
                {{ rows.length }} 条待导入<template v-if="skipped > 0">，跳过 {{ skipped }} 行空行</template>
              </template>
              <template v-else>支持 .xlsx / .xls / .csv（UTF-8 或 GBK），一次最多 {{ MAX_ROWS }} 条</template>
            </span>
          </button>
          <input
            ref="fileInput"
            class="file-input"
            type="file"
            accept=".csv,.xlsx,.xls,text/csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,application/vnd.ms-excel"
            tabindex="-1"
            aria-hidden="true"
            @change="onFileChange"
          />

          <p v-if="parseError" class="error" role="alert">
            <CircleAlert :size="14" aria-hidden="true" class="error-icon" />
            <span>{{ parseError }}</span>
          </p>
          <p v-if="templatePath" class="template-path">
            模板已保存到 {{ templatePath }}
          </p>
        </section>

        <!-- 3 结果 -->
        <section v-if="done" class="block">
          <h3 class="block-title"><span class="step">3</span>导入结果</h3>
          <p class="summary" role="status" aria-live="polite">
            成功 {{ counts.added }} 条<template v-if="counts.exists > 0"> · 已存在 {{ counts.exists }} 条</template><template v-if="counts.failed > 0"> · 失败 {{ counts.failed }} 条</template>
          </p>
          <ul class="result-list">
            <li
              v-for="r in results"
              :key="r.line"
              class="result-row"
              :class="`is-${r.status}`"
            >
              <Check v-if="r.status === 'added'" :size="14" class="result-icon" aria-hidden="true" />
              <Minus v-else-if="r.status === 'exists'" :size="14" class="result-icon" aria-hidden="true" />
              <CircleAlert v-else :size="14" class="result-icon" aria-hidden="true" />
              <span class="result-main">
                <span class="result-title">
                  <span class="result-name">{{ rowTitle(r) }}</span>
                  <span v-if="r.code" class="result-code tabular-nums">{{ formatCode(r.code) }}</span>
                </span>
                <span class="result-sub">{{ statusText(r) }} · 第 {{ fileLineOf(r) }} 行</span>
              </span>
            </li>
          </ul>
        </section>
      </div>

      <template #footer>
        <div class="footer-actions">
          <NButton v-if="!done && !importing" quaternary @click="close">取消</NButton>
          <NButton
            v-if="!done"
            type="primary"
            :loading="importing"
            :disabled="!canImport"
            @click="doImport"
          >
            开始导入
          </NButton>
          <NButton v-else type="primary" @click="close">完成</NButton>
        </div>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.import-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}
.block {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.block-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}
/* 序号不是装饰：三步是固定顺序，让「先选组、再选文件、最后看结果」一眼可见 */
.step {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: var(--radius-full);
  background: var(--color-surface-2);
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-family: var(--font-mono);
}

.template-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}
.link-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: none;
  background: none;
  color: var(--color-accent);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  cursor: pointer;
}
.link-btn:hover {
  text-decoration: underline;
}
.link-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}
.template-hint {
  font-size: 11px;
  color: var(--color-text-tertiary);
  line-height: 1.5;
}

/* 拖放区做成一个按钮：它只做一件事（选文件），键盘与读屏都按按钮处理 */
.dropzone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: var(--space-4);
  border: 1px dashed var(--color-border-1);
  border-radius: var(--radius-md);
  background: var(--color-surface-1);
  color: var(--color-text-secondary);
  font-family: var(--font-sans);
  cursor: pointer;
  text-align: center;
  transition: border-color var(--transition-fast), background var(--transition-fast);
}
.dropzone:hover,
.dropzone.is-dragging {
  border-color: var(--color-accent);
  background: var(--color-accent-dim);
}
.dropzone:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}
.dropzone.has-file {
  border-style: solid;
}
.dropzone-icon {
  color: var(--color-text-tertiary);
}
.dropzone-main {
  font-size: var(--text-sm);
  color: var(--color-text-primary);
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dropzone-sub {
  font-size: 11px;
  color: var(--color-text-tertiary);
}
.file-input {
  display: none;
}

.error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin: 0;
  font-size: var(--text-xs);
  line-height: 1.5;
  color: var(--color-warning);
}
.error-icon {
  flex-shrink: 0;
  margin-top: 2px;
}
.template-path {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-tertiary);
  word-break: break-all;
}

.summary {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
}
.result-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
}
.result-row {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--color-surface-1);
}
.result-icon {
  flex-shrink: 0;
  margin-top: 1px;
}
/* 状态不只用颜色表达：图标形状 + 文案都在说同一件事 */
.is-added .result-icon { color: var(--color-accent); }
.is-exists .result-icon { color: var(--color-text-tertiary); }
.is-failed .result-icon { color: var(--color-warning); }
.result-main {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.result-title {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}
.result-name {
  font-size: var(--text-sm);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.result-code {
  flex-shrink: 0;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  color: var(--color-text-tertiary);
}
.result-sub {
  font-size: 11px;
  line-height: 1.5;
  color: var(--color-text-tertiary);
  word-break: break-all;
}
.is-failed .result-sub {
  color: var(--color-warning);
}

.footer-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>
