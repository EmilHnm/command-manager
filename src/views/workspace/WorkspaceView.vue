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

      <div
        ref="sidebarContentRef"
        class="sidebar-content"
        :class="{ 'has-quick-access': Boolean(quickAccessTarget) }"
      >
        <div class="sidebar-section-tree" :style="treeSectionStyle">
          <GroupTree
            :groups="groups"
            :selected-group-id="selectedGroupId"
            :stopping-group-id="stoppingGroupId"
            :running-group-id="runningGroup ? selectedGroupId : null"
            @select-command="handleSelectCommand"
            @select-group="selectedGroupId = $event"
            @run-group="handleRunGroup"
            @stop-group="handleStopGroup"
          />
        </div>

        <template v-if="quickAccessTarget">
          <!-- Horizontal resizable sash divider -->
          <div
            v-if="!quickAccessCollapsed"
            class="sidebar-sash"
            :class="{ dragging: isDraggingSash }"
            title="Kéo để chỉnh tỉ lệ (Nháy đúp để về 50:50)"
            @pointerdown="handleSashPointerDown"
            @dblclick="resetQuickAccessRatio"
          >
            <div class="sash-grip" />
          </div>

          <!-- Section 2: Quick Access Panel -->
          <div
            class="sidebar-section-qa"
            :class="{ collapsed: quickAccessCollapsed }"
            :style="qaSectionStyle"
          >
            <QuickAccessPanel
              :target="quickAccessTarget"
              :collapsed="quickAccessCollapsed"
              @toggle-collapse="toggleQuickAccessCollapsed"
              @run="handleQuickAccessRun"
              @paste="handleQuickAccessPaste"
            />
          </div>
        </template>
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
            :disabled="!selectedGroup || runningGroup"
            @click="selectedGroup && handleRunGroup(selectedGroup)"
          >
            <LoaderCircle v-if="runningGroup" :size="12" class="spin" />
            <Play v-else :size="12" />
            <span>{{ runningGroup ? 'Đang Khởi Chạy...' : 'Chạy Nhóm' }}</span>
          </button>
          <button
            v-if="runningProcessesCount > 1"
            class="btn btn-danger btn-sm"
            title="Dừng toàn bộ tiến trình đang chạy"
            :disabled="stoppingAll"
            @click="handleStopAll"
          >
            <LoaderCircle v-if="stoppingAll" :size="11" class="spin" />
            <Square v-else :size="11" />
            <span>{{ stoppingAll ? 'Đang Dừng...' : 'Dừng Tất Cả' }}</span>
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

    <!-- Stop Process Modal (MOD-01) -->
    <StopProcessModal
      :visible="showStopModal"
      :command-id="stoppingCmdId"
      :command-name="stoppingCmdName"
      :pid="stoppingCmdPid"
      :loading="stopInProgress"
      @confirm="confirmStopProcess"
      @cancel="showStopModal = false"
    />
    <!-- Active Daemons Manager Modal -->
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
import { RefreshCw, Play, Square, Layers, Zap, X, LoaderCircle } from 'lucide-vue-next';
import GroupTree from '@/components/explorer/GroupTree.vue';
import QuickAccessPanel from '@/components/explorer/QuickAccessPanel.vue';
import DockHost from '@/components/terminal/DockHost.vue';
import StopProcessModal from '@/components/dialogs/StopProcessModal.vue';
import BackgroundProcessesModal from '@/components/dialogs/BackgroundProcessesModal.vue';
import { useGroups } from '@/composables/useGroups';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';
import type { CommandGroupWithCommands, CommandDefinition } from '@/types/models';

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

// Quick Access section sizing & state
const QA_RATIO_KEY = 'cm_quick_access_ratio_v1';
const QA_COLLAPSED_KEY = 'cm_quick_access_collapsed_v1';

const savedQaRatio = typeof window !== 'undefined' ? Number(localStorage.getItem(QA_RATIO_KEY)) : 50;
const savedQaCollapsed = typeof window !== 'undefined' ? localStorage.getItem(QA_COLLAPSED_KEY) === 'true' : false;

const quickAccessRatio = ref<number>(!isNaN(savedQaRatio) && savedQaRatio >= 20 && savedQaRatio <= 80 ? savedQaRatio : 50);
const quickAccessCollapsed = ref<boolean>(savedQaCollapsed);
const isDraggingSash = ref(false);
const sidebarContentRef = ref<HTMLElement | null>(null);

const quickAccessTarget = computed(() => dockHostRef.value?.quickAccessTarget ?? null);

const treeSectionStyle = computed(() => {
  if (!quickAccessTarget.value) return { flex: '1', height: '100%' };
  if (quickAccessCollapsed.value) return { height: 'calc(100% - 28px)', flex: 'none' };
  return { height: `calc(${quickAccessRatio.value}% - 3px)`, flex: 'none' };
});

