import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { CommandGroupWithCommands } from '@/types/models';

export function useGroups() {
  const groups = ref<CommandGroupWithCommands[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const fetchGroups = async () => {
    loading.value = true;
    error.value = null;
    try {
      groups.value = await ipcClient.listGroups();
    } catch (err: any) {
      error.value = err?.message || 'Không thể tải danh sách nhóm';
    } finally {
      loading.value = false;
    }
  };

  const saveGroup = async (group: {
    id?: number;
    group_name: string;
    autostart: boolean;
    execution_mode: 'startup' | 'sequential';
    commandIds?: number[];
  }) => {
    loading.value = true;
    error.value = null;
    try {
      await ipcClient.saveGroup(group);
      await fetchGroups();
      return true;
    } catch (err: any) {
      error.value = err?.message || 'Không thể lưu nhóm lệnh';
      return false;
    } finally {
      loading.value = false;
    }
  };

  const deleteGroup = async (id: number) => {
    loading.value = true;
    error.value = null;
    try {
      await ipcClient.deleteGroup(id);
      await fetchGroups();
      return true;
    } catch (err: any) {
      error.value = err?.message || 'Không thể xóa nhóm lệnh';
      return false;
    } finally {
      loading.value = false;
    }
  };

  const toggleAutostart = async (groupId: number, currentVal: boolean) => {
    const target = groups.value.find(g => g.id === groupId);
    if (!target) return;
    const commandIds = target.commands.map(c => c.id);
    await saveGroup({
      id: groupId,
      group_name: target.group_name,
      autostart: !currentVal,
      execution_mode: target.execution_mode,
      commandIds,
    });
  };

  return {
    groups,
    loading,
    error,
    fetchGroups,
    saveGroup,
    deleteGroup,
    toggleAutostart,
  };
}
