<template>
  <div class="library-view">
    <!-- Top Action Bar -->
    <header class="view-header">
      <div class="header-left">
        <div class="title-row">
          <h2 class="view-title">Thư Viện Lệnh (SCR-02)</h2>
          <span class="engine-badge">{{ APP_ENGINE_TAG }}</span>
        </div>
        <span class="view-subtitle">Định nghĩa các câu lệnh, phân định argv vs shell tường minh, kiểm soát môi trường thực thi</span>
      </div>

      <div class="header-right">
        <input
          ref="importInput"
          class="hidden-file-input"
          type="file"
          accept="application/json,.json"
          @change="handleImportFile"
        />
        <button class="btn btn-secondary btn-sm" :disabled="loading || importing" title="Nạp nhanh từ tệp JSON" @click="handleImportJson">
          <Upload :size="13" />
          <span>Nhập JSON</span>
        </button>
        <button class="btn btn-primary" :disabled="loading" @click="openCreateModal">
          <Plus :size="14" />
          <span>Tạo Lệnh Mới</span>
        </button>
      </div>
    </header>

    <div v-if="toastMessage" class="toast-container" role="status" aria-live="polite">
      <div class="library-toast" :class="toastType">
        <span>{{ toastMessage }}</span>
        <button class="toast-close" aria-label="Đóng thông báo" @click="dismissToast">
          <X :size="13" />
        </button>
      </div>
    </div>

    <!-- Quick Stats Cards (Telemetry Ribbon from Stitch SCR-02) -->
    <section class="stats-ribbon">
      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">TỔNG SỐ LỆNH</span>
          <Terminal :size="16" class="stat-icon" />
        </div>
        <div class="stat-value font-mono">{{ commands.length }}</div>
        <div class="stat-sub">Đã đăng ký trong Registry</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">SHELL WRAPPER</span>
          <Zap :size="16" class="stat-icon text-warning" />
        </div>
        <div class="stat-value font-mono">{{ shellCount }}</div>
        <div class="stat-sub">bash / zsh / sh / cmd</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">DIRECT ARGV</span>
          <FileCode :size="16" class="stat-icon text-success" />
        </div>
        <div class="stat-value font-mono">{{ argvCount }}</div>
        <div class="stat-sub">Zero-shell overhead</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">CHƯA GÁN NHÓM</span>
          <CheckCircle :size="16" class="stat-icon" :class="unassignedCount > 0 ? 'text-warning' : 'text-success'" />
        </div>
        <div class="stat-value font-mono" :class="{ 'text-warning': unassignedCount > 0 }">{{ unassignedCount }}</div>
        <div class="stat-sub">{{ unassignedCount > 0 ? 'Cần phân loại vào nhóm' : 'Tất cả đã phân nhóm' }}</div>
      </div>
    </section>

    <!-- Filter & Search Toolbar -->
    <div class="toolbar-section">
      <div class="search-input-wrap">
        <Search :size="14" class="search-icon" />
        <input
          v-model="searchQuery"
          class="search-input"
          placeholder="Tìm theo tên lệnh, chuỗi thực thi hoặc thư mục..."
        />
      </div>

      <div class="toolbar-filters">
        <select v-model="filterMode" class="filter-select">
          <option value="all">Tất cả kiểu (All)</option>
          <option value="argv">Chỉ Direct Argv</option>
          <option value="shell">Chỉ Shell Commands</option>
        </select>
      </div>
    </div>

    <!-- Commands Table Container -->
    <main class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th style="width: 110px;">TRẠNG THÁI</th>
            <th style="width: 220px;">TÊN LỆNH</th>
            <th style="width: 140px;">KIỂU THỰC THI</th>
            <th>CHUỖI LỆNH (COMMAND / ARGS)</th>
            <th style="width: 140px; text-align: right;">THAO TÁC</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td colspan="5" class="empty-cell">Đang đồng bộ thư viện lệnh...</td>
          </tr>
          <tr v-else-if="filteredCommands.length === 0">
            <td colspan="5" class="empty-cell">
              Không tìm thấy lệnh nào phù hợp với bộ lọc tìm kiếm
            </td>
          </tr>

          <tr v-for="cmd in filteredCommands" :key="cmd.id" class="table-row">
            <td>
              <span class="status-indicator-cell">
                <span
                  class="status-dot"
                  :class="{
                    active: getProcessStatus(cmd.id) === 'running',
                    failed: getProcessStatus(cmd.id) === 'failed',
                  }"
                />
                <span class="status-text">{{ getProcessStatus(cmd.id) }}</span>
              </span>
            </td>

            <td class="name-cell">
              <span class="cmd-name">{{ cmd.name }}</span>
              <span class="cmd-id">#{{ cmd.id }}</span>
            </td>

            <td>
              <span v-if="!cmd.is_shell" class="type-badge argv" title="Chạy trực tiếp binary qua mảng đối số argv">
                Direct Argv
              </span>
              <span v-else class="type-badge shell" title="Chạy qua vỏ lệnh shell (bash/sh/cmd)">
                Shell (bash/sh)
              </span>
            </td>

            <td class="code-cell">
              <div class="code-box">
                <code class="exec-string">{{ cmd.execution_string }}</code>
                <button
                  class="copy-btn-mini"
                  title="Sao chép câu lệnh"
                  @click="copyCommandText(cmd.execution_string, cmd.id)"
                >
                  <Check v-if="copiedId === cmd.id" :size="12" class="text-success" />
                  <Copy v-else :size="12" />
                </button>
              </div>
            </td>

            <td style="text-align: right;">
              <div class="actions-group">
                <button
                  class="action-btn run-btn"
                  title="Chạy lệnh ngay"
                  @click="runCommand(cmd)"
                >
                  <Play :size="13" />
                </button>
                <button
                  class="action-btn edit-btn"
                  title="Chỉnh sửa lệnh"
                  @click="openEditModal(cmd)"
                >
                  <Edit3 :size="13" />
                </button>
                <button
                  class="action-btn duplicate-btn"
                  title="Nhân bản lệnh"
                  @click="duplicateCommand(cmd)"
                >
                  <Copy :size="13" />
                </button>
                <button
                  class="action-btn delete-btn"
                  title="Xóa lệnh"
                  @click="requestDelete(cmd)"
                >
                  <Trash2 :size="13" />
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </main>

    <!-- Footer Stats -->
    <footer class="view-footer">
      <span>Tổng cộng: {{ commands.length }} câu lệnh</span>
      <span>•</span>
      <span>{{ shellCount }} lệnh Shell</span>
      <span>•</span>
      <span>{{ argvCount }} lệnh Direct Argv</span>
      <span>•</span>
      <span class="sqlite-status">SQLite WAL: Synced</span>
    </footer>

    <!-- Editor Modal MOD-02 -->
    <CommandEditorModal
      :visible="showEditorModal"
      :command="editingCommand"
      @save="handleSaveCommand"
      @close="showEditorModal = false"
    />

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      :visible="showDeleteDialog"
      title="Xóa Câu Lệnh"
      :message="`Bạn có chắc chắn muốn xóa lệnh '${deletingCommand?.name}'? Các nhóm chứa lệnh này cũng sẽ tự động loại bỏ liên kết.`"
      confirm-text="Xóa Lệnh"
      :danger="true"
      @confirm="confirmDelete"
      @cancel="showDeleteDialog = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import { useRouter } from 'vue-router';
