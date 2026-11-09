import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type { TerminalProfile } from '@/types/models';

/**
 * The kinds of terminal a new tab can open, read once per app run: listing
 * WSL distributions starts `wsl.exe`. `reload` reads them again (Settings).
 */
const profiles = ref<TerminalProfile[]>([]);
const loading = ref(false);
const error = ref('');
let pending: Promise<TerminalProfile[]> | null = null;

const load = (force = false): Promise<TerminalProfile[]> => {
  if (pending && !force) return pending;
  loading.value = true;
  error.value = '';
  pending = ipcClient
    .listTerminalProfiles()
    .then((list) => {
      profiles.value = list;
      return list;
    })
    .catch((e) => {
      error.value = String(e);
      pending = null;
      return profiles.value;
    })
    .finally(() => {
      loading.value = false;
    });
  return pending;
};

export function useTerminalProfiles() {
  void load();
  return {
    profiles,
    loading,
    error,
    reload: () => load(true),
  };
}
