<template>
  <div class="groups-view">
    <!-- Header -->
    <header class="view-header">
      <div class="header-left">
        <div class="title-row">
          <h2 class="view-title">Nhóm Lệnh & Sequencer (SCR-03)</h2>
          <span class="engine-badge">{{ APP_ORCHESTRATOR_TAG }}</span>
        </div>
        <span class="view-subtitle">Điều phối khởi động theo thứ tự hoặc tuần tự chờ xong (Orchestration Pipeline)</span>
      </div>

      <div class="header-right">
        <button class="btn btn-secondary btn-sm" :disabled="groups.length === 0" @click="handleRunAllGroups">
          <Play :size="13" />
          <span>Chạy Tất Cả Nhóm</span>
        </button>
        <button class="btn btn-primary" @click="openCreateGroupModal">
          <Plus :size="14" />
          <span>Tạo Nhóm Mới</span>
        </button>
      </div>
    </header>

    <!-- Telemetry Ribbon Cards (from Stitch SCR-03) -->
    <section class="stats-ribbon">
      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">TỔNG SỐ NHÓM</span>
          <Folder :size="16" class="stat-icon" />
        </div>
        <div class="stat-value font-mono">{{ groups.length }}</div>
        <div class="stat-sub">Pipeline điều phối hoạt động</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">AUTOSTART KÍCH HOẠT</span>
          <Zap :size="16" class="stat-icon text-warning" />
        </div>
        <div class="stat-value font-mono">{{ autostartCount }}</div>
        <div class="stat-sub">Khởi động cùng ứng dụng</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">ĐANG CHẠY</span>
          <Zap :size="16" class="stat-icon text-accent" />
        </div>
        <div class="stat-value font-mono">{{ runningProcessCount }}</div>
        <div class="stat-sub">Tiến trình thuộc các nhóm</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">TỔNG LỆNH TRONG CHUỖI</span>
          <Layers :size="16" class="stat-icon text-success" />
        </div>
        <div class="stat-value font-mono">{{ totalCommandsInGroups }}</div>
        <div class="stat-sub">Các bước thực thi tuần tự</div>
      </div>
    </section>

    <!-- Groups Grid / Sequencer -->
    <main class="groups-content">
      <div v-if="groups.length === 0" class="empty-groups">
        <Layers :size="36" class="empty-icon" />
        <h3>Chưa có nhóm lệnh nào</h3>
        <p>Bấm "+ Tạo Nhóm Mới" để gom cụm các câu lệnh thành chuỗi điều phối tự động.</p>
      </div>

      <div v-else class="groups-grid">
        <div v-for="group in groups" :key="group.id" class="group-card">
          <!-- Card Header -->
          <div class="card-header">
            <div class="header-main">
              <div class="group-title-row">
                <Folder class="folder-icon" :size="16" />
                <h3 class="group-title">{{ group.group_name }}</h3>
                <span class="mode-pill">{{ group.execution_mode === 'sequential' ? 'Tuần tự, chờ xong' : 'Khởi động theo thứ tự' }}</span>
              </div>
              <span class="commands-count">{{ group.commands.length }} tác vụ</span>
            </div>

            <div class="autostart-toggle-row">
              <span class="autostart-label">Autostart:</span>
              <button
                class="toggle-switch"
                :class="{ active: group.autostart }"
                title="Bật/tắt tự khởi động cùng app"
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
                  <span class="order-badge">{{ formatOrder(idx) }}.</span>
                  <div class="cmd-info">
                    <span class="cmd-text">{{ cmd.name }}</span>
                    <code class="cmd-snippet">{{ cmd.execution_string }}</code>
                  </div>
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
          <!-- Validation Error Banner -->
          <div v-if="wasSubmittedGroup && groupErrors.group_name" class="validation-banner" role="alert">
            <AlertCircle :size="14" />
            <span>{{ groupErrors.group_name }}</span>
          </div>

          <div class="form-group">
            <label class="form-label">Tên nhóm lệnh (*)</label>
            <input
              v-model="groupForm.group_name"
              class="input"
              :class="{ 'has-error': (wasSubmittedGroup || touchedGroup.group_name) && groupErrors.group_name }"
              placeholder="Ví dụ: Web Platform Development..."
              @blur="touchedGroup.group_name = true"
              @input="touchedGroup.group_name = true"
            />
            <span v-if="(wasSubmittedGroup || touchedGroup.group_name) && groupErrors.group_name" class="field-error-msg">
              <AlertCircle :size="12" />
              <span>{{ groupErrors.group_name }}</span>
            </span>
          </div>

          <label class="checkbox-row">
            <input v-model="groupForm.autostart" type="checkbox" />
            <div class="checkbox-text">
              <span class="checkbox-title">Tự động khởi động nhóm này khi mở ứng dụng (Autostart)</span>
              <span class="checkbox-desc">Process Manager sẽ tự động kích hoạt nhóm sau khi giữ Single-Instance Lock.</span>
            </div>
          </label>

          <div class="form-group">
            <label class="form-label">Chế độ chạy nhóm</label>
            <select v-model="groupForm.execution_mode" class="input">
              <option value="startup">Khởi động theo thứ tự (không chờ lệnh trước thoát)</option>
              <option value="sequential">Tuần tự, chờ xong (dừng khi lỗi)</option>
            </select>
          </div>

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
          <button class="btn btn-primary" @click="saveGroupForm">
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
import { ref, computed, onMounted, watch } from 'vue';
import { useRouter } from 'vue-router';
import {
  Plus, Folder, Play, Square, Edit3, Trash2, X, Save,
  Zap, Layers, AlertCircle
} from 'lucide-vue-next';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useGroups } from '@/composables/useGroups';
import { useCommands } from '@/composables/useCommands';
import { useRunSession } from '@/composables/useRunSession';
import { APP_ORCHESTRATOR_TAG } from '@/config/version';
import type { CommandGroupWithCommands } from '@/types/models';

