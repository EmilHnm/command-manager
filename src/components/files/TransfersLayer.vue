<template>
  <!-- Label that follows the pointer while dragging files -->
  <div
    v-if="ghost"
    class="file-drag-ghost"
    :class="{ invalid: ghost.invalid }"
    :style="{ left: `${ghost.x + 14}px`, top: `${ghost.y + 14}px` }"
  >
    <component :is="ghost.icon" :size="13" />
    <span>{{ ghost.text }}</span>
  </div>

  <!-- Transfers drawer (⇅ in the status bar, Ctrl+Shift+J) -->
  <section v-if="drawerOpen" class="transfers-drawer" @keydown.esc="drawerOpen = false">
    <header class="td-header">
      <div class="td-title">
        <ArrowUpDown :size="14" />
        <span>Truyền File</span>
        <span v-if="activeTotals.count > 0" class="td-sub">
          {{ activeTotals.count }} đang chạy · {{ activeTotals.percent }}% · {{ formatBytes(activeTotals.bytesPerSec) }}/s
        </span>
      </div>
      <div class="td-actions">
        <button type="button" class="btn btn-ghost btn-sm" :disabled="!hasFinished" @click="clearFinished">
          Xóa mục đã xong
        </button>
        <button type="button" class="td-icon-btn" title="Đóng (Esc)" @click="drawerOpen = false">
          <X :size="14" />
        </button>
      </div>
    </header>

    <div class="td-list custom-scrollbar">
      <!-- Server files open in a local app -->
      <div v-if="edits.length > 0" class="td-edits">
        <div class="td-section-title">Tệp đang sửa · tự lưu lên máy chủ khi bạn lưu</div>
        <div v-for="item in edits" :key="item.local_path" class="td-edit" :class="`edit-${item.status}`">
          <FilePen :size="13" class="td-edit-icon" />
          <div class="td-job-text">
            <span class="td-route">
              {{ describe(item.location) }}
              <span class="td-path">{{ item.remote_path }}</span>
            </span>
            <span class="td-meta" :class="{ 'td-failed': item.status === 'conflict' || item.status === 'error' }">
              {{ editStatusLabel(item) }}
            </span>
          </div>
          <div class="td-edit-actions">
            <button
              v-if="item.status === 'conflict'"
              type="button"
              class="btn btn-secondary btn-sm"
              title="Ghi bản của bạn đè lên tệp trên máy chủ"
              @click="pushEdit(item.local_path, true)"
            >
              Ghi đè
            </button>
            <button
              v-else
              type="button"
              class="td-icon-btn"
              title="Tải lên ngay"
              :disabled="item.status === 'uploading'"
              @click="pushEdit(item.local_path, false)"
            >
              <Upload :size="13" />
            </button>
            <button
              type="button"
              class="td-icon-btn"
              title="Ngừng theo dõi và xóa bản sao trên máy này"
              @click="stopEdit(item.local_path)"
            >
              <X :size="13" />
            </button>
          </div>
        </div>
      </div>

      <div v-if="jobs.length === 0 && edits.length === 0" class="td-empty">
        Chưa có lượt truyền nào. Kéo tệp giữa hai Files pane, hoặc kéo tệp từ trình quản lý file của
        máy vào một Files pane.
      </div>

      <div v-for="job in jobs" :key="job.id" class="td-job" :class="`status-${job.status}`">
        <div class="td-job-main" @click="toggleJob(job.id)">
          <ChevronDown v-if="expanded.has(job.id)" :size="13" class="td-chevron" />
          <ChevronRight v-else :size="13" class="td-chevron" />
          <div class="td-job-text">
            <span class="td-route">
              {{ describe(job.sources[0]?.location) }}
              <span class="td-path">{{ sourceLabel(job) }}</span>
              <ArrowRight :size="11" />
              {{ describe(job.dest.location) }}
              <span class="td-path">{{ job.dest.path }}</span>
              <span v-if="isRelayed(job)" class="td-relay" title="Hai máy chủ khác nhau: dữ liệu đi qua máy này">
                ⇄ qua máy này
              </span>
            </span>
            <span class="td-meta">
              {{ job.mode === 'move' ? 'Di chuyển' : 'Sao chép' }} ·
              {{ job.files_done }}/{{ job.files_total }} tệp ·
              {{ formatBytes(job.done_bytes) }} / {{ formatBytes(job.total_bytes) }}
              <template v-if="job.bytes_per_sec > 0"> · {{ formatBytes(job.bytes_per_sec) }}/s</template>
              <template v-if="job.files_skipped > 0"> · bỏ qua {{ job.files_skipped }}</template>
              <span v-if="job.files_failed > 0" class="td-failed"> · {{ job.files_failed }} lỗi</span>
            </span>
          </div>
          <div class="td-progress">
            <div class="td-bar">
              <div class="td-bar-fill" :style="{ width: `${percent(job)}%` }" />
            </div>
            <span class="td-status">{{ statusLabel(job) }}</span>
          </div>
          <div class="td-job-actions" @click.stop>
            <button
              v-if="job.status === 'running' || job.status === 'scanning'"
              type="button"
              class="td-icon-btn"
              title="Hủy lượt truyền"
              @click="cancel(job.id)"
            >
              <X :size="13" />
            </button>
            <button
              v-else-if="job.status === 'failed' || job.status === 'cancelled'"
              type="button"
              class="td-icon-btn"
              title="Thử lại các tệp lỗi"
              @click="retry(job.id)"
            >
              <RotateCw :size="13" />
            </button>
          </div>
        </div>

        <div v-if="expanded.has(job.id)" class="td-items">
          <div v-if="job.error" class="td-error">{{ job.error }}</div>
          <div v-for="warning in job.warnings" :key="warning" class="td-warning">{{ warning }}</div>
          <div v-for="item in visibleItems(job)" :key="item.id" class="td-item" :class="`item-${item.status}`">
            <component :is="itemIcon(item.status)" :size="12" class="td-item-icon" />
            <span class="td-item-name">{{ item.name }}{{ item.is_dir ? '/' : '' }}</span>
            <span v-if="item.error" class="td-item-error" :title="item.error">{{ item.error }}</span>
            <span v-else-if="!item.is_dir" class="td-item-size">
              <template v-if="item.status === 'running'">{{ itemPercent(item) }}% · </template>
              <template v-if="item.resumed_from > 0">
                chép tiếp từ {{ formatBytes(item.resumed_from) }} ·
              </template>
              {{ formatBytes(item.size) }}
            </span>
          </div>
          <div v-if="job.items_truncated" class="td-warning">Danh sách quá dài, chỉ hiện một phần.</div>
        </div>
      </div>
    </div>
  </section>

  <!-- MOD-20: one conflict at a time -->
  <div v-if="conflict" class="modal-backdrop" @keydown.esc="answer('skip')">
    <div class="conflict-modal" role="dialog" aria-modal="true">
      <header class="cm-header">
        <AlertTriangle :size="15" class="cm-icon" />
        <span>{{ conflict.kind_mismatch ? 'Trùng tên với loại khác' : 'Tệp đã tồn tại' }}</span>
      </header>
      <div class="cm-body">
        <div class="cm-path font-mono">{{ conflict.path }}</div>
        <table v-if="!conflict.kind_mismatch" class="cm-compare">
          <thead>
            <tr>
              <th />
              <th>Kích thước</th>
              <th>Sửa đổi</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Mới</td>
              <td>{{ formatBytes(conflict.source_size) }}</td>
              <td>
                {{ formatTime(conflict.source_mtime) }}
                <span v-if="sourceNewer" class="cm-newer">mới hơn</span>
              </td>
            </tr>
            <tr>
              <td>Đang có</td>
              <td>{{ formatBytes(conflict.target_size) }}</td>
              <td>{{ formatTime(conflict.target_mtime) }}</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="cm-note">
          Ở đích đã có một mục khác loại (tệp ↔ thư mục) cùng tên; chỉ có thể bỏ qua hoặc giữ cả hai.
        </p>

        <div class="cm-options">
          <label v-for="option in options" :key="option.value" class="cm-option">
            <input v-model="choice" type="radio" :value="option.value" />
            <span>{{ option.label }}</span>
          </label>
        </div>
        <label class="cm-option cm-apply">
          <input v-model="applyToRest" type="checkbox" />
          <span>Áp dụng cho các xung đột còn lại trong lượt truyền này</span>
        </label>
      </div>
      <footer class="cm-footer">
        <button type="button" class="btn btn-ghost" @click="answer('cancel_job')">Hủy cả lượt</button>
        <button type="button" class="btn btn-primary" @click="answer(choice)">Tiếp tục</button>
      </footer>
    </div>
  </div>

  <!-- Short notice (finished jobs, errors) -->
  <div
    v-if="notice"
    class="transfer-toast"
    :class="notice.kind"
    @click="drawerOpen = true; notice = null"
  >
    <ArrowUpDown :size="13" />
    <span>{{ notice.text }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  AlertTriangle,
  ArrowRight,
  ArrowUpDown,
  Ban,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  CircleX,
  Files,
  LoaderCircle,
  MinusCircle,
  MoveRight,
  RotateCw,
  TerminalSquare,
  FilePen,
  Upload,
  X,
} from 'lucide-vue-next';
import type {
  ConflictAction,
  EditInfo,
  FileLocation,
  ItemSnapshot,
  ItemStatus,
  JobSnapshot,
} from '@/types/files';
import { formatBytes } from '@/types/files';
import { useTransfers } from '@/composables/useTransfers';
import { useFileDrag, type TerminalDropInfo } from '@/composables/useFileDrag';
import { useSshHosts } from '@/composables/useSshHosts';

