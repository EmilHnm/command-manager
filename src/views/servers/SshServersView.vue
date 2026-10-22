<template>
  <div class="ssh-servers-view">
    <!-- Top Action Header (Toolbar) -->
    <header class="servers-header">
      <div class="header-left">
        <div class="header-title-row">
          <div class="header-icon-box">
            <Globe :size="18" class="header-icon" />
          </div>
          <div class="header-texts">
            <h1 class="page-title">Quản Lý Máy Chủ SSH</h1>
            <span class="page-subtitle">
              {{ hosts.length }} máy chủ đã lưu • {{ activeHostsCount }} máy chủ đang có kết nối mở
            </span>
          </div>
        </div>
      </div>

      <div v-if="activeTab === 'hosts'" class="header-right">
        <!-- Search Field -->
        <div class="search-box">
          <Search :size="13" class="search-icon" />
          <input
            v-model="searchQuery"
            type="text"
            class="search-input"
            placeholder="Tìm theo tên, địa chỉ, người dùng, thẻ..."
          />
          <button
            v-if="searchQuery"
            type="button"
            class="search-clear-btn"
            title="Xóa tìm kiếm"
            @click="searchQuery = ''"
          >
            <X :size="11" />
          </button>
        </div>

        <button
          type="button"
          class="btn btn-secondary btn-import-config"
          title="Quét và nhập cấu hình từ ~/.ssh/config"
          @click="showConfigImportModal = true"
        >
          <FileCode :size="13" />
          <span>Nhập ~/.ssh/config</span>
        </button>

        <button
          type="button"
          class="btn btn-primary btn-add-host"
          title="Thêm cấu hình máy chủ SSH mới"
          @click="openAddHostModal"
        >
          <Plus :size="14" />
          <span>Thêm Máy Chủ</span>
        </button>
      </div>
    </header>

    <!-- Hosts | Keychain -->
    <nav class="view-tabs" role="tablist">
      <button
        type="button"
        role="tab"
        class="view-tab"
        :class="{ active: activeTab === 'hosts' }"
        :aria-selected="activeTab === 'hosts'"
        @click="setTab('hosts')"
      >
        <Server :size="13" />
        <span>Máy Chủ</span>
        <span class="view-tab-count">{{ hosts.length }}</span>
      </button>
      <button
        type="button"
        role="tab"
        class="view-tab"
        :class="{ active: activeTab === 'keychain' }"
        :aria-selected="activeTab === 'keychain'"
        @click="setTab('keychain')"
      >
        <KeyRound :size="13" />
        <span>Keychain</span>
        <span class="view-tab-count">{{ keychainKeys.length + identities.length }}</span>
      </button>
    </nav>

    <main v-if="activeTab === 'keychain'" class="servers-content custom-scrollbar">
      <KeychainPanel />
    </main>

    <!-- Tag Filter Bar if tags exist -->
    <div v-if="activeTab === 'hosts' && allTags.length > 0" class="tag-filter-bar">
      <span class="tag-filter-label">Lọc thẻ:</span>
      <button
        type="button"
        class="filter-tag-chip"
        :class="{ active: selectedTagFilter === null }"
        @click="selectedTagFilter = null"
      >
        Tất cả ({{ hosts.length }})
      </button>
      <button
        v-for="tag in allTags"
        :key="tag"
        type="button"
        class="filter-tag-chip"
        :class="{ active: selectedTagFilter === tag }"
        @click="selectedTagFilter = selectedTagFilter === tag ? null : tag"
      >
        #{{ tag }}
      </button>
    </div>

    <!-- Main Content Area -->
    <main v-if="activeTab === 'hosts'" class="servers-content custom-scrollbar">
      <!-- Loading State -->
      <div v-if="loading && hosts.length === 0" class="state-container">
        <LoaderCircle :size="28" class="spin text-primary" />
        <span>Đang tải danh sách máy chủ SSH...</span>
      </div>

      <!-- Error State -->
      <div v-else-if="error" class="state-container text-danger">
        <AlertTriangle :size="28" />
        <span>Không thể tải danh sách máy chủ: {{ error }}</span>
        <button type="button" class="btn btn-secondary btn-sm mt-2" @click="fetchHosts">
          Thử Lại
        </button>
      </div>

      <!-- Empty State -->
      <div v-else-if="filteredHosts.length === 0" class="empty-state-card">
        <div class="empty-icon-wrapper">
          <Server :size="40" class="empty-icon" />
        </div>
        <h2 class="empty-title">
          {{ searchQuery || selectedTagFilter ? 'Không tìm thấy máy chủ phù hợp' : 'Chưa có máy chủ SSH nào' }}
        </h2>
        <p class="empty-desc">
          {{
            searchQuery || selectedTagFilter
              ? 'Thử thay đổi từ khóa tìm kiếm hoặc bỏ bộ lọc thẻ hiện tại.'
              : 'Lưu các cấu hình máy chủ để mở terminal từ xa và thao tác tập tin chỉ với một cú nhấp chuột.'
          }}
        </p>

        <div v-if="!searchQuery && !selectedTagFilter" class="empty-actions">
          <button
            type="button"
            class="btn btn-primary"
            @click="openAddHostModal"
          >
            <Plus :size="14" />
            <span>Thêm Máy Chủ Mới</span>
          </button>
          <button
            type="button"
            class="btn btn-secondary"
            @click="showConfigImportModal = true"
          >
            <FileCode :size="13" />
            <span>Nhập từ ~/.ssh/config</span>
          </button>
        </div>
      </div>

      <!-- Hosts Grid / List -->
      <div v-else class="hosts-table-container">
        <table class="hosts-table">
          <thead>
            <tr>
              <th class="th-name">MÁY CHỦ</th>
              <th class="th-conn">KẾT NỐI & JUMP</th>
              <th class="th-auth">XÁC THỰC</th>
              <th class="th-tags">THẺ</th>
              <th class="th-status">TRẠNG THÁI</th>
              <th class="th-actions text-right">THAO TÁC</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="h in filteredHosts"
              :key="h.id"
              class="host-row"
              :class="{ 'has-open-tabs': h.open_channels > 0 }"
            >
              <!-- Name & Host Color Pip -->
              <td class="td-name">
                <div class="name-cell">
                  <div
                    class="host-color-pip"
                    :style="{ backgroundColor: h.color || '#744791' }"
                  />
                  <div class="host-title-group">
                    <span class="host-name font-semibold">{{ h.name }}</span>
                    <div class="host-badges-row">
                      <span
                        v-if="h.use_system_ssh"
                        class="badge badge-system"
                        title="Chạy qua binary OpenSSH của hệ điều hành"
                      >
                        🌐 OpenSSH Hệ Thống
                      </span>
                      <span
                        v-if="h.agent_forwarding"
                        class="badge badge-agent"
                        title="Chuyển tiếp SSH Agent (-A)"
                      >
                        Agent-Forwarding
                      </span>
                    </div>
                  </div>
                </div>
              </td>

              <!-- Connection Address -->
              <td class="td-conn">
                <div class="conn-cell font-mono">
                  <span class="conn-string">
                    <span class="text-primary">{{ effectiveUsername(h) }}</span>@<span>{{ h.host }}</span><span class="text-muted">:{{ h.port }}</span>
                  </span>
                  <div v-if="h.jump_host_id" class="jump-pill">
                    <ArrowRight :size="10" />
                    <span>Jump: {{ getJumpHostName(h.jump_host_id) }}</span>
                  </div>
                </div>
              </td>

              <!-- Auth Method & Secrets -->
              <td class="td-auth">
                <div class="auth-cell">
                  <span class="auth-method-badge" :class="`method-${h.auth_method}`">
                    <Key v-if="h.auth_method === 'key'" :size="10" />
                    <Lock v-else-if="h.auth_method === 'password'" :size="10" />
                    <Cpu v-else-if="h.auth_method === 'agent'" :size="10" />
                    <Sparkles v-else :size="10" />
                    <span>{{ authMethodLabel(h.auth_method) }}</span>
                  </span>

                  <span
                    v-if="h.identity_id && identityById.get(h.identity_id)"
                    class="vault-badge"
                    title="Tên đăng nhập và mật khẩu lấy từ identity trong Keychain"
                  >
                    <UserRound :size="10" />
                    <span>{{ identityById.get(h.identity_id)?.name }}</span>
                  </span>
                  <span
                    v-if="effectiveKeyName(h)"
                    class="vault-badge"
                    title="Khóa trong Keychain"
                  >
                    <KeyRound :size="10" />
                    <span>{{ effectiveKeyName(h) }}</span>
                  </span>
                  <span
                    v-if="h.secret_store !== 'none'"
                    class="vault-badge"
                    :title="`Mật khẩu lưu trong ${h.secret_store === 'keyring' ? 'Keyring HĐH' : 'SQLite mã hóa'}`"
                  >
                    <ShieldCheck :size="10" />
                    <span>{{ h.secret_store === 'keyring' ? 'Keyring' : 'SQLite' }}</span>
                  </span>
                </div>
              </td>

              <!-- Tags -->
              <td class="td-tags">
                <div class="tags-list">
                  <span
                    v-for="tag in h.tags"
                    :key="tag"
                    class="tag-pill"
                    @click="selectedTagFilter = tag"
                  >
                    #{{ tag }}
                  </span>
                  <span v-if="h.tags.length === 0" class="text-muted text-xs">—</span>
                </div>
              </td>

              <!-- Status & Last Connected -->
              <td class="td-status">
                <div class="status-cell">
                  <div v-if="h.open_channels > 0" class="live-status-chip">
                    <span class="pulse-dot" />
                    <span class="font-mono">{{ h.open_channels }} tab mở</span>
                  </div>
                  <span v-else class="status-idle text-muted text-xs">Chưa kết nối</span>

                  <span v-if="h.last_connected_at" class="last-conn-time text-xs text-muted">
                    {{ formatLastConnected(h.last_connected_at) }}
                  </span>
                </div>
              </td>

              <!-- Actions -->
              <td class="td-actions text-right">
                <div class="actions-wrapper">
                  <!-- Connect Terminal Button -->
                  <button
                    type="button"
                    class="btn btn-connect"
                    :disabled="connectingHostId === h.id"
                    title="Mở phiên làm việc Terminal SSH"
                    @click="handleConnectHost(h)"
                  >
                    <LoaderCircle v-if="connectingHostId === h.id" :size="12" class="spin" />
                    <Terminal v-else :size="12" />
                    <span>Kết Nối</span>
                  </button>

                  <!-- Actions Dropdown Trigger -->
                  <div class="dropdown-container">
                    <button
                      type="button"
                      class="btn-icon-dots"
                      title="Tùy chọn thao tác"
                      :class="{ active: activeMenuHostId === h.id }"
                      @click.stop="toggleActionsMenu(h.id, $event)"
                    >
                      <MoreVertical :size="14" />
                    </button>

                    <!-- Popup Menu -->
                    <div
                      v-if="activeMenuHostId === h.id"
                      class="actions-menu-popup"
                      :style="menuStyle"
                      @click.stop
                    >
                      <button
                        type="button"
                        class="menu-item"
                        @click="handleEditHost(h)"
                      >
                        <Pencil :size="12" />
                        <span>Chỉnh Sửa</span>
                      </button>

                      <button
                        type="button"
                        class="menu-item"
                        :disabled="h.use_system_ssh"
                        :title="h.use_system_ssh ? 'Máy chủ dùng OpenSSH hệ thống không có trình quản lý file' : undefined"
                        @click="handleOpenFiles(h)"
                      >
                        <FolderOpen :size="12" />
                        <span>Mở Trình Quản Lý File</span>
                      </button>

                      <button
                        type="button"
                        class="menu-item"
                        @click="handleDuplicateHost(h)"
                      >
                        <Copy :size="12" />
                        <span>Nhân Bản</span>
                      </button>

                      <button
                        v-if="h.open_channels > 0"
                        type="button"
                        class="menu-item text-warning"
                        @click="handleDisconnectHost(h)"
                      >
                        <Unplug :size="12" />
                        <span>Đóng Kết Nối Mở</span>
                      </button>

                      <div class="menu-divider" />

                      <button
                        type="button"
                        class="menu-item text-danger"
                        @click="handlePromptDelete(h)"
                      >
                        <Trash2 :size="12" />
                        <span>Xóa Máy Chủ</span>
                      </button>
                    </div>
                  </div>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </main>

    <!-- Modal MOD-17: SshHostEditorModal -->
    <SshHostEditorModal
      :visible="showEditorModal"
      :host-to-edit="hostToEdit"
      :existing-hosts="hosts"
      @cancel="closeEditorModal"
      @saved="onHostSaved"
    />

    <!-- Modal Nhập ~/.ssh/config: SshConfigImportModal -->
    <SshConfigImportModal
      :visible="showConfigImportModal"
      @close="showConfigImportModal = false"
      @imported="fetchHosts"
    />

    <!-- MOD-13 Confirm Dialog for Delete -->
    <ConfirmDialog
      :visible="showDeleteConfirm"
      title="Xác Nhận Xóa Máy Chủ SSH"
      :message="deleteMessage"
      confirm-text="Xóa Máy Chủ"
      cancel-text="Hủy"
      type="danger"
      @confirm="confirmDelete"
      @cancel="showDeleteConfirm = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  Globe,
  Plus,
  Search,
  X,
  FileCode,
  Server,
  LoaderCircle,
  AlertTriangle,
  ArrowRight,
  Key,
  Lock,
  Cpu,
  Sparkles,
  ShieldCheck,
  Terminal,
  MoreVertical,
  Pencil,
  Copy,
  Unplug,
  Trash2,
  KeyRound,
  UserRound,
  FolderOpen,
} from 'lucide-vue-next';
import { useKeychain } from '@/composables/useKeychain';
import KeychainPanel from '@/components/keychain/KeychainPanel.vue';
import { useSshHosts } from '@/composables/useSshHosts';
import type { SshHost, SshHostView, SshAuthMethod } from '@/types/ssh';
import { jumpChain } from '@/types/ssh';
import SshHostEditorModal from '@/components/dialogs/SshHostEditorModal.vue';
import SshConfigImportModal from '@/components/dialogs/SshConfigImportModal.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { ipcClient } from '@/ipc/client';

