import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { RunSession, ActiveProcessInfo, ProcessLifecycleStatus } from '@/types/models';

// State chia sẻ toàn app
const activeSession = ref<RunSession | null>(null);
const activeProcesses = ref<Map<number, ActiveProcessInfo>>(new Map());
const sessionByGroup = new Map<number, number>();

let statusListenerSetup: Promise<void> | null = null;

async function ensureProcessStatusListener() {
  if (statusListenerSetup) return statusListenerSetup;

  statusListenerSetup = ipcClient.onProcessStatus((process) => {
    const next = new Map(activeProcesses.value);
    const current = next.get(process.commandId);
    next.set(process.commandId, {
      ...current,
      ...process,
      commandName: process.commandName || current?.commandName || `Lệnh #${process.commandId}`,
    });
    activeProcesses.value = next;
  }).then(() => undefined);

  return statusListenerSetup;
}

export function useRunSession() {
  const loading = ref(false);

  const refreshProcesses = async () => {
    await ensureProcessStatusListener();
    const processes = await ipcClient.listProcesses();
    activeProcesses.value = new Map(processes.map((process) => [process.commandId, process]));
    return processes;
  };

  const startGroupSession = async (
    groupId: number,
    groupName?: string,
    _commands?: { id: number; name: string }[],
  ) => {
    loading.value = true;
    try {
      await ensureProcessStatusListener();
      const session = await ipcClient.startSession(groupId);
      activeSession.value = {
        ...session,
        group_name: groupName || session.group_name,
      };
      sessionByGroup.set(groupId, session.id);
      await refreshProcesses();
      return session;
    } finally {
      loading.value = false;
    }
  };

  const stopGroupSession = async (sessionId?: number) => {
    const idToStop = sessionId || activeSession.value?.id;
    loading.value = true;
    try {
      if (idToStop) {
        await ipcClient.stopSession(idToStop);
      }
      const runningList = Array.from(activeProcesses.value.entries()).filter(
        ([, proc]) => proc.status === 'running' && (!sessionId || proc.sessionId === sessionId)
      );
      await Promise.all(runningList.map(async ([cmdId]) => {
        try {
          await ipcClient.stopProcess(cmdId, false);
        } catch (e) {
          console.warn(`[useRunSession] Failed to stop process #${cmdId}:`, e);
        }
      }));
      const next = new Map(activeProcesses.value);
      next.forEach((proc, commandId) => {
        if (!sessionId || proc.sessionId === sessionId) {
          next.set(commandId, { ...proc, status: 'stopped' });
        }
      });
      activeProcesses.value = next;
      if (!sessionId || activeSession.value?.id === idToStop) activeSession.value = null;
    } finally {
      loading.value = false;
    }
  };

  const stopCommandProcess = async (commandId: number, force = false) => {
    await ipcClient.stopProcess(commandId, force);
    const existing = activeProcesses.value.get(commandId);
    if (existing) {
      const next = new Map(activeProcesses.value);
      next.set(commandId, { ...existing, status: 'stopped', exitCode: force ? 137 : null });
      activeProcesses.value = next;
    }
  };

  const stopAllProcesses = async (force = false) => {
    loading.value = true;
    try {
      const runningIds = Array.from(activeProcesses.value.entries())
        .filter(([, process]) => process.status === 'running')
        .map(([commandId]) => commandId);
      await Promise.all(runningIds.map(async (commandId) => {
        try {
          await ipcClient.stopProcess(commandId, force);
        } catch (error) {
          console.warn(`[useRunSession] Không thể dừng process #${commandId}:`, error);
        }
      }));
      const next = new Map(activeProcesses.value);
      next.forEach((process, commandId) => {
        if (process.status === 'running') {
          next.set(commandId, { ...process, status: 'stopped' });
        }
      });
      activeProcesses.value = next;
      activeSession.value = null;
    } finally {
      loading.value = false;
    }
  };

  const stopGroupById = async (groupId: number) => {
    const sessionId = sessionByGroup.get(groupId);
    if (sessionId !== undefined) {
      await stopGroupSession(sessionId);
      sessionByGroup.delete(groupId);
      return;
    }
    const processIds = Array.from(activeProcesses.value.entries())
      .filter(([, process]) => process.status === 'running' && process.groupId === groupId)
      .map(([commandId]) => commandId);
    await Promise.all(processIds.map(commandId => ipcClient.stopProcess(commandId).catch(() => undefined)));
  };

  const getProcessStatus = (commandId: number): ProcessLifecycleStatus => {
    return activeProcesses.value.get(commandId)?.status || 'idle';
  };

  const getProcessInfo = (commandId: number): ActiveProcessInfo | undefined => {
    return activeProcesses.value.get(commandId);
  };

  const updateProcessName = (commandId: number, name: string) => {
    const existing = activeProcesses.value.get(commandId);
    if (existing) {
      const next = new Map(activeProcesses.value);
      next.set(commandId, { ...existing, commandName: name });
      activeProcesses.value = next;
    }
    ipcClient.setTerminalName?.(commandId, name);
  };

  return {
    activeSession,
    activeProcesses,
    loading,
    startGroupSession,
    stopGroupSession,
    stopCommandProcess,
    stopAllProcesses,
    stopGroupById,
    refreshProcesses,
    getProcessStatus,
    getProcessInfo,
    updateProcessName,
  };
}
