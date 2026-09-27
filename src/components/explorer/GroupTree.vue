<template>
  <div class="group-tree-container">
    <!-- Search Box -->
    <div class="search-box">
      <Search :size="13" class="search-icon" />
      <input
        v-model="filterText"
        type="text"
        placeholder="Lọc nhóm & lệnh... (Ctrl+F)"
        class="tree-search-input"
        ref="searchInput"
      />
    </div>

    <!-- Target Group Quick Summary Box (from Stitch SCR-01) -->
    <div v-if="targetGroup" class="target-group-card">
      <div class="target-card-header">
        <div class="target-title-wrap">
          <Folder :size="14" class="target-icon" />
          <span class="target-title">{{ targetGroup.group_name }}</span>
        </div>
        <span class="target-status-badge" :class="{ running: isTargetGroupRunning }">
          <span class="pulse-dot" v-if="isTargetGroupRunning" />
          {{ isTargetGroupRunning ? 'RUNNING' : 'IDLE' }}
        </span>
      </div>
      <div class="target-meta">
        <span>Tuần tự (Sequential)</span>
        <span>•</span>
        <span>{{ targetGroup.commands.length }} tác vụ</span>
      </div>
      <div class="target-actions">
        <button
          class="btn btn-success btn-sm target-btn"
          title="Chạy toàn bộ nhóm"
          @click="handleRunGroup(targetGroup)"
        >
          <Play :size="11" />
          <span>Chạy</span>
        </button>
        <button
          class="btn btn-danger btn-sm target-btn"
          title="Dừng toàn bộ nhóm"
          @click="handleStopGroup(targetGroup.id)"
        >
          <Square :size="10" />
          <span>Dừng</span>
        </button>
      </div>
    </div>

    <!-- Groups & Execution Tree List -->
    <div class="groups-list">
      <div v-if="filteredGroups.length === 0" class="no-groups">
        Không có nhóm nào phù hợp
      </div>

      <div
        v-for="group in filteredGroups"
        :key="group.id"
        class="group-node"
      >
        <!-- Group Header -->
        <div class="group-header" @click="selectGroup(group.id); toggleExpand(group.id)">
          <div class="header-left">
            <component
              :is="expandedGroups.has(group.id) ? ChevronDown : ChevronRight"
              :size="14"
              class="chevron-icon"
            />
            <Folder :size="13" class="folder-mini-icon" />
            <span class="group-name">{{ group.group_name }}</span>
            <span class="cmd-count">({{ group.commands.length }})</span>
            <span v-if="group.autostart" class="autostart-pill" title="Tự khởi động cùng ứng dụng">Auto</span>
          </div>

          <div class="header-actions" @click.stop>
            <button
              class="action-btn play-btn"
              title="Khởi chạy toàn bộ nhóm"
              @click="handleRunGroup(group)"
            >
              <Play :size="12" />
            </button>
            <button
              class="action-btn stop-btn"
              title="Dừng toàn bộ nhóm"
              @click="handleStopGroup(group.id)"
            >
              <Square :size="11" />
            </button>
          </div>
        </div>

        <!-- Commands Under Group (Sequential Tree Items) -->
        <div v-show="expandedGroups.has(group.id)" class="commands-list">
          <div
            v-for="(cmd, idx) in group.commands"
            :key="cmd.id"
            class="command-item"
            :class="{
              running: getProcessStatus(cmd.id) === 'running',
              failed: getProcessStatus(cmd.id) === 'failed',
            }"
            @click="$emit('select-command', cmd.id, cmd.name)"
          >
            <div class="cmd-item-left">
              <span class="order-tag">{{ formatOrder(idx) }}.</span>
              <span
                class="status-dot"
                :class="{
                  active: getProcessStatus(cmd.id) === 'running',
                  failed: getProcessStatus(cmd.id) === 'failed',
                  starting: getProcessStatus(cmd.id) === 'starting',
                }"
              />
              <span class="cmd-title" :title="cmd.name">{{ cmd.name }}</span>
            </div>

            <div class="cmd-item-right">
              <span v-if="getProcessInfo(cmd.id)?.pid" class="mini-pid" title="Process ID">
                PID:{{ getProcessInfo(cmd.id)?.pid }}
              </span>
              <span v-if="cmd.is_shell" class="shell-badge">sh</span>
              <span v-else class="argv-badge">argv</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { Search, ChevronDown, ChevronRight, Play, Square, Folder } from 'lucide-vue-next';
import type { CommandGroupWithCommands } from '@/types/models';
import { useRunSession } from '@/composables/useRunSession';

const props = defineProps<{
  groups: CommandGroupWithCommands[];
  selectedGroupId?: number;
}>();

const emit = defineEmits<{
  (e: 'select-command', commandId: number, commandName: string): void;
  (e: 'run-group', group: CommandGroupWithCommands): void;
  (e: 'stop-group', groupId: number): void;
  (e: 'select-group', groupId: number): void;
}>();

const { getProcessStatus, getProcessInfo, activeProcesses } = useRunSession();

const filterText = ref('');
const searchInput = ref<HTMLInputElement | null>(null);
const expandedGroups = ref<Set<number>>(new Set([1, 2, 3])); // Mặc định mở các nhóm

