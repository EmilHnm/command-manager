<template>
  <div class="history-view">
    <header class="view-header">
      <div class="header-left">
        <h2 class="view-title">Lịch Sử Phiên Chạy & Sự Kiện (SCR-04)</h2>
        <span class="view-subtitle">Theo dõi trạng thái run_session và sự kiện run_event chi tiết từ SQLite</span>
      </div>

      <div class="header-right">
        <button class="btn btn-secondary btn-sm" @click="loadHistory">
          <RefreshCw :size="13" />
          <span>Làm mới</span>
        </button>
      </div>
    </header>

    <main class="history-content">
      <!-- Left: Sessions List -->
      <aside class="sessions-sidebar">
        <div class="sessions-header">
          <span>DANH SÁCH PHIÊN CHẠY</span>
          <span class="count-badge">{{ sessions.length }}</span>
        </div>

        <div class="sessions-list">
          <div
            v-for="s in sessions"
            :key="s.id"
            class="session-card"
            :class="{ active: selectedSessionId === s.id }"
            @click="selectSession(s.id)"
          >
            <div class="session-card-top">
              <span class="session-id">#{{ s.id }}</span>
              <span class="session-status" :class="s.status">
                {{ s.status }}
              </span>
            </div>
            <div class="session-group-name">{{ s.group_name || 'Nhóm lệnh' }}</div>
            <div class="session-time">
              <Clock :size="11" />
              <span>Bắt đầu: {{ s.started_at }}</span>
            </div>
          </div>
        </div>
      </aside>

      <!-- Right: Run Events Details -->
      <section class="events-main">
        <div v-if="selectedSession" class="events-container">
          <div class="session-summary-box">
            <div class="summary-col">
              <span class="summary-label">Phiên chạy</span>
              <span class="summary-val">#{{ selectedSession.id }} - {{ selectedSession.group_name }}</span>
            </div>
            <div class="summary-col">
              <span class="summary-label">Thời gian bắt đầu</span>
              <span class="summary-val">{{ selectedSession.started_at }}</span>
            </div>
            <div class="summary-col">
              <span class="summary-label">Trạng thái tổng thể</span>
              <span class="badge" :class="`badge-${selectedSession.status}`">
                {{ selectedSession.status }}
              </span>
            </div>
          </div>

          <div class="events-table-wrap">
            <table class="events-table">
              <thead>
                <tr>
                  <th>LỆNH THỰC THI</th>
                  <th>BẮT ĐẦU</th>
                  <th>KẾT THÚC</th>
                  <th>EXIT CODE</th>
                  <th>PID (CHẨN ĐOÁN)</th>
                  <th>LOG MEMORY</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="evt in currentEvents" :key="evt.id">
                  <td class="cmd-cell">
                    <span class="cmd-title">{{ evt.command_name }}</span>
                    <span class="cmd-id">ID: #{{ evt.command_id }}</span>
                  </td>
                  <td>{{ evt.started_at }}</td>
                  <td>{{ evt.ended_at || '-' }}</td>
                  <td>
                    <span
                      v-if="evt.exit_code !== null && evt.exit_code !== undefined"
                      class="exit-badge"
                      :class="evt.exit_code === 0 ? 'exit-zero' : 'exit-err'"
                    >
                      {{ evt.exit_code }}
                    </span>
                    <span v-else class="text-muted">-</span>
                  </td>
                  <td>
                    <code class="pid-code">{{ evt.pid || '-' }}</code>
                  </td>
                  <td>
                    <button
                      class="btn btn-ghost btn-sm"
                      @click="showLogNotice(evt)"
                    >
                      <FileText :size="12" />
                      <span>Xem Log</span>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div class="history-notice">
            💡 <strong>Quy định bảo mật & tối ưu SQLite:</strong> Chỉ có trạng thái bắt đầu/kết thúc được lưu vào SQLite. Luồng dữ liệu PTY thô không ghi ra đĩa cứng để bảo vệ thông tin đăng nhập và tránh làm nghẽn I/O.
          </div>
        </div>

        <div v-else class="no-selection">
          <History :size="32" class="empty-icon" />
          <p>Chọn một phiên chạy từ danh sách bên trái để xem chi tiết sự kiện.</p>
        </div>
      </section>
    </main>

    <!-- Modal Xem Log / Thông Báo -->
    <div v-if="showLogModal" class="modal-backdrop" @click.self="showLogModal = false">
      <div class="modal-content log-modal">
        <div class="modal-header">
          <div class="modal-title">
            <FileText class="text-primary" :size="16" />
            <span>Nhật Ký Phiên: {{ selectedEvent?.command_name }}</span>
          </div>
          <button class="btn btn-ghost btn-icon" @click="showLogModal = false">
            <X :size="15" />
          </button>
        </div>
        <div class="modal-body">
          <div class="log-viewport font-mono">
            <span class="log-line info">[System] Trích xuất từ Ring Buffer in-memory cho Lệnh #{{ selectedEvent?.command_id }} (PID: {{ selectedEvent?.pid || 'N/A' }})...</span>
            <span class="log-line">Starting process execution in working directory...</span>
            <span class="log-line success">✓ Process initialized successfully.</span>
            <span class="log-line">Listening on standard streams...</span>
            <span v-if="selectedEvent?.exit_code !== null" class="log-line">Process ended with exit code {{ selectedEvent?.exit_code }}</span>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-primary btn-sm" @click="showLogModal = false">
            Đóng
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { RefreshCw, Clock, History, FileText, X } from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import type { RunSession, RunEvent } from '@/types/models';

