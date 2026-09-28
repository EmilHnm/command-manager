<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('cancel')">
    <div class="confirm-modal-container" :class="{ 'is-danger': danger }">
      <!-- Hazard Ribbon for Destructive Actions -->
      <div v-if="danger" class="hazard-stripe"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge" :class="danger ? 'danger-badge' : 'warning-badge'">
            <ShieldAlert v-if="danger" :size="18" />
            <AlertTriangle v-else :size="18" />
          </div>
          <div>
            <div class="modal-title">{{ title }}</div>
            <div class="modal-subtitle">{{ subtitle }}</div>
          </div>
        </div>
        <button class="close-btn" title="Hủy bỏ (Esc)" @click="$emit('cancel')">
          <X :size="18" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- Message -->
        <p class="confirm-message">{{ message }}</p>

        <!-- Irreversible Notice for Destructive -->
        <div v-if="danger" class="irreversible-box">
          <AlertOctagon :size="15" class="alert-icon" />
          <div class="box-content">
            <strong class="alert-heading">⚠️ HÀNH ĐỘNG NÀY KHÔNG THỂ HỒI PHỤC!</strong>
            <p class="alert-desc">
              Dữ liệu sẽ bị xóa vĩnh viễn khỏi cơ sở dữ liệu SQLite cục bộ.
            </p>
          </div>
        </div>

        <!-- Privileged Warning Notice -->
        <div v-if="privilegedNotice" class="privileged-box">
          <Lock :size="15" class="lock-icon" />
          <div class="box-text">
            <strong>Khu Vực Đặc Quyền (Trusted Zone):</strong>
            Hành động này tác động trực tiếp tới tiến trình hệ thống hoặc cơ sở dữ liệu ứng dụng.
          </div>
        </div>

        <!-- Commitment Checkbox -->
        <div v-if="requireCheckbox" class="commitment-check-wrap">
          <label class="checkbox-label">
            <input v-model="confirmedCommitment" type="checkbox" class="commitment-checkbox" />
            <span>Tôi đã đọc kỹ cảnh báo và xác nhận thực hiện thao tác này.</span>
          </label>
        </div>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <div class="keyboard-hints">
          <span>⌨️</span>
          <span>Esc: Hủy</span>
          <span class="hint-divider">|</span>
          <span>Enter: Xác nhận</span>
        </div>
        <div class="footer-actions">
          <button class="btn-ghost" @click="$emit('cancel')">
            Hủy Bỏ (Esc)
          </button>
          <button
            class="btn-action"
            :class="danger ? 'btn-danger' : 'btn-primary'"
            :disabled="requireCheckbox && !confirmedCommitment"
            @click="handleConfirm"
          >
            <Trash2 v-if="danger" :size="15" />
            <Check v-else :size="15" />
            <span>{{ confirmText }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { ShieldAlert, AlertTriangle, AlertOctagon, Lock, X, Trash2, Check } from 'lucide-vue-next';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    title?: string;
    subtitle?: string;
    message: string;
    confirmText?: string;
    danger?: boolean;
    privilegedNotice?: boolean;
    requireCheckbox?: boolean;
  }>(),
  {
    title: 'Xác Nhận Thao Tác',
    subtitle: 'MOD-13 • Security Authorization Gate',
    confirmText: 'Xác Nhận',
    danger: false,
    privilegedNotice: false,
    requireCheckbox: false,
  }
);

const emit = defineEmits<{
  (e: 'confirm'): void;
  (e: 'cancel'): void;
}>();

const confirmedCommitment = ref(false);

watch(
  () => props.visible,
  (val) => {
    if (val) {
      confirmedCommitment.value = false;
    }
  }
);

const handleConfirm = () => {
  if (props.requireCheckbox && !confirmedCommitment.value) return;
  emit('confirm');
};

const handleGlobalKeydown = (event: KeyboardEvent) => {
  if (!props.visible) return;
  if (event.key === 'Escape') {
    emit('cancel');
  } else if (event.key === 'Enter') {
    handleConfirm();
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
});
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(11, 13, 19, 0.8);
  backdrop-filter: blur(6px);
  z-index: 120;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.confirm-modal-container {
  width: 100%;
  max-width: 520px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 30px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

.confirm-modal-container.is-danger {
  box-shadow: 0 20px 30px rgba(0, 0, 0, 0.7), 0 0 20px rgba(239, 68, 68, 0.25);
  border-color: rgba(239, 68, 68, 0.4);
}

.hazard-stripe {
  height: 4px;
  background: linear-gradient(90deg, #ef4444 0%, #f59e0b 50%, #ef4444 100%);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 34px;
  height: 34px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
}

.icon-badge.danger-badge {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

.icon-badge.warning-badge {
  background-color: rgba(245, 158, 11, 0.2);
  color: #f59e0b;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.modal-subtitle {
  font-size: 10px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  margin-top: 1px;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
}

.close-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.modal-body {
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.confirm-message {
  font-size: 13px;
  color: var(--text-primary);
  line-height: 1.6;

}

.irreversible-box {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: var(--radius-md);
  padding: 10px 12px;
}

.alert-icon {
  color: #ef4444;
  margin-top: 2px;
  flex-shrink: 0;
}

.alert-heading {
  display: block;
  font-size: 11px;
  color: #ef4444;
  letter-spacing: 0.3px;
}

.alert-desc {
  font-size: 11px;
  color: #fca5a5;
  margin-top: 2px;

}

.privileged-box {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background-color: rgba(116, 71, 145, 0.15);
  border: 1px solid rgba(116, 71, 145, 0.35);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  font-size: 12px;
  color: #e4b5ff;
  line-height: 1.5;
}

.lock-icon {
  margin-top: 2px;
  flex-shrink: 0;
  color: #e4b5ff;
}

.commitment-check-wrap {
  background-color: #141721;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 8px 10px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
  color: var(--text-secondary);
  cursor: pointer;
}

.commitment-checkbox {
  accent-color: #ef4444;
  cursor: pointer;
}

.modal-footer {
  padding: 12px 18px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.keyboard-hints {
  display: flex;
  align-items: center;
  gap: 4px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.hint-divider {
  margin: 0 2px;
  color: var(--border-subtle);
}

.footer-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-action {
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  border: none;
}

.btn-danger {
  background-color: #ef4444;
  color: #ffffff;
  box-shadow: 0 0 10px rgba(239, 68, 68, 0.4);
}

.btn-danger:hover:not(:disabled) {
  background-color: #dc2626;
}

.btn-danger:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  box-shadow: none;
}

.btn-primary {
  background-color: var(--primary);
  color: #ffffff;
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-hover);
}

.btn-ghost {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.btn-ghost:hover {
  color: var(--text-primary);
}
</style>
