<template>
  <div v-if="visible" class="modal-backdrop" @click.self="close" @keydown.esc="close">
    <div class="keychain-modal" role="dialog" aria-modal="true" aria-labelledby="ssh-key-title">
      <div class="modal-accent-bar" style="background-color: #744791" />

      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge" style="color: #e4b5ff">
            <KeyRound :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="ssh-key-title" class="modal-title">
              {{ created ? 'Đã Thêm Khóa Vào Keychain' : 'Thêm Khóa Vào Keychain' }}
            </h3>
            <span class="modal-subtitle">Khóa dùng chung cho nhiều máy chủ và identity</span>
          </div>
        </div>
        <button class="btn-close" title="Đóng (Esc)" @click="close">
          <X :size="16" />
        </button>
      </div>

      <!-- After saving: show the public key to put on servers -->
      <div v-if="created" class="modal-body">
        <div class="alert-banner alert-success" role="status">
          <CheckCircle2 :size="14" class="alert-icon" />
          <span>
            Đã lưu khóa <strong>{{ created.name }}</strong> ({{ created.algorithm }}).
            Thêm public key dưới đây vào <code>~/.ssh/authorized_keys</code> trên máy chủ.
          </span>
        </div>
        <div class="public-key-box">
          <span class="public-key-text">{{ created.public_key }}</span>
          <span class="field-hint font-mono">{{ created.fingerprint }}</span>
        </div>
      </div>

      <div v-else class="modal-body custom-scrollbar">
        <div v-if="formError" class="alert-banner alert-error" role="alert">
          <AlertCircle :size="14" class="alert-icon" />
          <span>{{ formError }}</span>
        </div>

        <div class="auth-method-tabs">
          <button
            type="button"
            class="method-tab"
            :class="{ active: mode === 'import' }"
            @click="mode = 'import'"
          >
            Dán / Nhập Từ Tệp
          </button>
          <button
            type="button"
            class="method-tab"
            :class="{ active: mode === 'generate' }"
            @click="mode = 'generate'"
          >
            Sinh Khóa Mới (ed25519)
          </button>
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-key-name">
            Tên Khóa <span class="required-star">*</span>
          </label>
          <input
            id="ssh-key-name"
            ref="nameInput"
            v-model="name"
            type="text"
            class="form-input"
            maxlength="100"
            placeholder="Ví dụ: Laptop công ty, CI deploy..."
          />
        </div>

        <template v-if="mode === 'import'">
          <div class="form-group">
            <div class="label-row">
              <label class="input-label" for="ssh-key-text">
                Nội Dung Khóa Riêng <span class="required-star">*</span>
              </label>
              <button type="button" class="btn btn-ghost btn-sm" @click="pickFile">
                <FolderOpen :size="12" />
                <span>Chọn tệp…</span>
              </button>
              <input
                ref="fileInput"
                type="file"
                class="hidden-file-input"
                @change="onFilePicked"
              />
            </div>
            <textarea
              id="ssh-key-text"
              v-model="privateKey"
              class="form-input form-textarea font-mono"
              rows="7"
              spellcheck="false"
              autocomplete="off"
              placeholder="-----BEGIN OPENSSH PRIVATE KEY-----&#10;...&#10;-----END OPENSSH PRIVATE KEY-----"
            />
            <span class="field-hint">
              Nhận khóa OpenSSH, PEM (RSA / ECDSA / PKCS#8) và PuTTY <code>.ppk</code>.
            </span>
          </div>

          <div class="form-group">
            <label class="input-label" for="ssh-key-passphrase">Passphrase (nếu khóa có)</label>
            <div class="password-input-wrapper">
              <input
                id="ssh-key-passphrase"
                v-model="passphrase"
                :type="showPassphrase ? 'text' : 'password'"
                class="form-input font-mono"
                autocomplete="new-password"
                placeholder="Để trống nếu khóa không có passphrase"
              />
              <button
                type="button"
                class="btn-toggle-eye"
                title="Hiện/Ẩn passphrase"
                @click="showPassphrase = !showPassphrase"
              >
                <EyeOff v-if="showPassphrase" :size="13" />
                <Eye v-else :size="13" />
              </button>
            </div>
            <span class="field-hint">
              Nhập ở đây thì app kiểm tra ngay và lưu cùng khóa; để trống thì app sẽ hỏi ở lần kết nối đầu tiên.
            </span>
          </div>
        </template>

        <div v-else class="field-hint">
          App sinh một khóa ed25519 mới. Sau khi lưu, chép public key vào
          <code>~/.ssh/authorized_keys</code> của máy chủ.
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-key-store">Nơi Lưu Khóa</label>
          <select id="ssh-key-store" v-model="store" class="form-select">
            <option value="keyring" :disabled="!keyringAvailable">
              Keyring Hệ Điều Hành{{ keyringAvailable ? ' (Khuyến nghị)' : ' (không có trên máy này)' }}
            </option>
            <option value="sqlite">SQLite Cục Bộ (Mã hóa AES-256-GCM)</option>
          </select>
          <span v-if="store === 'sqlite'" class="field-hint">
            Mã hóa bằng khóa riêng của máy này, không theo bản sao lưu sang máy khác.
          </span>
        </div>
      </div>

      <div class="modal-footer">
        <template v-if="created">
          <button type="button" class="btn btn-secondary" @click="copyPublicKey">
            <Check v-if="copied" :size="13" />
            <Copy v-else :size="13" />
            <span>{{ copied ? 'Đã Chép' : 'Chép Public Key' }}</span>
          </button>
          <div class="footer-right">
            <button type="button" class="btn btn-primary" @click="close">Xong</button>
          </div>
        </template>
        <template v-else>
          <span />
          <div class="footer-right">
            <button type="button" class="btn btn-ghost" @click="close">Hủy Bỏ</button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="saving || !canSave"
              @click="save"
            >
              <LoaderCircle v-if="saving" :size="13" class="spin" />
              <Save v-else :size="13" />
              <span>{{ mode === 'generate' ? 'Sinh & Lưu Khóa' : 'Lưu Khóa' }}</span>
            </button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import {
  AlertCircle,
  Check,
  CheckCircle2,
  Copy,
  Eye,
  EyeOff,
  FolderOpen,
  KeyRound,
  LoaderCircle,
  Save,
  X,
} from 'lucide-vue-next';
import type { SshKey, SshSecretStore } from '@/types/ssh';
import { useKeychain } from '@/composables/useKeychain';
import { ipcClient } from '@/ipc/client';

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved', key: SshKey): void;
}>();