const {
  jobs,
  edits,
  pushEdit,
  stopEdit,
  conflicts,
  drawerOpen,
  activeTotals,
  notice,
  cancel,
  retry,
  clearFinished,
  loadDetails,
  resolveConflict,
} = useTransfers();
const { drag, osDrag } = useFileDrag();
const { hosts } = useSshHosts();

const describe = (location?: FileLocation) => {
  if (!location || location.kind === 'local') return 'Máy này';
  return hosts.value.find((host) => host.id === location.host_id)?.name ?? 'Máy chủ';
};

/** What dropping on a terminal will do (see useFileDrag.dropOnTerminal). */
const terminalAction = (info: TerminalDropInfo, from: FileLocation, what: string) => {
  if (!info.hostId) return `Chèn đường dẫn ${what} vào terminal`;
  if (from.kind === 'ssh' && from.host_id === info.hostId) {
    return `Chèn đường dẫn ${what} (trên ${describe(from)}) vào terminal`;
  }
  const folder = info.cwd || '~ (thư mục home)';
  return `Tải ${what} lên ${describe({ kind: 'ssh', host_id: info.hostId })}:${folder} rồi chèn đường dẫn`;
};

const ghost = computed(() => {
  const state = drag.value;
  if (state?.dragging) {
    const count = state.entries.length;
    const what = count === 1 ? state.entries[0].name : `${count} mục`;
    if (state.invalid) return { x: state.x, y: state.y, invalid: true, icon: Ban, text: state.invalid };
    if (!state.target) {
      return {
        x: state.x,
        y: state.y,
        invalid: false,
        icon: Files,
        text: `${what} · thả vào Files pane hoặc terminal`,
      };
    }
    if (state.target.kind === 'terminal') {
      return {
        x: state.x,
        y: state.y,
        invalid: false,
        icon: TerminalSquare,
        text: terminalAction(state.target.info, state.location, what),
      };
    }
    const target = state.target;
    const verb = state.move ? 'Di chuyển' : 'Chép';
    const relay =
      state.location.kind === 'ssh' &&
      target.location.kind === 'ssh' &&
      state.location.host_id !== target.location.host_id
        ? ' ⇄ qua máy này'
        : '';
    return {
      x: state.x,
      y: state.y,
      invalid: false,
      icon: state.move ? MoveRight : Files,
      text: `${verb} ${what} → ${describe(target.location)}:${target.dir}${relay}${state.move ? '' : ' · Shift = Di chuyển'}`,
    };
  }
  const os = osDrag.value;
  if (os) {
    if (!os.target) {
      return { x: os.x, y: os.y, invalid: true, icon: Ban, text: 'Thả vào Files pane hoặc terminal' };
    }
    if (os.invalid) return { x: os.x, y: os.y, invalid: true, icon: Ban, text: os.invalid };
    if (os.target.kind === 'terminal') {
      return {
        x: os.x,
        y: os.y,
        invalid: false,
        icon: TerminalSquare,
        text: terminalAction(os.target.info, { kind: 'local' }, `${os.count || ''} tệp`),
      };
    }
    return {
      x: os.x,
      y: os.y,
      invalid: false,
      icon: Files,
      text: `Chép ${os.count || ''} tệp → ${describe(os.target.location)}:${os.target.dir}`,
    };
  }
  return null;
});

