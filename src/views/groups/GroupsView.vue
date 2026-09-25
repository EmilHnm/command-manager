<template>
  <div class="groups-view">
    <header class="view-header">
      <div class="header-left">
        <h2 class="view-title">Nhóm Lệnh & Điều Phối Thứ Tự (SCR-03)</h2>
        <span class="view-subtitle">Gom cụm câu lệnh, thiết lập cờ Autostart và thứ tự tuần tự execution_order</span>
      </div>

      <div class="header-right">
        <button class="btn btn-primary" @click="openCreateGroupModal">
          <Plus :size="14" />
          <span>Tạo Nhóm Mới</span>
        </button>
      </div>
    </header>

    <main class="groups-content">
      <div class="groups-grid">
        <div v-for="group in groups" :key="group.id" class="group-card">
          <!-- Card Header -->
          <div class="card-header">
            <div class="header-main">
              <div class="group-title-row">
                <Folder class="folder-icon" :size="16" />
                <h3 class="group-title">{{ group.group_name }}</h3>
              </div>
              <span class="commands-count">{{ group.commands.length }} lệnh tuần tự</span>
            </div>

            <div class="autostart-toggle-row">
              <span class="autostart-label">Tự khởi động:</span>
              <button
                class="toggle-switch"
                :class="{ active: group.autostart }"
                @click="toggleAutostart(group.id, group.autostart)"
              >
                <span class="toggle-slider" />
              </button>
            </div>
          </div>

          <!-- Sequential Commands List -->
          <div class="card-body">
            <div class="sequence-title">Thứ tự thực thi (execution_order):</div>
            <div class="sequence-list">
              <div
                v-for="(cmd, idx) in group.commands"
                :key="cmd.id"
                class="sequence-item"
              >
                <div class="item-left">
                  <span class="order-badge">0{{ idx + 1 }}.</span>
                  <span class="cmd-text">{{ cmd.name }}</span>
                  <span v-if="cmd.is_shell" class="shell-tag">shell</span>
                  <span v-else class="argv-tag">argv</span>
                </div>

                <div class="item-reorder-actions">
                  <button
                    class="order-btn"
                    :disabled="idx === 0"
                    title="Chuyển lên trước"
                    @click="moveCommand(group, idx, -1)"
                  >
                    ▲
                  </button>
                  <button
                    class="order-btn"
                    :disabled="idx === group.commands.length - 1"
                    title="Chuyển xuống sau"
                    @click="moveCommand(group, idx, 1)"
                  >
                    ▼
                  </button>
                </div>
              </div>

              <div v-if="group.commands.length === 0" class="empty-sequence">
                Chưa có lệnh nào trong nhóm này
              </div>
            </div>
          </div>

          <!-- Card Actions Footer -->
          <div class="card-footer">
            <div class="action-buttons-left">
              <button class="btn btn-success btn-sm" @click="handlePlayGroup(group)">
                <Play :size="12" />
                <span>Chạy Nhóm</span>
              </button>
              <button class="btn btn-danger btn-sm" @click="handleStopGroup(group.id)">
                <Square :size="11" />
                <span>Dừng Nhóm</span>
              </button>
            </div>

            <div class="action-buttons-right">
              <button class="btn btn-secondary btn-sm" @click="openEditGroupModal(group)">
                <Edit3 :size="12" />
                <span>Sửa Nhóm</span>
              </button>
              <button class="btn btn-ghost btn-icon btn-sm" title="Xóa nhóm" @click="requestDelete(group)">
                <Trash2 :size="13" class="text-danger" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Modal Tạo / Sửa Nhóm MOD-04 -->
    <div v-if="showGroupModal" class="modal-backdrop" @click.self="showGroupModal = false">
      <div class="modal-content group-modal">
        <div class="modal-header">
          <div class="modal-title">
            <Folder class="text-primary" :size="18" />
            <span>{{ editingGroupId ? 'Cấu Hình Nhóm Lệnh' : 'Tạo Nhóm Lệnh Mới' }}</span>
          </div>
          <button class="btn btn-ghost btn-icon" @click="showGroupModal = false">
            <X :size="15" />
          </button>
        </div>

        <div class="modal-body">
          <div class="form-group">
            <label class="form-label">Tên nhóm lệnh (*)</label>
            <input
              v-model="groupForm.group_name"
              class="input"
              placeholder="Ví dụ: Web Platform Development..."
            />
          </div>

          <label class="checkbox-row">
            <input v-model="groupForm.autostart" type="checkbox" />
            <div class="checkbox-text">
              <span class="checkbox-title">Tự động khởi động nhóm này khi mở ứng dụng (Autostart)</span>
              <span class="checkbox-desc">Process Manager sẽ tự động kích hoạt nhóm sau khi giữ Single-Instance Lock.</span>
            </div>
          </label>

          <div class="form-group">
            <label class="form-label">Chọn các câu lệnh thuộc nhóm này:</label>
            <div class="commands-picker">
              <label
                v-for="cmd in allCommands"
                :key="cmd.id"
                class="picker-item"
                :class="{ selected: selectedCommandIds.includes(cmd.id) }"
              >
                <input
                  type="checkbox"
                  :checked="selectedCommandIds.includes(cmd.id)"
                  @change="toggleCommandInPicker(cmd.id)"
                />
                <span class="picker-name">{{ cmd.name }}</span>
                <span class="picker-type">{{ cmd.is_shell ? 'Shell' : 'Argv' }}</span>
              </label>
            </div>
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showGroupModal = false">
            Hủy bỏ
          </button>
          <button class="btn btn-primary" :disabled="!groupForm.group_name?.trim()" @click="saveGroupForm">
            <Save :size="14" />
            <span>Lưu Cấu Hình Nhóm</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Confirm Dialog Delete Group -->
    <ConfirmDialog
      :visible="showDeleteDialog"
      title="Xóa Nhóm Lệnh"
      :message="`Bạn có chắc chắn muốn xóa nhóm '${deletingGroup?.group_name}'? Các câu lệnh đơn lẻ vẫn được giữ lại trong Thư Viện.`"
      confirm-text="Xóa Nhóm"
      :danger="true"
      @confirm="confirmDeleteGroup"
      @cancel="showDeleteDialog = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { Plus, Folder, Play, Square, Edit3, Trash2, X, Save } from 'lucide-vue-next';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useGroups } from '@/composables/useGroups';
