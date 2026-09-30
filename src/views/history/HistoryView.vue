<template>
  <div class="history-view">
    <!-- Header -->
    <header class="view-header">
      <div class="header-left">
        <div class="title-row">
          <h2 class="view-title">Lịch Sử Phiên & Nhật Ký Sự Kiện (SCR-04)</h2>
          <span class="engine-badge">Audit & Diagnostics</span>
          <span class="wal-badge">SQLite WAL: Synced</span>
        </div>
        <span class="view-subtitle">Theo dõi trạng thái run_session, tra cứu exit codes và nhật ký sự kiện tiến trình</span>
      </div>

      <div class="header-right">
        <button class="btn btn-secondary btn-sm" title="Làm mới dữ liệu từ SQLite" @click="loadHistory">
          <RefreshCw :size="13" />
          <span>Làm Mới</span>
        </button>
      </div>
    </header>

    <!-- Telemetry Ribbon Cards (from Stitch SCR-04) -->
    <section class="stats-ribbon">
      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">TỔNG SỐ PHIÊN</span>
          <History :size="16" class="stat-icon" />
        </div>
        <div class="stat-value font-mono">{{ sessions.length }}</div>
        <div class="stat-sub">Lịch sử run_session trong SQLite</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">TỈ LỆ THÀNH CÔNG</span>
          <CheckCircle2 :size="16" class="stat-icon text-success" />
        </div>
        <div class="stat-value font-mono text-success">{{ successRate }}%</div>
        <div class="stat-sub">Các phiên thoát với Exit Code 0</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">PHIÊN ĐANG CHẠY</span>
          <Zap :size="16" class="stat-icon text-warning" />
        </div>
        <div class="stat-value font-mono" :class="{ 'text-warning': activeSessionsCount > 0 }">{{ activeSessionsCount }} active</div>
        <div class="stat-sub">Tiến trình PTY đang quản lý</div>
      </div>

      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-title">RING BUFFER STREAM</span>
          <Activity :size="16" class="stat-icon text-accent" />
        </div>
        <div class="stat-value font-mono text-accent">{{ runningPtyCount }} active PTYs</div>
        <div class="stat-sub">In-memory circular buffer</div>
      </div>
    </section>

    <!-- Content: Sessions Sidebar & Events Details -->
    <main class="history-content">
      <!-- Left: Sessions List -->
      <aside class="sessions-sidebar">
        <div class="sessions-header">
          <span>DANH SÁCH PHIÊN CHẠY</span>
          <span class="count-badge">{{ filteredSessions.length }}</span>
        </div>

        <div class="search-box">
          <Search :size="13" class="search-icon" />
          <input
            v-model="sessionSearch"
            type="text"
            placeholder="Lọc phiên chạy..."
            class="session-search-input"
          />
        </div>

        <div class="sessions-list">
          <div v-if="filteredSessions.length === 0" class="empty-sessions">
            Không tìm thấy phiên chạy nào
          </div>

          <div
            v-for="s in filteredSessions"
            :key="s.id"
            class="session-card"
            :class="{ active: selectedSessionId === s.id }"
            @click="selectSession(s.id)"
          >
            <div class="session-card-top">
              <span class="session-id font-mono">#{{ s.id }}</span>
              <span class="session-status" :class="s.status">
                {{ s.status }}
              </span>
            </div>
            <div class="session-group-name">{{ s.template_name ? `Template: ${s.template_name}` : (s.group_name || 'Lệnh đơn lẻ') }}</div>
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
              <span class="summary-val font-mono">#{{ selectedSession.id }} - {{ selectedSession.template_name ? `Template: ${selectedSession.template_name}` : (selectedSession.group_name || 'Lệnh đơn lẻ') }}</span>
            </div>
            <div class="summary-col">
              <span class="summary-label">Thời gian bắt đầu</span>
              <span class="summary-val font-mono">{{ selectedSession.started_at }}</span>
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
                  <th style="text-align: right;">LOG STREAM</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="currentEvents.length === 0">
                  <td colspan="6" class="empty-cell">
                    Không có bản ghi sự kiện run_event nào cho phiên này
                  </td>
                </tr>

                <tr v-for="evt in currentEvents" :key="evt.id">
                  <td class="cmd-cell">
                    <span class="cmd-title">{{ evt.command_name }}</span>
                    <span class="cmd-id font-mono">ID: #{{ evt.command_id }}</span>
                  </td>
                  <td class="font-mono text-muted">{{ evt.started_at }}</td>
                  <td class="font-mono text-muted">{{ evt.ended_at || '-' }}</td>
                  <td>
                    <span
                      v-if="evt.exit_code !== null && evt.exit_code !== undefined"
                      class="exit-badge"
                      :class="evt.exit_code === 0 ? 'exit-zero' : 'exit-err'"
                    >
                      Exit: {{ evt.exit_code }}
                    </span>
                    <span v-else class="text-muted font-mono">-</span>
                  </td>
                  <td>
                    <code class="pid-code">{{ evt.pid || '-' }}</code>
                  </td>
                  <td style="text-align: right;">
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
          <History :size="36" class="empty-icon" />
          <p>Chọn một phiên chạy từ danh sách bên trái để xem chi tiết sự kiện.</p>
        </div>
      </section>
    </main>

    <section class="command-history-panel">
      <div class="command-history-header">
        <div>
          <h3>Lịch Sử Lệnh Đã Chạy</h3>
          <span>Các lệnh native đã chạy được upsert theo shell và kèm exit code gần nhất.</span>
        </div>
        <div class="command-history-actions">
          <div class="history-search-box">
            <Search :size="13" />
            <input v-model="commandHistoryQuery" type="text" placeholder="Tìm lệnh..." @keyup.enter="loadCommandHistory" />
          </div>
          <button class="btn btn-secondary btn-sm" @click="loadCommandHistory">Lọc</button>
          <select v-model="commandHistorySource" class="history-source-select" @change="loadCommandHistory">
            <option value="">Mọi nguồn</option>
            <option value="typed">Gõ tay</option>
            <option value="shell">Shell OSC</option>
            <option value="command">Command</option>
            <option value="template">Template</option>
          </select>
          <button v-if="commandHistories.length" class="btn btn-danger btn-sm" @click="clearCommandHistory">
            Xóa lịch sử
          </button>
        </div>
      </div>
      <div v-if="filteredCommandHistories.length" class="command-history-table-wrap">
        <table class="command-history-table">
          <thead>
            <tr>
              <th>LỆNH</th>
              <th>NGUỒN</th>
              <th>SHELL</th>
              <th>SỐ LẦN</th>
              <th>EXIT GẦN NHẤT</th>
              <th>LẦN CUỐI</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in filteredCommandHistories" :key="item.id">
              <td><code>{{ item.command_line }}</code></td>
              <td>{{ item.source }}</td>
              <td>{{ item.shell_kind }}</td>
              <td>{{ item.run_count }}</td>
              <td :class="item.last_exit_code === 0 ? 'exit-ok' : item.last_exit_code == null ? 'text-muted' : 'exit-failed'">
                {{ item.last_exit_code == null ? (item.source === 'typed' ? 'Chưa xác định' : 'Đang chạy') : item.last_exit_code }}
              </td>
              <td class="font-mono text-muted">{{ item.last_used_at }}</td>
              <td class="history-delete-cell">
                <button class="btn btn-ghost btn-sm" title="Lưu thành command" @click="saveAsCommand(item)">Lưu Command</button>
                <button class="btn btn-ghost btn-sm" title="Lưu thành template shell" @click="saveAsTemplate(item)">Lưu Template</button>
                <button class="btn btn-ghost btn-sm" title="Xóa dòng lịch sử" @click="promptDeleteHistory(item.id)">Xóa</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <div v-else class="command-history-empty">Chưa có lịch sử lệnh phù hợp.</div>
    </section>

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
          <div v-if="logLoading" class="log-viewport font-mono">
            Đang đọc Ring Buffer từ backend...
          </div>
          <div v-else-if="logError" class="log-viewport log-error font-mono">
            {{ logError }}
          </div>
          <pre v-else class="log-viewport font-mono">{{ logText || 'Ring Buffer hiện không có dữ liệu.' }}</pre>
        </div>
        <div class="modal-footer">
          <button class="btn btn-primary btn-sm" @click="showLogModal = false">
            Đóng
          </button>
        </div>
      </div>
    </div>

    <!-- Toast Notification -->
    <div v-if="toastMessage" class="history-toast" :class="toastType">
      <span>{{ toastMessage }}</span>
      <button class="toast-close" @click="toastMessage = ''">×</button>
    </div>

    <!-- Confirm Dialog Clear History (MOD-13) -->
    <ConfirmDialog
      :visible="showClearHistoryConfirm"
      title="Xóa Toàn Bộ Lịch Sử Lệnh"
      subtitle="MOD-13 • Execution History Purge"
      message="Bạn có chắc chắn muốn xóa toàn bộ lịch sử lệnh đã chạy? Toàn bộ danh mục lệnh đã lưu trong nhật ký sẽ bị xóa sạch khỏi cơ sở dữ liệu SQLite."
      confirm-text="Xóa Toàn Bộ Lịch Sử"
      loading-text="Đang Xóa Toàn Bộ..."
      :danger="true"
      :loading="isClearingHistory"
      @confirm="confirmClearHistory"
      @cancel="showClearHistoryConfirm = false"
    />

    <!-- Confirm Dialog Delete Single History (MOD-13) -->
    <ConfirmDialog
      :visible="showSingleDeleteConfirm"
      title="Xóa Dòng Lịch Sử"
      subtitle="MOD-13 • Record Removal Gate"
      message="Bạn có chắc chắn muốn xóa dòng lịch sử lệnh này?"
      confirm-text="Xóa Dòng"
      loading-text="Đang Xóa..."
      :danger="true"
      :loading="isDeletingHistory"
      @confirm="confirmDeleteSingleHistory"
      @cancel="showSingleDeleteConfirm = false"
    />

    <!-- Prompt Dialog Save As Command / Template (MOD-15) -->
    <PromptDialog
      :visible="showPromptModal"
      :title="promptConfig.title"
      :subtitle="promptConfig.subtitle"
      :hint="promptConfig.hint"
      :label="promptConfig.label"
      :command-preview="promptConfig.commandPreview"
      :command-badge="promptConfig.commandBadge"
      :default-value="promptConfig.defaultValue"
      :presets="promptConfig.presets"
      :icon="promptConfig.icon"
      :loading="isSavingPrompt"
      @confirm="handlePromptConfirm"
      @cancel="showPromptModal = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import {
  RefreshCw, Clock, History, FileText, X,
  CheckCircle2, Zap, Activity, Search
} from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import { useRunSession } from '@/composables/useRunSession';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import PromptDialog from '@/components/dialogs/PromptDialog.vue';
import type { CommandHistory, RunSession, RunEvent } from '@/types/models';