const router = useRouter();
const { groups, fetchGroups, saveGroup, deleteGroup, toggleAutostart } = useGroups();
const { commands: allCommands, fetchCommands } = useCommands();
const { startGroupSession, stopGroupById, activeProcesses } = useRunSession();

const showGroupModal = ref(false);
const editingGroupId = ref<number | null>(null);
const groupForm = ref<{ group_name: string; autostart: boolean; execution_mode: 'startup' | 'sequential' }>({
  group_name: '',
  autostart: false,
  execution_mode: 'startup',
});
const selectedCommandIds = ref<number[]>([]);

const showDeleteDialog = ref(false);
const deletingGroup = ref<CommandGroupWithCommands | null>(null);

const autostartCount = computed(() => groups.value.filter(g => g.autostart).length);

const totalCommandsInGroups = computed(() => {
  return groups.value.reduce((acc, g) => acc + g.commands.length, 0);
});

const runningProcessCount = computed(() => Array.from(activeProcesses.value.values())
  .filter(process => process.status === 'running').length);
const formatOrder = (index: number) => String(index + 1).padStart(2, '0');

const handleRunAllGroups = async () => {
  for (const group of groups.value) {
    if (group.commands.length > 0) {
      await startGroupSession(group.id, group.group_name, group.commands);
    }
  }
  router.push('/workspace');
};

onMounted(async () => {
  await Promise.all([fetchGroups(), fetchCommands()]);
});

const touchedGroup = ref<{ group_name?: boolean }>({});
const wasSubmittedGroup = ref(false);

const groupErrors = computed(() => {
  const errs: { group_name?: string } = {};
  if (!groupForm.value.group_name?.trim()) {
    errs.group_name = 'Tên nhóm lệnh không được để trống.';
  }
  return errs;
});

const openCreateGroupModal = () => {
  editingGroupId.value = null;
  groupForm.value = { group_name: '', autostart: false, execution_mode: 'startup' };
  selectedCommandIds.value = [];
  wasSubmittedGroup.value = false;
  touchedGroup.value = {};
  showGroupModal.value = true;
};

watch(showGroupModal, (val) => {
  if (!val) {
    editingGroupId.value = null;
    groupForm.value = { group_name: '', autostart: false, execution_mode: 'startup' };
    selectedCommandIds.value = [];
    wasSubmittedGroup.value = false;
    touchedGroup.value = {};
  }
});

const openEditGroupModal = (group: CommandGroupWithCommands) => {
  editingGroupId.value = group.id;
  groupForm.value = {
    group_name: group.group_name,
    autostart: group.autostart,
    execution_mode: group.execution_mode,
  };
  selectedCommandIds.value = group.commands.map(c => c.id);
  wasSubmittedGroup.value = false;
  touchedGroup.value = {};
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
  wasSubmittedGroup.value = true;
  if (groupErrors.value.group_name) return;
  await saveGroup({
    id: editingGroupId.value ?? undefined,
    group_name: groupForm.value.group_name,
    autostart: groupForm.value.autostart,
    execution_mode: groupForm.value.execution_mode,
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
    execution_mode: group.execution_mode,
    commandIds: newCommandIds,
  });
};

const handlePlayGroup = async (group: CommandGroupWithCommands) => {
  await startGroupSession(group.id, group.group_name, group.commands);
  router.push('/workspace');
};

const handleStopGroup = async (groupId: number) => {
  await stopGroupById(groupId);
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

/* 4 Quick Stats Cards (from Stitch SCR-03) */
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

.text-warning {
  color: var(--status-starting);
}

.text-success {
  color: var(--status-running);
}

.text-accent {
  color: #38bdf8;
}

.groups-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
}

.empty-groups {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  text-align: center;
  color: var(--text-muted);
  gap: 12px;
}

.empty-icon {
  color: var(--border-medium);
}

.empty-groups h3 {
  font-size: 15px;
  color: var(--text-primary);
  font-weight: 600;
}

.groups-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(440px, 1fr));
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
  padding: 12px 16px;
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

.drag-handle {
  color: var(--text-muted);
  cursor: grab;
}

.mode-pill {
  font-size: 9.5px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  padding: 1px 5px;
  border-radius: 3px;
}

.folder-icon {
  color: var(--primary-accent);
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

.cmd-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  overflow: hidden;
  max-width: 260px;
}

.cmd-text {
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cmd-snippet {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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

.input.has-error {
  border-color: var(--status-failed, #ef4444) !important;
  background-color: rgba(239, 68, 68, 0.05) !important;
}

.field-error-msg {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--status-failed, #ef4444);
  margin-top: 4px;
}

.validation-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: var(--radius-md);
  color: var(--status-failed, #ef4444);
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 12px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
