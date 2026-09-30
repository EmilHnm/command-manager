<template>
  <div class="settings-view">
    <header class="view-header">
      <div class="header-left">
        <div class="title-row">
          <h2 class="view-title">Cài Đặt Hệ Thống & Quản Lý Dữ Liệu (SCR-05)</h2>
          <span class="engine-badge">{{ APP_ENGINE_TAG }}</span>
        </div>
        <span class="view-subtitle">Cấu hình PTY, biến môi trường toàn cục, khởi động cùng OS và cơ chế sao lưu SQLite an toàn</span>
      </div>

      <div class="header-right">
        <button class="btn btn-primary btn-sm" :disabled="isSaving" @click="handleSaveSettings">
          <LoaderCircle v-if="isSaving" :size="13" class="spin" />
          <Save v-else :size="13" />
          <span>{{ isSaving ? 'Đang Lưu...' : 'Lưu Cài Đặt' }}</span>
        </button>
      </div>
    </header>

    <div class="tabs-nav">
      <button
        class="tab-btn"
        :class="{ active: currentTab === 'backup' }"
        @click="currentTab = 'backup'"
      >
        <Database :size="14" />
        <span>Sao Lưu & Phục Hồi (Disaster Recovery)</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'system' }"
        @click="currentTab = 'system'"
      >
        <Cpu :size="14" />
        <span>Hệ Thống & Khởi Động (OS & Autostart)</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'terminal' }"
        @click="currentTab = 'terminal'"
      >
        <Terminal :size="14" />
        <span>Terminal & Hiệu Năng (PTY & Memory)</span>
      </button>
    </div>

    <main class="settings-content">
      <!-- TAB 1: BACKUP & RESTORE (Primary Stitch Highlight) -->
      <div v-show="currentTab === 'backup'" class="tab-panel backup-layout">
        <input
          ref="restoreInput"
          class="hidden-file-input"
          type="file"
          accept=".sqlite,.db,application/octet-stream"
          @change="handleRestoreFile"
        />
        <!-- Card 1: Disaster Recovery Export -->
        <div class="setting-card">
          <div class="card-header-row">
            <div class="card-title-group">
              <Download :size="18" class="text-primary" />
              <h3 class="card-title">Xuất Bản Sao Lưu Toàn Diện (Backup Export)</h3>
            </div>
            <span class="badge badge-primary font-mono">SQLite VACUUM INTO</span>
          </div>

          <p class="card-desc">
            Sử dụng lệnh SQLite chuẩn <code>VACUUM INTO</code> để tạo ra bản snapshot độc lập toàn vẹn ngay khi cơ sở dữ liệu đang ở chế độ WAL. Không sinh read lock và bảo đảm tính toàn vẹn 100%.
          </p>

          <!-- DB Stats Preview Grid -->
          <div class="db-stats-grid">
            <div class="db-stat-item">
              <span class="db-stat-lbl">LỆNH ĐÃ ĐĂNG KÝ</span>
              <span class="db-stat-num font-mono">{{ backupInfo?.commandCount ?? '—' }} tác vụ</span>
            </div>
            <div class="db-stat-item">
              <span class="db-stat-lbl">NHÓM ĐIỀU PHỐI</span>
              <span class="db-stat-num font-mono">{{ backupInfo?.groupCount ?? '—' }} nhóm</span>
            </div>
            <div class="db-stat-item">
              <span class="db-stat-lbl">LỊCH SỬ PHIÊN CHẠY</span>
              <span class="db-stat-num font-mono">{{ backupInfo?.historyCount ?? '—' }} sự kiện</span>
            </div>
            <div class="db-stat-item">
              <span class="db-stat-lbl">TRẠNG THÁI CƠ SỞ DỮ LIỆU</span>
              <span class="db-stat-num font-mono text-accent">{{ backupInfo?.integrityOk ? 'Integrity OK' : 'Đang kiểm tra...' }}</span>
            </div>
          </div>

          <div class="precheck-badges">
            <span class="check-pill">✓ VACUUM INTO</span>
            <span class="check-pill">✓ WAL checkpoint</span>
            <span class="check-pill">✓ PRAGMA integrity_check</span>
          </div>

          <div class="action-row">
            <button class="btn btn-primary" :disabled="isExporting" @click="handleExportBackup">
              <LoaderCircle v-if="isExporting" :size="14" class="spin" />
              <Download v-else :size="14" />
              <span>{{ isExporting ? 'Đang tạo snapshot...' : 'Tạo Bản Sao Lưu Ngay (Export SQLite)' }}</span>
            </button>
            <span v-if="exportSuccessPath" class="export-success">
              ✓ Đã xuất thành công: <code>{{ exportSuccessPath }}</code>
            </span>
          </div>
        </div>

        <!-- Card 2: Disaster Recovery Restore with the backend rollback sequence -->
        <div class="setting-card danger-card">
          <div class="card-header-row">
            <div class="card-title-group">
              <Upload :size="18" class="text-danger" />
              <h3 class="card-title text-danger">Phục Hồi Dữ Liệu & Quy Trình Rollback</h3>
            </div>
            <span class="badge badge-failed font-mono">Atomic Swap with Fallback</span>
          </div>

          <p class="card-desc">
            Nạp file sao lưu từ máy tính vào ứng dụng. Toàn bộ tiến trình đang chạy sẽ được dừng an toàn,
            các kết nối SQLite sẽ đóng lại và file cơ sở dữ liệu sẽ được thay thế có <strong>tự động Rollback</strong> dự phòng.
          </p>

          <!-- Backend restore sequence: inspect, snapshot, stop, replace, rollback. -->
          <div class="rollback-sequence-box">
            <span class="sequence-box-title">QUY TRÌNH 5 BƯỚC KHÔI PHỤC AN TOÀN (ATOMIC SEQUENCE):</span>
            <ol class="step-list">
              <li><span class="step-num">01.</span> Kiểm tra toàn vẹn và migrate file nạp vào</li>
              <li><span class="step-num">02.</span> Tạo snapshot rollback cho DB hiện tại</li>
              <li><span class="step-num">03.</span> Dừng tiến trình, checkpoint WAL và đóng pool SQLite</li>
              <li><span class="step-num">04.</span> Thay thế DB nguyên tử và mở lại engine</li>
              <li><span class="step-num">05.</span> Tự động rollback nếu bất kỳ bước nào thất bại</li>
            </ol>
          </div>

          <div class="precheck-badges">
            <span class="check-pill text-danger">✓ Kiểm tra file trước khi nạp</span>
            <span class="check-pill text-danger">✓ Snapshot rollback</span>
            <span class="check-pill text-danger">✓ Tự động mở lại DB</span>
          </div>

          <div class="action-row">
            <button class="btn btn-danger" @click="chooseRestoreFile">
              <Upload :size="14" />
              <span>Chọn Tệp Sao Lưu Để Khôi Phục...</span>
            </button>
          </div>
        </div>
      </div>

      <!-- TAB 2: SYSTEM & LIFECYCLE -->
      <div v-show="currentTab === 'system'" class="tab-panel">
        <div class="setting-card">
          <h3 class="card-title">Khởi Động & Khóa Đơn Bản Thể</h3>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Tự khởi động cùng Hệ điều hành (OS Autostart)</span>
              <span class="row-desc">Tự động chạy ứng dụng ẩn trong khay hệ thống khi đăng nhập Windows/Linux.</span>
            </div>
            <button
              class="toggle-switch"
              :class="{ active: settings.autostartApp }"
              @click="settings.autostartApp = !settings.autostartApp"
            >
              <span class="toggle-slider" />
            </button>
          </div>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Khóa Single-Instance Cấp Ứng Dụng</span>
              <span class="row-desc">Ngăn chặn chạy trùng lặp app. Nếu mở instance thứ hai, cửa sổ hiện tại sẽ được focus.</span>
            </div>
            <span class="badge badge-primary">Đã Kích Hoạt</span>
          </div>
        </div>

        <div class="setting-card">
          <h3 class="card-title">Bộ Nhớ Đệm & Dừng Duyên Dáng (Graceful Shutdown)</h3>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Dung lượng Ring Buffer In-Memory mỗi PTY</span>
              <span class="row-desc">Giới hạn byte tối đa trong RAM để xả lại khi mở lại tab. Ngăn chặn log tràn bộ nhớ.</span>
            </div>
            <select v-model="settings.ringBufferSizeBytes" class="select-inline">
              <option :value="1048576">1 MB (Khuyến nghị cho máy cấu hình vừa)</option>
              <option :value="2097152">2 MB (Mặc định)</option>
              <option :value="4194304">4 MB (Dành cho tiến trình in log cực lớn)</option>
            </select>
          </div>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Thời gian chờ dừng tiến trình (Graceful Timeout)</span>
              <span class="row-desc">Khoảng thời gian gửi SIGTERM trước khi cưỡng chế SIGKILL khi thoát ứng dụng.</span>
            </div>
            <div class="timeout-picker">
              <input
                v-model.number="settings.shutdownTimeoutSec"
                type="number"
                min="5"
                max="15"
                class="input-number font-mono"
              />
              <span class="unit">giây</span>
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 3: TERMINAL & APPEARANCE -->
      <div v-show="currentTab === 'terminal'" class="tab-panel">
        <div class="setting-card">
          <h3 class="card-title">Cấu Hình Hiển Thị xterm.js</h3>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Phông chữ Terminal</span>
              <span class="row-desc">Sử dụng font monospace tối ưu cho lập trình viên.</span>
            </div>
            <select v-model="settings.fontFamily" class="select-inline">
              <option value="JetBrains Mono">JetBrains Mono (Khuyến nghị)</option>
              <option value="Fira Code">Fira Code</option>
              <option value="Consolas">Consolas</option>
            </select>
          </div>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Cỡ chữ (Font Size)</span>
              <span class="row-desc">Kích thước chữ trong màn hình dòng lệnh.</span>
            </div>
            <div class="timeout-picker">
              <input
                v-model.number="settings.fontSize"
                type="number"
                min="11"
                max="18"
                class="input-number font-mono"
              />
              <span class="unit">px</span>
            </div>
          </div>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Màu Chủ Đạo Giao Diện (Primary Theme)</span>
              <span class="row-desc">Theme Slate/Zinc kết hợp Primary Purple chuẩn kỹ thuật.</span>
            </div>
            <div class="color-preview-box">
              <span class="color-circle" style="background-color: #744791;" />
              <span class="color-hex">#744791 (Plum Violet)</span>
            </div>
          </div>
        </div>

        <div class="setting-card">
          <h3 class="card-title">Lịch sử lệnh & Gợi ý</h3>
          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Shell mặc định cho terminal mới</span>
              <span class="row-desc">Để trống để dùng thứ tự tự động theo hệ điều hành.</span>
            </div>
            <select v-model="settings.terminalShell" class="select-inline">
              <option value="">Tự động theo hệ điều hành</option>
              <option value="pwsh">PowerShell 7 (pwsh)</option>
              <option value="powershell">Windows PowerShell 5.1</option>
              <option value="cmd">Command Prompt (cmd)</option>
              <option v-if="!isWindows" value="bash">Bash</option>
              <option v-if="!isWindows" value="zsh">Zsh</option>
              <option v-if="!isWindows" value="sh">POSIX sh</option>
            </select>
          </div>
          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Nạp profile PowerShell</span>
              <span class="row-desc">Tắt để terminal mở nhanh hơn (-NoProfile): bỏ qua module, alias và theme như oh-my-posh trong $PROFILE. Áp dụng cho terminal mở sau khi lưu.</span>
            </div>
            <button
              class="toggle-switch"
              :class="{ active: settings.terminalLoadProfile }"
              @click="settings.terminalLoadProfile = !settings.terminalLoadProfile"
            >
              <span class="toggle-slider" />
            </button>
          </div>
          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Hiển thị ghost text</span>
              <span class="row-desc">Gợi ý do Command Manager tạo từ lịch sử dùng chung.</span>
            </div>
            <button
              class="toggle-switch"
              :class="{ active: settings.ghostTextEnabled }"
              @click="settings.ghostTextEnabled = !settings.ghostTextEnabled"
            >
              <span class="toggle-slider" />
            </button>
          </div>
          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Lưu lịch sử lệnh trong terminal</span>
              <span class="row-desc">Lịch sử thuộc về ứng dụng; shell trong terminal này không tự ghi history ra đĩa.</span>
            </div>
            <button
              class="toggle-switch"
              :class="{ active: settings.historyEnabled }"
              @click="settings.historyEnabled = !settings.historyEnabled"
            >
              <span class="toggle-slider" />
            </button>
          </div>
          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Số mục lịch sử tối đa</span>
              <span class="row-desc">Các mục cũ nhất sẽ tự động được loại bỏ sau khi ghi.</span>
            </div>
            <input v-model.number="settings.historyMaxEntries" type="number" min="100" max="100000" class="input-number font-mono" />
          </div>
          <label class="history-pattern-label" for="history-block-patterns">Mẫu không lưu (mỗi dòng một mẫu)</label>
          <textarea id="history-block-patterns" v-model="settings.historyBlockPatterns" rows="4" class="history-patterns" />

          <div class="setting-row os-history-row">
            <div class="row-info">
              <span class="row-label">Nhập lịch sử từ hệ điều hành</span>
              <span class="row-desc">Đọc lịch sử của shell trên máy (zsh, bash, PowerShell) vào lịch sử của ứng dụng để dùng cho gợi ý. File gốc chỉ được đọc; lệnh khớp mẫu không lưu sẽ bị bỏ qua. Nhập lại nhiều lần không làm trùng.</span>
              <ul v-if="osHistorySources.length" class="os-history-sources">
                <li v-for="source in osHistorySources" :key="source.path">
                  <span class="font-mono">{{ source.path }}</span>
                  <span class="os-history-meta">
                    {{ source.shellKinds.join(', ') }} ·
                    {{ source.error ? `lỗi: ${source.error}` : `${source.entries} lệnh` }}
                  </span>
                </li>
              </ul>
              <span v-else-if="osHistoryLoaded" class="row-desc">Không tìm thấy file lịch sử shell nào.</span>
            </div>
            <button
              class="btn btn-primary"
              :disabled="isImportingOsHistory || !osHistorySources.length"
              @click="showOsHistoryConfirm = true"
            >
              <LoaderCircle v-if="isImportingOsHistory" :size="14" class="spin" />
              <Download v-else :size="14" />
              <span>Nhập từ OS</span>
            </button>
          </div>
        </div>
      </div>
    </main>

    <!-- Restore Wizard Modal MOD-08 -->
    <RestoreWizard
      :visible="showRestoreModal"
      :file-path="selectedBackupPath"
      :backup-info="restoreInfo || backupInfo"
      :loading="isRestoringDb"
      @confirm-restore="confirmRestoreDatabase"
      @close="showRestoreModal = false"
    />

    <ConfirmDialog
      :visible="showOsHistoryConfirm"
      title="Nhập Lịch Sử Từ Hệ Điều Hành"
      subtitle="MOD-13 • Shell History Import"
      :message="osHistoryConfirmMessage"
      confirm-text="Nhập Lịch Sử"
      loading-text="Đang Nhập..."
      :privileged-notice="true"
      :loading="isImportingOsHistory"
      @confirm="confirmImportOsHistory"
      @cancel="showOsHistoryConfirm = false"
    />

    <div v-if="toastMessage" class="settings-toast" :class="toastType" role="status" aria-live="polite">
      <span>{{ toastMessage }}</span>
      <button class="toast-close" aria-label="Đóng thông báo" @click="dismissToast">×</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { Save, Cpu, Terminal, Database, Download, Upload, LoaderCircle } from 'lucide-vue-next';
