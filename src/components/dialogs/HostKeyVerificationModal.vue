<template>
  <div v-if="visible && currentRequest" class="modal-backdrop" @click.self="handleCancel">
    <div
      class="hostkey-modal"
      :class="{ 'is-danger': isKeyChanged }"
      role="dialog"
      aria-modal="true"
      aria-labelledby="hostkey-modal-title"
    >
      <!-- Top Accent Bar -->
      <div
        class="modal-accent-bar"
        :class="isKeyChanged ? 'bg-danger' : 'bg-warning'"
      />

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge" :class="isKeyChanged ? 'text-danger' : 'text-warning'">
            <ShieldAlert v-if="isKeyChanged" :size="18" />
            <ShieldCheck v-else :size="18" />
          </div>
          <div class="header-titles">
            <h3 id="hostkey-modal-title" class="modal-title">
              {{ isKeyChanged ? '⛔ Khóa Máy Chủ Đã Thay Đổi' : 'Xác Minh Máy Chủ SSH Mới' }}
            </h3>
            <span class="modal-subtitle font-mono">{{ currentRequest.prompt.label }}</span>
          </div>
        </div>
        <button class="btn-close" title="Hủy bỏ (Esc)" @click="handleCancel">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- Variant A: Key Changed (Strictly Blocked / Security Hazard) -->
        <template v-if="isKeyChanged">
          <div class="danger-banner">
            <AlertTriangle :size="16" class="banner-icon" />
            <div class="banner-text">
              <strong>CẢNH BÁO BẢO MẬT NGHIÊM TRỌNG!</strong>
              <span>Khóa của máy chủ KHÁC HOÀN TOÀN với khóa đã được lưu trước đó. Có thể quản trị viên vừa cài lại hệ điều hành, hoặc có kẻ tấn công trung gian (Man-in-the-Middle) đang giả mạo máy chủ.</span>
            </div>
          </div>

          <div class="fingerprint-box">
            <div class="fingerprint-row">
              <span class="fp-label">Khóa đã lưu:</span>
              <span class="fp-val text-muted font-mono">
                {{ currentRequest.prompt.algorithm }} {{ currentRequest.prompt.changed?.old_fingerprint || '(chưa rõ)' }}
              </span>
              <span v-if="currentRequest.prompt.changed" class="fp-source">
                ({{ currentRequest.prompt.changed.known_hosts_path }}:{{ currentRequest.prompt.changed.line }})
              </span>
            </div>
            <div class="fingerprint-row mt-2">
              <span class="fp-label">Khóa hiện tại:</span>
              <span class="fp-val text-danger font-mono font-bold">
                {{ currentRequest.prompt.algorithm }} {{ currentRequest.prompt.fingerprint }}
              </span>
            </div>
          </div>

          <div class="guidance-text">
            Kết nối đã bị hủy bỏ để đảm bảo an toàn. Nếu bạn chắc chắn server đã đổi khóa có chủ đích, hãy vào <strong>Máy chủ SSH → Sửa máy chủ → Cài đặt nâng cao → Quên host key đã lưu</strong> rồi thử kết nối lại.
          </div>
        </template>

        <!-- Variant B: New Host Key (TOFU verification) -->
        <template v-else>
          <div class="info-text">
            Đây là lần đầu tiên bạn kết nối tới máy chủ <strong>{{ currentRequest.prompt.label }}</strong>. Hãy đối chiếu dấu vân tay mật mã dưới đây với quản trị viên hệ thống để đảm bảo kết nối an toàn:
          </div>

          <div class="fingerprint-card">
            <div class="card-row">
              <span class="card-label">Loại thuật toán:</span>
              <span class="card-val font-mono">{{ currentRequest.prompt.algorithm }}</span>
            </div>
            <div class="card-row">
              <span class="card-label">Dấu vân tay SHA256:</span>
              <div class="fp-copy-wrapper">
                <code class="fp-code">{{ currentRequest.prompt.fingerprint }}</code>
                <button
                  type="button"
                  class="btn-copy-fp"
                  :title="copied ? 'Đã sao chép' : 'Sao chép fingerprint'"
                  @click="copyFingerprint"
                >
                  <Check v-if="copied" :size="12" class="text-success" />
                  <Copy v-else :size="12" />
                </button>
              </div>
            </div>
          </div>

          <label class="checkbox-row mt-2">
            <input v-model="rememberKey" type="checkbox" class="form-checkbox" />
            <span class="checkbox-label">Ghi nhớ khóa này vào danh sách máy chủ tin cậy (TOFU)</span>
          </label>
        </template>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <template v-if="isKeyChanged">
          <div class="footer-single">
            <button
              ref="cancelBtnRef"
              type="button"
              class="btn btn-secondary"
              @click="handleCancel"
            >
              Đóng Hộp Thoại (Esc)
            </button>
          </div>
        </template>
        <template v-else>
          <button
            ref="cancelBtnRef"
            type="button"
            class="btn btn-ghost"
            @click="handleCancel"
          >
            Hủy Kết Nối (Esc)
          </button>
          <button
            type="button"
            class="btn btn-primary btn-trust"
            @click="handleTrust"
          >
            <ShieldCheck :size="13" />
            <span>Tin Cậy & Kết Nối</span>
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import {
  ShieldAlert,
  ShieldCheck,
  X,
  AlertTriangle,
  Copy,
  Check,
} from 'lucide-vue-next';
import type { SshHostkeyRequestEvent } from '@/ipc/events';
import { ipcClient } from '@/ipc/client';