const { activeProcesses } = useRunSession();

const sessions = ref<RunSession[]>([]);
const events = ref<RunEvent[]>([]);
const selectedSessionId = ref<number | null>(null);
const sessionSearch = ref('');
const commandHistoryQuery = ref('');
const commandHistories = ref<CommandHistory[]>([]);
const commandHistorySource = ref('');

const showLogModal = ref(false);
const selectedEvent = ref<RunEvent | null>(null);
const logText = ref('');
const logError = ref('');
const logLoading = ref(false);
let stopHistoryListener: (() => void) | undefined;

watch(showLogModal, (val) => {
  if (!val) {
    selectedEvent.value = null;
    logText.value = '';
    logError.value = '';
    logLoading.value = false;
  }
});

onMounted(async () => {
  await loadHistory();
  await loadCommandHistory();
  const unlisten = await ipcClient.onHistoryAdded(() => {
    void loadCommandHistory();
  });
  stopHistoryListener = typeof unlisten === 'function' ? unlisten : undefined;
});

onBeforeUnmount(() => {
  stopHistoryListener?.();
});

const loadHistory = async () => {
  sessions.value = await ipcClient.listSessions();
  if (sessions.value.length > 0 && !selectedSessionId.value) {
    selectedSessionId.value = sessions.value[0].id;
  }
  events.value = await ipcClient.listEvents(selectedSessionId.value || undefined);
};

