import { ref, computed, onMounted, onActivated, onBeforeUnmount } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { CommandDefinition } from '@/types/models';

export function useQuickAccess() {
  const commands = ref<CommandDefinition[]>([]);
  const loading = ref(false);
  const searchQuery = ref('');
  let unlisten: (() => void) | null = null;

  const reload = async () => {
    loading.value = true;
    try {
      const all = await ipcClient.listCommands();
      commands.value = all
        .filter((c) => Boolean(c.quick_access))
        .sort((a, b) => a.name.localeCompare(b.name));
    } catch (e) {
      console.error('[useQuickAccess] Failed to load commands:', e);
    } finally {
      loading.value = false;
    }
  };

  const filteredCommands = computed(() => {
    const q = searchQuery.value.trim().toLowerCase();
    if (!q) return commands.value;
    return commands.value.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.execution_string.toLowerCase().includes(q)
    );
  });

  onMounted(async () => {
    await reload();
    try {
      unlisten = await ipcClient.onCommandsChanged(() => {
        void reload();
      });
    } catch (err) {
      console.warn('[useQuickAccess] onCommandsChanged listener setup failed:', err);
    }
  });

  onActivated(() => {
    void reload();
  });

  onBeforeUnmount(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  });

  return {
    commands,
    loading,
    searchQuery,
    filteredCommands,
    reload,
  };
}
