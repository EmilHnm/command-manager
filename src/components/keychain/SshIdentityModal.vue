<template>
  <div v-if="visible" class="modal-backdrop" @click.self="close" @keydown.esc="close">
    <div class="keychain-modal" role="dialog" aria-modal="true" aria-labelledby="ssh-identity-title">
      <div class="modal-accent-bar" style="background-color: #744791" />

      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge" style="color: #e4b5ff">
            <UserRound :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="ssh-identity-title" class="modal-title">
              {{ identity ? `Sửa Identity: ${identity.name}` : 'Thêm Identity' }}
            </h3>
            <span class="modal-subtitle">Tên đăng nhập + mật khẩu và/hoặc khóa, dùng lại cho nhiều máy chủ</span>
          </div>
        </div>
        <button class="btn-close" title="Đóng (Esc)" @click="close">
          <X :size="16" />
        </button>
      </div>

      <div class="modal-body custom-scrollbar">
        <div v-if="formError" class="alert-banner alert-error" role="alert">
          <AlertCircle :size="14" class="alert-icon" />
          <span>{{ formError }}</span>
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-identity-name">Tên Gợi Nhớ</label>
          <input
            id="ssh-identity-name"
            v-model="name"
            type="text"
            class="form-input"
            maxlength="100"
            placeholder="Để trống sẽ dùng tên đăng nhập"
          />
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-identity-username">
            Tên Đăng Nhập <span class="required-star">*</span>
          </label>
          <input
            id="ssh-identity-username"
            ref="usernameInput"
            v-model="username"
            type="text"
            class="form-input font-mono"
            placeholder="root, ubuntu, deploy..."
          />
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-identity-password">
            Mật Khẩu
            <span v-if="hasSavedPassword" class="text-muted font-normal">
              (Để trống nếu giữ nguyên mật khẩu đã lưu)
            </span>
          </label>
          <div class="password-input-wrapper">
            <input
              id="ssh-identity-password"
              v-model="password"
              :type="showPassword ? 'text' : 'password'"
              class="form-input font-mono"
              autocomplete="new-password"
              :placeholder="hasSavedPassword ? '•••••••• (đã lưu)' : 'Để trống nếu chỉ dùng khóa'"
              :disabled="store === 'none'"
            />
            <button
              type="button"
              class="btn-toggle-eye"
              title="Hiện/Ẩn mật khẩu"
              @click="showPassword = !showPassword"
            >
              <EyeOff v-if="showPassword" :size="13" />
              <Eye v-else :size="13" />
            </button>
          </div>
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-identity-store">Lưu Mật Khẩu</label>
          <select id="ssh-identity-store" v-model="store" class="form-select">
            <option value="none">Không lưu (hỏi mỗi lần kết nối)</option>
            <option value="keyring" :disabled="!keyringAvailable">
              Keyring Hệ Điều Hành{{ keyringAvailable ? ' (Khuyến nghị)' : ' (không có trên máy này)' }}
            </option>
            <option value="sqlite">SQLite Cục Bộ (Mã hóa AES-256-GCM)</option>
          </select>
          <span v-if="store === 'none' && hasSavedPassword" class="field-hint">
            Mật khẩu đã lưu sẽ bị xóa khi bấm Lưu.
          </span>
        </div>

        <div class="form-group">
          <label class="input-label" for="ssh-identity-key">Khóa</label>
          <select id="ssh-identity-key" v-model="keyId" class="form-select">
            <option :value="null">Không dùng khóa</option>
            <option v-for="key in keys" :key="key.id" :value="key.id">
              {{ key.name }} ({{ key.algorithm }})
            </option>
          </select>
          <span v-if="keys.length === 0" class="field-hint">
            Keychain chưa có khóa nào. Thêm khóa ở mục Khóa của Keychain.
          </span>
        </div>
      </div>

      <div class="modal-footer">
        <span />
        <div class="footer-right">
          <button type="button" class="btn btn-ghost" @click="close">Hủy Bỏ</button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="saving || !username.trim()"
            @click="save"
          >
            <LoaderCircle v-if="saving" :size="13" class="spin" />
            <Save v-else :size="13" />
            <span>{{ identity ? 'Lưu Thay Đổi' : 'Thêm Identity' }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { AlertCircle, Eye, EyeOff, LoaderCircle, Save, UserRound, X } from 'lucide-vue-next';
import type { SecretChange, SshIdentity, SshIdentityView, SshSecretStore } from '@/types/ssh';
import { useKeychain } from '@/composables/useKeychain';
import { ipcClient } from '@/ipc/client';

const props = defineProps<{
  visible: boolean;
  identity?: SshIdentityView | null;
}>();
const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved', identity: SshIdentity): void;
}>();

const { keys, saveIdentity } = useKeychain();

const name = ref('');
const username = ref('');
const password = ref('');
const showPassword = ref(false);
const store = ref<SshSecretStore>('none');
const keyId = ref<string | null>(null);
const keyringAvailable = ref(true);
const saving = ref(false);
const formError = ref<string | null>(null);
const usernameInput = ref<HTMLInputElement | null>(null);

const hasSavedPassword = computed(() => (props.identity?.secret_store ?? 'none') !== 'none');

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    const identity = props.identity;
    name.value = identity?.name ?? '';
    username.value = identity?.username ?? '';
    keyId.value = identity?.key_id ?? null;
    store.value = identity?.secret_store ?? 'none';
    password.value = '';
    showPassword.value = false;
    formError.value = null;
    saving.value = false;
    void ipcClient
      .sshSecretStatus()
      .then(({ keyring_available }) => (keyringAvailable.value = keyring_available))
      .catch(() => (keyringAvailable.value = false));
    void nextTick(() => usernameInput.value?.focus());
  },
  { immediate: true }
);

const close = () => {
  password.value = '';
  emit('close');
};

const save = async () => {
  if (!username.value.trim()) return;
  saving.value = true;
  formError.value = null;
  try {
    const change: SecretChange | undefined = password.value
      ? { action: 'set', value: password.value }
      : { action: 'keep' };
    const saved = await saveIdentity(
      props.identity?.id ?? null,
      { name: name.value, username: username.value, key_id: keyId.value },
      change,
      store.value
    );
    password.value = '';
    emit('saved', saved);
    emit('close');
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
};
</script>

<style scoped src="./keychain-modal.css"></style>

<style scoped>
.keychain-modal {
  max-width: 500px;
}

.required-star {
  color: var(--status-failed);
}

.form-input:disabled {
  opacity: 0.5;
}
</style>
