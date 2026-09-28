<template>
  <div v-if="visible" class="modal-backdrop" @click.self="!loading && $emit('cancel')">
    <div class="modal-content kill-panel-modal">
      <!-- Hazard Ribbon -->
      <div class="hazard-stripe" />

      <!-- Modal Header -->
      <div class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge danger-badge">
            <AlertOctagon :size="18" />
          </div>
          <div>
            <div class="modal-title">Dừng Toàn Bộ Terminal (Kill Panel)</div>
            <div class="modal-subtitle">Tiêu diệt các tiến trình PTY và đóng toàn bộ tab trong {{ panelLabel }}</div>
          </div>
        </div>
        <button class="btn btn-ghost btn-icon" :disabled="loading" title="Hủy bỏ (Esc)" @click="$emit('cancel')">
          <X :size="15" />
        </button>
      </div>

      <!-- Modal Body -->
      <div class="modal-body">
        <p class="modal-desc">
          Bạn có chắc chắn muốn dừng toàn bộ tiến trình hệ thống và đóng tất cả
          <strong>{{ tabs.length }} terminal</strong> trong <strong>{{ panelLabel }}</strong>?
        </p>

        <!-- Preview list of terminals that will be terminated -->
        <div class="terminals-list-box">
          <div class="list-header">
            <span>Danh sách terminal sẽ bị dừng ({{ tabs.length }})</span>
          </div>
          <div class="list-items">
            <div
              v-for="tab in tabs"
              :key="tab.id"
              class="terminal-item-row"
            >
              <div class="row-left">
                <span
                  class="status-dot"
                  :class="{ active: getProcessStatus(tab.commandId) === 'running' }"
                />
                <span class="tab-name">{{ tab.name }}</span>
                <span v-if="getProcessInfo(tab.commandId)?.pid" class="pid-badge">
                  PID: {{ getProcessInfo(tab.commandId)?.pid }}
                </span>
                <span v-if="tab.shellKind" class="shell-badge">
                  {{ tab.shellKind }}
                </span>
              </div>
              <div class="row-right">
                <span
                  class="status-tag"
                  :class="getProcessStatus(tab.commandId)"
                >
                  {{ getProcessStatus(tab.commandId) === 'running' ? 'Đang chạy' : 'Đã dừng' }}
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Stop Mode Options -->
        <div class="stop-options">
          <label class="stop-option-card" :class="{ selected: stopMode === 'graceful' }">
            <input v-model="stopMode" type="radio" value="graceful" :disabled="loading" />
            <div class="option-info">
              <span class="option-title">Dừng mềm tiêu chuẩn (Graceful Stop - SIGTERM)</span>
              <span class="option-desc">Gửi tín hiệu dừng tiêu chuẩn, cho phép tiến trình dọn dẹp file tạm và đóng kết nối mạng.</span>
            </div>
          </label>

          <label class="stop-option-card danger" :class="{ selected: stopMode === 'force' }">
            <input v-model="stopMode" type="radio" value="force" :disabled="loading" />
            <div class="option-info">
              <span class="option-title text-danger">Dừng cưỡng bức ngay lập tức (Force Kill - SIGKILL)</span>
              <span class="option-desc">Tiêu diệt tiến trình ngay lập tức. Dùng khi tiến trình bị treo hoặc không phản hồi tín hiệu dừng.</span>
            </div>
          </label>
        </div>

        <!-- Warning Callout -->
        <div class="warning-callout">
          <AlertTriangle :size="14" class="callout-icon" />
          <span>Thao tác này sẽ gửi tín hiệu kết thúc tiến trình tới kernel hệ điều hành và giải phóng bộ nhớ của các tab trong panel.</span>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="modal-footer">
        <button class="btn btn-secondary" :disabled="loading" @click="$emit('cancel')">
          Hủy bỏ (Esc)
        </button>
        <button
          class="btn btn-danger"
          :disabled="loading"
          @click="$emit('confirm', stopMode === 'force')"
        >
          <LoaderCircle v-if="loading" :size="13" class="spin" />
          <OctagonX v-else :size="13" />
          <span>{{ loading ? 'Đang dừng các tiến trình...' : `Dừng & Đóng Toàn Bộ (${tabs.length})` }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { AlertOctagon, AlertTriangle, OctagonX, X, LoaderCircle } from 'lucide-vue-next';
import type { OpenTabItem } from '@/composables/useSplitLayout';
import { useRunSession } from '@/composables/useRunSession';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    panelName: 'paneA' | 'paneB';
    tabs: OpenTabItem[];
    loading?: boolean;
    isSplitMode?: boolean;
  }>(),
  {
    loading: false,
    isSplitMode: true,
  }
);

defineEmits<{
  (e: 'confirm', force: boolean): void;
  (e: 'cancel'): void;
}>();