const qaSectionStyle = computed(() => {
  if (!quickAccessTarget.value) return {};
  if (quickAccessCollapsed.value) return { height: '28px', flex: 'none' };
  return { height: `calc(${100 - quickAccessRatio.value}% - 2px)`, flex: 'none' };
});

const toggleQuickAccessCollapsed = () => {
  quickAccessCollapsed.value = !quickAccessCollapsed.value;
  localStorage.setItem(QA_COLLAPSED_KEY, String(quickAccessCollapsed.value));
};

const resetQuickAccessRatio = () => {
  quickAccessRatio.value = 50;
  localStorage.setItem(QA_RATIO_KEY, '50');
};

const handleSashPointerDown = (e: PointerEvent) => {
  if (!sidebarContentRef.value) return;
  e.preventDefault();
  isDraggingSash.value = true;
  const containerRect = sidebarContentRef.value.getBoundingClientRect();
  const totalHeight = containerRect.height;
  const topOffset = containerRect.top;

  const onPointerMove = (ev: PointerEvent) => {
    const currentY = ev.clientY - topOffset;
    const clampedY = Math.max(120, Math.min(totalHeight - 120, currentY));
    const newRatio = Math.round((clampedY / totalHeight) * 100);
    quickAccessRatio.value = Math.max(20, Math.min(80, newRatio));
  };

  const onPointerUp = () => {
    isDraggingSash.value = false;
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
    localStorage.setItem(QA_RATIO_KEY, String(quickAccessRatio.value));
  };

  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  window.addEventListener('pointercancel', onPointerUp);
};

const handleQuickAccessRun = (cmd: CommandDefinition) => {
  dockHostRef.value?.sendToFocusedTerminal(cmd.execution_string, { execute: true });
};

const handleQuickAccessPaste = (cmd: CommandDefinition) => {
  dockHostRef.value?.sendToFocusedTerminal(cmd.execution_string, { execute: false });
};
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

const runningGroup = ref(false);
const stoppingAll = ref(false);
const stoppingGroupId = ref<number | null>(null);

const handleRunGroup = async (group: CommandGroupWithCommands) => {
  if (runningGroup.value) return;
  runningGroup.value = true;
  try {
    selectedGroupId.value = group.id;
    await startGroupSession(group.id, group.group_name, group.commands);
    group.commands.forEach(cmd => {
      dockHostRef.value?.openCommandTab(cmd.id, cmd.name, getProcessInfo(cmd.id)?.runEventId);
    });
  } finally {
    runningGroup.value = false;
  }
};

const handleStopGroup = async (groupId: number) => {
  if (stoppingGroupId.value !== null) return;
  stoppingGroupId.value = groupId;
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    await stopGroupById(groupId);
  } finally {
    stoppingGroupId.value = null;
  }
};

const handleStopAll = async () => {
  if (stoppingAll.value) return;
  stoppingAll.value = true;
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    await stopAllProcesses();
  } finally {
    stoppingAll.value = false;
  }
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
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
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

.sidebar-content.has-quick-access {
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-section-tree {
  overflow-y: auto;
  padding: 8px;
  min-height: 120px;
}

.sidebar-sash {
  height: 5px;
  background-color: var(--border-subtle);
  cursor: row-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  z-index: 10;
  transition: background-color 0.15s ease;
  user-select: none;
}

.sidebar-sash:hover,
.sidebar-sash.dragging {
  background-color: var(--primary-accent, #744791);
}

.sash-grip {
  width: 32px;
  height: 2px;
  border-radius: 2px;
  background-color: var(--surface-white-20, #ffffff33);
}

.sidebar-sash:hover .sash-grip,
.sidebar-sash.dragging .sash-grip {
  background-color: #fff;
}

.sidebar-section-qa {
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 120px;
}

.sidebar-section-qa.collapsed {
  min-height: 28px !important;
  max-height: 28px !important;
  height: 28px !important;
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
  border: 1px solid #64748b33;
}

.status-pill.running {
  background-color: var(--status-running-bg);
  color: #34d399;
  border-color: #10b9814d;
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
  background: #7f1d1de6;
  border: 1px solid #f8717159;
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
  box-shadow: 0 8px 24px #00000047;
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
  background: #ffffff1f;
}

.workspace-feedback.success {
  color: #bbf7d0;
  background: #123522f2;
  border: 1px solid #4ade808c;
}

.workspace-feedback.error {
  color: #fecaca;
  background: #7f1d1df2;
  border: 1px solid #f871718c;
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
  border-color: #10b98166;
  background-color: var(--status-running-bg, #10b9811a);
}

.zap-icon {
  color: #f59e0b;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