import { useCommands } from '@/composables/useCommands';
import { useRunSession } from '@/composables/useRunSession';
import type { CommandGroupWithCommands } from '@/types/models';

const router = useRouter();
const { groups, fetchGroups, saveGroup, deleteGroup, toggleAutostart } = useGroups();
const { commands: allCommands, fetchCommands } = useCommands();
const { startGroupSession, stopGroupSession } = useRunSession();

const showGroupModal = ref(false);
const editingGroupId = ref<number | null>(null);
const groupForm = ref<{ group_name: string; autostart: boolean }>({
  group_name: '',
  autostart: false,
});
const selectedCommandIds = ref<number[]>([]);

const showDeleteDialog = ref(false);
const deletingGroup = ref<CommandGroupWithCommands | null>(null);

onMounted(async () => {
  await Promise.all([fetchGroups(), fetchCommands()]);
});

const openCreateGroupModal = () => {
  editingGroupId.value = null;
  groupForm.value = { group_name: '', autostart: false };
  selectedCommandIds.value = [];
  showGroupModal.value = true;
};

const openEditGroupModal = (group: CommandGroupWithCommands) => {
  editingGroupId.value = group.id;
  groupForm.value = {
    group_name: group.group_name,
    autostart: group.autostart,
  };
  selectedCommandIds.value = group.commands.map(c => c.id);
  showGroupModal.value = true;
};

const toggleCommandInPicker = (commandId: number) => {
  const idx = selectedCommandIds.value.indexOf(commandId);
  if (idx !== -1) {
    selectedCommandIds.value.splice(idx, 1);
  } else {
    selectedCommandIds.value.push(commandId);
  }
};

const saveGroupForm = async () => {
  if (!groupForm.value.group_name.trim()) return;
  await saveGroup({
    id: editingGroupId.value ?? undefined,
    group_name: groupForm.value.group_name,
    autostart: groupForm.value.autostart,
    commandIds: selectedCommandIds.value,
  });
  showGroupModal.value = false;
};

