<template>
  <div v-if="visible && currentRequest" class="modal-backdrop">
    <div
      class="auth-prompt-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="auth-modal-title"
    >
      <div class="modal-accent-bar" />

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <Key :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="auth-modal-title" class="modal-title">
              Đăng Nhập Máy Chủ SSH
            </h3>
            <span class="modal-subtitle font-mono">{{ currentRequest.prompt.label }}</span>
          </div>
        </div>
        <button class="btn-close" title="Hủy bỏ (Esc)" @click="handleCancel">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <form class="modal-body" @submit.prevent="handleSubmit">
        <!-- Error if previous attempt failed -->
        <div v-if="currentRequest.prompt.error" class="error-banner" role="alert">
          <AlertCircle :size="14" class="error-icon" />
          <span>{{ currentRequest.prompt.error }}</span>
        </div>

        <!-- Server instructions (e.g., keyboard-interactive instructions or 2FA prompt) -->
        <div v-if="currentRequest.prompt.instructions" class="instructions-card">
          <Info :size="14" class="info-icon" />
          <span>{{ currentRequest.prompt.instructions }}</span>
        </div>

        <!-- Dynamic Prompt Fields -->
        <div class="fields-list">
          <div
            v-for="(p, idx) in currentRequest.prompt.prompts"
            :key="idx"
            class="form-group"
          >
            <label :for="`prompt-field-${idx}`" class="input-label">
              {{ p.text || (currentRequest.prompt.kind === 'password' ? 'Mật Khẩu:' : 'Mã Xác Thực:') }}
            </label>

            <div class="input-wrapper">
              <input
                :id="`prompt-field-${idx}`"
                :ref="(el) => { if (idx === 0) firstInputRef = el as HTMLInputElement; }"
                v-model="answers[idx]"
                :type="p.echo ? 'text' : (showPasswords[idx] ? 'text' : 'password')"
                class="form-input font-mono"
                autocomplete="off"
                spellcheck="false"
                required
              />
              <button
                v-if="!p.echo"
                type="button"
                class="btn-toggle-eye"
                title="Hiện/Ẩn ký tự"
                @click="showPasswords[idx] = !showPasswords[idx]"
              >
                <EyeOff v-if="showPasswords[idx]" :size="13" />
                <Eye v-else :size="13" />
              </button>
            </div>
          </div>
        </div>

        <!-- Optional Save Secret Option (only if storable) -->
        <div v-if="currentRequest.prompt.storable" class="save-secret-box">
          <label class="checkbox-row">
            <input v-model="rememberPassword" type="checkbox" class="form-checkbox" />
            <span class="checkbox-label">Lưu thông tin đăng nhập trên máy này</span>
          </label>

          <div v-if="rememberPassword" class="store-selector mt-2">
            <label class="input-label text-xs">Nơi Lưu Trữ:</label>
            <select v-model="secretStore" class="form-select text-xs">
              <option value="keyring">Keyring Hệ Điều Hành (Khuyến nghị)</option>
              <option value="sqlite">SQLite Cục Bộ (Mã hóa AES-256-GCM)</option>
            </select>
          </div>
        </div>

        <!-- Footer -->
        <div class="modal-footer">
          <button type="button" class="btn btn-ghost" @click="handleCancel">
            Hủy Bỏ (Esc)
          </button>
          <button
            type="submit"
            class="btn btn-primary btn-submit"
            :disabled="!canSubmit"
          >
            <LogIn :size="13" />
            <span>Đăng Nhập (Enter)</span>
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { Key, X, AlertCircle, Info, Eye, EyeOff, LogIn } from 'lucide-vue-next';
import type { SshAuthRequestEvent } from '@/ipc/events';
import type { SshSecretStore } from '@/types/ssh';
import { ipcClient } from '@/ipc/client';

const props = defineProps<{
  request?: SshAuthRequestEvent | null;
}>();

const emit = defineEmits<{
  (e: 'resolved'): void;
}>();

const visible = computed(() => Boolean(props.request));
const currentRequest = computed(() => props.request);

const answers = ref<string[]>([]);
const showPasswords = ref<boolean[]>([]);
const rememberPassword = ref(false);
const secretStore = ref<SshSecretStore>('keyring');
const firstInputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.request,
  (req) => {
    if (req) {
      answers.value = (req.prompt.prompts || []).map(() => '');
      showPasswords.value = (req.prompt.prompts || []).map(() => false);
      rememberPassword.value = req.prompt.store !== 'none';
      secretStore.value = req.prompt.store !== 'none' ? req.prompt.store : 'keyring';

      nextTick(() => {
        firstInputRef.value?.focus();
      });
    } else {
      // Clear values immediately for security
      answers.value = [];
    }
  },
  { immediate: true }
);

const canSubmit = computed(() => {
  return answers.value.length > 0 && answers.value.every((a) => a.trim().length > 0);
});

const handleCancel = async () => {
  if (!currentRequest.value) return;
  const reqId = currentRequest.value.request_id;
  try {
    await ipcClient.sshAuthRespond(reqId, []);
  } catch (e) {
    console.error('sshAuthRespond cancel error:', e);
  } finally {
    answers.value = [];
    emit('resolved');
  }
};

const handleSubmit = async () => {
  if (!canSubmit.value || !currentRequest.value) return;
  const reqId = currentRequest.value.request_id;
  const typedAnswers = [...answers.value];
  const storeToSave = rememberPassword.value ? secretStore.value : 'none';

  try {
    await ipcClient.sshAuthRespond(reqId, typedAnswers, storeToSave);
  } catch (e) {
    console.error('sshAuthRespond error:', e);
  } finally {
    answers.value = [];
    emit('resolved');
  }
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1060;
  background-color: rgba(11, 13, 19, 0.85);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.auth-prompt-modal {
  width: 100%;
  max-width: 480px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.7);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalScaleIn 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modalScaleIn {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.modal-accent-bar {
  height: 3px;
  width: 100%;
  background-color: var(--primary);
}

.modal-header {
  height: 48px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-sidebar);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.icon-badge {
  color: var(--primary);
  display: flex;
  align-items: center;
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-muted);
}

.btn-close {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
}

.btn-close:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.modal-body {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.error-banner {
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: var(--radius-md);
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--status-failed);
}

.instructions-card {
  background-color: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
  font-size: 12px;
  color: var(--text-secondary);
  display: flex;
  align-items: flex-start;
  gap: 8px;
  line-height: 1.4;
}

.info-icon {
  flex-shrink: 0;
  margin-top: 1px;
  color: var(--primary);
}

.fields-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.input-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.form-input {
  width: 100%;
  height: 32px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  padding: 0 32px 0 10px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.form-input:focus {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.btn-toggle-eye {
  position: absolute;
  right: 6px;
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 4px;
  border-radius: var(--radius-sm);
}

.btn-toggle-eye:hover {
  color: var(--text-primary);
}

.save-secret-box {
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 12px;
}

.checkbox-row {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.form-checkbox {
  accent-color: var(--primary);
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.checkbox-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
}

.store-selector {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-select {
  height: 28px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  padding: 0 8px;
  outline: none;
}

.form-select:focus {
  border-color: var(--border-focus);
}

.modal-footer {
  margin-top: 4px;
  padding-top: 12px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.btn-submit {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