// --- Drawer
const expanded = ref(new Set<string>());
const hasFinished = computed(() => jobs.value.some((job) => job.status !== 'running' && job.status !== 'scanning'));

const toggleJob = async (id: string) => {
  const next = new Set(expanded.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
    await loadDetails(id).catch(() => undefined);
  }
  expanded.value = next;
};

const sourceLabel = (job: JobSnapshot) => {
  const first = job.sources[0]?.path ?? '';
  return job.sources.length > 1 ? `${first} +${job.sources.length - 1}` : first;
};

const isRelayed = (job: JobSnapshot) => {
  const from = job.sources[0]?.location;
  const to = job.dest.location;
  return from?.kind === 'ssh' && to.kind === 'ssh' && from.host_id !== to.host_id;
};

const percent = (job: JobSnapshot) =>
  job.total_bytes > 0 ? Math.min(100, Math.round((job.done_bytes / job.total_bytes) * 100)) : job.status === 'done' ? 100 : 0;

const statusLabel = (job: JobSnapshot) => {
  switch (job.status) {
    case 'scanning':
      return 'Đang quét…';
    case 'running':
      return `${percent(job)}%`;
    case 'done':
      return '✓ Xong';
    case 'failed':
      return job.error ? '✗ Lỗi' : `✗ ${job.files_failed} lỗi`;
    default:
      return 'Đã hủy';
  }
};

