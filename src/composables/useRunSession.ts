import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { RunSession, ActiveProcessInfo, ProcessLifecycleStatus } from '@/types/models';

// State chia sẻ toàn app
const activeSession = ref<RunSession | null>(null);
const activeProcesses = ref<Map<number, ActiveProcessInfo>>(new Map());

// Khởi tạo một số process đang chạy cho preview
activeProcesses.value.set(2, {
  commandId: 2,
  commandName: 'NestJS Backend API',
  pid: 2845,
  status: 'running',
  startedAt: '14:20:10',
});
activeProcesses.value.set(1, {
  commandId: 1,
  commandName: 'Vite Frontend Dev',
  pid: 2841,
  status: 'running',
  startedAt: '14:20:12',
});

export function useRunSession() {
  const loading = ref(false);

  const startGroupSession = async (groupId: number, groupName?: string, commands?: { id: number; name: string }[]) => {
    loading.value = true;
    try {
      const session = await ipcClient.startSession(groupId);
      activeSession.value = {
        ...session,
        group_name: groupName || session.group_name,
      };

      // Đánh dấu các lệnh trong nhóm sang trạng thái starting / running
      if (commands && commands.length > 0) {
        commands.forEach((cmd, idx) => {
          activeProcesses.value.set(cmd.id, {
            commandId: cmd.id,
            commandName: cmd.name,
            pid: 3000 + Math.floor(Math.random() * 1000),
            status: idx === 0 ? 'running' : 'starting',
            startedAt: new Date().toLocaleTimeString(),
            sessionId: session.id,
          });
        });
      }

      return session;
    } finally {
      loading.value = false;
    }
  };

  const stopGroupSession = async (sessionId?: number) => {
    const idToStop = sessionId || activeSession.value?.id;
    if (!idToStop) return;
    loading.value = true;
    try {
      await ipcClient.stopSession(idToStop);
      if (activeSession.value?.id === idToStop) {
        activeSession.value = null;
      }
      // Đổi trạng thái các process sang stopped
      activeProcesses.value.forEach((proc, cId) => {
        if (!sessionId || proc.sessionId === sessionId) {
          activeProcesses.value.set(cId, { ...proc, status: 'stopped' });
        }
      });
    } finally {
      loading.value = false;
    }
  };

  const stopCommandProcess = async (commandId: number, force = false) => {
    await ipcClient.stopProcess(commandId, force);
    const existing = activeProcesses.value.get(commandId);
    if (existing) {
      activeProcesses.value.set(commandId, {
        ...existing,
        status: 'stopped',
        exitCode: force ? 137 : 0,
      });
    }
  };

  const getProcessStatus = (commandId: number): ProcessLifecycleStatus => {
    return activeProcesses.value.get(commandId)?.status || 'idle';
  };

  const getProcessInfo = (commandId: number): ActiveProcessInfo | undefined => {
    return activeProcesses.value.get(commandId);
  };

  return {
    activeSession,
    activeProcesses,
    loading,
    startGroupSession,
    stopGroupSession,
    stopCommandProcess,
    getProcessStatus,
    getProcessInfo,
  };
}