import {
  Search, Plus, Play, Edit3, Copy, Trash2,
  Terminal, Zap, FileCode, CheckCircle, Upload, Check, X
} from 'lucide-vue-next';
import CommandEditorModal from '@/components/dialogs/CommandEditorModal.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useCommands } from '@/composables/useCommands';
import { useGroups } from '@/composables/useGroups';
import { useRunSession } from '@/composables/useRunSession';
import { APP_ENGINE_TAG } from '@/config/version';
import type { CommandDefinition } from '@/types/models';

const router = useRouter();
const { commands, loading, error, fetchCommands, saveCommand, deleteCommand } = useCommands();
const { groups, fetchGroups } = useGroups();
const { getProcessStatus } = useRunSession();

const searchQuery = ref('');
const filterMode = ref<'all' | 'argv' | 'shell'>('all');

const showEditorModal = ref(false);
const editingCommand = ref<CommandDefinition | null>(null);

const showDeleteDialog = ref(false);
const deletingCommand = ref<CommandDefinition | null>(null);

const copiedId = ref<number | null>(null);
const importInput = ref<HTMLInputElement | null>(null);
const importing = ref(false);
const toastMessage = ref('');
const toastType = ref<'error' | 'success'>('success');
let toastTimer: ReturnType<typeof setTimeout> | undefined;

const showToast = (message: string, type: 'error' | 'success') => {
  toastMessage.value = message.trim() || (type === 'error' ? 'Không thể lưu câu lệnh.' : 'Đã hoàn tất thao tác.');
  toastType.value = type;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = '';
  }, 4500);
};

const dismissToast = () => {
  toastMessage.value = '';
  if (toastTimer) clearTimeout(toastTimer);
};

watch(error, (message) => {
  if (message) showToast(message, 'error');
});

onBeforeUnmount(() => {
  if (toastTimer) clearTimeout(toastTimer);
});