const visibleItems = (job: JobSnapshot) => job.items.filter((item) => !item.is_dir || item.status === 'failed');

const itemIcon = (status: ItemStatus) => {
  switch (status) {
    case 'running':
      return LoaderCircle;
    case 'done':
      return CheckCircle2;
    case 'failed':
      return CircleX;
    case 'skipped':
      return MinusCircle;
    case 'cancelled':
      return Ban;
    default:
      return Circle;
  }
};

const itemPercent = (item: ItemSnapshot) =>
  item.size > 0 ? Math.min(100, Math.round((item.done_bytes / item.size) * 100)) : 0;

const editStatusLabel = (item: EditInfo) => {
  switch (item.status) {
    case 'opened':
      return 'Đang mở, chưa lưu lần nào';
    case 'uploading':
      return 'Đang tải lên…';
    case 'synced':
      return `Đã lưu ${item.synced_at ? formatTime(item.synced_at) : ''} · ${item.uploads} lần`;
    case 'conflict':
      return item.error ?? 'Tệp trên máy chủ đã đổi';
    case 'error':
      return item.error ?? 'Không lưu được';
    default:
      return '';
  }
};

// --- MOD-20
const conflict = computed(() => conflicts.value[0] ?? null);
const choice = ref<ConflictAction>('skip');
const applyToRest = ref(false);
const sourceNewer = computed(() => {
  const c = conflict.value;
  return Boolean(c?.source_mtime && c.target_mtime && c.source_mtime > c.target_mtime);
});
const options = computed(() => {
  const keepBoth = { value: 'keep_both' as const, label: `Giữ cả hai → ${conflict.value?.keep_both_name ?? ''}` };
  const skip = { value: 'skip' as const, label: 'Bỏ qua' };
  if (conflict.value?.kind_mismatch) return [skip, keepBoth];
  return [
    { value: 'overwrite' as const, label: 'Ghi đè' },
    { value: 'overwrite_if_newer' as const, label: 'Ghi đè nếu tệp mới hơn' },
    skip,
    keepBoth,
  ];
});

// The last choice is remembered for the next dialog in this session.
let lastChoice: ConflictAction | null = null;
watch(
  conflict,
  (prompt) => {
    if (!prompt) return;
    applyToRest.value = false;
    const allowed = options.value.map((option) => option.value as ConflictAction);
    choice.value = lastChoice && allowed.includes(lastChoice) ? lastChoice : prompt.suggested;
  },
  { immediate: true }
);

const answer = (action: ConflictAction) => {
  if (action !== 'cancel_job') lastChoice = action;
  void resolveConflict(action, applyToRest.value);
};

const formatTime = (secs?: number | null) =>
  secs ? new Date(secs * 1000).toLocaleString('vi-VN', { dateStyle: 'short', timeStyle: 'short' }) : '—';
</script>

<style scoped>
.file-drag-ghost {
  position: fixed;
  z-index: 3000;
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 480px;
  padding: 5px 10px;
  border-radius: var(--radius-sm);
  background-color: var(--bg-surface);
  border: 1px solid var(--border-focus);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
  color: var(--text-primary);
  font-size: 12px;
  pointer-events: none;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-drag-ghost.invalid {
  border-color: var(--status-failed);
  color: var(--status-failed);
}

.transfers-drawer {
  position: fixed;
  left: 64px;
  right: 0;
  bottom: 28px;
  z-index: 900;
  height: 260px;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-surface);
  border-top: 1px solid var(--border-focus);
  box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.4);
}

