<template>
  <div class="settings-view">
    <header class="view-header">
      <div class="header-left">
        <h2 class="view-title">Cài Đặt Hệ Thống & Quản Lý Dữ Liệu (SCR-05)</h2>
        <span class="view-subtitle">Cấu hình khởi động, bộ nhớ đệm PTY và cơ chế sao lưu SQLite an toàn</span>
      </div>

      <div class="header-right">
        <button class="btn btn-primary btn-sm" @click="handleSaveSettings">
          <Save :size="13" />
          <span>Lưu Cài Đặt</span>
        </button>
      </div>
    </header>

    <div class="tabs-nav">
      <button
        class="tab-btn"
        :class="{ active: currentTab === 'system' }"
        @click="currentTab = 'system'"
      >
        <Cpu :size="14" />
        <span>Hệ Thống & Vòng Đời</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'terminal' }"
        @click="currentTab = 'terminal'"
      >
        <Terminal :size="14" />
        <span>Giao Diện & Terminal</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'backup' }"
        @click="currentTab = 'backup'"
      >
        <Database :size="14" />
        <span>Sao Lưu & Khôi Phục DB</span>
      </button>
    </div>

    <main class="settings-content">
      <!-- TAB 1: SYSTEM & LIFECYCLE -->
      <div v-show="currentTab === 'system'" class="tab-panel">
        <div class="setting-card">
          <h3 class="card-title">Khởi Động & Khóa Đơn Bản Thể</h3>

          <div class="setting-row">
            <div class="row-info">
              <span class="row-label">Tự khởi động cùng Hệ điều hành (OS Autostart)</span>
              <span class="row-desc">Tự động mở ứng dụng khi đăng nhập Windows/Linux qua tauri-plugin-autostart.</span>
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
                class="input-number"
              />
              <span class="unit">giây</span>
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 2: TERMINAL & APPEARANCE -->
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
                class="input-number"
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
      </div>

      <!-- TAB 3: BACKUP & RESTORE -->
      <div v-show="currentTab === 'backup'" class="tab-panel">
        <div class="setting-card">
          <h3 class="card-title">Xuất Bản Sao Lưu SQLite (Export / Backup)</h3>
          <p class="card-desc">
            Sử dụng lệnh SQLite chuẩn <code>VACUUM INTO</code> để tạo ra bản snapshot độc lập toàn vẹn ngay khi cơ sở dữ liệu đang ở chế độ WAL.
            Ứng dụng tự động chạy <code>PRAGMA integrity_check</code> trước khi xác nhận thành công.
          </p>

          <div class="action-row">
            <button class="btn btn-primary" :disabled="isExporting" @click="handleExportBackup">
              <Download :size="14" />
              <span>{{ isExporting ? 'Đang tạo snapshot...' : 'Tạo Bản Sao Lưu Ngay (Export SQLite)' }}</span>
            </button>
            <span v-if="exportSuccessPath" class="export-success">
              ✓ Đã xuất thành công: <code>{{ exportSuccessPath }}</code>
            </span>
          </div>
        </div>

        <div class="setting-card danger-card">
          <h3 class="card-title text-danger">Khôi Phục Dữ Liệu SQLite (Import / Restore)</h3>
          <p class="card-desc">
            Nạp file sao lưu từ máy tính vào ứng dụng. Toàn bộ tiến trình đang chạy sẽ được dừng an toàn,
            các kết nối SQLite sẽ đóng lại và file cơ sở dữ liệu sẽ được thay thế có <strong>tự động Rollback</strong> dự phòng.
          </p>

          <div class="action-row">
            <button class="btn btn-danger" @click="openRestoreModal">
              <Upload :size="14" />
              <span>Chọn Tệp Sao Lưu Để Khôi Phục...</span>
            </button>
          </div>
        </div>
      </div>
    </main>

    <!-- Restore Wizard Modal MOD-08 -->
    <RestoreWizard
      :visible="showRestoreModal"
      @confirm-restore="confirmRestoreDatabase"
      @close="showRestoreModal = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { Save, Cpu, Terminal, Database, Download, Upload } from 'lucide-vue-next';
import RestoreWizard from '@/components/dialogs/RestoreWizard.vue';
import { ipcClient } from '@/ipc/client';
import type { SystemSettings } from '@/types/models';

const currentTab = ref<'system' | 'terminal' | 'backup'>('system');

const settings = ref<SystemSettings>({
  autostartApp: true,
  ringBufferSizeBytes: 2097152,
  shutdownTimeoutSec: 8,
  fontSize: 13,
  fontFamily: 'JetBrains Mono',
});

const isExporting = ref(false);
const exportSuccessPath = ref<string | null>(null);
const showRestoreModal = ref(false);

onMounted(async () => {
  const loaded = await ipcClient.getSettings();
  if (loaded) {
    settings.value = loaded;
  }
});

const handleSaveSettings = async () => {
  await ipcClient.saveSettings(settings.value);
  alert('Đã lưu thành công cài đặt hệ thống!');
};

const handleExportBackup = async () => {
  isExporting.value = true;
  exportSuccessPath.value = null;
  try {
    const res = await ipcClient.exportBackup();
    exportSuccessPath.value = res.path;
  } finally {
    isExporting.value = false;
  }
};

const openRestoreModal = () => {
  showRestoreModal.value = true;
};

const confirmRestoreDatabase = async () => {
  await ipcClient.importBackup('backup.sqlite');
  showRestoreModal.value = false;
  alert('Đã khôi phục cơ sở dữ liệu thành công! Ứng dụng đã nạp lại dữ liệu mới.');
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

.view-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
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
  max-width: 780px;
}

.setting-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.danger-card {
  border-color: rgba(239, 68, 68, 0.3);
  background-color: rgba(239, 68, 68, 0.02);
}

.card-title {
  font-size: 13.5px;
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
</style>
