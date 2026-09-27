<template>
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="visible" class="modal-backdrop" @click.self="handleClose">
        <div class="modal-card bg-processes-modal" role="dialog" aria-modal="true">
          <!-- Modal Header -->
          <div class="modal-header">
            <div class="header-left">
              <div class="header-icon-wrap">
                <Zap :size="16" />
              </div>
              <div class="header-title-group">
                <h3>Tiến Trình Chạy Ngầm (Active Daemons)</h3>
                <span class="header-subtitle">
                  Quản lý vòng đời và bộ nhớ đệm PTY của các tiến trình background
                </span>
              </div>
            </div>

            <div class="header-right">
              <span class="active-badge font-mono">
                <span class="pulse-dot" v-if="runningProcesses.length > 0" />
                {{ runningProcesses.length }} Active
              </span>

              <button
                v-if="runningProcesses.length > 0"
                class="btn btn-danger btn-sm"
                :disabled="stoppingAll || stopInProgress !== null"
                title="Dừng tất cả các tiến trình ngầm đang chạy"
                @click="handleStopAll"
              >
                <LoaderCircle v-if="stoppingAll" :size="12" class="spin" />
                <Square v-else :size="12" />
                <span>{{ stoppingAll ? 'Đang dừng...' : 'Dừng Tất Cả Ngầm' }}</span>
              </button>

              <button class="btn btn-ghost btn-icon btn-sm" title="Đóng menu (Esc)" @click="handleClose">
                <X :size="15" />
              </button>
            </div>
          </div>

          <div
            v-if="stopFeedback"
            class="stop-feedback"
            :class="stopFeedback.type"
            role="status"
            aria-live="polite"
          >
            <span>{{ stopFeedback.message }}</span>
            <button
              class="feedback-close"
              type="button"
              aria-label="Đóng thông báo"
              title="Đóng thông báo"
              @click="dismissStopFeedback"
            >
              <X :size="13" />
            </button>
          </div>

          <!-- Modal Body -->
          <div class="modal-body">
            <!-- Empty State -->
            <div v-if="runningProcesses.length === 0" class="empty-state">
              <div class="empty-icon-wrap">
                <CheckCircle2 :size="36" />
              </div>
              <h4>Không có tiến trình ngầm nào đang chạy</h4>
              <p>
                Tất cả các lệnh và terminal shell đều đã kết thúc hoặc bị dừng sạch sẽ.
              </p>
            </div>

            <!-- Process Items List -->
            <div v-else class="process-list">
              <div
                v-for="proc in runningProcesses"
                :key="proc.commandId"
                class="process-card"
                :class="{ 'is-detached': !isTabOpen(proc.commandId) }"
              >
                <div class="card-main">
                  <div class="proc-info-group">
                    <div class="proc-title-row">
                      <span class="status-dot active" />
                      <span class="proc-name">{{ proc.commandName }}</span>
                      <span class="cmd-id-tag font-mono">#{{ proc.commandId }}</span>
                      <span v-if="proc.pid" class="pid-tag font-mono">PID: {{ proc.pid }}</span>

                      <!-- UI Tab Status Badge -->
                      <span
                        class="tab-status-badge"
                        :class="isTabOpen(proc.commandId) ? 'attached' : 'detached'"
                      >
                        <Eye v-if="isTabOpen(proc.commandId)" :size="11" />
                        <EyeOff v-else :size="11" />
                        <span>{{ isTabOpen(proc.commandId) ? 'Tab đang hiển thị' : 'Tab đang ẩn (Detached)' }}</span>
                      </span>
                    </div>

                    <!-- Telemetry Stats -->
                    <div class="proc-telemetry">
                      <span class="telemetry-item">
                        <span class="label">Buffer:</span>
                        <span class="val font-mono">{{ formatBuffer(proc.bufferBytes) }}</span>
                      </span>
                      <span class="divider">•</span>
                      <span class="telemetry-item">
                        <span class="label">Trạng thái:</span>
                        <span class="val status-running-text font-mono">Running</span>
                      </span>
                    </div>
                  </div>

                  <!-- Actions -->
                  <div class="proc-actions">
                    <button
                      class="btn btn-sm"
                      :class="isTabOpen(proc.commandId) ? 'btn-ghost' : 'btn-primary'"
                      :title="isTabOpen(proc.commandId) ? 'Chuyển focus đến tab terminal' : 'Mở lại tab terminal UI trên Dockview'"
                      @click="handleOpenTab(proc)"
                    >
                      <ExternalLink :size="12" />
                      <span>{{ isTabOpen(proc.commandId) ? 'Focus Tab' : 'Mở lại Tab' }}</span>
                    </button>

                    <button
                      class="btn btn-ghost btn-sm"
                      title="Xem nhanh log từ Ring Buffer"
                      @click="togglePeekLog(proc)"
                    >
                      <FileText :size="12" />
                      <span>{{ peekingCommandId === proc.commandId ? 'Ẩn Log' : 'Xem Log' }}</span>
                    </button>

                    <button
                      class="btn btn-danger-subtle btn-sm"
                      title="Dừng tiến trình (Gửi SIGTERM)"
                      :disabled="stopInProgress !== null || stoppingAll"
                      @click="handleStopProcess(proc.commandId, false)"
                    >
                      <LoaderCircle v-if="stopInProgress === proc.commandId" :size="11" class="spin" />
                      <Square v-else :size="11" />
                      <span>{{ stopInProgress === proc.commandId ? 'Đang dừng...' : 'Dừng' }}</span>
                    </button>

                    <button
                      class="btn btn-danger btn-sm"
                      title="Ép buộc diệt tiến trình (Gửi SIGKILL)"
                      :disabled="stopInProgress !== null || stoppingAll"
                      @click="handleStopProcess(proc.commandId, true)"
                    >
                      <LoaderCircle v-if="stopInProgress === proc.commandId" :size="11" class="spin" />
                      <Skull v-else :size="11" />
                      <span>{{ stopInProgress === proc.commandId ? 'Đang dừng...' : 'Force Kill' }}</span>
                    </button>
                  </div>
                </div>

                <!-- Peek Log Terminal Drawer -->
                <div v-if="peekingCommandId === proc.commandId" class="peek-log-drawer">
                  <div class="peek-log-header">
                    <div class="peek-title">
                      <Terminal :size="12" />
                      <span>Ring Buffer Output Peak (Lệnh #{{ proc.commandId }})</span>
                    </div>
                    <div class="peek-actions">
                      <button class="btn btn-ghost btn-xs" title="Làm mới log buffer" @click="fetchPeekLog(proc)">
                        <RefreshCw :size="11" :class="{ spinning: loadingPeek }" />
                        <span>Làm mới</span>
                      </button>
                      <button class="btn btn-ghost btn-xs" title="Sao chép toàn bộ log" @click="copyPeekLog">
                        <Copy :size="11" />
                        <span>{{ copied ? 'Đã sao chép!' : 'Sao chép' }}</span>
                      </button>
                    </div>
                  </div>

                  <div class="peek-log-content font-mono">
                    <pre v-if="peekLogText">{{ peekLogText }}</pre>
                    <div v-else class="peek-loading">
                      <span v-if="loadingPeek">Đang tải dữ liệu từ bộ nhớ đệm Ring Buffer...</span>
                      <span v-else>Không có dữ liệu log trong buffer.</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Modal Footer Hint -->
          <div class="modal-footer">
            <div class="footer-hint">
              <Info :size="13" class="hint-icon" />
              <span>
                <strong>Hợp đồng vòng đời:</strong> Đóng thẻ tab terminal chỉ có tác dụng ẩn giao diện. Tất cả tiến trình ngầm vẫn tiếp tục chạy và lưu luồng log vào bộ nhớ đệm Ring Buffer 2MB.
              </span>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue';
import {
  Zap,
  Square,
  X,
  CheckCircle2,
  Eye,
  EyeOff,
  ExternalLink,
  FileText,
  Skull,
  LoaderCircle,
  Terminal,
  RefreshCw,
  Copy,
  Info,
} from 'lucide-vue-next';
import { useRunSession } from '@/composables/useRunSession';
import { ipcClient } from '@/ipc/client';
import type { ActiveProcessInfo } from '@/types/models';

const props = defineProps<{
  visible: boolean;
  openTabCommandIds?: number[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'open-tab', proc: { commandId: number; commandName: string; runEventId?: string }): void;
}>();

const { activeProcesses, stopCommandProcess, stopGroupSession } = useRunSession();

const peekingCommandId = ref<number | null>(null);
const peekLogText = ref('');
const loadingPeek = ref(false);
const copied = ref(false);
const stopFeedback = ref<{ type: 'success' | 'error'; message: string } | null>(null);
const stopInProgress = ref<number | null>(null);
const stoppingAll = ref(false);
let stopFeedbackTimer: ReturnType<typeof setTimeout> | undefined;

const runningProcesses = computed(() => {
  const list: ActiveProcessInfo[] = [];
  activeProcesses.value.forEach((p) => {
    if (p.status === 'running') list.push(p);
  });
  return list;
});

const isTabOpen = (commandId: number): boolean => {
  return props.openTabCommandIds?.includes(commandId) ?? false;
};

const handleClose = () => {
  peekingCommandId.value = null;
  emit('close');
};

const dismissStopFeedback = () => {
  stopFeedback.value = null;
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
  stopFeedbackTimer = undefined;
};

const showStopFeedback = (feedback: { type: 'success' | 'error'; message: string }) => {
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
  stopFeedback.value = feedback;
  stopFeedbackTimer = setTimeout(() => {
    stopFeedback.value = null;
    stopFeedbackTimer = undefined;
  }, 4000);
};

const handleOpenTab = (proc: ActiveProcessInfo) => {
  emit('open-tab', {
    commandId: proc.commandId,
    commandName: proc.commandName,
    runEventId: proc.runEventId,
  });
  handleClose();
};

const handleStopProcess = async (commandId: number, force: boolean) => {
  dismissStopFeedback();
  stopInProgress.value = commandId;
  try {
    await stopCommandProcess(commandId, force);
    showStopFeedback({
      type: 'success',
      message: `Đã xác nhận process #${commandId} dừng hoàn toàn.`,
    });
    if (peekingCommandId.value === commandId) peekingCommandId.value = null;
  } catch (error) {
    showStopFeedback({
      type: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
    console.error('[BackgroundProcessesModal] Không thể dừng tiến trình:', error);
  } finally {
    stopInProgress.value = null;
  }
};

const handleStopAll = async () => {
  dismissStopFeedback();
  stoppingAll.value = true;
  try {
    await stopGroupSession();
    showStopFeedback({
      type: 'success',
      message: 'Đã xác nhận các process đã dừng hoàn toàn.',
    });
    peekingCommandId.value = null;
  } catch (error) {
    showStopFeedback({
      type: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
    console.error('[BackgroundProcessesModal] Không thể dừng tất cả tiến trình ngầm:', error);
  } finally {
    stoppingAll.value = false;
  }
};

const togglePeekLog = async (proc: ActiveProcessInfo) => {
  if (peekingCommandId.value === proc.commandId) {
    peekingCommandId.value = null;
    peekLogText.value = '';
    return;
  }
  peekingCommandId.value = proc.commandId;
  await fetchPeekLog(proc);
};

const fetchPeekLog = async (proc: ActiveProcessInfo) => {
  loadingPeek.value = true;
  try {
    const text = await ipcClient.reattachPty(proc.commandId, proc.runEventId);
    // Strip escape codes nhẹ hoặc giữ nguyên xterm plain
    peekLogText.value = text || '[Ring buffer rỗng]';
  } catch (err) {
    peekLogText.value = `[Lỗi khi đọc log buffer: ${String(err)}]`;
  } finally {
    loadingPeek.value = false;
  }
};

const formatBuffer = (bytes?: number) => {
  if (!bytes) return '0 B';
  return bytes < 1024 ? `${bytes} B` : `${Math.round(bytes / 1024)} KB`;
};

const copyPeekLog = async () => {
  if (!peekLogText.value) return;
  await navigator.clipboard.writeText(peekLogText.value);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
};

watch(
  () => props.visible,
  (val) => {
    if (!val) {
      dismissStopFeedback();
      peekingCommandId.value = null;
      peekLogText.value = '';
    }
  }
);

onBeforeUnmount(() => {
  if (stopFeedbackTimer) clearTimeout(stopFeedbackTimer);
});
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  background-color: rgba(11, 13, 19, 0.75);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.bg-processes-modal {
  width: 100%;
  max-width: 680px;
  max-height: 85vh;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(116, 71, 145, 0.2);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  height: 52px;
  padding: 0 18px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.stop-feedback {
  margin: 10px 18px 0;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  line-height: 1.4;
}

.stop-feedback.success {
  color: #bbf7d0;
  background: rgba(18, 53, 34, 0.95);
  border: 1px solid rgba(74, 222, 128, 0.55);
}

.stop-feedback.error {
  color: #fecaca;
  background: rgba(127, 29, 29, 0.95);
  border: 1px solid rgba(248, 113, 113, 0.55);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.header-icon-wrap {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  background: var(--primary-subtle);
  color: #cda8ee;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-title-group h3 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.header-subtitle {
  font-size: 11px;
  color: var(--text-secondary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.active-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 9999px;
  background: rgba(16, 185, 129, 0.15);
  color: #34d399;
  border: 1px solid rgba(16, 185, 129, 0.3);
  display: flex;
  align-items: center;
  gap: 6px;
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
  animation: pulseDot 2s infinite;
}

.modal-body {
  flex: 1;
  padding: 16px;
  overflow-y: auto;
}

.empty-state {
  text-align: center;
  padding: 40px 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.empty-icon-wrap {
  color: var(--status-running);
  margin-bottom: 4px;
}

.empty-state h4 {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.empty-state p {
  font-size: 12.5px;
  color: var(--text-secondary);
  max-width: 400px;
}

.process-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.process-card {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: all 0.15s ease;
}

.process-card.is-detached {
  border-color: rgba(245, 158, 11, 0.3);
  background-color: rgba(245, 158, 11, 0.03);
}

.card-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.proc-info-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.proc-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--status-idle);
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.proc-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.cmd-id-tag {
  font-size: 10px;
  color: var(--text-muted);
}

.pid-tag {
  font-size: 10px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  padding: 1px 5px;
  border-radius: 2px;
  color: var(--text-secondary);
}

.tab-status-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 10.5px;
  padding: 1px 6px;
  border-radius: 3px;
}

.tab-status-badge.attached {
  background: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
  border: 1px solid rgba(59, 130, 246, 0.3);
}

.tab-status-badge.detached {
  background: rgba(245, 158, 11, 0.15);
  color: #fbbf24;
  border: 1px solid rgba(245, 158, 11, 0.3);
}

.proc-telemetry {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-secondary);
}

.divider {
  color: var(--border-subtle);
}

.status-running-text {
  color: var(--status-running);
  font-weight: 500;
}

.proc-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-danger-subtle {
  background-color: rgba(239, 68, 68, 0.12);
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.25);
}

.btn-danger-subtle:hover {
  background-color: #ef4444;
  color: #ffffff;
}

/* Peek Log Drawer */
.peek-log-drawer {
  background-color: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.peek-log-header {
  height: 28px;
  background-color: #12151f;
  padding: 0 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
}

.peek-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10.5px;
  color: var(--text-muted);
}

.peek-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.peek-log-content {
  max-height: 180px;
  overflow-y: auto;
  padding: 8px 10px;
  font-size: 11px;
  color: #f1f5f9;
  line-height: 1.4;
  white-space: pre-wrap;
  word-break: break-all;
}

.peek-log-content pre {
  margin: 0;
  font-family: inherit;
}

.peek-loading {
  color: var(--text-muted);
  font-style: italic;
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.modal-footer {
  padding: 10px 16px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.footer-hint {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.5;
}

.hint-icon {
  color: var(--primary-accent);
  flex-shrink: 0;
  margin-top: 1px;
}

.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity 0.2s ease;
}

.modal-fade-enter-from,
.modal-fade-leave-to {
  opacity: 0;
}
</style>
