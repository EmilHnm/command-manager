<template>
  <div v-if="visible" class="modal-backdrop">
    <div
      class="config-import-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="import-modal-title"
    >
      <div class="modal-accent-bar" />

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <FileCode :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="import-modal-title" class="modal-title">
              Nhập Máy Chủ Từ ~/.ssh/config
            </h3>
            <span v-if="configPath" class="modal-subtitle font-mono">{{ configPath }}</span>
            <span v-else class="modal-subtitle">Đang quét tệp cấu hình...</span>
          </div>
        </div>
        <button class="btn-close" title="Đóng (Esc)" @click="$emit('close')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body custom-scrollbar">
        <div v-if="loading" class="loading-state">
          <LoaderCircle :size="24" class="spin text-primary" />
          <span>Đang phân tích cấu hình SSH của máy...</span>
        </div>

        <div v-else-if="scanError" class="error-banner" role="alert">
          <AlertCircle :size="14" class="error-icon" />
          <span>Không thể đọc tệp config: {{ scanError }}</span>
        </div>

        <div v-else-if="hosts.length === 0" class="empty-state">
          <FileQuestion :size="32" class="empty-icon" />
          <div class="empty-title">Không tìm thấy máy chủ nào</div>
          <div class="empty-desc">
            Tệp <code>~/.ssh/config</code> trống hoặc chưa được thiết lập bất kỳ khối <code>Host</code> nào.
          </div>
        </div>

        <template v-else>
          <!-- Guidance banner -->
          <div class="info-banner">
            <Info :size="14" class="info-icon" />
            <div class="info-text">
              Đã tìm thấy <strong>{{ hosts.length }}</strong> máy chủ trong tệp cấu hình. Chọn các máy chủ bạn muốn thêm vào danh sách quản lý.
            </div>
          </div>

          <!-- Select All Controls -->
          <div class="selection-bar">
            <label class="checkbox-row">
              <input
                type="checkbox"
                class="form-checkbox"
                :checked="isAllSelected"
                :indeterminate.prop="isIndeterminate"
                @change="toggleSelectAll"
              />
              <span class="checkbox-label">
                Chọn tất cả ({{ selectedAliases.size }}/{{ hosts.length }})
              </span>
            </label>
          </div>

          <!-- Hosts List -->
          <div class="hosts-list">
            <div
              v-for="h in hosts"
              :key="h.alias"
              class="host-card"
              :class="{ selected: selectedAliases.has(h.alias) }"
              @click="toggleHost(h.alias)"
            >
              <div class="card-left">
                <input
                  type="checkbox"
                  class="form-checkbox"
                  :checked="selectedAliases.has(h.alias)"
                  @click.stop="toggleHost(h.alias)"
                />
                <div class="host-info">
                  <div class="host-name-row">
                    <span class="host-alias font-mono">{{ h.alias }}</span>
                    <span v-if="h.unsupported.length > 0" class="unsupported-badge" :title="`Chỉ thị chưa hỗ trợ: ${h.unsupported.join(', ')}`">
                      ⚠️ Cần OpenSSH hệ thống
                    </span>
                  </div>
                  <div class="host-spec font-mono text-xs text-muted">
                    {{ h.user ? `${h.user}@` : '' }}{{ h.host_name || h.alias }}:{{ h.port || 22 }}
                    <span v-if="h.identity_file" class="spec-prop">
                      • Khóa: {{ h.identity_file }}
                    </span>
                    <span v-if="h.proxy_jump" class="spec-prop">
                      • Jump: {{ h.proxy_jump }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <button type="button" class="btn btn-ghost" @click="$emit('close')">
          Đóng
        </button>
        <button
          type="button"
          class="btn btn-primary btn-import"
          :disabled="importing || selectedAliases.size === 0"
          @click="handleImport"
        >
          <LoaderCircle v-if="importing" :size="13" class="spin" />
          <Download v-else :size="13" />
          <span>{{ importing ? 'Đang Nhập...' : `Nhập ${selectedAliases.size} Máy Chủ Đã Chọn` }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import {
  FileCode,
  X,
  LoaderCircle,
  AlertCircle,
  FileQuestion,
  Info,
  Download,
} from 'lucide-vue-next';
import type { ConfigHost } from '@/types/ssh';
import { ipcClient } from '@/ipc/client';
import { useSshHosts } from '@/composables/useSshHosts';

const props = defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'imported'): void;
}>();

