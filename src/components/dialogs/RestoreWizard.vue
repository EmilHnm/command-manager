<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('close')">
    <div class="modal-content wizard-modal">
      <div class="modal-header">
        <div class="modal-title">
          <ShieldAlert class="text-warning" :size="18" />
          <span>Khôi Phục Dữ Liệu SQLite & Rollback (MOD-08)</span>
        </div>
        <button class="btn btn-ghost btn-icon" @click="$emit('close')">
          <X :size="15" />
        </button>
      </div>

      <div class="modal-body">
        <div class="file-info-box">
          <span class="file-label">Tệp sao lưu được chọn:</span>
          <code class="file-path">{{ filePath || 'cm_backup_2026-09-25_160821.sqlite' }}</code>
        </div>

        <!-- Integrity Check Results -->
        <div class="check-results-card">
          <div class="card-title">Kết quả kiểm tra tính toàn vẹn (Pre-flight Check):</div>
          <ul class="checks-list">
            <li class="check-item ok">
              <CheckCircle2 :size="14" />
              <span>Cấu trúc SQLite hợp lệ (PRAGMA integrity_check: {{ backupInfo?.integrityOk ? 'ok' : 'chưa kiểm tra' }})</span>
            </li>
            <li class="check-item ok">
              <CheckCircle2 :size="14" />
              <span>Phiên bản lược đồ đang dùng: v{{ backupInfo?.schemaVersion || '—' }}</span>
            </li>
            <li class="check-item ok">
              <CheckCircle2 :size="14" />
              <span>Dữ liệu hiện tại: {{ backupInfo?.commandCount ?? '—' }} commands, {{ backupInfo?.groupCount ?? '—' }} groups, {{ backupInfo?.historyCount ?? '—' }} history records</span>
            </li>
          </ul>
        </div>

        <!-- Safety Pipeline Explanation -->
        <div class="pipeline-card">
          <div class="card-title">Quy trình an toàn 7 bước được thực hiện:</div>
          <ol class="pipeline-steps">
            <li>Tạo bản snapshot dự phòng <code>app.db.bak</code> của dữ liệu hiện tại</li>
            <li>Dừng an toàn mọi tiến trình & phiên chạy đang active</li>
            <li>Đóng toàn bộ connection pool, dọn sạch <code>app.db-wal</code> và <code>app.db-shm</code></li>
            <li>Thay thế tệp SQLite mới vào vị trí và mở lại kết nối</li>
            <li><strong>Tự động Rollback:</strong> Hoàn trả bản snapshot cũ nếu có bất kỳ lỗi nào xảy ra</li>
          </ol>
        </div>

        <!-- User Confirmation Checkbox -->
        <label class="confirm-checkbox-row">
          <input v-model="userConfirmed" type="checkbox" />
          <span class="confirm-text">
            Tôi xác nhận rằng tệp sao lưu này đến từ <strong>nguồn tin cậy (Trusted Zone)</strong> và đồng ý ghi đè toàn bộ dữ liệu hiện tại.
          </span>
        </label>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" @click="$emit('close')">
          Hủy bỏ
        </button>
        <button
          class="btn btn-danger"
          :disabled="!userConfirmed || isRestoring"
          @click="handleRestore"
        >
          <RotateCcw v-if="!isRestoring" :size="14" />
          <span>{{ isRestoring ? 'Đang khôi phục...' : 'Bắt Đầu Khôi Phục & Tải Lại' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { ShieldAlert, CheckCircle2, RotateCcw, X } from 'lucide-vue-next';
import type { BackupIntegrityResult } from '@/types/models';

const props = defineProps<{
  visible: boolean;
  filePath?: string;
  backupInfo?: BackupIntegrityResult | null;
}>();

const emit = defineEmits<{
  (e: 'confirm-restore'): void;
  (e: 'close'): void;
}>();

const userConfirmed = ref(false);
const isRestoring = ref(false);

watch(
  () => props.visible,
  () => {
    userConfirmed.value = false;
    isRestoring.value = false;
  },
  { immediate: true }
);

const handleRestore = () => {
  if (!userConfirmed.value) return;
  isRestoring.value = true;
  emit('confirm-restore');
};
</script>

<style scoped>
.wizard-modal {
  max-width: 580px;
}

.text-warning {
  color: var(--status-warning);
}

.file-info-box {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.file-label {
  font-size: 11px;
  color: var(--text-muted);
}

.file-path {
  font-size: 11.5px;
  font-family: var(--font-mono);
  color: #cda8ee;
  word-break: break-all;
}

.check-results-card, .pipeline-card {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.card-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.checks-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.check-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.check-item.ok {
  color: var(--status-running);
}

.pipeline-steps {
  padding-left: 20px;
  font-size: 11.5px;
  color: var(--text-secondary);
  line-height: 1.6;
}

.pipeline-steps code {
  font-family: var(--font-mono);
  color: var(--text-primary);
  background: var(--bg-surface);
  padding: 1px 4px;
  border-radius: 2px;
}

.confirm-checkbox-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  cursor: pointer;
  padding: 8px 10px;
  background-color: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: var(--radius-md);
}

.confirm-checkbox-row input {
  margin-top: 3px;
}

.confirm-text {
  font-size: 12px;
  color: var(--text-primary);
  line-height: 1.5;
}
</style>
