<template>
  <footer class="status-bar">
    <div class="status-left">
      <!-- Active Processes Indicator -->
      <div class="status-item" :class="{ 'has-running': activeRunningCount > 0 }">
        <span class="status-dot" :class="{ active: activeRunningCount > 0 }" />
        <span class="status-label">{{ activeRunningCount }} tiến trình đang chạy</span>
      </div>

      <!-- Ring Buffer Size in Memory -->
      <div class="status-item" title="Dung lượng dữ liệu hiện có trong Ring Buffer của các PTY đang chạy">
        <Cpu :size="12" class="status-icon" />
        <span>Buffer: {{ formattedBufferSize }} / {{ formattedBufferLimit }}</span>
      </div>

      <!-- SQLite Mode -->
      <div class="status-item" title="SQLite vận hành chế độ WAL (1 Writer, 2 Readers)">
        <Database :size="12" class="status-icon" />
        <span>SQLite: WAL</span>
      </div>
    </div>

    <div class="status-right">
      <!-- Single Instance Status -->
      <div class="status-item" title="Trạng thái khóa Single-Instance của ứng dụng">
        <Lock :size="12" class="status-icon" />
        <span>Single-Instance: {{ singleInstanceStatus }}</span>
      </div>

      <!-- Autostart App status -->
      <div class="status-item">
        <span>Tự khởi động: </span>
        <span class="highlight-text">{{ autostartEnabled ? 'Bật' : 'Tắt' }}</span>
      </div>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { Cpu, Database, Lock } from 'lucide-vue-next';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient, isTauriRuntime } from '@/ipc/client';

const { activeProcesses } = useRunSession();
const ringBufferLimit = ref(2 * 1024 * 1024);
const autostartEnabled = ref(false);

const activeRunningCount = computed(() => {
  let count = 0;
  activeProcesses.value.forEach(p => {
    if (p.status === 'running') count++;
  });
  return count;
});

const formattedBufferSize = computed(() => {
  const totalBytes = Array.from(activeProcesses.value.values())
    .reduce((total, process) => total + (process.bufferBytes || 0), 0);
  if (totalBytes < 1024) return `${totalBytes}B`;
  return `${Math.round(totalBytes / 1024)}KB`;
});

const formattedBufferLimit = computed(() => `${Math.round(ringBufferLimit.value / (1024 * 1024))}MB × PTY`);
const singleInstanceStatus = computed(() => isTauriRuntime() ? 'Active' : 'Preview');

onMounted(async () => {
  try {
    const settings = await ipcClient.getSettings();
    ringBufferLimit.value = settings.ringBufferSizeBytes;
    autostartEnabled.value = settings.autostartApp;
  } catch {
    // Browser preview remains usable without native settings IPC.
  }
});
</script>

<style scoped>
.status-bar {
  height: var(--statusbar-height);
  background-color: #0b0d13;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  font-size: 11px;
  color: var(--text-secondary);
  user-select: none;
  z-index: 100;
  flex-shrink: 0;
}

.status-left, .status-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-icon {
  color: var(--text-muted);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  background-color: var(--status-idle);
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
  animation: pulseDot 2s infinite;
}

.has-running .status-label {
  color: var(--text-primary);
  font-weight: 500;
}

.highlight-text {
  color: #cda8ee;
  font-weight: 500;
}
</style>