const loadCommandHistory = async () => {
  commandHistories.value = await ipcClient.listCommandHistory(commandHistoryQuery.value);
};

const filteredCommandHistories = computed(() => commandHistorySource.value
  ? commandHistories.value.filter((item) => item.source === commandHistorySource.value)
  : commandHistories.value);

const showClearHistoryConfirm = ref(false);
const showSingleDeleteConfirm = ref(false);
const deletingHistoryId = ref<string | null>(null);

const showPromptModal = ref(false);
const promptConfig = ref<{
  type: 'command' | 'template';
  item: CommandHistory | null;
  title: string;
  subtitle: string;
  hint: string;
  label: string;
  commandPreview: string;
  commandBadge: string;
  defaultValue: string;
  presets: string[];
  icon: 'terminal' | 'bookmark' | 'edit';
}>({
  type: 'command',
  item: null,
  title: '',
  subtitle: '',
  hint: '',
  label: '',
  commandPreview: '',
  commandBadge: '',
  defaultValue: '',
  presets: [],
  icon: 'terminal',
});

const toastMessage = ref('');
const toastType = ref<'success' | 'error'>('success');
let toastTimer: ReturnType<typeof setTimeout> | undefined;

const showToast = (message: string, type: 'success' | 'error' = 'success') => {
  toastMessage.value = message;
  toastType.value = type;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = '';
  }, 3500);
};