import RestoreWizard from '@/components/dialogs/RestoreWizard.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { ipcClient } from '@/ipc/client';
import { APP_ENGINE_TAG } from '@/config/version';
import type { BackupIntegrityResult, OsHistorySource, SystemSettings } from '@/types/models';

const currentTab = ref<'system' | 'terminal' | 'backup'>('system');
const isWindows = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent);

const settings = ref<SystemSettings>({
  autostartApp: true,
  ringBufferSizeBytes: 2097152,
  shutdownTimeoutSec: 8,
  fontSize: 13,
  fontFamily: 'JetBrains Mono',
  historyEnabled: true,
  historyMaxEntries: 5000,
  historyBlockPatterns: 'password\\s*=\npasswd\\s*=\npwd\\s*=\n(?:^|\\s)(?:-p|--password)(?:\\s|=)\\S+\n(?:^|\\s)(?:token|bearer)(?:\\s|=|:)\\S+\nauthorization:\\s*',
  terminalShell: '',
  ghostTextEnabled: true,
  terminalLoadProfile: true,
});

const isExporting = ref(false);
const exportSuccessPath = ref<string | null>(null);
const backupInfo = ref<BackupIntegrityResult | null>(null);
const showRestoreModal = ref(false);
const restoreInput = ref<HTMLInputElement | null>(null);
const selectedBackupPath = ref('');
const selectedBackupFile = ref<File | null>(null);
const restoreInfo = ref<BackupIntegrityResult | null>(null);
const toastMessage = ref('');
const toastType = ref<'error' | 'success'>('success');
let toastTimer: ReturnType<typeof setTimeout> | undefined;