const moveCommand = async (group: CommandGroupWithCommands, index: number, direction: number) => {
  const newIndex = index + direction;
  if (newIndex < 0 || newIndex >= group.commands.length) return;

  const newCommandIds = group.commands.map(c => c.id);
  const temp = newCommandIds[index];
  newCommandIds[index] = newCommandIds[newIndex];
  newCommandIds[newIndex] = temp;

  await saveGroup({
    id: group.id,
    group_name: group.group_name,
    autostart: group.autostart,
    commandIds: newCommandIds,
  });
};

const handlePlayGroup = async (group: CommandGroupWithCommands) => {
  await startGroupSession(group.id, group.group_name, group.commands);
  router.push('/workspace');
};

const handleStopGroup = async (groupId: number) => {
  await stopGroupSession();
};

const requestDelete = (group: CommandGroupWithCommands) => {
  deletingGroup.value = group;
  showDeleteDialog.value = true;
};

const confirmDeleteGroup = async () => {
  if (deletingGroup.value) {
    await deleteGroup(deletingGroup.value.id);
    showDeleteDialog.value = false;
    deletingGroup.value = null;
  }
};
</script>

<style scoped>
.groups-view {
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

.groups-content {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}

.groups-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
  gap: 16px;
}

.group-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  transition: border-color 0.15s ease;
}

.group-card:hover {
  border-color: var(--border-medium);
}

.card-header {
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: rgba(255, 255, 255, 0.015);
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.folder-icon {
  color: #cda8ee;
}

.group-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.commands-count {
  font-size: 11px;
  color: var(--text-muted);
}

.autostart-toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.autostart-label {
  font-size: 11px;
  color: var(--text-secondary);
}

.toggle-switch {
  width: 34px;
  height: 18px;
  background-color: var(--border-medium);
  border-radius: 9999px;
  border: none;
  cursor: pointer;
  position: relative;
  transition: background-color 0.15s ease;
  padding: 2px;
}

.toggle-switch.active {
  background-color: var(--primary);
}

.toggle-slider {
  display: block;
  width: 14px;
  height: 14px;
  background-color: #ffffff;
  border-radius: 50%;
  transition: transform 0.15s ease;
}

.toggle-switch.active .toggle-slider {
  transform: translateX(16px);
}

.card-body {
  padding: 14px 16px;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sequence-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.sequence-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.sequence-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-size: 12px;
}

.item-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.order-badge {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: #cda8ee;
}

.cmd-text {
  font-weight: 500;
  color: var(--text-primary);
}

.shell-tag, .argv-tag {
  font-size: 9px;
  padding: 1px 4px;
  border-radius: 2px;
  font-family: var(--font-mono);
}

.shell-tag {
  background: rgba(245, 158, 11, 0.12);
  color: #fbbf24;
}

.argv-tag {
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
}

.item-reorder-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.order-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 9px;
  width: 18px;
  height: 18px;
  cursor: pointer;
  border-radius: 2px;
}

.order-btn:hover:not(:disabled) {
  background: var(--bg-surface);
  color: var(--text-primary);
}

.order-btn:disabled {
  opacity: 0.2;
  cursor: default;
}

.card-footer {
  padding: 10px 16px;
  background-color: #12151f;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.action-buttons-left, .action-buttons-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.empty-sequence {
  font-size: 11.5px;
  color: var(--text-muted);
  text-align: center;
  padding: 12px;
}

/* Modal Group Form */
.group-modal {
  max-width: 520px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.checkbox-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  cursor: pointer;
}

.checkbox-row input {
  margin-top: 3px;
}

.checkbox-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.checkbox-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.checkbox-desc {
  font-size: 11px;
  color: var(--text-muted);
}

.commands-picker {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 6px;
  background: var(--bg-app-base);
}

.picker-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  background: var(--bg-surface);
  transition: all 0.1s ease;
}

.picker-item:hover {
  background: var(--bg-surface-hover);
}

.picker-item.selected {
  border: 1px solid var(--primary);
  background: var(--primary-subtle);
}

.picker-name {
  font-size: 12px;
  flex: 1;
  margin-left: 8px;
  color: var(--text-primary);
}

.picker-type {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.text-primary {
  color: var(--primary);
}

.text-danger {
  color: var(--status-failed);
}
</style>
