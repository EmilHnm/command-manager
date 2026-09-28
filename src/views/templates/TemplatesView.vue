<template>
  <div class="templates-view">
    <!-- View Header & Telemetry Ribbon -->
    <div class="view-header">
      <div class="header-left">
        <div class="breadcrumb">
          <span>Workspace</span>
          <span class="slash">/</span>
          <span class="current">Thư Viện Mẫu Lệnh (SCR-06)</span>
        </div>
        <div class="header-title-row">
          <h1 class="view-title">Thư Viện Mẫu Lệnh (Command Templates)</h1>
          <span class="version-badge">v2.2 Templates Engine</span>
        </div>
        <p class="view-subtitle">
          Định nghĩa mẫu lệnh tham số hóa dạng <code>&#123;&#123;param&#125;&#125;</code>, bảo vệ secret, lưu preset và thực thi an toàn.
        </p>
      </div>

      <div class="header-actions">
        <button class="btn-primary" @click="openCreateModal">
          <Plus :size="16" />
          <span>+ Tạo Template Mới</span>
        </button>
      </div>
    </div>

    <div v-if="runError" class="template-error" role="alert">
      <span>{{ runError }}</span>
      <button type="button" aria-label="Đóng thông báo lỗi" @click="runError = ''">
        <X :size="14" />
      </button>
    </div>

    <!-- Telemetry Metric Ribbon (4 Stat Cards) -->
    <div class="telemetry-ribbon">
      <div class="metric-card">
        <div class="metric-icon purple">
          <FileCode :size="18" />
        </div>
        <div class="metric-content">
          <div class="metric-value">{{ templates.length }}</div>
          <div class="metric-label">TỔNG TEMPLATES</div>
          <div class="metric-sub">Bản mẫu trong hệ thống</div>
        </div>
      </div>

      <div class="metric-card">
        <div class="metric-icon amber">
          <Terminal :size="18" />
        </div>
        <div class="metric-content">
          <div class="metric-value">{{ shellTemplatesCount }}</div>
          <div class="metric-label">SHELL TEMPLATES</div>
          <div class="metric-sub">Môi trường bash/sh wrapper</div>
        </div>
      </div>

      <div class="metric-card">
        <div class="metric-icon emerald">
          <ShieldCheck :size="18" />
        </div>
        <div class="metric-content">
          <div class="metric-value">{{ argvTemplatesCount }}</div>
          <div class="metric-label">DIRECT ARGV TEMPLATES</div>
          <div class="metric-sub">Thực thi nhị phân an toàn</div>
        </div>
      </div>

      <div class="metric-card">
        <div class="metric-icon cyan">
          <Bookmark :size="18" />
        </div>
        <div class="metric-content">
          <div class="metric-value">{{ totalPresetsCount }}</div>
          <div class="metric-label">SAVED PRESETS</div>
          <div class="metric-sub">Bộ tham số lưu sẵn</div>
        </div>
      </div>
    </div>

    <!-- Toolbar: Search & Filters -->
    <div class="toolbar">
      <div class="search-box">
        <Search :size="15" class="search-icon" />
        <input
          v-model="searchQuery"
          type="text"
          class="search-input"
          placeholder="Lọc template theo tên, mô tả hoặc biểu thức {{placeholder}}..."
        />
        <button v-if="searchQuery" class="clear-search" @click="searchQuery = ''">
          <X :size="14" />
        </button>
      </div>

      <div class="filter-group">
        <span class="filter-label">Kiểu thực thi:</span>
        <select v-model="filterExecMode" class="filter-select">
          <option value="all">Tất cả kiểu (All)</option>
          <option value="shell">Shell Execution</option>
          <option value="argv">Direct Argv</option>
        </select>
      </div>
    </div>

    <!-- Templates Data Table -->
    <div class="table-container">
      <table class="templates-table">
        <thead>
          <tr>
            <th>TÊN TEMPLATE</th>
            <th>KIỂU THỰC THI</th>
            <th>CẤU TRÚC MẪU (TEMPLATE STRING)</th>
            <th>THAM SỐ</th>
            <th>LẦN CHẠY CUỐI</th>
            <th class="text-right">HÀNH ĐỘNG</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="tpl in filteredTemplates" :key="tpl.id">
            <!-- Name & Description -->
            <td class="name-cell">
              <div class="tpl-name-wrap">
                <Puzzle :size="16" class="tpl-icon" />
                <div>
                  <div class="tpl-name">{{ tpl.name }}</div>
                  <div v-if="tpl.description" class="tpl-desc">{{ tpl.description }}</div>
                </div>
              </div>
            </td>

            <!-- Execution Mode Badge -->
            <td>
              <span
                class="badge-exec"
                :class="tpl.is_shell ? 'mode-shell' : 'mode-argv'"
              >
                {{ tpl.is_shell ? 'Shell (bash)' : 'Direct Argv' }}
              </span>
            </td>

            <!-- Template String -->
            <td class="string-cell">
              <div class="mono-code-block">
                <code>{{ tpl.template_string }}</code>
              </div>
            </td>

            <!-- Params Summary Badge -->
            <td>
              <div class="params-badge-wrap">
                <span class="badge-params">
                  {{ tpl.params.length }} params
                </span>
                <span v-if="countSecrets(tpl) > 0" class="badge-secret" title="Có tham số bảo mật">
                  <Lock :size="10" /> {{ countSecrets(tpl) }} secret
                </span>
                <span v-if="tpl.preset_count" class="badge-preset">
                  📌 {{ tpl.preset_count }} presets
                </span>
              </div>
            </td>

            <!-- Last Run -->
            <td class="time-cell">
              <span class="time-text">{{ tpl.last_run_at || 'Chưa chạy' }}</span>
            </td>

            <!-- Action Buttons -->
            <td class="actions-cell">
              <div class="action-buttons">
                <button
                  class="btn-act btn-run"
                  title="Chạy template (Mở MOD-10)"
                  @click="openRunModal(tpl.id)"
                >
                  <Play :size="13" />
                  <span>Chạy</span>
                </button>
                <button
                  class="btn-act btn-edit"
                  title="Chỉnh sửa template (Mở MOD-11)"
                  @click="openEditModal(tpl.id)"
                >
                  <Edit3 :size="13" />
                </button>
                <button
                  class="btn-act btn-duplicate"
                  title="Nhân bản template"
                  @click="handleDuplicate(tpl)"
                >
                  <Copy :size="13" />
                </button>
                <button
                  class="btn-act btn-delete"
                  title="Xóa template"
                  @click="handleDelete(tpl.id, tpl.name)"
                >
                  <Trash2 :size="13" />
                </button>
              </div>
            </td>
          </tr>

          <tr v-if="filteredTemplates.length === 0">
            <td colspan="6" class="empty-row">
              <div class="empty-state">
                <FileCode :size="32" class="empty-icon" />
                <div class="empty-title">Không tìm thấy Template nào</div>
                <div class="empty-sub">
                  Thử thay đổi từ khóa tìm kiếm hoặc nhấn nút "+ Tạo Template Mới" để bắt đầu.
                </div>
              </div>
            </td>
          </tr>
        </tbody>
      </table>

      <!-- Table Summary Footer -->
      <div class="table-footer">
        <span>Hiển thị {{ filteredTemplates.length }} / {{ templates.length }} templates</span>
        <span class="dot-sep">•</span>
        <span>{{ totalPresetsCount }} Bộ Presets sẵn có</span>
      </div>
    </div>

    <!-- Modals -->
    <TemplateRunModal
      :visible="runModalVisible"
      :template-id="selectedTemplateId"
      @close="runModalVisible = false"
      @run="handleRunExecution"
    />

    <TemplateEditorModal
      :visible="editorModalVisible"
      :template-id="selectedTemplateId"
      @close="editorModalVisible = false"
      @save="fetchTemplates"
    />

    <!-- Confirm Dialog Delete Template (MOD-13) -->
    <ConfirmDialog
      :visible="showDeleteConfirm"
      title="Xóa Mẫu Lệnh"
      subtitle="MOD-13 • Template Removal Gate"
      :message="`Bạn có chắc chắn muốn xóa Template '${deletingTemplate?.name}'? Toàn bộ các bộ tham số Preset liên kết cũng sẽ bị xóa vĩnh viễn khỏi SQLite.`"
      confirm-text="Xóa Template"
      :danger="true"
      @confirm="confirmDeleteTemplate"
      @cancel="showDeleteConfirm = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  Plus,
  FileCode,
  Terminal,
  ShieldCheck,
  Bookmark,
  Search,
  X,
  Puzzle,
  Lock,
  Play,
  Edit3,
  Copy,
  Trash2,
} from 'lucide-vue-next';
import { useTemplates } from '@/composables/useTemplates';
import { ipcClient } from '@/ipc/client';
import TemplateRunModal from '@/components/dialogs/TemplateRunModal.vue';
import TemplateEditorModal from '@/components/dialogs/TemplateEditorModal.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import type { CommandTemplate } from '@/types/models';