const router = useRouter();
const route = useRoute();
const { hosts, loading, error, fetchHosts, deleteHost, disconnectHost } = useSshHosts();
const {
  keys: keychainKeys,
  identities,
  keyById,
  identityById,
  fetchKeychain,
} = useKeychain();

// `/servers?tab=keychain` opens the Keychain directly.
const activeTab = ref<'hosts' | 'keychain'>(route.query.tab === 'keychain' ? 'keychain' : 'hosts');
const setTab = (tab: 'hosts' | 'keychain') => {
  activeTab.value = tab;
  void router.replace({ query: { ...route.query, tab: tab === 'keychain' ? 'keychain' : undefined } });
};

/** The username the connection will use: an identity's wins, as in the backend. */
const effectiveUsername = (h: SshHostView) =>
  (h.identity_id && identityById.value.get(h.identity_id)?.username) || h.username;

const effectiveKeyName = (h: SshHostView) => {
  const keyId = h.key_id ?? (h.identity_id ? identityById.value.get(h.identity_id)?.key_id : null);
  return keyId ? keyById.value.get(keyId)?.name : undefined;
};

const searchQuery = ref('');
const selectedTagFilter = ref<string | null>(null);
const connectingHostId = ref<string | null>(null);
const activeMenuHostId = ref<string | null>(null);

