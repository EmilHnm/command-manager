<template>
  <div class="workspace-view">
    <!-- Left Explorer: Group Tree -->
    <aside class="workspace-sidebar">
      <div class="sidebar-header">
        <div class="sidebar-header-left">
          <Layers :size="14" class="header-icon" />
          <span class="header-title">Nhóm Lệnh & Phiên</span>
        </div>
        <button class="btn btn-ghost btn-icon btn-sm" title="Làm mới danh sách" @click="refreshData">
          <RefreshCw :size="13" />
        </button>
      </div>

      <div class="sidebar-content">
        <GroupTree
          :groups="groups"
          :selected-group-id="selectedGroupId"
          @select-command="handleSelectCommand"
          @select-group="selectedGroupId = $event"
          @run-group="handleRunGroup"
          @stop-group="handleStopGroup"
        />
      </div>
    </aside>

    <!-- Main Workspace: Session Ribbon & Dockview Terminals Host -->
    <main class="workspace-main">
      <!-- Top Session Status Strip (from Stitch SCR-01) -->
      <div class="session-status-strip">
        <div class="strip-left">
          <div class="session-badge">
            <span class="session-prefix">SESSION:</span>
            <span class="session-name">{{ currentSessionName }}</span>
            <span class="session-id-tag">#{{ currentSessionId }}</span>
          </div>

          <div
            class="status-pill cursor-pointer"
            :class="{ running: runningProcessesCount > 0 }"
            title="Bấm để mở Menu Quản lý Tiến trình Ngầm (Active Daemons)"
            @click="showBgModal = true"
          >
            <span class="pulse-dot" v-if="runningProcessesCount > 0" />
            <span>{{ runningProcessesCount > 0 ? `${runningProcessesCount} Active Daemons` : 'Idle' }}</span>
          </div>

          <div class="strip-stat">
            <span class="stat-label">Subprocesses:</span>
            <span class="stat-val font-mono">{{ runningProcessesCount }} / {{ totalCommandsCount }}</span>
          </div>
        </div>

        <div class="strip-right">
          <button
            class="btn btn-ghost btn-sm bg-daemons-trigger"
            :class="{ active: runningProcessesCount > 0 }"
            title="Mở Menu Quản lý Tiến trình Ngầm (Active Daemons)"
            @click="showBgModal = true"
          >
            <Zap :size="12" class="zap-icon" />
            <span>⚡ {{ runningProcessesCount }} Ngầm</span>
          </button>
          <button
            class="btn btn-success btn-sm"
            title="Chạy toàn bộ nhóm đang chọn"
            :disabled="!selectedGroup"
            @click="selectedGroup && handleRunGroup(selectedGroup)"
          >
            <Play :size="12" />
            <span>Chạy Nhóm</span>
          </button>
          <button
            v-if="runningProcessesCount > 1"
            class="btn btn-danger btn-sm"
            title="Dừng toàn bộ tiến trình đang chạy"
            @click="handleStopAll"
          >
            <Square :size="11" />
            <span>Dừng Tất Cả</span>
          </button>
        </div>
      </div>

      <!-- Dockhost Viewport -->
      <div class="dock-viewport">
        <div v-if="launchError" class="workspace-error" role="alert">
          Không thể chạy lệnh: {{ launchError }}
        </div>
        <div
          v-if="stopFeedback"
          class="workspace-feedback"
          :class="stopFeedback.type"
          role="status"
          aria-live="polite"
        >
          <span>{{ stopFeedback.message }}</span>
          <button
            class="feedback-close"
            type="button"
            aria-label="Đóng thông báo"
            title="Đóng thông báo"
            @click="dismissStopFeedback"
          >
            <X :size="13" />
          </button>
        </div>
        <DockHost
          ref="dockHostRef"
          @request-stop-process="handleRequestStop"
        />
      </div>
    </main>

    <!-- Modal Dừng Tiến Trình MOD-01 -->
    <StopProcessModal
      :visible="showStopModal"
      :command-id="stoppingCmdId"
      :command-name="stoppingCmdName"
      :pid="stoppingCmdPid"
      :loading="stopInProgress"
      @confirm="confirmStopProcess"
      @cancel="showStopModal = false"
    />
    <!-- Modal Quản Lý Tiến Trình Chạy Ngầm (Active Daemons Manager) -->
    <BackgroundProcessesModal
      :visible="showBgModal"
      :open-tab-command-ids="dockHostRef?.openTabCommandIds"
      :open-tabs="dockHostRef?.openTabs"
      @close="showBgModal = false"
      @open-tab="handleOpenBgTab"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { RefreshCw, Play, Square, Layers, Zap, X } from 'lucide-vue-next';
import GroupTree from '@/components/explorer/GroupTree.vue';
import DockHost from '@/components/terminal/DockHost.vue';
import StopProcessModal from '@/components/dialogs/StopProcessModal.vue';
import BackgroundProcessesModal from '@/components/dialogs/BackgroundProcessesModal.vue';
import { useGroups } from '@/composables/useGroups';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';
import type { CommandGroupWithCommands } from '@/types/models';

