import { computed, ref } from 'vue';
import { ipcClient } from '@/ipc/client';
import type {
  SecretChange,
  SshIdentity,
  SshIdentityInput,
  SshIdentityView,
  SshKey,
  SshKeyView,
  SshSecretStore,
} from '@/types/ssh';

// Shared across the Servers view, its Keychain tab and MOD-17, so a key
// created in one place shows up in the others.
const keys = ref<SshKeyView[]>([]);
const identities = ref<SshIdentityView[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

export function useKeychain() {
  const fetchKeychain = async () => {
    loading.value = true;
    error.value = null;
    try {
      const list = await ipcClient.sshKeychainList();
      keys.value = list.keys;
      identities.value = list.identities;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      console.error('[useKeychain] fetchKeychain error:', e);
    } finally {
      loading.value = false;
    }
  };

  const importKey = async (
    name: string,
    privateKey: string,
    passphrase: string | undefined,
    store: SshSecretStore
  ): Promise<SshKey> => {
    const key = await ipcClient.sshKeyImport(name, privateKey, passphrase, store);
    await fetchKeychain();
    return key;
  };

  const generateKey = async (name: string, store: SshSecretStore): Promise<SshKey> => {
    const key = await ipcClient.sshKeyGenerate(name, store);
    await fetchKeychain();
    return key;
  };

  const renameKey = async (id: string, name: string): Promise<SshKey> => {
    const key = await ipcClient.sshKeyRename(id, name);
    await fetchKeychain();
    return key;
  };

  const deleteKey = async (id: string): Promise<void> => {
    await ipcClient.sshKeyDelete(id, true);
    await fetchKeychain();
  };

  const saveIdentity = async (
    id: string | null,
    input: SshIdentityInput,
    password: SecretChange | undefined,
    store: SshSecretStore
  ): Promise<SshIdentity> => {
    const saved = id
      ? await ipcClient.sshIdentityUpdate(id, input, password, store)
      : await ipcClient.sshIdentityCreate(input, password, store);
    await fetchKeychain();
    return saved;
  };

  const deleteIdentity = async (id: string): Promise<void> => {
    await ipcClient.sshIdentityDelete(id, true);
    await fetchKeychain();
  };

  const keyById = computed(() => new Map(keys.value.map((key) => [key.id, key])));
  const identityById = computed(
    () => new Map(identities.value.map((identity) => [identity.id, identity]))
  );

  return {
    keys,
    identities,
    loading,
    error,
    keyById,
    identityById,
    fetchKeychain,
    importKey,
    generateKey,
    renameKey,
    deleteKey,
    saveIdentity,
    deleteIdentity,
  };
}