const router = useRouter();
const route = useRoute();
const {
  templates,
  shellTemplatesCount,
  argvTemplatesCount,
  totalPresetsCount,
  fetchTemplates,
  createTemplate,
  deleteTemplate,
} = useTemplates();

const searchQuery = ref('');
const filterExecMode = ref<'all' | 'shell' | 'argv'>('all');

const runModalVisible = ref(false);
const editorModalVisible = ref(false);
const selectedTemplateId = ref<number | null>(null);
const runError = ref('');
const showDeleteConfirm = ref(false);
const deletingTemplate = ref<{ id: number; name: string } | null>(null);

onMounted(async () => {
  await fetchTemplates();
  openTemplateFromQuery();
});

watch(() => route.query.openTemplateId, () => openTemplateFromQuery());

const filteredTemplates = computed(() => {
  return templates.value.filter(tpl => {
    // Mode filter
    if (filterExecMode.value === 'shell' && !tpl.is_shell) return false;
    if (filterExecMode.value === 'argv' && tpl.is_shell) return false;

    // Search query
    if (!searchQuery.value.trim()) return true;
    const q = searchQuery.value.toLowerCase();
    return (
      tpl.name.toLowerCase().includes(q) ||
      (tpl.description && tpl.description.toLowerCase().includes(q)) ||
      tpl.template_string.toLowerCase().includes(q)
    );
  });
});