const { createHost } = useSshHosts();

const loading = ref(false);
const importing = ref(false);
const scanError = ref<string | null>(null);
const configPath = ref<string | null>(null);
const hosts = ref<ConfigHost[]>([]);
const selectedAliases = ref<Set<string>>(new Set());

const isAllSelected = computed(() => {
  return hosts.value.length > 0 && selectedAliases.value.size === hosts.value.length;
});

const isIndeterminate = computed(() => {
  return (
    selectedAliases.value.size > 0 &&
    selectedAliases.value.size < hosts.value.length
  );
});

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    loading.value = true;
    scanError.value = null;
    hosts.value = [];
    selectedAliases.value.clear();

    try {
      const scan = await ipcClient.sshConfigScan();
      configPath.value = scan.path || null;
      hosts.value = scan.hosts || [];
      // Select all by default
      selectedAliases.value = new Set(hosts.value.map((h) => h.alias));
    } catch (e) {
      scanError.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  },
  { immediate: true }
);

const toggleHost = (alias: string) => {
  if (selectedAliases.value.has(alias)) {
    selectedAliases.value.delete(alias);
  } else {
    selectedAliases.value.add(alias);
  }
};

const toggleSelectAll = () => {
  if (isAllSelected.value) {
    selectedAliases.value.clear();
  } else {
    selectedAliases.value = new Set(hosts.value.map((h) => h.alias));
  }
};

const handleImport = async () => {
  if (selectedAliases.value.size === 0) return;
  importing.value = true;

  try {
    for (const h of hosts.value) {
      if (!selectedAliases.value.has(h.alias)) continue;
      const useSystem = h.unsupported.length > 0;
      await createHost({
        name: h.alias,
        host: h.host_name || h.alias,
        port: h.port || 22,
        username: h.user || 'root',
        auth_method: h.identity_file ? 'key' : 'auto',
        key_path: h.identity_file || null,
        jump_host_id: null,
        use_system_ssh: useSystem,
        agent_forwarding: h.forward_agent,
        default_dir: null,
        color: '#744791',
        tags: ['imported-config'],
      });
    }
    emit('imported');
    emit('close');
  } catch (e) {
    scanError.value = e instanceof Error ? e.message : String(e);
  } finally {
    importing.value = false;
  }
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1050;
  background-color: rgba(11, 13, 19, 0.8);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.config-import-modal {
  width: 100%;
  max-width: 600px;
  max-height: 85vh;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalScaleIn 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modalScaleIn {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.modal-accent-bar {
  height: 3px;
  width: 100%;
  background-color: var(--primary);
}

.modal-header {
  height: 48px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-sidebar);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.icon-badge {
  color: var(--primary);
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-muted);
}

.btn-close {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
}

.btn-close:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.modal-body {
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.loading-state, .empty-state {
  padding: 32px 16px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-muted);
  text-align: center;
}

.empty-icon {
  color: var(--text-muted);
  opacity: 0.6;
}

.empty-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.empty-desc {
  font-size: 12px;
  max-width: 380px;
  line-height: 1.4;
}

.empty-desc code {
  background-color: rgba(255, 255, 255, 0.06);
  padding: 1px 4px;
  border-radius: 3px;
}

.error-banner {
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--status-failed);
}

.info-banner {
  background-color: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-secondary);
}

.info-icon {
  color: var(--primary);
  flex-shrink: 0;
}

.selection-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-subtle);
}

.checkbox-row {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.form-checkbox {
  accent-color: var(--primary);
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.checkbox-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}

.hosts-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.host-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 8px 12px;
  cursor: pointer;
  transition: all 0.12s ease;
}

.host-card:hover {
  background-color: var(--bg-surface-hover);
  border-color: rgba(255, 255, 255, 0.15);
}

.host-card.selected {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.card-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.host-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.host-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.host-alias {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.unsupported-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  background-color: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.3);
  color: var(--status-warning);
}

.host-spec {
  font-size: 11px;
}

.spec-prop {
  margin-left: 4px;
}

.modal-footer {
  height: 48px;
  padding: 0 16px;
  background-color: var(--bg-sidebar);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.btn-import {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
