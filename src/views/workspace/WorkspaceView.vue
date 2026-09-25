<template>
  <div class="workspace-view">
    <!-- Left Explorer: Group Tree -->
    <aside class="workspace-sidebar">
      <div class="sidebar-header">
        <span class="header-title">Nhóm Lệnh & Phiên</span>
        <button class="btn btn-ghost btn-icon btn-sm" title="Làm mới danh sách" @click="refreshData">
          <RefreshCw :size="13" />
        </button>
      </div>

      <div class="sidebar-content">
        <GroupTree
          :groups="groups"
          @select-command="handleSelectCommand"
          @run-group="handleRunGroup"
          @stop-group="handleStopGroup"
        />
      </div>
    </aside>

    <!-- Main Workspace: Dockview Terminals Host -->
    <main class="workspace-main">
      <DockHost
        ref="dockHostRef"
        @request-stop-process="handleRequestStop"
      />
    </main>

    <!-- Modal Dừng Tiến Trình MOD-01 -->
    <StopProcessModal
      :visible="showStopModal"
      :command-id="stoppingCmdId"
      :command-name="stoppingCmdName"
      :pid="stoppingCmdPid"
      @confirm="confirmStopProcess"
      @cancel="showStopModal = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { RefreshCw } from 'lucide-vue-next';
import GroupTree from '@/components/explorer/GroupTree.vue';
import DockHost from '@/components/terminal/DockHost.vue';
import StopProcessModal from '@/components/dialogs/StopProcessModal.vue';
import { useGroups } from '@/composables/useGroups';
import { useRunSession } from '@/composables/useRunSession';
import type { CommandGroupWithCommands } from '@/types/models';

const dockHostRef = ref<InstanceType<typeof DockHost> | null>(null);

const { groups, fetchGroups } = useGroups();
const { startGroupSession, stopGroupSession, stopCommandProcess, getProcessInfo } = useRunSession();

const showStopModal = ref(false);
const stoppingCmdId = ref(0);
const stoppingCmdName = ref('');
const stoppingCmdPid = ref<number | undefined>(undefined);

onMounted(async () => {
  await fetchGroups();
});

const refreshData = async () => {
  await fetchGroups();
};

const handleSelectCommand = (commandId: number, commandName: string) => {
  dockHostRef.value?.openCommandTab(commandId, commandName);
};

const handleRunGroup = async (group: CommandGroupWithCommands) => {
  await startGroupSession(group.id, group.group_name, group.commands);
  // Tự động mở tab cho từng lệnh trong nhóm
  group.commands.forEach(cmd => {
    dockHostRef.value?.openCommandTab(cmd.id, cmd.name);
  });
};

const handleStopGroup = async (groupId: number) => {
  await stopGroupSession();
};

const handleRequestStop = (commandId: number) => {
  stoppingCmdId.value = commandId;
  const info = getProcessInfo(commandId);
  stoppingCmdName.value = info?.commandName || `Lệnh #${commandId}`;
  stoppingCmdPid.value = info?.pid;
  showStopModal.value = true;
};

const confirmStopProcess = async (commandId: number, force: boolean) => {
  await stopCommandProcess(commandId, force);
  showStopModal.value = false;
};
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
  background-color: var(--bg-terminal);
  overflow: hidden;
}
</style>