const showToast = (message: string, type: 'error' | 'success') => {
  toastMessage.value = message.trim() || 'Đã hoàn tất thao tác.';
  toastType.value = type;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = '';
  }, 4500);
};

const dismissToast = () => {
  toastMessage.value = '';
  if (toastTimer) clearTimeout(toastTimer);
};

onBeforeUnmount(() => {
  if (toastTimer) clearTimeout(toastTimer);
});

const osHistorySources = ref<OsHistorySource[]>([]);
const osHistoryLoaded = ref(false);
const showOsHistoryConfirm = ref(false);
const isImportingOsHistory = ref(false);

const osHistoryConfirmMessage = computed(() => {
  const total = osHistorySources.value.reduce((sum, source) => sum + source.entries, 0);
  return `Nhập ${total} lệnh từ ${osHistorySources.value.length} file lịch sử shell vào lịch sử của ứng dụng. `
    + 'Lệnh khớp mẫu không lưu sẽ bị bỏ qua; nếu vượt giới hạn số mục, các mục cũ nhất sẽ bị loại bỏ.';
});

const loadOsHistorySources = async () => {
  try {
    osHistorySources.value = await ipcClient.listOsHistorySources();
  } catch (error) {
    osHistorySources.value = [];
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    osHistoryLoaded.value = true;
  }
};