const showEditorModal = ref(false);
const hostToEdit = ref<SshHostView | null>(null);
const showConfigImportModal = ref(false);

const showDeleteConfirm = ref(false);
const hostToDelete = ref<SshHostView | null>(null);

const activeHostsCount = computed(() => {
  return hosts.value.filter((h) => h.open_channels > 0).length;
});

const allTags = computed(() => {
  const set = new Set<string>();
  for (const h of hosts.value) {
    for (const t of h.tags || []) {
      set.add(t);
    }
  }
  return Array.from(set).sort();
});

const filteredHosts = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  const tag = selectedTagFilter.value;

  return hosts.value.filter((h) => {
    if (tag && !(h.tags || []).includes(tag)) {
      return false;
    }
    if (!q) return true;
    return (
      h.name.toLowerCase().includes(q) ||
      h.host.toLowerCase().includes(q) ||
      effectiveUsername(h).toLowerCase().includes(q) ||
      (h.tags || []).some((t) => t.toLowerCase().includes(q))
    );
  });
});

/** "outer → inner" for a host behind several jump hosts. */
const getJumpHostName = (jumpHostId: string) => {
  const chain = jumpChain(hosts.value, jumpHostId);
  return chain.length > 0 ? chain.map((h) => h.name).join(' → ') : jumpHostId;
};