const dockHostRef = ref<InstanceType<typeof DockHost> | null>(null);
const route = useRoute();
const router = useRouter();

const { groups, fetchGroups } = useGroups();
const { activeSession, startGroupSession, stopGroupById, stopAllProcesses, stopCommandProcess, getProcessInfo, refreshProcesses, activeProcesses } = useRunSession();

const showStopModal = ref(false);
const showBgModal = ref(false);
const stoppingCmdId = ref(0);
const stoppingCmdName = ref('');
const stoppingCmdPid = ref<number | undefined>(undefined);
const launchError = ref('');
const stopFeedback = ref<{ type: 'success' | 'error'; message: string } | null>(null);
const stopInProgress = ref(false);
let stopFeedbackTimer: ReturnType<typeof setTimeout> | undefined;
const workspaceReady = ref(false);
const launchingCommand = ref(false);
const selectedGroupId = ref<number | undefined>(undefined);

onMounted(async () => {
  await fetchGroups();
  selectedGroupId.value ??= groups.value[0]?.id;
  workspaceReady.value = true;
  await openRequestedCommand();
});

watch(() => [
  route.query.runCommand,
  route.query.runCommandId,
  route.query.runCommandName,
  route.query.runEventId,
], () => {
  if (workspaceReady.value) void openRequestedCommand();
});

const openRequestedCommand = async () => {
  const rawRunEventId = route.query.runEventId;
  const rawTerminalCommandId = route.query.runCommandId;
  const rawTerminalCommandName = route.query.runCommandName;
  if (
    typeof rawRunEventId === 'string' &&
    typeof rawTerminalCommandId === 'string' &&
    typeof rawTerminalCommandName === 'string'
  ) {
    if (launchingCommand.value) return;
    const terminalCommandId = Number(rawTerminalCommandId);
    if (!Number.isInteger(terminalCommandId)) return;

    launchingCommand.value = true;
    launchError.value = '';
    try {
      await nextTick();
      await refreshProcesses();
      dockHostRef.value?.openCommandTab(terminalCommandId, rawTerminalCommandName, rawRunEventId);
      await router.replace({ path: '/workspace' });
    } catch (error) {
      launchError.value = error instanceof Error ? error.message : String(error);
      console.error('[Workspace] Không thể mở terminal template:', error);
    } finally {
      launchingCommand.value = false;
    }
    return;
  }

  const rawCommandId = route.query.runCommand;
  if (launchingCommand.value || typeof rawCommandId !== 'string') return;

  const commandId = Number(rawCommandId);
  if (!Number.isInteger(commandId)) return;

  launchingCommand.value = true;
  launchError.value = '';
  try {
    await nextTick();
    const command = (await ipcClient.listCommands()).find(item => item.id === commandId);
    if (!command) throw new Error(`Không tìm thấy lệnh #${commandId}`);
    await refreshProcesses();
    const result = await ipcClient.runCommand(command.id);
    await refreshProcesses();
    dockHostRef.value?.openCommandTab(command.id, command.name, result.runEventId);
    await router.replace({ path: '/workspace' });
  } catch (error) {
    launchError.value = error instanceof Error ? error.message : String(error);
    console.error('[Workspace] Không thể chạy lệnh từ thư viện:', error);
  } finally {
    launchingCommand.value = false;
  }
};

const refreshData = async () => {
  await fetchGroups();
};

const currentSessionName = computed(() => {
  return activeSession.value?.group_name
    || groups.value.find(group => group.id === selectedGroupId.value)?.group_name
    || 'Chưa chọn nhóm';
});

const currentSessionId = computed(() => {
  return activeSession.value?.id ?? '—';
});

const selectedGroup = computed(() => groups.value.find(group => group.id === selectedGroupId.value));

const runningProcessesCount = computed(() => {
  let count = 0;
  activeProcesses.value.forEach(p => {
    if (p.status === 'running') count++;
  });
  return count;
});

const totalCommandsCount = computed(() => {
  let total = 0;
  groups.value.forEach(g => {
    total += g.commands.length;
  });
  return total;
});

const handleSelectCommand = (commandId: number, commandName: string) => {
  dockHostRef.value?.openCommandTab(commandId, commandName, getProcessInfo(commandId)?.runEventId);
};

const handleOpenBgTab = (proc: { commandId: number; commandName: string; runEventId?: string; shellKind?: string }) => {
  dockHostRef.value?.openCommandTab(proc.commandId, proc.commandName, proc.runEventId, proc.shellKind);
};