const confirmImportOsHistory = async () => {
  if (isImportingOsHistory.value) return;
  isImportingOsHistory.value = true;
  try {
    const results = await ipcClient.importOsHistory();
    const imported = results.reduce((sum, row) => sum + row.imported, 0);
    const skipped = results.reduce((sum, row) => sum + row.skipped, 0);
    showOsHistoryConfirm.value = false;
    showToast(
      `Đã nhập ${imported} lệnh vào lịch sử${skipped ? `, bỏ qua ${skipped} lệnh khớp mẫu không lưu` : ''}.`,
      'success',
    );
  } catch (error) {
    showOsHistoryConfirm.value = false;
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isImportingOsHistory.value = false;
  }
};

onMounted(async () => {
  void loadOsHistorySources();
  try {
    const [loaded, info] = await Promise.all([
      ipcClient.getSettings(),
      ipcClient.getBackupInfo(),
    ]);
    if (loaded) settings.value = loaded;
    backupInfo.value = info;
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  }
});

const isSaving = ref(false);
const isRestoringDb = ref(false);

const handleSaveSettings = async () => {
  if (isSaving.value) return;
  isSaving.value = true;
  try {
    await ipcClient.saveSettings(settings.value);
    showToast('Đã lưu thành công cài đặt hệ thống.', 'success');
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isSaving.value = false;
  }
};

