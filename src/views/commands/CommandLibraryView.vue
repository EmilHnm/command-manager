<template>
  <div class="library-view">
    <!-- Top Action Bar -->
    <header class="view-header">
      <div class="header-left">
        <h2 class="view-title">Thư Viện Lệnh (SCR-02)</h2>
        <span class="view-subtitle">Định nghĩa các câu lệnh, phân định argv vs shell tường minh</span>
      </div>

      <div class="header-right">
        <div class="search-input-wrap">
          <Search :size="14" class="search-icon" />
          <input
            v-model="searchQuery"
            class="search-input"
            placeholder="Tìm theo tên lệnh hoặc chuỗi thực thi..."
          />
        </div>

        <select v-model="filterMode" class="filter-select">
          <option value="all">Tất cả kiểu (All)</option>
          <option value="argv">Chỉ Direct Argv</option>
          <option value="shell">Chỉ Shell Commands</option>
        </select>

        <button class="btn btn-primary" @click="openCreateModal">
          <Plus :size="14" />
          <span>Tạo Lệnh Mới</span>
        </button>
      </div>
    </header>

    <!-- Commands Table Container -->
    <main class="table-container">
      <table class="data-table">
        <thead>
          <tr>
            <th style="width: 220px;">TÊN LỆNH</th>
            <th style="width: 140px;">KIỂU THỰC THI</th>
            <th>CHUỖI LỆNH (EXECUTION STRING)</th>
            <th style="width: 150px;">THAO TÁC</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="filteredCommands.length === 0">
            <td colspan="4" class="empty-cell">
              Không tìm thấy lệnh nào phù hợp với bộ lọc
            </td>
          </tr>

          <tr v-for="cmd in filteredCommands" :key="cmd.id" class="table-row">
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
              <code class="exec-string">{{ cmd.execution_string }}</code>
            </td>

            <td>
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
                  class="action-btn copy-btn"
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
import { ref, computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { Search, Plus, Play, Edit3, Copy, Trash2 } from 'lucide-vue-next';
import CommandEditorModal from '@/components/dialogs/CommandEditorModal.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useCommands } from '@/composables/useCommands';
import type { CommandDefinition } from '@/types/models';

const router = useRouter();
const { commands, fetchCommands, saveCommand, deleteCommand } = useCommands();

const searchQuery = ref('');
const filterMode = ref<'all' | 'argv' | 'shell'>('all');

const showEditorModal = ref(false);
const editingCommand = ref<CommandDefinition | null>(null);

const showDeleteDialog = ref(false);
const deletingCommand = ref<CommandDefinition | null>(null);

onMounted(async () => {
  await fetchCommands();
});

const filteredCommands = computed(() => {
  return commands.value.filter(cmd => {
    // Mode filter
    if (filterMode.value === 'argv' && cmd.is_shell) return false;
    if (filterMode.value === 'shell' && !cmd.is_shell) return false;

    // Search query
    if (!searchQuery.value.trim()) return true;
    const q = searchQuery.value.toLowerCase();
    return cmd.name.toLowerCase().includes(q) || cmd.execution_string.toLowerCase().includes(q);
  });
});

const shellCount = computed(() => commands.value.filter(c => c.is_shell).length);
const argvCount = computed(() => commands.value.filter(c => !c.is_shell).length);

const openCreateModal = () => {
  editingCommand.value = null;
  showEditorModal.value = true;
};

const openEditModal = (cmd: CommandDefinition) => {
  editingCommand.value = { ...cmd };
  showEditorModal.value = true;
};

const duplicateCommand = async (cmd: CommandDefinition) => {
  await saveCommand({
    name: `${cmd.name} (Copy)`,
    execution_string: cmd.execution_string,
    is_shell: cmd.is_shell,
  });
};

const handleSaveCommand = async (cmd: Partial<CommandDefinition>) => {
  await saveCommand(cmd);
  showEditorModal.value = false;
};

const requestDelete = (cmd: CommandDefinition) => {
  deletingCommand.value = cmd;
  showDeleteDialog.value = true;
};

const confirmDelete = async () => {
  if (deletingCommand.value) {
    await deleteCommand(deletingCommand.value.id);
    showDeleteDialog.value = false;
    deletingCommand.value = null;
  }
};

const runCommand = (cmd: CommandDefinition) => {
  router.push('/workspace');
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

.view-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
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

.search-input-wrap {
  display: flex;
  align-items: center;
  background-color: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 4px 10px;
  width: 260px;
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
  padding: 16px 20px;
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
  padding: 10px 14px;
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

.code-cell {
  max-width: 400px;
}

.exec-string {
  font-family: var(--font-mono);
  font-size: 11px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 3px 6px;
  border-radius: var(--radius-sm);
  color: #e2e8f0;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions-group {
  display: flex;
  align-items: center;
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
</style>