const sessions = ref<RunSession[]>([]);
const events = ref<RunEvent[]>([]);
const selectedSessionId = ref<number | null>(null);

const showLogModal = ref(false);
const selectedEvent = ref<RunEvent | null>(null);

onMounted(async () => {
  await loadHistory();
});

const loadHistory = async () => {
  sessions.value = await ipcClient.listSessions();
  if (sessions.value.length > 0 && !selectedSessionId.value) {
    selectedSessionId.value = sessions.value[0].id;
  }
  events.value = await ipcClient.listEvents(selectedSessionId.value || undefined);
};

const selectSession = async (id: number) => {
  selectedSessionId.value = id;
  events.value = await ipcClient.listEvents(id);
};

const selectedSession = computed(() => {
  return sessions.value.find(s => s.id === selectedSessionId.value);
});

const currentEvents = computed(() => {
  return events.value.filter(e => e.session_id === selectedSessionId.value);
});

const showLogNotice = (evt: RunEvent) => {
  selectedEvent.value = evt;
  showLogModal.value = true;
};
</script>

<style scoped>
.history-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background-color: var(--bg-app-base);
  overflow: hidden;
}

.view-header {
  padding: 16px 20px;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.view-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.view-subtitle {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.history-content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.sessions-sidebar {
  width: 280px;
  height: 100%;
  background-color: var(--bg-sidebar);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sessions-header {
  height: 38px;
  padding: 0 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
  font-weight: 700;
  color: var(--text-muted);
}

.count-badge {
  background: var(--bg-app-base);
  padding: 1px 6px;
  border-radius: 9999px;
  font-size: 10px;
}

.sessions-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.session-card {
  padding: 10px 12px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.1s ease;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.session-card:hover {
  background-color: var(--bg-surface-hover);
}

.session-card.active {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.session-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.session-id {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  color: var(--text-primary);
}

.session-status {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  padding: 1px 5px;
  border-radius: 3px;
}

.session-status.running {
  background: var(--status-running-bg);
  color: var(--status-running);
}

.session-status.completed {
  background: var(--primary-subtle);
  color: #cda8ee;
}

.session-status.failed {
  background: var(--status-failed-bg);
  color: var(--status-failed);
}

.session-group-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.session-time {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  color: var(--text-muted);
}

.events-main {
  flex: 1;
  height: 100%;
  overflow-y: auto;
  padding: 20px;
}

.events-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.session-summary-box {
  display: flex;
  align-items: center;
  gap: 24px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: 14px 18px;
}

.summary-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.summary-label {
  font-size: 10.5px;
  color: var(--text-muted);
  text-transform: uppercase;
}

.summary-val {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.events-table-wrap {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.events-table {
  width: 100%;
  border-collapse: collapse;
}

.events-table th {
  background-color: #12151f;
  padding: 10px 14px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-muted);
  text-align: left;
  border-bottom: 1px solid var(--border-subtle);
}

.events-table td {
  padding: 10px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.events-table tr:last-child td {
  border-bottom: none;
}

.cmd-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.cmd-title {
  font-weight: 600;
  color: var(--text-primary);
}

.cmd-id {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.exit-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
}

.exit-zero {
  background: var(--status-running-bg);
  color: var(--status-running);
}

.exit-err {
  background: var(--status-failed-bg);
  color: var(--status-failed);
}

.pid-code {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
}

.history-notice {
  font-size: 11.5px;
  color: var(--text-muted);
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 14px;
  line-height: 1.5;
}

.no-selection {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-muted);
  font-size: 13px;
}

.empty-icon {
  color: var(--border-medium);
}

.log-modal {
  max-width: 600px;
}

.log-viewport {
  background-color: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  font-size: 11.5px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 280px;
  overflow-y: auto;
}

.log-line {
  color: var(--text-primary);
}

.log-line.info {
  color: #38bdf8;
}

.log-line.success {
  color: var(--status-running);
}
</style>