const handleExportBackup = async () => {
  isExporting.value = true;
  exportSuccessPath.value = null;
  try {
    const res = await ipcClient.exportBackup();
    exportSuccessPath.value = res.path;
    showToast('Đã tạo bản sao lưu SQLite thành công.', 'success');
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isExporting.value = false;
  }
};

const chooseRestoreFile = () => {
  restoreInput.value?.click();
};

const handleRestoreFile = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = '';
  if (!file) return;

  selectedBackupFile.value = file;
  selectedBackupPath.value = file.name;
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    let binary = '';
    bytes.forEach(byte => { binary += String.fromCharCode(byte); });
    restoreInfo.value = await ipcClient.verifyBackupBytes(btoa(binary));
    if (!restoreInfo.value.integrityOk) {
      showToast('Tệp sao lưu không vượt qua integrity_check.', 'error');
      selectedBackupFile.value = null;
      return;
    }
    showRestoreModal.value = true;
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
    selectedBackupFile.value = null;
  }
};

const confirmRestoreDatabase = async () => {
  if (!selectedBackupFile.value) {
    showToast('Chưa chọn file backup.', 'error');
    return;
  }
  isRestoringDb.value = true;
  try {
    const bytes = new Uint8Array(await selectedBackupFile.value.arrayBuffer());
    let binary = '';
    bytes.forEach(byte => { binary += String.fromCharCode(byte); });
    await ipcClient.importBackupBytes(btoa(binary));
    selectedBackupFile.value = null;
    showRestoreModal.value = false;
    showToast('Đã khôi phục cơ sở dữ liệu thành công.', 'success');
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error), 'error');
  } finally {
    isRestoringDb.value = false;
  }
};
</script>