const countSecrets = (tpl: CommandTemplate) => {
  return tpl.params.filter(p => p.is_secret).length;
};

const openCreateModal = () => {
  selectedTemplateId.value = null;
  editorModalVisible.value = true;
};

const openEditModal = (id: number) => {
  selectedTemplateId.value = id;
  editorModalVisible.value = true;
};

const openRunModal = (id: number) => {
  selectedTemplateId.value = id;
  runError.value = '';
  runModalVisible.value = true;
};

const openTemplateFromQuery = () => {
  const rawId = route.query.openTemplateId;
  if (typeof rawId !== 'string') return;
  const id = Number(rawId);
  if (!Number.isInteger(id) || !templates.value.some(template => template.id === id)) return;
  openRunModal(id);
  void router.replace({ path: '/templates' });
};

const handleDuplicate = async (tpl: CommandTemplate) => {
  const dup: Omit<CommandTemplate, 'id'> = {
    name: `${tpl.name} (Bản sao)`,
    description: tpl.description,
    template_string: tpl.template_string,
    is_shell: tpl.is_shell,
    params: JSON.parse(JSON.stringify(tpl.params)),
  };
  await createTemplate(dup);
};

const handleDelete = (id: number, name: string) => {
  deletingTemplate.value = { id, name };
  showDeleteConfirm.value = true;
};

const confirmDeleteTemplate = async () => {
  if (!deletingTemplate.value) return;
  try {
    await deleteTemplate(deletingTemplate.value.id);
  } finally {
    showDeleteConfirm.value = false;
    deletingTemplate.value = null;
  }
};

const handleRunExecution = async (payload: {
  templateId: number;
  commandName: string;
  executionString: string;
  isShell: boolean;
  paramValues: Record<string, string>;
}) => {
  runError.value = '';
  try {
    const terminal = await ipcClient.runTemplate(payload.templateId, payload.paramValues);
    await fetchTemplates();
    runModalVisible.value = false;
    router.push({
      path: '/workspace',
      query: {
        runEventId: terminal.runEventId,
        runCommandId: String(terminal.commandId),
        runCommandName: payload.commandName,
      },
    });
  } catch (error) {
    runError.value = error instanceof Error ? error.message : String(error);
    console.error('[Templates] Không thể chạy template:', error);
  }
};
</script>

<style scoped>
.templates-view {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  overflow-y: auto;
  background-color: var(--bg-app-base);
}

.view-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.slash {
  color: var(--border-subtle);
}

.current {
  color: var(--text-secondary);
}

.header-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.view-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
}

.version-badge {
  font-size: 10px;
  font-weight: 700;
  background: var(--primary-subtle);
  color: #e4b5ff;
  border: 1px solid var(--primary);
  padding: 2px 8px;
  border-radius: 9999px;
}

.view-subtitle {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 4px;
}

.view-subtitle code {
  font-family: var(--font-mono);
  color: #4cd7f6;
  background: rgba(0, 0, 0, 0.3);
  padding: 1px 4px;
  border-radius: 3px;
}

.telemetry-ribbon {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.metric-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  display: flex;
  align-items: center;
  gap: 12px;
}