const { importKey, generateKey } = useKeychain();

const mode = ref<'import' | 'generate'>('import');
const name = ref('');
const privateKey = ref('');
const passphrase = ref('');
const showPassphrase = ref(false);
const store = ref<SshSecretStore>('keyring');
const keyringAvailable = ref(true);
const saving = ref(false);
const formError = ref<string | null>(null);
const created = ref<SshKey | null>(null);
const copied = ref(false);
const nameInput = ref<HTMLInputElement | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);

const canSave = computed(
  () => name.value.trim().length > 0 && (mode.value === 'generate' || privateKey.value.trim().length > 0)
);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    mode.value = 'import';
    name.value = '';
    privateKey.value = '';
    passphrase.value = '';
    showPassphrase.value = false;
    formError.value = null;
    created.value = null;
    copied.value = false;
    saving.value = false;
    void ipcClient
      .sshSecretStatus()
      .then(({ keyring_available }) => {
        keyringAvailable.value = keyring_available;
        store.value = keyring_available ? 'keyring' : 'sqlite';
      })
      .catch(() => {
        keyringAvailable.value = false;
        store.value = 'sqlite';
      });
    void nextTick(() => nameInput.value?.focus());
  },
  { immediate: true }
);

const close = () => {
  // Secrets typed into the form do not outlive it.
  privateKey.value = '';
  passphrase.value = '';
  emit('close');
};

const pickFile = () => fileInput.value?.click();

const onFilePicked = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = '';
  if (!file) return;
  if (file.size > 64 * 1024) {
    formError.value = 'Tệp quá lớn để là một khóa riêng.';
    return;
  }
  privateKey.value = await file.text();
  if (!name.value.trim()) name.value = file.name.replace(/\.(pem|ppk|key)$/i, '');
};

const save = async () => {
  if (!canSave.value) return;
  saving.value = true;
  formError.value = null;
  try {
    const key =
      mode.value === 'generate'
        ? await generateKey(name.value.trim(), store.value)
        : await importKey(
            name.value.trim(),
            privateKey.value,
            passphrase.value || undefined,
            store.value
          );
    privateKey.value = '';
    passphrase.value = '';
    created.value = key;
    emit('saved', key);
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
};

const copyPublicKey = async () => {
  if (!created.value) return;
  try {
    await ipcClient.writeClipboardText(created.value.public_key);
    copied.value = true;
    window.setTimeout(() => (copied.value = false), 1500);
  } catch (e) {
    console.error('[SshKeyModal] copy failed:', e);
  }
};
</script>

<style scoped src="./keychain-modal.css"></style>

<style scoped>
.keychain-modal {
  max-width: 540px;
}

.label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.hidden-file-input {
  display: none;
}

.required-star {
  color: var(--status-failed);
}
</style>