<style scoped>
.settings-view {
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

.view-subtitle {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.tabs-nav {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 8px 20px 0;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
}

.tab-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.1s ease;
}

.tab-btn:hover {
  color: var(--text-primary);
}

.tab-btn.active {
  color: #cda8ee;
  background-color: var(--bg-app-base);
  border-top: 2px solid var(--primary);
  font-weight: 600;
}

.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}

.tab-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 820px;
}

.backup-layout {
  max-width: 860px;
}

.hidden-file-input {
  display: none;
}

.settings-toast {
  position: fixed;
  top: 58px;
  right: 20px;
  z-index: 120;
  display: flex;
  align-items: flex-start;
  gap: 12px;
  width: min(360px, calc(100vw - 40px));
  padding: 11px 12px;
  border-radius: var(--radius-md);
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.35);
  font-size: 12px;
  line-height: 1.4;
}

.settings-toast.error {
  color: #fecaca;
  background: #3b1418;
  border: 1px solid rgba(248, 113, 113, 0.55);
}

.settings-toast.success {
  color: #bbf7d0;
  background: #123522;
  border: 1px solid rgba(74, 222, 128, 0.55);
}

.toast-close {
  flex: 0 0 auto;
  margin: -2px -3px 0 auto;
  color: currentColor;
  background: transparent;
  border: 0;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
}