const authMethodLabel = (method: SshAuthMethod) => {
  switch (method) {
    case 'key':
      return 'Khóa Riêng';
    case 'password':
      return 'Mật Khẩu';
    case 'agent':
      return 'SSH Agent';
    default:
      return 'Tự Động';
  }
};

const formatLastConnected = (isoDate: string) => {
  try {
    const d = new Date(isoDate);
    if (isNaN(d.getTime())) return isoDate;
    return d.toLocaleString('vi-VN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return isoDate;
  }
};

const openAddHostModal = () => {
  hostToEdit.value = null;
  showEditorModal.value = true;
};

const handleEditHost = (h: SshHostView) => {
  activeMenuHostId.value = null;
  hostToEdit.value = h;
  showEditorModal.value = true;
};

const handleDuplicateHost = (h: SshHostView) => {
  activeMenuHostId.value = null;
  hostToEdit.value = {
    ...h,
    id: '',
    name: `${h.name} (bản sao)`,
    open_channels: 0,
    last_connected_at: null,
  };
  showEditorModal.value = true;
};

const closeEditorModal = () => {
  showEditorModal.value = false;
  hostToEdit.value = null;
};

const onHostSaved = (_saved: SshHost) => {
  closeEditorModal();
};

// The menu is `position: fixed` next to its button: the table container
// clips overflow, which would cut off the menu of the last rows.
const MENU_HEIGHT = 150;
const menuStyle = ref<Record<string, string>>({});

const toggleActionsMenu = (hostId: string, event: MouseEvent) => {
  if (activeMenuHostId.value === hostId) {
    activeMenuHostId.value = null;
    return;
  }
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const right = `${Math.max(8, window.innerWidth - rect.right)}px`;
  menuStyle.value =
    rect.bottom + 4 + MENU_HEIGHT > window.innerHeight
      ? { right, bottom: `${window.innerHeight - rect.top + 4}px` }
      : { right, top: `${rect.bottom + 4}px` };
  activeMenuHostId.value = hostId;
};

const handleDisconnectHost = async (h: SshHostView) => {
  activeMenuHostId.value = null;
  try {
    await disconnectHost(h.id);
  } catch (e) {
    console.error('Không thể ngắt kết nối host:', e);
  }
};

const handlePromptDelete = (h: SshHostView) => {
  activeMenuHostId.value = null;
  hostToDelete.value = h;
  showDeleteConfirm.value = true;
};

const deleteMessage = computed(() => {
  if (!hostToDelete.value) return '';
  const extra =
    hostToDelete.value.open_channels > 0
      ? ` CẢNH BÁO: Đang có ${hostToDelete.value.open_channels} phiên terminal mở tới máy chủ này. Xóa máy chủ sẽ lập tức ngắt toàn bộ các phiên đó!`
      : '';
  return `Bạn có chắc chắn muốn xóa cấu hình máy chủ "${hostToDelete.value.name}" (${hostToDelete.value.username}@${hostToDelete.value.host})?${extra}`;
});

const confirmDelete = async () => {
  if (!hostToDelete.value) return;
  try {
    await deleteHost(hostToDelete.value.id, true);
  } catch (e) {
    console.error('Không thể xóa host:', e);
  } finally {
    showDeleteConfirm.value = false;
    hostToDelete.value = null;
  }
};

const handleOpenFiles = (h: SshHostView) => {
  activeMenuHostId.value = null;
  void router.push({
    path: '/workspace',
    query: {
      openFiles: h.id,
      filesLabel: h.name,
      ...(h.default_dir ? { filesPath: h.default_dir } : {}),
      ...(h.color ? { filesAccent: h.color } : {}),
    },
  });
};

const handleConnectHost = async (h: SshHostView) => {
  connectingHostId.value = h.id;
  try {
    const terminal = await ipcClient.sshTerminalOpen(h.id);
    // Navigate to workspace with query to open tab
    await router.push({
      path: '/workspace',
      query: {
        runEventId: terminal.runEventId,
        runCommandId: String(terminal.commandId),
        runCommandName: h.name,
        sshHostId: h.id,
      },
    });
  } catch (e) {
    console.error('Lỗi khi mở phiên SSH:', e);
  } finally {
    connectingHostId.value = null;
  }
};

const handleGlobalClick = () => {
  activeMenuHostId.value = null;
};

// A fixed menu would stay put while its row scrolls away.
const closeMenuOnScroll = () => {
  activeMenuHostId.value = null;
};

const closeMenuOnEscape = (event: KeyboardEvent) => {
  if (event.key === 'Escape' && activeMenuHostId.value) activeMenuHostId.value = null;
};

onMounted(async () => {
  window.addEventListener('click', handleGlobalClick);
  window.addEventListener('scroll', closeMenuOnScroll, true);
  window.addEventListener('resize', closeMenuOnScroll);
  window.addEventListener('keydown', closeMenuOnEscape);
  await Promise.all([fetchHosts(), fetchKeychain()]);
});

onUnmounted(() => {
  window.removeEventListener('click', handleGlobalClick);
  window.removeEventListener('scroll', closeMenuOnScroll, true);
  window.removeEventListener('resize', closeMenuOnScroll);
  window.removeEventListener('keydown', closeMenuOnEscape);
});
</script>

<style scoped>
.ssh-servers-view {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-app-base);
  color: var(--text-primary);
  overflow: hidden;
}