const props = defineProps<{
  request?: SshHostkeyRequestEvent | null;
}>();

const emit = defineEmits<{
  (e: 'resolved'): void;
}>();

const visible = computed(() => Boolean(props.request));
const currentRequest = computed(() => props.request);

const isKeyChanged = computed(() => {
  return Boolean(currentRequest.value?.prompt.changed);
});

const rememberKey = ref(true);
const copied = ref(false);
const cancelBtnRef = ref<HTMLButtonElement | null>(null);

watch(
  () => props.request,
  (req) => {
    if (req) {
      rememberKey.value = true;
      copied.value = false;
      nextTick(() => {
        // Focus the cancel button by default so accidental Enter doesn't trust a rogue key
        cancelBtnRef.value?.focus();
      });
    }
  }
);

const copyFingerprint = async () => {
  if (!currentRequest.value) return;
  try {
    await navigator.clipboard.writeText(currentRequest.value.prompt.fingerprint);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch (e) {
    console.error('Không thể copy fingerprint:', e);
  }
};

const handleCancel = async () => {
  if (!currentRequest.value) return;
  const reqId = currentRequest.value.request_id;
  try {
    await ipcClient.sshHostkeyRespond(reqId, false, false);
  } catch (e) {
    console.error('sshHostkeyRespond error:', e);
  } finally {
    emit('resolved');
  }
};

const handleTrust = async () => {
  if (!currentRequest.value) return;
  const reqId = currentRequest.value.request_id;
  try {
    await ipcClient.sshHostkeyRespond(reqId, true, rememberKey.value);
  } catch (e) {
    console.error('sshHostkeyRespond error:', e);
  } finally {
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

.hostkey-modal {
  width: 100%;
  max-width: 520px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.7);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalScaleIn 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

.hostkey-modal.is-danger {
  border-color: rgba(239, 68, 68, 0.4);
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
}

.bg-warning {
  background-color: var(--status-warning, #f59e0b);
}

.bg-danger {
  background-color: var(--status-failed, #ef4444);
}

.text-warning {
  color: var(--status-warning, #f59e0b);
}

.text-danger {
  color: var(--status-failed, #ef4444);
}

.text-success {
  color: var(--status-running, #10b981);
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

.info-text {
  font-size: 12.5px;
  color: var(--text-secondary);
  line-height: 1.5;
}

.danger-banner {
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  color: #ffb4ab;
}

.banner-text {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  line-height: 1.4;
}

.fingerprint-box {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  flex-direction: column;
}

.fingerprint-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 11.5px;
}

.fp-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.fp-val {
  word-break: break-all;
}

.fp-source {
  font-size: 10.5px;
  color: var(--text-muted);
}

.fingerprint-card {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.card-row {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.card-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
}

.card-val {
  font-size: 12px;
  color: var(--text-primary);
}

.fp-copy-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: var(--radius-sm);
  padding: 6px 10px;
  gap: 8px;
}

.fp-code {
  font-size: 11.5px;
  color: var(--primary);
  font-family: var(--font-mono);
  word-break: break-all;
  user-select: all;
}

.btn-copy-fp {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.btn-copy-fp:hover {
  color: var(--text-primary);
  background-color: rgba(255, 255, 255, 0.08);
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
  color: var(--text-secondary);
}

.guidance-text {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.45;
}

.modal-footer {
  height: 48px;
  padding: 0 16px;
  background-color: var(--bg-sidebar);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.footer-single {
  width: 100%;
  display: flex;
  justify-content: flex-end;
}

.btn-trust {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
