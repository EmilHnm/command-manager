<template>
  <div v-if="visible" class="shutdown-overlay">
    <div class="shutdown-card">
      <div class="brand-header">
        <img src="/logo.svg" class="brand-logo" alt="Command Manager Logo" />
        <h2 class="app-title">Command Manager</h2>
      </div>

      <div class="status-headline">
        <RotateCw class="spin-icon" :size="20" />
        <span>{{ phase === 'forcing' ? 'Đang cưỡng chế dừng tiến trình và thoát...' : 'Đang dừng an toàn các tiến trình...' }}</span>
      </div>

      <p v-if="phase === 'stopping'" class="description">
        Đang yêu cầu các tiến trình con dừng lại.
        Nếu chưa dừng sau <strong>{{ countdown }} giây</strong>, ứng dụng sẽ cưỡng chế dừng.
      </p>
      <p v-else class="description">Đang dừng các tiến trình còn lại và đóng ứng dụng.</p>
      <p v-if="error" role="alert" class="shutdown-error">{{ error }}</p>

      <!-- Active Stopping List -->
      <div v-if="processesList.length" class="stopping-list">
        <div
          v-for="proc in processesList"
          :key="proc.commandId"
          class="stopping-item"
        >
          <span class="proc-name">{{ proc.commandName }}</span>
          <span v-if="proc.pid" class="proc-pid">PID: {{ proc.pid }}</span>
          <span class="proc-status">Đang dọn dẹp kết nối...</span>
        </div>
      </div>

      <!-- Progress Bar -->
      <div class="progress-bar-wrap">
        <div class="progress-bar" :style="{ width: `${progressPercent}%` }" />
      </div>

      <div class="footer-actions">
        <button class="btn btn-danger btn-sm" :disabled="phase === 'forcing'" @click="$emit('force-kill')">
          <LoaderCircle v-if="phase === 'forcing'" :size="12" class="spin" />
          <Square v-else :size="12" />
          <span>{{ phase === 'forcing' ? 'Đang Cưỡng Chế Thoát...' : 'Cưỡng Chế Thoát Ngay (Force Kill)' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { RotateCw, Square, LoaderCircle } from 'lucide-vue-next';
import type { ActiveProcessInfo } from '@/types/models';

const props = defineProps<{
  visible: boolean;
  countdown: number;
  timeout: number;
  phase: 'stopping' | 'forcing';
  error: string;
  activeProcesses: Map<number, ActiveProcessInfo>;
}>();

defineEmits<{
  (e: 'force-kill'): void;
}>();

const progressPercent = computed(() => {
  const total = Math.max(1, props.timeout);
  const current = props.countdown;
  return Math.min(100, Math.max(0, ((total - current) / total) * 100));
});

const processesList = computed(() => {
  return Array.from(props.activeProcesses.values()).filter(p => p.status === 'running' || p.status === 'starting');
});
</script>

<style scoped>
.shutdown-overlay {
  position: fixed;
  inset: 0;
  background-color: var(--surface-overlay, #0b0d13eb);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  user-select: none;
}

.shutdown-card {
  width: 90%;
  max-width: 520px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-xl);
  padding: 30px;
  box-shadow: 0 25px 50px -12px #000000cc;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 16px;
}

.brand-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.brand-logo {
  width: 28px;
  height: 28px;
  object-fit: contain;
  border-radius: var(--radius-sm);
}

.app-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.status-headline {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  font-weight: 600;
  color: #cda8ee;
}

.spin-icon {
  animation: spin 1.5s linear infinite;
  color: var(--primary);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.description {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.6;
}

.shutdown-error {
  color: var(--status-error, #f87171);
  font-size: 12px;
}

.description code {
  font-family: var(--font-mono);
  background: var(--bg-app-base);
  padding: 1px 4px;
  border-radius: 2px;
  color: var(--text-primary);
}

.stopping-list {
  width: 100%;
  max-height: 140px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px;
  text-align: left;
}

.stopping-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11.5px;
}

.proc-name {
  color: var(--text-primary);
  font-weight: 500;
}

.proc-pid {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-muted);
}

.proc-status {
  font-size: 11px;
  color: var(--status-starting);
}

.progress-bar-wrap {
  width: 100%;
  height: 6px;
  background-color: var(--bg-app-base);
  border-radius: 9999px;
  overflow: hidden;
}

.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, var(--primary), #a855f7);
  transition: width 1s linear;
}

.footer-actions {
  margin-top: 6px;
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