.view-tabs {
  display: flex;
  gap: 2px;
  padding: 0 16px;
  background-color: var(--bg-sidebar);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.view-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 12px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
}

.view-tab:hover {
  color: var(--text-primary);
}

.view-tab.active {
  color: var(--text-primary);
  border-bottom-color: var(--primary);
}

.view-tab-count {
  font-size: 10.5px;
  padding: 0 6px;
  border-radius: 8px;
  background-color: var(--bg-surface-hover);
  color: var(--text-muted);
}

.servers-header {
  height: 52px;
  padding: 0 16px;
  background-color: var(--bg-sidebar);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  gap: 16px;
}

.header-left {
  display: flex;
  align-items: center;
}

.header-title-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.header-icon-box {
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm);
  background-color: var(--primary-subtle);
  border: 1px solid var(--border-focus);
  color: #e4b5ff;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-texts {
  display: flex;
  flex-direction: column;
}

.page-title {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  color: var(--text-primary);
  letter-spacing: -0.2px;
}

.page-subtitle {
  font-size: 11px;
  color: var(--text-muted);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.search-box {
  position: relative;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 10px;
  color: var(--text-muted);
  pointer-events: none;
}

.search-input {
  width: 280px;
  height: 32px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 0 28px 0 30px;
  font-size: 12px;
  color: var(--text-primary);
  outline: none;
  transition: all 0.15s ease;
}

.search-input:focus {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--primary-glow);
  width: 320px;
}

