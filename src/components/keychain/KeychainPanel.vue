<template>
  <div class="keychain-panel">
    <div v-if="error" class="panel-error">
      <AlertTriangle :size="14" />
      <span>Không tải được Keychain: {{ error }}</span>
      <button type="button" class="btn btn-secondary btn-sm" @click="fetchKeychain">Thử Lại</button>
    </div>

    <!-- Keys -->
    <section class="kc-section">
      <header class="kc-section-header">
        <div class="kc-section-title">
          <KeyRound :size="14" />
          <span>Khóa</span>
          <span class="kc-count">{{ keys.length }}</span>
        </div>
        <button type="button" class="btn btn-primary btn-sm" @click="showKeyModal = true">
          <Plus :size="13" />
          <span>Thêm Khóa</span>
        </button>
      </header>

      <div v-if="keys.length === 0" class="kc-empty">
        Chưa có khóa nào. Dán một khóa sẵn có (OpenSSH, PEM, PuTTY) hoặc sinh khóa ed25519 mới,
        rồi chọn nó cho nhiều máy chủ như Keychain của Termius.
      </div>

      <div v-else class="kc-list">
        <div v-for="key in keys" :key="key.id" class="kc-row">
          <div class="kc-icon"><KeyRound :size="15" /></div>
          <div class="kc-main">
            <div class="kc-name-line">
              <span class="kc-name">{{ key.name }}</span>
              <span class="kc-badge">{{ algorithmLabel(key.algorithm) }}</span>
              <span v-if="key.encrypted" class="kc-badge" title="Khóa có passphrase">
                <Lock :size="10" /> passphrase
              </span>
              <span class="kc-badge kc-badge-store">
                <ShieldCheck :size="10" /> {{ key.secret_store === 'keyring' ? 'Keyring' : 'SQLite' }}
              </span>
            </div>
            <span class="kc-meta font-mono" :title="key.public_key">{{ key.fingerprint }}</span>
            <span class="kc-meta">{{ keyUsageLabel(key) }}</span>
          </div>
          <div class="kc-actions">
            <button
              type="button"
              class="kc-action"
              :title="copiedKeyId === key.id ? 'Đã chép' : 'Chép public key'"
              @click="copyPublicKey(key)"
            >
              <Check v-if="copiedKeyId === key.id" :size="13" />
              <Copy v-else :size="13" />
            </button>
            <button type="button" class="kc-action" title="Đổi tên" @click="renaming = key">
              <Pencil :size="13" />
            </button>
            <button
              type="button"
              class="kc-action kc-action-danger"
              title="Xóa khóa"
              @click="deletingKey = key"
            >
              <Trash2 :size="13" />
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- Identities -->
    <section class="kc-section">
      <header class="kc-section-header">
        <div class="kc-section-title">
          <UserRound :size="14" />
          <span>Identity</span>
          <span class="kc-count">{{ identities.length }}</span>
        </div>
        <button type="button" class="btn btn-primary btn-sm" @click="openIdentity(null)">
          <Plus :size="13" />
          <span>Thêm Identity</span>
        </button>
      </header>

      <div v-if="identities.length === 0" class="kc-empty">
        Identity gom tên đăng nhập, mật khẩu và khóa thành một mục. Chọn identity cho máy chủ thì
        đổi mật khẩu một lần là mọi máy chủ dùng nó đều cập nhật.
      </div>

      <div v-else class="kc-list">
        <div v-for="identity in identities" :key="identity.id" class="kc-row">
          <div class="kc-icon"><UserRound :size="15" /></div>
          <div class="kc-main">
            <div class="kc-name-line">
              <span class="kc-name">{{ identity.name }}</span>
              <span class="kc-meta font-mono">{{ identity.username }}</span>
            </div>
            <div class="kc-name-line">
              <span v-if="identity.secret_store !== 'none'" class="kc-badge kc-badge-store">
                <Lock :size="10" /> Mật khẩu ·
                {{ identity.secret_store === 'keyring' ? 'Keyring' : 'SQLite' }}
              </span>
              <span v-if="identity.key_id" class="kc-badge">
                <KeyRound :size="10" /> {{ keyById.get(identity.key_id)?.name ?? 'Khóa' }}
              </span>
              <span
                v-if="identity.secret_store === 'none' && !identity.key_id"
                class="kc-meta"
              >
                Không lưu mật khẩu, không có khóa: app sẽ hỏi mật khẩu khi kết nối
              </span>
            </div>
            <span class="kc-meta">
              {{ identity.used_by_hosts > 0 ? `Dùng bởi ${identity.used_by_hosts} máy chủ` : 'Chưa dùng' }}
            </span>
          </div>
          <div class="kc-actions">
            <button type="button" class="kc-action" title="Sửa" @click="openIdentity(identity)">
              <Pencil :size="13" />
            </button>
            <button
              type="button"
              class="kc-action kc-action-danger"
              title="Xóa identity"
              @click="deletingIdentity = identity"
            >
              <Trash2 :size="13" />
            </button>
          </div>
        </div>
      </div>
    </section>

    <SshKeyModal :visible="showKeyModal" @close="showKeyModal = false" />
    <SshIdentityModal
      :visible="showIdentityModal"
      :identity="editingIdentity"
      @close="showIdentityModal = false"
    />

    <PromptDialog
      :visible="renaming !== null"
      title="Đổi Tên Khóa"
      label="Tên khóa"
      :default-value="renaming?.name ?? ''"
      :max-length="100"
      confirm-text="Lưu"
      icon="edit"
      @confirm="confirmRename"
      @cancel="renaming = null"
    />

    <ConfirmDialog
      :visible="deletingKey !== null"
      title="Xóa Khóa Khỏi Keychain"
      :message="deleteKeyMessage"
      confirm-text="Xóa Khóa"
      danger
      @confirm="confirmDeleteKey"
      @cancel="deletingKey = null"
    />

    <ConfirmDialog
      :visible="deletingIdentity !== null"
      title="Xóa Identity"
      :message="deleteIdentityMessage"
      confirm-text="Xóa Identity"
      danger
      @confirm="confirmDeleteIdentity"
      @cancel="deletingIdentity = null"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import {
  AlertTriangle,
  Check,
  Copy,
  KeyRound,
  Lock,
  Pencil,
  Plus,
  ShieldCheck,
  Trash2,
  UserRound,
} from 'lucide-vue-next';
import type { SshIdentityView, SshKeyView } from '@/types/ssh';
import { useKeychain } from '@/composables/useKeychain';
import { useSshHosts } from '@/composables/useSshHosts';
import { ipcClient } from '@/ipc/client';
import SshKeyModal from './SshKeyModal.vue';
import SshIdentityModal from './SshIdentityModal.vue';
import PromptDialog from '@/components/dialogs/PromptDialog.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';