.metric-icon {
  width: 38px;
  height: 38px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.metric-icon.purple {
  background-color: rgba(116, 71, 145, 0.2);
  color: #e4b5ff;
}

.metric-icon.amber {
  background-color: rgba(245, 158, 11, 0.2);
  color: #f59e0b;
}

.metric-icon.emerald {
  background-color: rgba(16, 185, 129, 0.2);
  color: #4edea3;
}

.metric-icon.cyan {
  background-color: rgba(6, 182, 212, 0.2);
  color: #4cd7f6;
}

.metric-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  line-height: 1;
}

.metric-label {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
  margin-top: 4px;
}

.metric-sub {
  font-size: 10px;
  color: var(--text-muted);
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  background-color: var(--bg-surface);
  padding: 8px 12px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
}

.search-box {
  display: flex;
  align-items: center;
  gap: 8px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 6px 10px;
  flex: 1;
  max-width: 500px;
}

.search-icon {
  color: var(--text-muted);
}

.search-input {
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 12px;
  flex: 1;
}

.clear-search {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
}

.filter-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label {
  font-size: 11px;
  color: var(--text-secondary);
}

.filter-select {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
}

.table-container {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.templates-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.templates-table th {
  background-color: #141721;
  color: var(--text-secondary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  padding: 10px 14px;
  text-align: left;
  border-bottom: 1px solid var(--border-subtle);
}

.templates-table td {
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle);
  vertical-align: middle;
}

.templates-table tr:hover {
  background-color: var(--bg-surface-hover);
}

.tpl-name-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.tpl-icon {
  color: #4cd7f6;
  flex-shrink: 0;
}

.tpl-name {
  font-weight: 600;
  color: var(--text-primary);
  font-size: 13px;
}

.tpl-desc {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 2px;
}

.badge-exec {
  font-size: 10px;
  font-weight: 700;
  padding: 3px 8px;
  border-radius: 4px;
  display: inline-block;
}

.badge-exec.mode-shell {
  background-color: rgba(245, 158, 11, 0.15);
  color: #f59e0b;
  border: 1px solid rgba(245, 158, 11, 0.3);
}

.badge-exec.mode-argv {
  background-color: rgba(16, 185, 129, 0.15);
  color: #4edea3;
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.mono-code-block {
  background-color: #0b0d13;
  border: 1px solid #242a3e;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  max-width: 360px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mono-code-block code {
  font-family: var(--font-mono);
  font-size: 11px;
  color: #4edea3;
}

.params-badge-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.badge-params {
  font-size: 10px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 2px 6px;
  border-radius: 3px;
  color: var(--text-secondary);
}

.badge-secret {
  font-size: 10px;
  background-color: rgba(245, 158, 11, 0.15);
  color: #f59e0b;
  border: 1px solid rgba(245, 158, 11, 0.3);
  padding: 2px 6px;
  border-radius: 3px;
  display: flex;
  align-items: center;
  gap: 3px;
}

.badge-preset {
  font-size: 10px;
  background-color: rgba(6, 182, 212, 0.15);
  color: #4cd7f6;
  border: 1px solid rgba(6, 182, 212, 0.3);
  padding: 2px 6px;
  border-radius: 3px;
}

.time-text {
  font-size: 11px;
  color: var(--text-muted);
}

.actions-cell {
  text-align: right;
}

.action-buttons {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.btn-act {
  background: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
}

.btn-act:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.btn-run {
  background-color: rgba(16, 185, 129, 0.15);
  border-color: rgba(16, 185, 129, 0.4);
  color: #4edea3;
  font-weight: 600;
}

.btn-run:hover {
  background-color: #10b981;
  color: #ffffff;
}

.btn-delete:hover {
  background-color: var(--status-failed);
  border-color: var(--status-failed);
  color: #ffffff;
}

.empty-state {
  padding: 32px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--text-muted);
}

.empty-icon {
  color: var(--text-muted);
}

.empty-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-secondary);
}

.empty-sub {
  font-size: 12px;
}

.table-footer {
  padding: 10px 14px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 8px;
}

.dot-sep {
  color: var(--border-subtle);
}

.btn-primary {
  background-color: var(--primary);
  color: #ffffff;
  border: none;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  box-shadow: 0 0 10px var(--primary-glow);
}

.btn-primary:hover {
  background-color: var(--primary-hover);
}

.template-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 9px 12px;
  color: #fecaca;
  background: rgba(127, 29, 29, 0.88);
  border: 1px solid rgba(248, 113, 113, 0.45);
  border-radius: var(--radius-sm);
  font-size: 12px;
}

.template-error button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  color: inherit;
  background: transparent;
  border: 0;
  cursor: pointer;
}
</style>