.search-clear-btn {
  position: absolute;
  right: 6px;
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 3px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
}

.tag-filter-bar {
  padding: 8px 16px;
  background-color: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  gap: 8px;
  overflow-x: auto;
  flex-shrink: 0;
}

.tag-filter-label {
  font-size: 11px;
  color: var(--text-muted);
  font-weight: 500;
}

.filter-tag-chip {
  height: 22px;
  padding: 0 8px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: 9999px;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.12s ease;
}

.filter-tag-chip:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.filter-tag-chip.active {
  background-color: var(--primary-container);
  border-color: var(--primary);
  color: #ffffff;
}

.servers-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.state-container {
  height: 240px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-muted);
  font-size: 13px;
}

.empty-state-card {
  max-width: 480px;
  margin: 60px auto;
  padding: 32px 24px;
  background-color: var(--bg-surface);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.empty-icon-wrapper {
  width: 64px;
  height: 64px;
  border-radius: 9999px;
  background-color: var(--primary-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #e4b5ff;
  margin-bottom: 16px;
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  margin: 0 0 8px 0;
  color: var(--text-primary);
}

.empty-desc {
  font-size: 12.5px;
  color: var(--text-muted);
  margin: 0 0 20px 0;
  line-height: 1.5;
}

.empty-actions {
  display: flex;
  gap: 10px;
}

.hosts-table-container {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.hosts-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.hosts-table th {
  padding: 8px 12px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-muted);
  letter-spacing: 0.5px;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-sidebar);
}

.host-row {
  border-bottom: 1px solid var(--border-subtle);
  transition: background-color 0.12s ease;
}

.host-row:last-child {
  border-bottom: none;
}

.host-row:hover {
  background-color: var(--bg-surface-hover);
}

.hosts-table td {
  padding: 10px 12px;
  vertical-align: middle;
}

.name-cell {
  display: flex;
  align-items: center;
  gap: 10px;
}

.host-color-pip {
  width: 8px;
  height: 8px;
  border-radius: 9999px;
  flex-shrink: 0;
}

.host-title-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.host-name {
  font-size: 13px;
  color: var(--text-primary);
}

.host-badges-row {
  display: flex;
  gap: 4px;
}

.badge {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 3px;
  font-weight: 500;
}

.badge-system {
  background-color: rgba(59, 130, 246, 0.15);
  border: 1px solid rgba(59, 130, 246, 0.3);
  color: #60a5fa;
}

.badge-agent {
  background-color: rgba(16, 185, 129, 0.15);
  border: 1px solid rgba(16, 185, 129, 0.3);
  color: #34d399;
}

.conn-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.conn-string {
  font-size: 12.5px;
}

.jump-pill {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-muted);
}

