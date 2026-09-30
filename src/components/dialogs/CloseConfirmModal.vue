<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('cancel')">
    <div
      class="close-modal-card"
      :class="{ 'has-running': runningList.length > 0 }"
      role="dialog"
      aria-modal="true"
      aria-labelledby="close-modal-title"
    >
      <!-- Top Accent Bar -->
      <div class="card-accent-bar" :class="{ warning: runningList.length > 0 }"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="badge-icon" :class="{ warning: runningList.length > 0 }">
            <AlertTriangle v-if="runningList.length > 0" :size="20" />
            <LogOut v-else :size="20" />
          </div>
          <div>
            <h3 id="close-modal-title" class="modal-title">Xác Nhận Đóng Ứng Dụng</h3>
            <p class="modal-subtitle">Command Manager Desktop</p>
          </div>
        </div>
        <button class="close-icon-btn" :disabled="loading" title="Hủy bỏ (Esc)" @click="!loading && $emit('cancel')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- If there are running processes -->
        <div v-if="runningList.length > 0" class="running-warning-banner">
          <div class="warning-title">
            <AlertCircle :size="16" />
            <span>Có <strong>{{ runningList.length }}</strong> tiến trình đang chạy trong nền</span>
          </div>
          <p class="warning-desc">
            Nếu bạn thoát, tất cả tiến trình đang thực thi sẽ bị dừng lại an toàn (Graceful Shutdown).
            Bạn có thể chọn <strong>"Ẩn Xuống Khay"</strong> để các tiến trình tiếp tục chạy.
          </p>

          <!-- List of active processes -->
          <div class="running-list-container">
            <div
              v-for="proc in runningList"
              :key="proc.commandId"
              class="running-item"
            >
              <div class="item-left">
                <span class="status-dot"></span>
                <span class="item-name" :title="proc.commandName">{{ proc.commandName }}</span>
              </div>
              <div class="item-right">
                <span v-if="proc.pid" class="pid-tag">PID: {{ proc.pid }}</span>
                <span class="state-tag">{{ proc.status === 'starting' ? 'Đang khởi động' : 'Đang chạy' }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- If no running processes -->
        <div v-else class="idle-message-box">
          <p class="idle-text">
            Bạn có chắc chắn muốn thoát khỏi <strong>Command Manager</strong> không?
          </p>
          <p class="idle-subtext">
            Các cấu hình, lịch sử và trạng thái phiên làm việc hiện tại đã được tự động lưu vào cơ sở dữ liệu.
          </p>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="modal-footer">
        <button class="btn btn-ghost" :disabled="loading" @click="!loading && $emit('cancel')">
          Hủy Bỏ (Esc)
        </button>

        <button
          class="btn btn-secondary tray-action-btn"
          :disabled="loading"
          title="Thu nhỏ ứng dụng xuống System Tray và tiếp tục chạy nền"
          @click="!loading && $emit('hide-tray')"
        >
          <PanelTopClose :size="14" />
          <span>Ẩn Xuống Khay</span>
        </button>

        <button
          class="btn btn-danger exit-action-btn"
          :disabled="loading"
          @click="!loading && $emit('confirm')"
        >
          <LoaderCircle v-if="loading" :size="14" class="spin" />
          <LogOut v-else :size="14" />
          <span>{{ loading ? loadingText : (runningList.length > 0 ? 'Dừng & Thoát' : 'Thoát Ứng Dụng') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch, onMounted, onUnmounted } from 'vue';
import { AlertTriangle, AlertCircle, LogOut, PanelTopClose, X, LoaderCircle } from 'lucide-vue-next';
import type { ActiveProcessInfo } from '@/types/models';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    activeProcesses?: Map<number, ActiveProcessInfo>;
    loading?: boolean;
    loadingText?: string;
  }>(),
  {
    loading: false,
    loadingText: 'Đang thoát...',
  }
);

const emit = defineEmits<{
  (e: 'confirm'): void;
  (e: 'cancel'): void;
  (e: 'hide-tray'): void;
}>();

const runningList = computed(() => {
  if (!props.activeProcesses) return [];
  return Array.from(props.activeProcesses.values()).filter(
    (p) => p.status === 'running' || p.status === 'starting'
  );
});

const handleKeydown = (e: KeyboardEvent) => {
  if (!props.visible || props.loading) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    emit('cancel');
  }
};

watch(
  () => props.visible,
  (val) => {
    if (val) {
      window.addEventListener('keydown', handleKeydown);
    } else {
      window.removeEventListener('keydown', handleKeydown);
    }
  }
);

onMounted(() => {
  if (props.visible) {
    window.addEventListener('keydown', handleKeydown);
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(11, 13, 19, 0.85);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9998;
  animation: fadeIn 0.15s ease-out;
}

.close-modal-card {
  width: 90%;
  max-width: 480px;
  background-color: var(--surface-container);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-xl, 12px);
  box-shadow: 0 24px 48px -12px rgba(0, 0, 0, 0.75), 0 0 0 1px rgba(255, 255, 255, 0.05);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  position: relative;
  animation: modalSlideIn 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.card-accent-bar {
  height: 3px;
  width: 100%;
  background: linear-gradient(90deg, var(--primary), var(--primary-accent));
}

.card-accent-bar.warning {
  background: linear-gradient(90deg, #f59e0b, #ef4444);
}

.modal-header {
  padding: 18px 20px 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.badge-icon {
  width: 38px;
  height: 38px;
  border-radius: var(--radius-lg, 10px);
  background-color: var(--primary-subtle);
  color: var(--primary-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.badge-icon.warning {
  background-color: rgba(245, 158, 11, 0.15);
  color: #f59e0b;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.3;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.3;
}

.close-icon-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 6px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.close-icon-btn:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.modal-body {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.running-warning-banner {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.warning-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: #f59e0b;
}

.warning-desc {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.5;
}

.warning-desc strong {
  color: var(--text-primary);
}

.running-list-container {
  max-height: 140px;
  overflow-y: auto;
  background-color: var(--surface-container-lowest);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md, 8px);
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.running-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: 6px;
  background-color: var(--surface-container-low);
  font-size: 12px;
}

.item-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: #10b981;
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
  flex-shrink: 0;
  animation: pulseDot 2s infinite ease-in-out;
}

@keyframes pulseDot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(0.85); }
}

.item-name {
  color: var(--text-primary);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 220px;
}

.item-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.pid-tag {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  padding: 2px 5px;
  background-color: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  border-radius: 4px;
}

.state-tag {
  font-size: 11px;
  color: #10b981;
}

.idle-message-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px 0;
}

.idle-text {
  font-size: 13px;
  color: var(--text-primary);
  line-height: 1.5;
}

.idle-subtext {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.5;
}

.modal-footer {
  padding: 14px 20px 18px 20px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  border-top: 1px solid var(--border-subtle);
  background-color: rgba(0, 0, 0, 0.15);
}

.tray-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.exit-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes modalSlideIn {
  from {
    opacity: 0;
    transform: scale(0.95) translateY(8px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}
.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