const handleRunGroup = async (group: CommandGroupWithCommands) => {
  selectedGroupId.value = group.id;
  await startGroupSession(group.id, group.group_name, group.commands);
  // Tự động mở tab cho từng lệnh trong nhóm
  group.commands.forEach(cmd => {
    dockHostRef.value?.openCommandTab(cmd.id, cmd.name, getProcessInfo(cmd.id)?.runEventId);
  });
};

const handleStopGroup = async (groupId: number) => {
  await stopGroupById(groupId);
};

const handleStopAll = async () => {
  await stopAllProcesses();
};

const handleRequestStop = (commandId: number) => {
  stoppingCmdId.value = commandId;
  const info = getProcessInfo(commandId);
  stoppingCmdName.value = info?.commandName || `Lệnh #${commandId}`;
  stoppingCmdPid.value = info?.pid;
  showStopModal.value = true;
};

const dismissStopFeedback = () => {
  stopFeedback.value = null;
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
  stopFeedbackTimer = undefined;
};

const showStopFeedback = (feedback: { type: 'success' | 'error'; message: string }) => {
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
  stopFeedback.value = feedback;
  stopFeedbackTimer = setTimeout(() => {
    stopFeedback.value = null;
    stopFeedbackTimer = undefined;
  }, 4000);
};

const confirmStopProcess = async (commandId: number, force: boolean) => {
  dismissStopFeedback();
  stopInProgress.value = true;
  try {
    await stopCommandProcess(commandId, force);
    showStopFeedback({
      type: 'success',
      message: `Đã xác nhận process của "${stoppingCmdName.value}" đã dừng hoàn toàn.`,
    });
    showStopModal.value = false;
  } catch (error) {
    showStopFeedback({
      type: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
  } finally {
    stopInProgress.value = false;
  }
};

onBeforeUnmount(() => {
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
});
</script>

<style scoped>
.workspace-view {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.workspace-sidebar {
  width: var(--side-panel-width);
  height: 100%;
  background-color: var(--bg-sidebar);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-header {
  height: 38px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
}

.sidebar-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.header-icon {
  color: var(--primary-accent);
}

.header-title {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

.sidebar-content {
  flex: 1;
  padding: 8px;
  overflow-y: auto;
}

.workspace-main {
  flex: 1;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-terminal);
  overflow: hidden;
}

/* Stitch SCR-01 Session Status Strip */
.session-status-strip {
  height: 36px;
  background-color: #12151f;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  flex-shrink: 0;
  user-select: none;
}

.strip-left, .strip-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.session-badge {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
}

.session-prefix {
  font-weight: 700;
  color: var(--text-muted);
  font-size: 9.5px;
  letter-spacing: 0.05em;
}

.session-name {
  font-weight: 600;
  color: var(--text-primary);
}

.session-id-tag {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--primary-accent);
  background-color: var(--primary-subtle);
  padding: 1px 4px;
  border-radius: 2px;
}

.status-pill {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  font-weight: 600;
  padding: 2px 7px;
  border-radius: 9999px;
  background-color: var(--status-idle-bg);
  color: var(--status-idle);
  border: 1px solid rgba(100, 116, 139, 0.2);
}

.status-pill.running {
  background-color: var(--status-running-bg);
  color: #34d399;
  border-color: rgba(16, 185, 129, 0.3);
}

.strip-stat {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
}

.stat-label {
  color: var(--text-muted);
  font-size: 10px;
}

.stat-val {
  color: var(--text-secondary);
  font-weight: 500;
}

.dock-viewport {
  flex: 1;
  width: 100%;
  height: calc(100% - 36px);
  overflow: hidden;
}

.workspace-error {
  position: absolute;
  z-index: 2;
  margin: 8px;
  padding: 8px 10px;
  color: #fca5a5;
  background: rgba(127, 29, 29, 0.9);
  border: 1px solid rgba(248, 113, 113, 0.35);
  border-radius: var(--radius-sm);
  font-size: 12px;
}

.workspace-feedback {
  position: absolute;
  z-index: 2;
  top: 8px;
  right: 8px;
  max-width: min(520px, calc(100% - 16px));
  padding: 8px 10px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
}

.feedback-close {
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

.feedback-close:hover {
  background: rgba(255, 255, 255, 0.12);
}

.workspace-feedback.success {
  color: #bbf7d0;
  background: rgba(18, 53, 34, 0.95);
  border: 1px solid rgba(74, 222, 128, 0.55);
}

.workspace-feedback.error {
  color: #fecaca;
  background: rgba(127, 29, 29, 0.95);
  border: 1px solid rgba(248, 113, 113, 0.55);
}

.cursor-pointer {
  cursor: pointer;
}

.bg-daemons-trigger {
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.bg-daemons-trigger:hover {
  border-color: var(--primary);
  color: var(--text-primary);
}

.bg-daemons-trigger.active {
  color: #34d399;
  border-color: rgba(16, 185, 129, 0.4);
  background-color: rgba(16, 185, 129, 0.1);
}

.zap-icon {
  color: #f59e0b;
}
</style>