.auth-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}

.auth-method-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 3px;
  background-color: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.vault-badge {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 10.5px;
  padding: 2px 5px;
  border-radius: 3px;
  background-color: var(--primary-subtle);
  border: 1px solid var(--border-focus);
  color: #e4b5ff;
}

.tags-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.tag-pill {
  font-size: 10.5px;
  padding: 1px 6px;
  border-radius: 3px;
  background-color: rgba(255, 255, 255, 0.04);
  color: var(--text-muted);
  cursor: pointer;
}

.tag-pill:hover {
  color: var(--primary);
  background-color: var(--primary-subtle);
}

.status-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.live-status-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--status-running);
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background-color: var(--status-running);
  box-shadow: 0 0 6px var(--status-running);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.actions-wrapper {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.btn-connect {
  height: 28px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  background-color: var(--primary-container);
  border: 1px solid var(--border-focus);
  color: #ffffff;
  font-size: 11.5px;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  transition: all 0.12s ease;
}

.btn-connect:hover {
  background-color: var(--primary-hover);
  box-shadow: 0 0 8px var(--primary-glow);
}

.dropdown-container {
  position: relative;
}

.btn-icon-dots {
  width: 28px;
  height: 28px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.12s ease;
}

.btn-icon-dots:hover,
.btn-icon-dots.active {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-subtle);
  color: var(--text-primary);
}

.actions-menu-popup {
  position: fixed;
  z-index: 1000;
  width: 160px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.menu-item {
  width: 100%;
  height: 28px;
  padding: 0 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}

.menu-item:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.menu-item.text-warning {
  color: var(--status-warning);
}

.menu-item.text-danger {
  color: var(--status-failed);
}

.menu-item.text-danger:hover {
  background-color: rgba(239, 68, 68, 0.15);
}

.menu-divider {
  height: 1px;
  background-color: var(--border-subtle);
  margin: 2px 0;
}
</style>
