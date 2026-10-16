import { ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type {
  SshHost,
  SshHostInput,
  SshHostView,
  SecretsInput,
  SshTestReport,
} from '@/types/ssh';

const hosts = ref<SshHostView[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

export function useSshHosts() {
  const fetchHosts = async () => {
    loading.value = true;
    error.value = null;
    try {
      hosts.value = await ipcClient.sshHostsList();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      console.error('[useSshHosts] fetchHosts error:', e);
    } finally {
      loading.value = false;
    }
  };

  const createHost = async (
    input: SshHostInput,
    secrets?: SecretsInput
  ): Promise<SshHost> => {
    const created = await ipcClient.sshHostsCreate(input, secrets);
    await fetchHosts();
    return created;
  };

  const updateHost = async (
    id: string,
    input: SshHostInput,
    secrets?: SecretsInput
  ): Promise<SshHost> => {
    const updated = await ipcClient.sshHostsUpdate(id, input, secrets);
    await fetchHosts();
    return updated;
  };

  const deleteHost = async (id: string, confirmed: boolean): Promise<void> => {
    await ipcClient.sshHostsDelete(id, confirmed);
    await fetchHosts();
  };

  const testConnection = async (
    host: SshHostInput,
    hostId?: string,
    password?: string,
    passphrase?: string,
    privateKey?: string
  ): Promise<SshTestReport> => {
    return ipcClient.sshTestConnection(host, hostId, password, passphrase, privateKey);
  };

  const forgetSecret = async (hostId: string): Promise<void> => {
    await ipcClient.sshForgetSecret(hostId);
    await fetchHosts();
  };

  const forgetHostKey = async (hostId: string): Promise<number> => {
    const removed = await ipcClient.sshForgetHostKey(hostId);
    await fetchHosts();
    return removed;
  };

  const disconnectHost = async (hostId: string): Promise<void> => {
    await ipcClient.sshDisconnect(hostId);
    await fetchHosts();
  };

  return {
    hosts,
    loading,
    error,
    fetchHosts,
    createHost,
    updateHost,
    deleteHost,
    testConnection,
    forgetSecret,
    forgetHostKey,
    disconnectHost,
  };
}