const { getProcessStatus, getProcessInfo } = useRunSession();

const stopMode = ref<'graceful' | 'force'>('graceful');

const panelLabel = computed(() => {
  if (!props.isSplitMode) return 'Workspace';
  return props.panelName === 'paneA' ? 'Khung A (Trái/Trên)' : 'Khung B (Phải/Dưới)';
});

watch(
  () => props.visible,
  (val) => {
    if (val) {
      stopMode.value = 'graceful';
    }
  },
  { immediate: true }
);
</script>

<style scoped>
.kill-panel-modal {
  max-width: 520px;
  position: relative;
  overflow: hidden;
  background-color: var(--bg-surface, #1a1e2b);
  border: 1px solid var(--border-medium, #2d3448);
  border-radius: var(--radius-lg, 8px);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.7), 0 0 20px rgba(239, 68, 68, 0.2);
}

.hazard-stripe {
  height: 3px;
  width: 100%;
  background: repeating-linear-gradient(
    45deg,
    #ef4444,
    #ef4444 10px,
    #b91c1c 10px,
    #b91c1c 20px
  );
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md, 6px);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.danger-badge {
  background-color: rgba(239, 68, 68, 0.15);
  color: var(--status-failed, #ef4444);
  border: 1px solid rgba(239, 68, 68, 0.3);
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.modal-subtitle {
  font-size: 11.5px;
  color: var(--text-muted, #64748b);
  margin-top: 1px;
}

.modal-body {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.modal-desc {
  font-size: 13px;
  color: var(--text-secondary, #94a3b8);
  line-height: 1.5;
}

.modal-desc strong {
  color: var(--text-primary, #f1f5f9);
}

/* Danh sách terminal box */
.terminals-list-box {
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: var(--radius-sm, 4px);
  overflow: hidden;
}

.list-header {
  padding: 7px 12px;
  background-color: rgba(26, 30, 43, 0.6);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.list-items {
  max-height: 150px;
  overflow-y: auto;
  scrollbar-width: thin;
}

.terminal-item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(36, 42, 62, 0.5);
  font-size: 12px;
}

.terminal-item-row:last-child {
  border-bottom: none;
}

.row-left {
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--status-idle, #64748b);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: var(--status-running, #10b981);
  box-shadow: 0 0 6px var(--status-running, #10b981);
}

.tab-name {
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 180px;
}

.pid-badge {
  font-size: 10px;
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  background-color: var(--bg-surface, #1a1e2b);
  color: var(--text-muted, #64748b);
  padding: 1px 5px;
  border-radius: 2px;
  border: 1px solid var(--border-subtle, #242a3e);
}

.shell-badge {
  font-size: 9.5px;
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  color: var(--primary-accent, #e4b5ff);
  background-color: rgba(116, 71, 145, 0.15);
  padding: 1px 4px;
  border-radius: 2px;
}

.status-tag {
  font-size: 10.5px;
  padding: 2px 6px;
  border-radius: 3px;
  font-weight: 500;
}

.status-tag.running {
  background-color: rgba(16, 185, 129, 0.15);
  color: var(--status-running, #10b981);
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.status-tag.idle,
.status-tag.stopped {
  background-color: rgba(100, 116, 139, 0.15);
  color: var(--text-muted, #64748b);
  border: 1px solid rgba(100, 116, 139, 0.3);
}

/* Stop Mode Radio Cards */
.stop-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.stop-option-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: var(--radius-sm, 4px);
  cursor: pointer;
  background-color: rgba(26, 30, 43, 0.4);
  transition: all 0.15s ease;
}

.stop-option-card:hover {
  background-color: rgba(35, 40, 58, 0.7);
  border-color: var(--border-medium, #2d3448);
}

.stop-option-card.selected {
  background-color: rgba(116, 71, 145, 0.12);
  border-color: var(--primary, #744791);
}

.stop-option-card.danger.selected {
  background-color: rgba(239, 68, 68, 0.12);
  border-color: rgba(239, 68, 68, 0.6);
}

.stop-option-card input[type="radio"] {
  margin-top: 2px;
  accent-color: var(--primary, #744791);
}

.stop-option-card.danger input[type="radio"] {
  accent-color: var(--status-failed, #ef4444);
}

.option-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.option-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
}

.option-desc {
  font-size: 11px;
  color: var(--text-muted, #64748b);
  line-height: 1.4;
}

.text-danger {
  color: var(--status-failed, #ef4444);
}

.warning-callout {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: rgba(249, 115, 22, 0.1);
  border: 1px solid rgba(249, 115, 22, 0.25);
  border-radius: var(--radius-sm, 4px);
  font-size: 11px;
  color: #fdba74;
  line-height: 1.4;
}

.callout-icon {
  flex-shrink: 0;
  color: #f97316;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 20px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  background-color: rgba(18, 21, 31, 0.5);
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