const targetGroup = computed(() => {
  return props.groups.find(group => group.id === props.selectedGroupId) || props.groups[0] || null;
});

const formatOrder = (index: number) => String(index + 1).padStart(2, '0');
const selectGroup = (groupId: number) => emit('select-group', groupId);
const handleFindShortcut = (event: KeyboardEvent) => {
  if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'f') return;
  const target = event.target as HTMLElement | null;
  if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA') return;
  event.preventDefault();
  searchInput.value?.focus();
  searchInput.value?.select();
};

onMounted(() => window.addEventListener('keydown', handleFindShortcut));
onBeforeUnmount(() => window.removeEventListener('keydown', handleFindShortcut));

const isTargetGroupRunning = computed(() => {
  if (!targetGroup.value) return false;
  return targetGroup.value.commands.some(c => getProcessStatus(c.id) === 'running');
});

const toggleExpand = (groupId: number) => {
  if (expandedGroups.value.has(groupId)) {
    expandedGroups.value.delete(groupId);
  } else {
    expandedGroups.value.add(groupId);
  }
};

const filteredGroups = computed(() => {
  if (!filterText.value.trim()) return props.groups;
  const q = filterText.value.toLowerCase();
  return props.groups.filter(
    g => g.group_name.toLowerCase().includes(q) ||
         g.commands.some(c => c.name.toLowerCase().includes(q))
  );
});

const handleRunGroup = (group: CommandGroupWithCommands) => {
  emit('run-group', group);
};

const handleStopGroup = (groupId: number) => {
  emit('stop-group', groupId);
};
</script>

<style scoped>
.group-tree-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}

.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 4px 8px;
  margin-bottom: 8px;
}

.search-icon {
  color: var(--text-muted);
}

.tree-search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: 11.5px;
  color: var(--text-primary);
  font-family: var(--font-sans);
}

/* Target Group Quick Card */
.target-group-card {
  background: linear-gradient(145deg, var(--surface-container), #151824);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  margin-bottom: 10px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.target-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.target-title-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}

.target-icon {
  color: var(--primary-accent);
  flex-shrink: 0;
}

.target-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.target-status-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 9999px;
  background-color: var(--status-idle-bg);
  color: var(--status-idle);
  display: flex;
  align-items: center;
  gap: 4px;
}

.target-status-badge.running {
  background-color: var(--status-running-bg);
  color: #34d399;
}

.pulse-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background-color: var(--status-running);
  animation: pulseDot 1.5s infinite;
}

.target-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10.5px;
  color: var(--text-muted);
}

.target-actions {
  display: flex;
  gap: 6px;
  margin-top: 4px;
}

.target-btn {
  flex: 1;
  padding: 3px 0;
  font-size: 11px;
}

.groups-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.folder-mini-icon {
  color: var(--primary-accent);
  flex-shrink: 0;
}

.group-node {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  cursor: pointer;
  background-color: rgba(255, 255, 255, 0.02);
  transition: background-color 0.1s ease;
}

.group-header:hover {
  background-color: var(--bg-surface-hover);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 5px;
  overflow: hidden;
  flex: 1;
}

.chevron-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.group-name {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cmd-count {
  font-size: 10.5px;
  color: var(--text-muted);
}

.autostart-pill {
  font-size: 9px;
  background-color: var(--primary-subtle);
  color: #cda8ee;
  border-radius: 2px;
  padding: 0 4px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: 6px;
}

.action-btn {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  cursor: pointer;
  background: transparent;
  transition: all 0.1s ease;
}

.play-btn {
  color: var(--status-running);
}
.play-btn:hover {
  background-color: var(--status-running-bg);
  border-color: rgba(16, 185, 129, 0.4);
}

.stop-btn {
  color: var(--status-failed);
}
.stop-btn:hover {
  background-color: var(--status-failed-bg);
  border-color: rgba(239, 68, 68, 0.4);
}

.commands-list {
  padding: 2px 4px 6px 14px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.command-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.1s ease;
}

.command-item:hover {
  background-color: var(--bg-surface-hover);
}

.cmd-item-left {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}

.order-tag {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background-color: var(--status-idle);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 5px var(--status-running);
}

.status-dot.failed {
  background-color: var(--status-failed);
}

.cmd-title {
  font-size: 11.5px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-item:hover .cmd-title {
  color: var(--text-primary);
}

.command-item.running {
  background-color: rgba(16, 185, 129, 0.08);
  border: 1px solid rgba(16, 185, 129, 0.25);
}

.command-item.failed {
  background-color: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
}

.cmd-item-right {
  display: flex;
  align-items: center;
  gap: 4px;
}

.mini-pid {
  font-size: 9px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.shell-badge {
  font-size: 9px;
  font-family: var(--font-mono);
  color: #fbbf24;
  background: rgba(245, 158, 11, 0.12);
  padding: 0 3px;
  border-radius: 2px;
}

.argv-badge {
  font-size: 9px;
  font-family: var(--font-mono);
  color: #34d399;
  background: rgba(16, 185, 129, 0.12);
  padding: 0 3px;
  border-radius: 2px;
}

.no-groups {
  padding: 20px;
  text-align: center;
  font-size: 11.5px;
  color: var(--text-muted);
}
</style>
