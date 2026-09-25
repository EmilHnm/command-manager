import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { CommandDefinition } from '@/types/models';

export function useCommands() {
  const commands = ref<CommandDefinition[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const fetchCommands = async () => {
    loading.value = true;
    error.value = null;
    try {
      commands.value = await ipcClient.listCommands();
    } catch (err: any) {
      error.value = err?.message || 'Không thể tải danh sách lệnh';
    } finally {
      loading.value = false;
    }
  };

  const saveCommand = async (command: Partial<CommandDefinition>) => {
    loading.value = true;
    error.value = null;
    try {
      await ipcClient.saveCommand(command);
      await fetchCommands();
      return true;
    } catch (err: any) {
      error.value = err?.message || 'Không thể lưu câu lệnh';
      return false;
    } finally {
      loading.value = false;
    }
  };

  const deleteCommand = async (id: number) => {
    loading.value = true;
    error.value = null;
    try {
      await ipcClient.deleteCommand(id);
      await fetchCommands();
      return true;
    } catch (err: any) {
      error.value = err?.message || 'Không thể xóa câu lệnh';
      return false;
    } finally {
      loading.value = false;
    }
  };

  return {
    commands,
    loading,
    error,
    fetchCommands,
    saveCommand,
    deleteCommand,
  };
}