onMounted(async () => {
  await Promise.all([fetchCommands(), fetchGroups()]);
});

const filteredCommands = computed(() => {
  return commands.value.filter(cmd => {
    // Mode filter
    if (filterMode.value === 'argv' && cmd.is_shell) return false;
    if (filterMode.value === 'shell' && !cmd.is_shell) return false;

    // Search query
    if (!searchQuery.value.trim()) return true;
    const q = searchQuery.value.toLowerCase();
    return (
      cmd.name.toLowerCase().includes(q) ||
      cmd.execution_string.toLowerCase().includes(q)
    );
  });
});

const shellCount = computed(() => commands.value.filter(c => c.is_shell).length);
const argvCount = computed(() => commands.value.filter(c => !c.is_shell).length);

const unassignedCount = computed(() => {
  const assignedIds = new Set<number>();
  groups.value.forEach(g => {
    g.commands.forEach(c => assignedIds.add(c.id));
  });
  return commands.value.filter(c => !assignedIds.has(c.id)).length;
});

const openCreateModal = () => {
  editingCommand.value = null;
  showEditorModal.value = true;
};

const openEditModal = (cmd: CommandDefinition) => {
  editingCommand.value = { ...cmd };
  showEditorModal.value = true;
};

const duplicateCommand = async (cmd: CommandDefinition) => {
  const saved = await saveCommand({
    name: `${cmd.name} (Copy)`,
    execution_string: cmd.execution_string,
    is_shell: cmd.is_shell,
  });
  if (saved) showToast('Đã nhân bản câu lệnh.', 'success');
};

const handleSaveCommand = async (cmd: Partial<CommandDefinition>) => {
  if (await saveCommand(cmd)) {
    showEditorModal.value = false;
    showToast(cmd.id ? 'Đã cập nhật câu lệnh.' : 'Đã tạo câu lệnh mới.', 'success');
  }
};

const requestDelete = (cmd: CommandDefinition) => {
  deletingCommand.value = cmd;
  showDeleteDialog.value = true;
};

const confirmDelete = async () => {
  if (deletingCommand.value) {
    if (await deleteCommand(deletingCommand.value.id)) {
      showDeleteDialog.value = false;
      deletingCommand.value = null;
      showToast('Đã xóa câu lệnh.', 'success');
    }
  }
};

const runCommand = (cmd: CommandDefinition) => {
  router.push({ path: '/workspace', query: { runCommand: String(cmd.id) } });
};

const copyCommandText = async (text: string, id: number) => {
  try {
    await navigator.clipboard.writeText(text);
    copiedId.value = id;
    setTimeout(() => {
      if (copiedId.value === id) copiedId.value = null;
    }, 1500);
  } catch (e) {
    console.error('Failed to copy', e);
  }
};

const handleImportJson = () => {
  importInput.value?.click();
};

const handleImportFile = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = '';
  if (!file) return;

  importing.value = true;
  try {
    const parsed: unknown = JSON.parse(await file.text());
    const records = Array.isArray(parsed)
      ? parsed
      : parsed && typeof parsed === 'object' && Array.isArray((parsed as { commands?: unknown }).commands)
        ? (parsed as { commands: unknown[] }).commands
        : [];

    if (records.length === 0) throw new Error('Tệp JSON không có danh sách commands hợp lệ.');

    let imported = 0;
    for (const record of records) {
      if (!record || typeof record !== 'object') continue;
      const item = record as Record<string, unknown>;
      const name = typeof item.name === 'string' ? item.name.trim() : '';
      const executionString = typeof item.execution_string === 'string'
        ? item.execution_string.trim()
        : typeof item.executionString === 'string'
          ? item.executionString.trim()
          : '';
      if (!name || !executionString) continue;

      const saved = await saveCommand({
        name,
        execution_string: executionString,
        is_shell: Boolean(item.is_shell ?? item.isShell),
        cwd: typeof item.cwd === 'string' ? item.cwd : undefined,
      });
      if (saved) imported++;
    }

    if (imported === 0) throw new Error('Không có bản ghi hợp lệ để nhập.');
    showToast(`Đã nhập ${imported}/${records.length} câu lệnh vào SQLite.`, 'success');
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err), 'error');
  } finally {
    importing.value = false;
  }
};
</script>

<style scoped>
.library-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background-color: var(--bg-app-base);
  overflow: hidden;
}

.view-header {
  padding: 16px 20px;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.view-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.engine-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  background-color: var(--primary-subtle);
  color: var(--primary-accent);
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  border: 1px solid rgba(116, 71, 145, 0.35);
}