.card-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-title-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* DB Stats Grid (from Stitch SCR-05) */
.db-stats-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 10px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px 14px;
}

.db-stat-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.db-stat-lbl {
  font-size: 10px;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.05em;
}

.db-stat-num {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.text-accent {
  color: #38bdf8;
}

/* 7-Step Rollback Visual Sequence Box (from Stitch SCR-05) */
.rollback-sequence-box {
  background-color: var(--bg-app-base);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: var(--radius-md);
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sequence-box-title {
  font-size: 10.5px;
  font-weight: 700;
  color: #f87171;
  letter-spacing: 0.05em;
}

.step-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.step-list li {
  font-size: 11.5px;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 6px;
}

.step-list li code {
  font-family: var(--font-mono);
  background: var(--bg-surface);
  padding: 1px 4px;
  border-radius: 2px;
  color: #cda8ee;
  font-size: 10.5px;
}

.step-num {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  color: var(--primary-accent);
}

.precheck-badges {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.check-pill {
  font-size: 10.5px;
  font-weight: 600;
  background-color: rgba(16, 185, 129, 0.12);
  color: #34d399;
  border: 1px solid rgba(16, 185, 129, 0.25);
  padding: 2px 8px;
  border-radius: 9999px;
}

.check-pill.text-danger {
  background-color: rgba(239, 68, 68, 0.12);
  color: #f87171;
  border-color: rgba(239, 68, 68, 0.25);
}

.setting-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.danger-card {
  border-color: rgba(239, 68, 68, 0.35);
  background-color: rgba(239, 68, 68, 0.02);
}

.card-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.card-desc {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.6;
}

.card-desc code {
  font-family: var(--font-mono);
  background: var(--bg-app-base);
  padding: 1px 4px;
  border-radius: 2px;
  color: #cda8ee;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-top: 1px solid var(--border-subtle);
  gap: 20px;
}

.row-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row-label {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.row-desc {
  font-size: 11.5px;
  color: var(--text-muted);
}

.os-history-row {
  margin-top: 12px;
}

.os-history-row .btn {
  flex-shrink: 0;
}

.os-history-sources {
  list-style: none;
  margin: 6px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 11.5px;
  color: var(--text-secondary);
}

.os-history-sources li {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.os-history-meta {
  color: var(--text-muted);
}

.select-inline {
  background-color: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 6px 10px;
  color: var(--text-primary);
  font-size: 12px;
  outline: none;
}

.timeout-picker {
  display: flex;
  align-items: center;
  gap: 6px;
}

.input-number {
  width: 60px;
  background-color: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 6px 8px;
  color: var(--text-primary);
  font-size: 12px;
  text-align: center;
}

.unit {
  font-size: 12px;
  color: var(--text-muted);
}

.toggle-switch {
  width: 36px;
  height: 20px;
  background-color: var(--border-medium);
  border-radius: 9999px;
  border: none;
  cursor: pointer;
  position: relative;
  transition: background-color 0.15s ease;
  padding: 2px;
}

.toggle-switch.active {
  background-color: var(--primary);
}

.toggle-slider {
  display: block;
  width: 16px;
  height: 16px;
  background-color: #ffffff;
  border-radius: 50%;
  transition: transform 0.15s ease;
}

.toggle-switch.active .toggle-slider {
  transform: translateX(16px);
}

.color-preview-box {
  display: flex;
  align-items: center;
  gap: 8px;
}

.color-circle {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid #ffffff;
}

.color-hex {
  font-size: 11.5px;
  font-family: var(--font-mono);
  color: #cda8ee;
}

.action-row {
  display: flex;
  align-items: center;
  gap: 14px;
}

.export-success {
  font-size: 11.5px;
  color: var(--status-running);
}

.export-success code {
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.text-danger {
  color: var(--status-failed);
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
