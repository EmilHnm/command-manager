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
              <!-- Header cột để căn lề thẳng hàng, đều tăm tắp -->
              <div class="list-column-headers">
                <span class="col-hdr ident">Tiến trình / Tên</span>
                <span class="col-hdr status">Trạng thái Tab &amp; Bộ nhớ</span>
                <span class="col-hdr actions">Thao tác</span>
              </div>

              <div
                v-for="proc in runningProcesses"
                :key="proc.commandId"
                class="process-card"
                :class="{ 'is-detached': !isTabOpen(proc.commandId) }"
              >
                <div class="card-main">
                  <!-- CỘT 1: ĐỊNH DANH TIẾN TRÌNH & TÊN (Căn lề chuẩn, không rớt dòng) -->
                  <div class="proc-col-ident">
                    <div class="proc-title-line">
                      <span class="status-dot active" title="Đang chạy ngầm" />
                      <span
                        class="proc-type-badge font-mono"
                        :class="isManualTerminal(proc) ? 'badge-terminal' : 'badge-command'"
                      >
                        <Terminal v-if="isManualTerminal(proc)" :size="10" />
                        <Code2 v-else :size="10" />
                        <span>{{ isManualTerminal(proc) ? 'Terminal' : 'Lệnh' }}</span>
                      </span>
                      <span
                        class="proc-name"
                        :title="getProcessDisplayName(proc)"
                      >
                        {{ getProcessDisplayName(proc) }}
                      </span>
                    </div>

                    <div class="proc-meta-line font-mono">
                      <span class="cmd-id-tag">#{{ proc.commandId }}</span>
                      <span class="meta-dot">·</span>
                      <span v-if="proc.pid" class="pid-tag">PID: {{ proc.pid }}</span>
                      <template v-if="proc.shellKind">
                        <span class="meta-dot">·</span>
                        <span class="shell-tag">{{ proc.shellKind }}</span>
                      </template>
                    </div>
                  </div>

                  <!-- CỘT 2: TRẠNG THÁI TAB UI & PTY BUFFER (Cố định width, căn lề thẳng hàng) -->
                  <div class="proc-col-status">
                    <span
                      class="tab-status-badge"
                      :class="isTabOpen(proc.commandId) ? 'attached' : 'detached'"
                    >
                      <Eye v-if="isTabOpen(proc.commandId)" :size="11" />
                      <EyeOff v-else :size="11" />
                      <span>{{ isTabOpen(proc.commandId) ? 'Tab đang hiển thị' : 'Tab đang ẩn (Detached)' }}</span>
                    </span>

                    <div class="proc-telemetry">
                      <span class="telemetry-item font-mono">
                        <span class="label">Buffer:</span>
                        <span class="val">{{ formatBuffer(proc.bufferBytes) }}</span>
                      </span>
                      <span class="meta-dot">·</span>
                      <span class="val status-running-text font-mono">Running</span>
                    </div>
                  </div>

                  <!-- CỘT 3: THAO TÁC HÀNH ĐỘNG -->
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
import { ref, computed, watch, onBeforeUnmount, nextTick } from 'vue';
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
  Code2,
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
  openTabs?: { id: string; commandId: number; name: string; shellKind?: string; isManual?: boolean }[];
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'open-tab', proc: { commandId: number; commandName: string; runEventId?: string; shellKind?: string }): void;
}>();

const { activeProcesses, stopCommandProcess, stopAllProcesses, refreshProcesses } = useRunSession();

const peekingCommandId = ref<number | null>(null);
const peekLogText = ref('');
const loadingPeek = ref(false);
const copied = ref(false);
const stopFeedback = ref<{ type: 'success' | 'error'; message: string } | null>(null);
const stopInProgress = ref<number | null>(null);
const stoppingAll = ref(false);
let stopFeedbackTimer: ReturnType<typeof setTimeout> | undefined;