.view-subtitle {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.hidden-file-input {
  display: none;
}

.toast-container {
  position: fixed;
  top: 58px;
  right: 20px;
  z-index: 100;
  width: min(360px, calc(100vw - 40px));
  pointer-events: none;
}

.library-toast {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 11px 12px;
  border-radius: var(--radius-md);
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.35);
  font-size: 12px;
  line-height: 1.4;
  pointer-events: auto;
}

.library-toast.error {
  color: #fecaca;
  background: #3b1418;
  border: 1px solid rgba(248, 113, 113, 0.55);
}

.library-toast.success {
  color: #bbf7d0;
  background: #123522;
  border: 1px solid rgba(74, 222, 128, 0.55);
}

.toast-close {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  margin: -2px -3px 0 auto;
  color: currentColor;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.toast-close:hover {
  background: rgba(255, 255, 255, 0.12);
}

/* 4 Quick Stats Cards (from Stitch SCR-02) */
.stats-ribbon {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
  padding: 14px 20px 0;
  flex-shrink: 0;
}

.stat-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 3px;
  box-shadow: var(--shadow-sm);
  transition: border-color 0.15s ease;
}

.stat-card:hover {
  border-color: var(--border-medium);
}

.stat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.stat-title {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.05em;
}

.stat-icon {
  color: var(--text-muted);
}

.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  margin-top: 2px;
}

.stat-sub {
  font-size: 10.5px;
  color: var(--text-secondary);
}

/* Toolbar & Filters */
.toolbar-section {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px 0;
  gap: 12px;
  flex-shrink: 0;
}

.search-input-wrap {
  display: flex;
  align-items: center;
  background-color: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 5px 10px;
  flex: 1;
  max-width: 420px;
  gap: 6px;
}

.search-icon {
  color: var(--text-muted);
}

.search-input {
  background: transparent;
  border: none;
  outline: none;
  font-size: 12px;
  color: var(--text-primary);
  width: 100%;
}

.toolbar-filters {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-select {
  background-color: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 5px 10px;
  font-size: 12px;
  color: var(--text-secondary);
  outline: none;
}

.table-container {
  flex: 1;
  overflow-y: auto;
  padding: 12px 20px;
}

.data-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background-color: var(--bg-surface);
  overflow: hidden;
}

.data-table th {
  background-color: #12151f;
  padding: 10px 14px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-muted);
  text-align: left;
  border-bottom: 1px solid var(--border-subtle);
}

.table-row td {
  padding: 9px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--border-subtle);
  vertical-align: middle;
}

.table-row:last-child td {
  border-bottom: none;
}

.table-row:hover {
  background-color: rgba(255, 255, 255, 0.02);
}

.status-indicator-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--status-idle);
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.status-dot.failed {
  background-color: var(--status-failed);
}

.status-text {
  font-size: 11px;
  text-transform: capitalize;
  color: var(--text-secondary);
}

.name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cmd-name {
  font-weight: 600;
  color: var(--text-primary);
}

.cmd-id {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.type-badge {
  display: inline-block;
  padding: 2px 7px;
  border-radius: 4px;
  font-size: 10.5px;
  font-weight: 600;
}

.type-badge.argv {
  background-color: rgba(16, 185, 129, 0.12);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.25);
}

.type-badge.shell {
  background-color: rgba(245, 158, 11, 0.12);
  color: #fbbf24;
  border: 1px solid rgba(245, 158, 11, 0.25);
}

.cwd-cell {
  max-width: 160px;
}

.cwd-text {
  font-size: 11px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: block;
}

.code-cell {
  max-width: 380px;
}

.code-box {
  display: flex;
  align-items: center;
  gap: 4px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 2px 6px;
}

.exec-string {
  font-family: var(--font-mono);
  font-size: 11px;
  color: #e2e8f0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.copy-btn-mini {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 2px;
}

.copy-btn-mini:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface);
}

.actions-group {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
}

.action-btn {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  background-color: var(--bg-app-base);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.1s ease;
}

.action-btn:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.run-btn:hover {
  color: var(--status-running);
  border-color: rgba(16, 185, 129, 0.4);
}

.delete-btn:hover {
  color: var(--status-failed);
  border-color: rgba(239, 68, 68, 0.4);
}

.empty-cell {
  text-align: center;
  padding: 30px;
  color: var(--text-muted);
  font-size: 12.5px;
}

.view-footer {
  padding: 8px 20px;
  background-color: var(--bg-surface);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-muted);
}

.sqlite-status {
  color: #38bdf8;
  font-family: var(--font-mono);
}

.text-warning {
  color: var(--status-starting);
}

.text-success {
  color: var(--status-running);
}
</style>