const { keys, identities, error, keyById, fetchKeychain, renameKey, deleteKey, deleteIdentity } =
  useKeychain();
const { fetchHosts } = useSshHosts();

const showKeyModal = ref(false);
const showIdentityModal = ref(false);
const editingIdentity = ref<SshIdentityView | null>(null);
const renaming = ref<SshKeyView | null>(null);
const deletingKey = ref<SshKeyView | null>(null);
const deletingIdentity = ref<SshIdentityView | null>(null);
const copiedKeyId = ref<string | null>(null);

const algorithmLabel = (algorithm: string) => {
  if (algorithm === 'ssh-ed25519') return 'ED25519';
  if (algorithm === 'ssh-rsa') return 'RSA';
  if (algorithm.startsWith('ecdsa-')) return 'ECDSA';
  return algorithm;
};

const keyUsageLabel = (key: SshKeyView) => {
  const parts: string[] = [];
  if (key.used_by_hosts > 0) parts.push(`${key.used_by_hosts} máy chủ`);
  if (key.used_by_identities > 0) parts.push(`${key.used_by_identities} identity`);
  return parts.length > 0 ? `Dùng bởi ${parts.join(' · ')}` : 'Chưa dùng';
};

const openIdentity = (identity: SshIdentityView | null) => {
  editingIdentity.value = identity;
  showIdentityModal.value = true;
};

