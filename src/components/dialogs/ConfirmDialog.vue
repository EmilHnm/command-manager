<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('cancel')">
    <div class="modal-content confirm-modal">
      <div class="modal-header">
        <div class="modal-title">
          <ShieldAlert v-if="danger" class="text-danger" :size="18" />
          <AlertTriangle v-else class="text-warning" :size="18" />
          <span>{{ title }}</span>
        </div>
        <button class="btn btn-ghost btn-icon" @click="$emit('cancel')">
          <X :size="15" />
        </button>
      </div>

      <div class="modal-body">
        <p class="confirm-message">{{ message }}</p>

        <!-- Privileged Warning Box -->
        <div v-if="privilegedNotice" class="privileged-box">
          <Lock :size="14" class="lock-icon" />
          <div class="box-text">
            <strong>Khu vực Đặc quyền / Vùng Tin cậy:</strong>
            Mọi lệnh lưu trong hệ thống đều được ứng dụng tin cậy để thực thi trực tiếp trên máy chủ / desktop.
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" @click="$emit('cancel')">
          Hủy bỏ
        </button>
        <button
          class="btn"
          :class="danger ? 'btn-danger' : 'btn-primary'"
          @click="$emit('confirm')"
        >
          {{ confirmText }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ShieldAlert, AlertTriangle, Lock, X } from 'lucide-vue-next';

withDefaults(
  defineProps<{
    visible: boolean;
    title?: string;
    message: string;
    confirmText?: string;
    danger?: boolean;
    privilegedNotice?: boolean;
  }>(),
  {
    title: 'Xác nhận thao tác',
    confirmText: 'Xác nhận',
    danger: false,
    privilegedNotice: false,
  }
);

defineEmits<{
  (e: 'confirm'): void;
  (e: 'cancel'): void;
}>();
</script>

<style scoped>
.confirm-modal {
  max-width: 460px;
}

.text-danger {
  color: var(--status-failed);
}

.text-warning {
  color: var(--status-warning);
}

.confirm-message {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.6;
}

.privileged-box {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background-color: rgba(116, 71, 145, 0.12);
  border: 1px solid rgba(116, 71, 145, 0.3);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  font-size: 12px;
  color: #cda8ee;
  line-height: 1.5;
}

.lock-icon {
  margin-top: 2px;
  flex-shrink: 0;
  color: #cda8ee;
}
</style>
