<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('cancel')">
    <div class="modal-content stop-modal">
      <div class="modal-header">
        <div class="modal-title">
          <Square class="text-danger" :size="16" />
          <span>Dừng Tiến Trình Lệnh (MOD-01)</span>
        </div>
        <button class="btn btn-ghost btn-icon" @click="$emit('cancel')">
          <X :size="15" />
        </button>
      </div>

      <div class="modal-body">
        <p class="modal-desc">
          Bạn có chắc chắn muốn dừng lệnh <strong>"{{ commandName }}"</strong> (PID: {{ pid || 'N/A' }})?
        </p>

        <div class="stop-options">
          <label class="stop-option-card" :class="{ selected: stopMode === 'graceful' }">
            <input v-model="stopMode" type="radio" value="graceful" />
            <div class="option-info">
              <span class="option-title">Dừng mềm (Graceful Stop - SIGTERM)</span>
              <span class="option-desc">Gửi tín hiệu dừng tiêu chuẩn, cho phép tiến trình dọn dẹp file và kết nối.</span>
            </div>
          </label>

          <label class="stop-option-card" :class="{ selected: stopMode === 'force' }">
            <input v-model="stopMode" type="radio" value="force" />
            <div class="option-info">
              <span class="option-title text-danger">Dừng cưỡng bức (Force Kill - SIGKILL)</span>
              <span class="option-desc">Tiêu diệt tiến trình ngay lập tức. Dùng khi tiến trình bị treo hoặc không phản hồi.</span>
            </div>
          </label>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" @click="$emit('cancel')">
          Hủy bỏ
        </button>
        <button
          class="btn btn-danger"
          @click="$emit('confirm', commandId, stopMode === 'force')"
        >
          <Square :size="13" />
          <span>{{ stopMode === 'force' ? 'Cưỡng Chế Dừng' : 'Dừng Tiến Trình' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { Square, X } from 'lucide-vue-next';

defineProps<{
  visible: boolean;
  commandId: number;
  commandName: string;
  pid?: number;
}>();

defineEmits<{
  (e: 'confirm', commandId: number, force: boolean): void;
  (e: 'cancel'): void;
}>();

const stopMode = ref<'graceful' | 'force'>('graceful');
</script>

<style scoped>
.stop-modal {
  max-width: 480px;
}

.text-danger {
  color: var(--status-failed);
}

.modal-desc {
  font-size: 13px;
  color: var(--text-secondary);
}

.stop-options {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 6px;
}

.stop-option-card {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
  background-color: var(--bg-app-base);
  cursor: pointer;
  transition: all 0.15s ease;
}

.stop-option-card:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-medium);
}

.stop-option-card.selected {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.option-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.option-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.option-desc {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.4;
}
</style>