const promptDeleteHistory = (id: string) => {
  deletingHistoryId.value = id;
  showSingleDeleteConfirm.value = true;
};

const isDeletingHistory = ref(false);

const confirmDeleteSingleHistory = async () => {
  if (!deletingHistoryId.value) return;
  isDeletingHistory.value = true;
  try {
    await ipcClient.deleteCommandHistory(deletingHistoryId.value);
    await loadCommandHistory();
    showToast('Đã xóa dòng lịch sử lệnh.', 'success');
    showSingleDeleteConfirm.value = false;
    deletingHistoryId.value = null;
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isDeletingHistory.value = false;
  }
};

const saveAsCommand = (item: CommandHistory) => {
  promptConfig.value = {
    type: 'command',
    item,
    title: 'Lưu Lệnh Mới Từ Lịch Sử',
    subtitle: 'MOD-15 • Technical Input Gate • Shell History Replay',
    hint: 'Nhập tên gợi nhớ để lưu câu lệnh này vào Thư Viện Lệnh. Lệnh sẽ có sẵn để chạy lại hoặc đưa vào Nhóm.',
    label: 'Tên Câu Lệnh Mới (*)',
    commandPreview: item.command_line,
    commandBadge: item.shell_kind === 'argv' ? 'Direct Argv' : 'Shell',
    defaultValue: item.command_line.slice(0, 48),
    presets: ['Build', 'Test', 'Dev Server', 'Migration', 'Backup'],
    icon: 'terminal',
  };
  showPromptModal.value = true;
};