.td-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  padding: 0 12px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.td-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
}

.td-sub {
  font-weight: 400;
  text-transform: none;
  letter-spacing: 0;
  color: var(--text-muted);
}

.td-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.td-icon-btn {
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
}

.td-icon-btn:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.td-list {
  flex: 1;
  overflow-y: auto;
  padding: 6px 12px;
}

.td-empty {
  padding: 24px;
  text-align: center;
  color: var(--text-muted);
  font-size: 12px;
}

.td-job {
  border-bottom: 1px solid var(--border-subtle);
}

.td-edits {
  margin-bottom: 8px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-focus);
}

.td-section-title {
  font-size: 10.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--text-muted);
  padding: 4px 0;
}

.td-edit {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.td-edit-icon {
  color: #e4b5ff;
}

.edit-synced .td-edit-icon {
  color: var(--status-running, #10b981);
}

.edit-conflict .td-edit-icon,
.edit-error .td-edit-icon {
  color: var(--status-failed, #ef4444);
}

.td-edit-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.td-job-main {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) 200px 30px;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  cursor: pointer;
}

.td-chevron {
  color: var(--text-muted);
}

.td-job-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.td-route {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.td-path {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
}

.td-relay {
  font-size: 10.5px;
  color: #f59e0b;
}

.td-meta {
  font-size: 11px;
  color: var(--text-muted);
}

.td-failed {
  color: var(--status-failed);
}

.td-progress {
  display: flex;
  align-items: center;
  gap: 8px;
}

.td-bar {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background-color: var(--bg-surface-hover);
  overflow: hidden;
}

.td-bar-fill {
  height: 100%;
  background-color: var(--primary, #744791);
  transition: width 0.2s ease;
}

.status-done .td-bar-fill {
  background-color: var(--status-running, #10b981);
}

.status-failed .td-bar-fill {
  background-color: var(--status-failed, #ef4444);
}

.td-status {
  width: 64px;
  text-align: right;
  font-size: 11px;
  color: var(--text-secondary);
}

.td-items {
  padding: 0 0 8px 24px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 160px;
  overflow-y: auto;
}

.td-item {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
}

.td-item-icon {
  color: var(--text-muted);
}

.item-done .td-item-icon {
  color: var(--status-running, #10b981);
}

.item-failed .td-item-icon,
.td-item-error {
  color: var(--status-failed, #ef4444);
}

.item-running .td-item-icon {
  animation: files-spin 0.9s linear infinite;
}

.td-item-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.td-item-size {
  color: var(--text-muted);
  white-space: nowrap;
}

.td-item-error {
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.td-error,
.td-warning {
  font-size: 11.5px;
  color: #f59e0b;
}

.td-error {
  color: var(--status-failed);
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1050;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(11, 13, 19, 0.75);
}

.conflict-modal {
  width: 460px;
  max-width: calc(100vw - 32px);
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
  overflow: hidden;
}

.cm-header {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 44px;
  padding: 0 16px;
  font-size: 13.5px;
  font-weight: 600;
  background-color: var(--bg-sidebar);
  border-bottom: 1px solid var(--border-subtle);
}

.cm-icon {
  color: #f59e0b;
}

.cm-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  font-size: 12.5px;
}

.cm-path {
  font-size: 11.5px;
  color: var(--text-secondary);
  word-break: break-all;
}

.cm-compare {
  border-collapse: collapse;
  font-size: 12px;
}

.cm-compare th {
  text-align: left;
  font-weight: 500;
  color: var(--text-muted);
  font-size: 11px;
  padding: 2px 16px 2px 0;
}

.cm-compare td {
  padding: 2px 16px 2px 0;
}

.cm-newer {
  margin-left: 6px;
  font-size: 10.5px;
  color: var(--status-running, #10b981);
}

.cm-note {
  margin: 0;
  color: var(--text-secondary);
}

.cm-options {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cm-option {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.cm-apply {
  padding-top: 6px;
  border-top: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.cm-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 16px;
  background-color: var(--bg-sidebar);
  border-top: 1px solid var(--border-subtle);
}

.transfer-toast {
  position: fixed;
  right: 16px;
  bottom: 40px;
  z-index: 950;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 420px;
  padding: 8px 12px;
  border-radius: var(--radius-md);
  background-color: var(--bg-surface);
  border: 1px solid var(--border-focus);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  font-size: 12px;
  cursor: pointer;
}

.transfer-toast.error {
  border-color: var(--status-failed);
  color: var(--status-failed);
}

@keyframes files-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