const copyPublicKey = async (key: SshKeyView) => {
  try {
    await ipcClient.writeClipboardText(key.public_key);
    copiedKeyId.value = key.id;
    window.setTimeout(() => {
      if (copiedKeyId.value === key.id) copiedKeyId.value = null;
    }, 1500);
  } catch (e) {
    console.error('[KeychainPanel] copy failed:', e);
  }
};

const confirmRename = async (name: string) => {
  const key = renaming.value;
  renaming.value = null;
  if (!key || !name.trim() || name.trim() === key.name) return;
  try {
    await renameKey(key.id, name.trim());
  } catch (e) {
    console.error('[KeychainPanel] rename failed:', e);
  }
};

const deleteKeyMessage = computed(() => {
  const key = deletingKey.value;
  if (!key) return '';
  const usage =
    key.used_by_hosts + key.used_by_identities > 0
      ? ` Khóa đang được ${keyUsageLabel(key).replace('Dùng bởi ', '')} dùng; các mục đó sẽ không còn khóa này.`
      : '';
  return `Xóa khóa "${key.name}" (${key.fingerprint})?${usage} Không thể hoàn tác.`;
});

const deleteIdentityMessage = computed(() => {
  const identity = deletingIdentity.value;
  if (!identity) return '';
  const usage =
    identity.used_by_hosts > 0
      ? ` ${identity.used_by_hosts} máy chủ đang dùng identity này sẽ quay về tên đăng nhập và mật khẩu riêng của chúng.`
      : '';
  return `Xóa identity "${identity.name}" (${identity.username})?${usage}`;
});

const confirmDeleteKey = async () => {
  const key = deletingKey.value;
  deletingKey.value = null;
  if (!key) return;
  try {
    await deleteKey(key.id);
    await fetchHosts();
  } catch (e) {
    console.error('[KeychainPanel] delete key failed:', e);
  }
};

const confirmDeleteIdentity = async () => {
  const identity = deletingIdentity.value;
  deletingIdentity.value = null;
  if (!identity) return;
  try {
    await deleteIdentity(identity.id);
    await fetchHosts();
  } catch (e) {
    console.error('[KeychainPanel] delete identity failed:', e);
  }
};

onMounted(fetchKeychain);
</script>

<style scoped>
.keychain-panel {
  display: flex;
  flex-direction: column;
  gap: 20px;
  max-width: 920px;
}

.panel-error {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--status-failed);
}

.kc-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.kc-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-subtle);
}

.kc-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
}

.kc-count {
  font-size: 10.5px;
  padding: 1px 6px;
  border-radius: 8px;
  background-color: var(--bg-surface-hover);
  color: var(--text-muted);
}

.kc-empty {
  padding: 16px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-muted);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-md);
}

.kc-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.kc-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.kc-row:hover {
  border-color: var(--border-focus);
}

.kc-icon {
  width: 30px;
  height: 30px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  background-color: var(--primary-subtle);
  color: #e4b5ff;
}

.kc-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.kc-name-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.kc-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.kc-meta {
  font-size: 11px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kc-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 10.5px;
  padding: 1px 5px;
  border-radius: 3px;
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.kc-badge-store {
  background-color: var(--primary-subtle);
  border-color: var(--border-focus);
  color: #e4b5ff;
}

.kc-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.kc-action {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
}

.kc-action:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-subtle);
  color: var(--text-primary);
}

.kc-action-danger:hover {
  color: var(--status-failed);
}
</style>