const saveAsTemplate = (item: CommandHistory) => {
  promptConfig.value = {
    type: 'template',
    item,
    title: 'Lưu Mẫu Lệnh Shell Mới',
    subtitle: 'MOD-15 • Template Generator • Shell History Replay',
    hint: 'Tạo một mẫu lệnh mới từ dòng lệnh lịch sử để cấu hình thêm tham số {{param}} sau này.',
    label: 'Tên Mẫu Lệnh Mới (*)',
    commandPreview: item.command_line,
    commandBadge: 'Shell Template',
    defaultValue: item.command_line.slice(0, 48),
    presets: ['FFmpeg', 'Docker', 'Git', 'Script', 'Dev'],
    icon: 'bookmark',
  };
  showPromptModal.value = true;
};

const isSavingPrompt = ref(false);

const handlePromptConfirm = async (name: string) => {
  const item = promptConfig.value.item;
  if (!item || !name.trim()) return;
  isSavingPrompt.value = true;

  try {
    if (promptConfig.value.type === 'command') {
      await ipcClient.saveCommand({
        name: name.trim(),
        execution_string: item.command_line,
        is_shell: item.shell_kind !== 'argv',
        shell_kind: item.shell_kind === 'argv' ? undefined : item.shell_kind,
      });
      showToast(`Đã lưu câu lệnh "${name.trim()}" vào Thư Viện Lệnh.`, 'success');
    } else {
      await ipcClient.saveTemplate({
        name: name.trim(),
        description: 'Tạo từ lịch sử lệnh',
        template_string: item.command_line,
        is_shell: true,
        params: [],
      });
      showToast(`Đã lưu Mẫu Lệnh "${name.trim()}" thành công.`, 'success');
    }
    showPromptModal.value = false;
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isSavingPrompt.value = false;
  }
};

const clearCommandHistory = () => {
  showClearHistoryConfirm.value = true;
};

const isClearingHistory = ref(false);

const confirmClearHistory = async () => {
  isClearingHistory.value = true;
  try {
    await ipcClient.clearCommandHistory();
    commandHistories.value = [];
    showClearHistoryConfirm.value = false;
    showToast('Đã xóa toàn bộ lịch sử lệnh.', 'success');
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isClearingHistory.value = false;
  }
};

const selectSession = async (id: number) => {
  selectedSessionId.value = id;
  events.value = await ipcClient.listEvents(id);
};

const filteredSessions = computed(() => {
  if (!sessionSearch.value.trim()) return sessions.value;
  const q = sessionSearch.value.toLowerCase();
  return sessions.value.filter(s =>
    (s.group_name && s.group_name.toLowerCase().includes(q)) ||
    s.id.toString().includes(q) ||
    s.status.toLowerCase().includes(q)
  );
});

const selectedSession = computed(() => {
  return sessions.value.find(s => s.id === selectedSessionId.value);
});

const currentEvents = computed(() => {
  return events.value.filter(e => e.session_id === selectedSessionId.value);
});

const successRate = computed(() => {
  if (sessions.value.length === 0) return 100;
  const successCount = sessions.value.filter(s => s.status === 'completed').length;
  return Math.round((successCount / sessions.value.length) * 100);
});

const activeSessionsCount = computed(() => {
  return sessions.value.filter(s => s.status === 'running').length;
});

const runningPtyCount = computed(() => {
  let count = 0;
  activeProcesses.value.forEach(p => {
    if (p.status === 'running') count++;
  });
  return count;
});

const showLogNotice = async (evt: RunEvent) => {
  selectedEvent.value = evt;
  logText.value = '';
  logError.value = '';
  showLogModal.value = true;
  logLoading.value = true;
  try {
    logText.value = await ipcClient.getEventLog(evt.id);
  } catch (error) {
    logError.value = error instanceof Error
      ? `${error.message}. Ring Buffer chỉ khả dụng khi tiến trình còn đang chạy.`
      : String(error);
  } finally {
    logLoading.value = false;
  }
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

.title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.view-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.engine-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  background-color: var(--primary-subtle);
  color: var(--primary-accent);
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  border: 1px solid rgba(116, 71, 145, 0.35);
}