// Tự động làm mới danh sách khi mở modal
watch(
  () => props.visible,
  (val) => {
    if (val) {
      void refreshProcesses();
    }
  },
);

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

const isManualTerminal = (proc: ActiveProcessInfo): boolean => {
  const openTab = props.openTabs?.find(t => t.commandId === proc.commandId);
  if (openTab?.isManual) return true;
  return proc.commandId <= 0 ||
    Boolean(proc.runEventId?.startsWith('terminal:')) ||
    (Boolean(proc.shellKind) && proc.shellKind !== 'command');
};

const getProcessDisplayName = (proc: ActiveProcessInfo): string => {
  // 1. Kiểm tra openTabs nếu tab đang hiển thị
  const openTab = props.openTabs?.find(t => t.commandId === proc.commandId);
  if (openTab && openTab.name && openTab.name.trim() !== '') {
    return openTab.name;
  }
  // 2. Kiểm tra từ ipcClient lưu trong cache / localStorage
  const storedName = ipcClient.getTerminalName?.(proc.commandId);
  if (storedName && storedName.trim() !== '') {
    return storedName;
  }
  // 3. Nếu là terminal thủ công
  if (isManualTerminal(proc)) {
    if (proc.commandName && proc.commandName !== 'Terminal' && proc.commandName.trim() !== '') {
      return proc.commandName;
    }
    const shell = proc.shellKind ? proc.shellKind.toUpperCase() : 'POWERSHELL';
    return `Terminal (${shell})`;
  }
  // 4. Lệnh thông thường
  return proc.commandName || `Lệnh #${proc.commandId}`;
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
  const displayName = getProcessDisplayName(proc);
  emit('open-tab', {
    commandId: proc.commandId,
    commandName: displayName,
    runEventId: proc.runEventId,
    shellKind: proc.shellKind,
  });
  handleClose();
};

const handleStopProcess = async (commandId: number, force: boolean) => {
  dismissStopFeedback();
  stopInProgress.value = commandId;
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
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
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    await stopAllProcesses();
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
  max-width: 860px;
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
  gap: 10px;
}

.list-column-headers {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 14px 4px;
  font-size: 10.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  user-select: none;
}

.col-hdr.ident {
  flex: 1 1 270px;
  min-width: 0;
}

.col-hdr.status {
  flex: 0 0 185px;
}

.col-hdr.actions {
  flex: 0 0 auto;
  width: 320px;
  text-align: right;
}

.process-card {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
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
  gap: 16px;
  min-height: 42px;
}

.proc-col-ident {
  flex: 1 1 270px;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.proc-title-line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--status-idle);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
}

.proc-type-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
  font-weight: 600;
  padding: 1.5px 6px;
  border-radius: 4px;
  flex-shrink: 0;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.proc-type-badge.badge-terminal {
  background: rgba(168, 85, 247, 0.16);
  color: #c084fc;
  border: 1px solid rgba(168, 85, 247, 0.35);
}

.proc-type-badge.badge-command {
  background: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
  border: 1px solid rgba(59, 130, 246, 0.35);
}

.proc-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.proc-meta-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10.5px;
  color: var(--text-muted);
}

.cmd-id-tag {
  color: var(--text-muted);
}

.pid-tag {
  font-size: 10px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  padding: 0 5px;
  border-radius: 2px;
  color: var(--text-secondary);
}

.shell-tag {
  font-size: 10px;
  color: var(--primary-accent, #e4b5ff);
  text-transform: uppercase;
}

.meta-dot {
  color: var(--border-subtle);
}

.proc-col-status {
  flex: 0 0 185px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex-shrink: 0;
}

.tab-status-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  padding: 2px 7px;
  border-radius: 3px;
  width: fit-content;
  white-space: nowrap;
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
  gap: 6px;
  font-size: 11px;
  color: var(--text-secondary);
}

.status-running-text {
  color: var(--status-running);
  font-weight: 500;
}

.proc-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
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