.wal-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  background-color: rgba(6, 182, 212, 0.12);
  color: #38bdf8;
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  border: 1px solid rgba(6, 182, 212, 0.3);
}

.view-subtitle {
  font-size: 11.5px;
  color: var(--text-secondary);
}

/* 4 Quick Stats Cards (from Stitch SCR-04) */
.stats-ribbon {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
  padding: 14px 20px 0;
  flex-shrink: 0;
}

.stat-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 3px;
  box-shadow: var(--shadow-sm);
  transition: border-color 0.15s ease;
}

.stat-card:hover {
  border-color: var(--border-medium);
}

.stat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.stat-title {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.05em;
}

.stat-icon {
  color: var(--text-muted);
}

.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  margin-top: 2px;
}

.stat-sub {
  font-size: 10.5px;
  color: var(--text-secondary);
}

.text-warning {
  color: var(--status-starting);
}

.text-success {
  color: var(--status-running);
}

.text-accent {
  color: #38bdf8;
}

.history-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  padding-top: 14px;
}

.sessions-sidebar {
  width: 290px;
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

.search-box {
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 6px;
  background-color: var(--bg-app-base);
  border-bottom: 1px solid var(--border-subtle);
}

.search-icon {
  color: var(--text-muted);
}

.session-search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: 11.5px;
  color: var(--text-primary);
}

.count-badge {
  background: var(--bg-app-base);
  padding: 1px 6px;
  border-radius: 9999px;
  font-size: 10px;
}

.empty-sessions {
  text-align: center;
  padding: 24px 12px;
  color: var(--text-muted);
  font-size: 12px;
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

.command-history-panel {
  margin: 14px 20px 20px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
  flex-shrink: 0;
}

.command-history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-subtle);
}

.command-history-header h3 {
  color: var(--text-primary);
  font-size: 13px;
  margin-bottom: 3px;
}

.command-history-header span {
  color: var(--text-muted);
  font-size: 11px;
}

.command-history-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.history-search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 190px;
  padding: 5px 8px;
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.history-search-box input {
  width: 100%;
  color: var(--text-primary);
  background: transparent;
  border: 0;
  outline: 0;
  font-size: 11px;
}

.btn-danger {
  color: #fca5a5;
  border-color: rgba(248, 113, 113, 0.35);
}

.command-history-table-wrap {
  max-height: 220px;
  overflow: auto;
}

.command-history-table {
  width: 100%;
  border-collapse: collapse;
}

.command-history-table th,
.command-history-table td {
  padding: 8px 12px;
  text-align: left;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
}

.command-history-table th {
  position: sticky;
  top: 0;
  color: var(--text-muted);
  background: #12151f;
  font-size: 10px;
}

.command-history-table code {
  display: block;
  max-width: 420px;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.exit-ok {
  color: var(--status-running);
}

.exit-failed {
  color: var(--status-failed);
}

.history-delete-cell {
  text-align: right !important;
}

.command-history-empty {
  padding: 14px;
  color: var(--text-muted);
  font-size: 11px;
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

.log-error {
  color: #fca5a5;
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

.history-toast {
  position: fixed;
  bottom: 38px;
  right: 20px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  z-index: 150;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
  animation: toastIn 0.2s ease-out;
}

.history-toast.success {
  background-color: #141721;
  border: 1px solid var(--status-running);
  color: #a7f3d0;
}

.history-toast.error {
  background-color: #141721;
  border: 1px solid var(--status-failed);
  color: #fca5a5;
}

.toast-close {
  background: transparent;
  border: none;
  color: inherit;
  font-size: 16px;
  cursor: pointer;
  line-height: 1;
  opacity: 0.7;
}

.toast-close:hover {
  opacity: 1;
}

@keyframes toastIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
