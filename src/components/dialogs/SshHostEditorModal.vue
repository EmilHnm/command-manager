<template>
  <div v-if="visible" class="modal-backdrop">
    <div
      class="ssh-editor-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="ssh-editor-title"
    >
      <div class="modal-accent-bar" :style="{ backgroundColor: form.color || '#744791' }" />

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge" :style="{ color: form.color || '#e4b5ff' }">
            <Server :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="ssh-editor-title" class="modal-title">
              {{ isEditing ? `Sửa Máy Chủ SSH: ${hostToEdit?.name || ''}` : 'Thêm Máy Chủ SSH Mới' }}
            </h3>
            <span class="modal-subtitle font-mono">
              {{ form.username ? `${form.username}@` : '' }}{{ form.host || 'host' }}:{{ form.port || 22 }}
            </span>
          </div>
        </div>
        <button class="btn-close" title="Hủy bỏ (Esc)" @click="$emit('cancel')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body Form -->
      <div class="modal-body custom-scrollbar">
        <!-- Error Banner if any -->
        <div v-if="formError" class="alert-banner alert-error" role="alert">
          <AlertCircle :size="14" class="alert-icon" />
          <span>{{ formError }}</span>
        </div>

        <!-- Connection Test Result Banner -->
        <div
          v-if="testResult"
          class="alert-banner"
          :class="testResult.ok ? 'alert-success' : 'alert-error'"
          role="status"
        >
          <CheckCircle2 v-if="testResult.ok" :size="14" class="alert-icon" />
          <AlertTriangle v-else :size="14" class="alert-icon" />
          <div class="alert-text">
            <strong>{{ testResult.ok ? 'Kết nối thành công!' : 'Kiểm tra kết nối thất bại:' }}</strong>
            <span v-if="testResult.error" class="block font-mono text-xs mt-1">{{ testResult.error }}</span>
          </div>
        </div>

        <!-- Basic Connection Info -->
        <div class="form-section">
          <div class="section-title">Thông Tin Kết Nối Cơ Bản</div>

          <div class="form-group">
            <label class="input-label" for="ssh-name">
              Tên Gợi Nhớ <span class="required-star">*</span>
            </label>
            <input
              id="ssh-name"
              v-model="form.name"
              type="text"
              class="form-input"
              placeholder="Ví dụ: Production API, Staging Web, DB Master..."
              required
            />
          </div>

          <div class="form-row grid-host-port">
            <div class="form-group">
              <label class="input-label" for="ssh-host">
                Địa Chỉ Máy Chủ (Host / IP) <span class="required-star">*</span>
              </label>
              <input
                id="ssh-host"
                v-model="form.host"
                type="text"
                class="form-input font-mono"
                placeholder="10.0.4.12 hoặc api.example.com"
                required
              />
            </div>
            <div class="form-group">
              <label class="input-label" for="ssh-port">Cổng (Port)</label>
              <input
                id="ssh-port"
                v-model.number="form.port"
                type="number"
                min="1"
                max="65535"
                class="form-input font-mono"
                placeholder="22"
              />
            </div>
          </div>

          <div class="form-group">
            <label class="input-label" for="ssh-username">
              Tên Đăng Nhập (Username) <span v-if="!selectedIdentity" class="required-star">*</span>
              <span v-else class="text-muted font-normal">(lấy từ identity "{{ selectedIdentity.name }}")</span>
            </label>
            <input
              v-if="selectedIdentity"
              id="ssh-username"
              :value="selectedIdentity.username"
              type="text"
              class="form-input font-mono"
              disabled
            />
            <input
              v-else
              id="ssh-username"
              v-model="form.username"
              type="text"
              class="form-input font-mono"
              placeholder="root, ubuntu, deploy..."
              required
            />
          </div>
        </div>

        <!-- Authentication Method -->
        <div class="form-section">
          <div class="section-title">Xác Thực & Bảo Mật</div>

          <div class="form-group">
            <label class="input-label" for="ssh-identity">Identity (Keychain)</label>
            <div class="select-with-button">
              <select id="ssh-identity" v-model="form.identity_id" class="form-select">
                <option :value="null">Không dùng, nhập trực tiếp</option>
                <option v-for="identity in identities" :key="identity.id" :value="identity.id">
                  {{ identity.name }} ({{ identity.username }})
                </option>
              </select>
              <button
                type="button"
                class="btn btn-ghost btn-sm"
                title="Tạo identity mới trong Keychain"
                @click="showIdentityModal = true"
              >
                <Plus :size="12" />
                <span>Identity</span>
              </button>
            </div>
            <span v-if="selectedIdentity" class="field-hint">
              Tên đăng nhập và mật khẩu lấy từ identity{{
                identityKeyName ? `, khóa "${identityKeyName}" cũng vậy trừ khi bạn chọn khóa khác bên dưới` : ''
              }}.
            </span>
          </div>

          <div class="form-group">
            <label class="input-label">Phương Thức Xác Thực</label>
            <div class="auth-method-tabs">
              <button
                type="button"
                class="method-tab"
                :class="{ active: form.auth_method === 'auto' }"
                @click="form.auth_method = 'auto'"
              >
                Tự động
              </button>
              <button
                type="button"
                class="method-tab"
                :class="{ active: form.auth_method === 'key' }"
                @click="form.auth_method = 'key'"
              >
                Khóa Private
              </button>
              <button
                type="button"
                class="method-tab"
                :class="{ active: form.auth_method === 'password' }"
                @click="form.auth_method = 'password'"
              >
                Mật Khẩu
              </button>
              <button
                type="button"
                class="method-tab"
                :class="{ active: form.auth_method === 'agent' }"
                @click="form.auth_method = 'agent'"
              >
                SSH Agent
              </button>
            </div>
          </div>

          <!-- Private key: a file on disk or the key text pasted here -->
          <div v-if="usesKey" class="form-group">
            <label class="input-label">Khóa Riêng (Private Key)</label>
            <div class="auth-method-tabs key-source-tabs">
              <button
                type="button"
                class="method-tab"
                :class="{ active: keyMode === 'keychain' }"
                :disabled="form.use_system_ssh"
                :title="form.use_system_ssh ? 'OpenSSH hệ thống chỉ dùng được tệp khóa' : undefined"
                @click="keyMode = 'keychain'"
              >
                Từ Keychain
              </button>
              <button
                type="button"
                class="method-tab"
                :class="{ active: keyMode === 'file' }"
                @click="keyMode = 'file'"
              >
                Tệp khóa
              </button>
              <button
                type="button"
                class="method-tab"
                :class="{ active: keyMode === 'content' }"
                :disabled="form.use_system_ssh"
                :title="form.use_system_ssh ? 'OpenSSH hệ thống chỉ dùng được tệp khóa' : undefined"
                @click="keyMode = 'content'"
              >
                Dán nội dung khóa
              </button>
            </div>
          </div>

          <div v-if="usesKey && keyMode === 'keychain'" class="form-group">
            <label class="input-label" for="ssh-key-id">Khóa Trong Keychain</label>
            <div class="select-with-button">
              <select id="ssh-key-id" v-model="selectedKeyId" class="form-select">
                <option :value="null">
                  {{ identityKeyName ? `Theo identity (${identityKeyName})` : '— Chọn khóa —' }}
                </option>
                <option v-for="key in keychainKeys" :key="key.id" :value="key.id">
                  {{ key.name }} ({{ key.fingerprint.slice(0, 19) }}…)
                </option>
              </select>
              <button
                type="button"
                class="btn btn-ghost btn-sm"
                title="Thêm khóa mới vào Keychain"
                @click="showKeyModal = true"
              >
                <Plus :size="12" />
                <span>Khóa</span>
              </button>
            </div>
            <span class="field-hint">
              Khóa và passphrase của nó được lưu trong Keychain, dùng chung cho mọi máy chủ chọn khóa này.
            </span>
          </div>

          <div v-if="usesKey && keyMode === 'file'" class="form-group">
            <label class="input-label" for="ssh-key-path">Đường Dẫn Tệp Khóa</label>
            <input
              id="ssh-key-path"
              v-model="form.key_path"
              type="text"
              class="form-input font-mono"
              placeholder="~/.ssh/id_ed25519 hoặc C:\Users\...\.ssh\id_rsa"
            />
            <span class="field-hint">Để trống sẽ tự động tìm các khóa mặc định trong thư mục ~/.ssh/</span>
          </div>

          <div v-if="usesKeyContent" class="form-group">
            <label class="input-label" for="ssh-private-key">
              Nội Dung Khóa Riêng
              <span v-if="hasSavedKeyContent" class="text-muted font-normal">
                (Để trống nếu giữ nguyên khóa đã lưu)
              </span>
            </label>
            <textarea
              id="ssh-private-key"
              v-model="privateKeyInput"
              class="form-input form-textarea font-mono"
              rows="6"
              spellcheck="false"
              autocomplete="off"
              :placeholder="hasSavedKeyContent ? '•••••••• (đã lưu)' : '-----BEGIN OPENSSH PRIVATE KEY-----\n...\n-----END OPENSSH PRIVATE KEY-----'"
            />
            <span class="field-hint">
              Hỗ trợ khóa OpenSSH / PEM (ed25519, ECDSA, RSA). Khóa PuTTY (.ppk) cần xuất sang OpenSSH bằng PuTTYgen.
            </span>
          </div>

          <!-- Password (an identity brings its own) -->
          <div v-if="usesPassword && !selectedIdentity" class="form-group">
            <label class="input-label" for="ssh-password">
              Mật Khẩu Đăng Nhập
              <span v-if="hasSavedSecrets" class="text-muted font-normal">
                (Để trống nếu giữ nguyên mật khẩu đã lưu)
              </span>
            </label>
            <div class="password-input-wrapper">
              <input
                id="ssh-password"
                v-model="passwordInput"
                :type="showPassword ? 'text' : 'password'"
                class="form-input font-mono"
                placeholder="Nhập mật khẩu SSH..."
                autocomplete="new-password"
              />
              <button
                type="button"
                class="btn-toggle-eye"
                title="Hiện/Ẩn mật khẩu"
                @click="showPassword = !showPassword"
              >
                <EyeOff v-if="showPassword" :size="13" />
                <Eye v-else :size="13" />
              </button>
            </div>
          </div>

          <!-- Key passphrase (a Keychain key keeps its own) -->
          <div v-if="usesKey && keyMode !== 'keychain'" class="form-group">
            <label class="input-label" for="ssh-passphrase">
              Passphrase Của Khóa (Nếu có)
              <span v-if="hasSavedSecrets" class="text-muted font-normal">
                (Để trống nếu giữ nguyên passphrase)
              </span>
            </label>
            <div class="password-input-wrapper">
              <input
                id="ssh-passphrase"
                v-model="passphraseInput"
                :type="showPassphrase ? 'text' : 'password'"
                class="form-input font-mono"
                placeholder="Nhập passphrase..."
                autocomplete="new-password"
              />
              <button
                type="button"
                class="btn-toggle-eye"
                title="Hiện/Ẩn passphrase"
                @click="showPassphrase = !showPassphrase"
              >
                <EyeOff v-if="showPassphrase" :size="13" />
                <Eye v-else :size="13" />
              </button>
            </div>
          </div>

          <!-- Where secrets are kept -->
          <div v-if="hasHostSecrets" class="secret-box">
            <label class="checkbox-row" :class="{ disabled: usesKeyContent }">
              <input
                v-model="rememberSecrets"
                type="checkbox"
                class="form-checkbox"
                :disabled="usesKeyContent"
              />
              <span class="checkbox-label">Lưu mật khẩu / passphrase / khóa trên máy này</span>
            </label>

            <div v-if="rememberSecrets" class="form-group mt-2">
              <label class="input-label text-xs" for="ssh-secret-store">Nơi Lưu Trữ An Toàn</label>
              <select id="ssh-secret-store" v-model="secretStore" class="form-select text-xs">
                <option value="keyring" :disabled="!keyringAvailable">
                  Keyring Hệ Điều Hành{{ keyringAvailable ? ' (Khuyến nghị)' : ' (không có trên máy này)' }}
                </option>
                <option value="sqlite">SQLite Cục Bộ (Mã hóa AES-256-GCM)</option>
              </select>
              <span v-if="secretStore === 'sqlite'" class="field-hint">
                Mã hóa bằng khóa riêng của máy này, không theo bản sao lưu sang máy khác.
              </span>
            </div>
            <span v-if="usesKeyContent" class="field-hint mt-1">
              Nội dung khóa dán vào bắt buộc phải được lưu, vì app không còn nơi nào khác để đọc lại.
            </span>
            <span v-else-if="!rememberSecrets" class="field-hint mt-1">
              Không lưu: mật khẩu / passphrase nhập ở trên chỉ dùng cho "Kiểm tra kết nối"; khi kết nối, app sẽ hỏi qua hộp thoại.
            </span>
          </div>
        </div>

        <!-- Advanced Collapsible Section -->
        <div class="advanced-section">
          <button
            type="button"
            class="advanced-toggle-btn"
            @click="showAdvanced = !showAdvanced"
          >
            <ChevronRight :size="14" class="toggle-icon" :class="{ open: showAdvanced }" />
            <span>Cài Đặt Nâng Cao & Tùy Biến</span>
          </button>

          <div v-if="showAdvanced" class="advanced-body">
            <!-- Jump Host (ProxyJump) -->
            <div class="form-group">
              <label class="input-label" for="ssh-jump-host">
                Jump Host (ProxyJump / Bastion)
              </label>
              <select
                id="ssh-jump-host"
                v-model="form.jump_host_id"
                class="form-select font-mono"
              >
                <option :value="null">Không dùng (Kết nối trực tiếp)</option>
                <option
                  v-for="candidate in availableJumpHosts"
                  :key="candidate.id"
                  :value="candidate.id"
                >
                  {{ candidate.name }} ({{ candidate.username }}@{{ candidate.host }}:{{ candidate.port }})
                </option>
              </select>
              <span class="field-hint">Kết nối trung gian một bước qua máy chủ nhảy</span>
            </div>

            <!-- System OpenSSH Escape Hatch -->
            <div class="form-group checkbox-block">
              <label class="checkbox-row">
                <input v-model="form.use_system_ssh" type="checkbox" class="form-checkbox" />
                <span class="checkbox-label">Dùng OpenSSH Hệ Thống</span>
              </label>
              <span class="field-hint">
                Chạy trực tiếp binary <code>ssh</code> của hệ điều hành. Hỗ trợ ProxyCommand, FIDO token, nhưng không hỗ trợ SFTP Files pane.
              </span>
            </div>

            <!-- Agent Forwarding -->
            <div class="form-group checkbox-block">
              <label class="checkbox-row">
                <input v-model="form.agent_forwarding" type="checkbox" class="form-checkbox" />
                <span class="checkbox-label">Chuyển Tiếp SSH Agent (-A)</span>
              </label>
            </div>

            <!-- Default Remote Dir -->
            <div class="form-group">
              <label class="input-label" for="ssh-default-dir">Thư Mục Mặc Định Từ Xa</label>
              <input
                id="ssh-default-dir"
                v-model="form.default_dir"
                type="text"
                class="form-input font-mono"
                placeholder="Ví dụ: /var/www/my-project hoặc ~/workspace"
              />
            </div>

            <!-- Color Swatch -->
            <div class="form-group">
              <label class="input-label">Màu Đại Diện</label>
              <div class="color-palette">
                <button
                  v-for="c in COLOR_PALETTE"
                  :key="c"
                  type="button"
                  class="color-btn"
                  :class="{ selected: form.color === c }"
                  :style="{ backgroundColor: c }"
                  :title="c"
                  @click="form.color = form.color === c ? null : c"
                />
              </div>
            </div>

            <!-- Tags -->
            <div class="form-group">
              <label class="input-label" for="ssh-tags">Thẻ Phân Loại (Tags)</label>
              <input
                id="ssh-tags"
                v-model="tagsInput"
                type="text"
                class="form-input"
                placeholder="prod, aws, database (phân cách bằng dấu phẩy)"
              />
            </div>

            <!-- Management actions when editing -->
            <div v-if="isEditing && hostToEdit" class="host-forget-actions mt-3">
              <div class="actions-label">Quản lý Khóa & Bí Mật:</div>
              <div class="actions-row">
                <button
                  type="button"
                  class="btn btn-ghost btn-sm text-danger"
                  title="Xóa khóa đã lưu trong known_hosts của app để xác minh lại"
                  :disabled="forgettingHostKey"
                  @click="handleForgetHostKey"
                >
                  <KeyRound :size="12" />
                  <span>Quên Host Key Đã Lưu</span>
                </button>
                <button
                  v-if="hostToEdit.secret_store !== 'none'"
                  type="button"
                  class="btn btn-ghost btn-sm text-danger"
                  title="Xóa mật khẩu/passphrase đã lưu khỏi vault"
                  :disabled="forgettingSecret"
                  @click="handleForgetSecret"
                >
                  <Trash2 :size="12" />
                  <span>Quên Mật Khẩu Đã Lưu</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Buttons -->
      <div class="modal-footer">
        <button
          type="button"
          class="btn btn-secondary btn-test"
          :disabled="testingConnection || !canTest"
          @click="handleTestConnection"
        >
          <LoaderCircle v-if="testingConnection" :size="13" class="spin" />
          <Wifi v-else :size="13" />
          <span>{{ testingConnection ? 'Đang Kiểm Tra...' : 'Kiểm Tra Kết Nối' }}</span>
        </button>

        <div class="footer-right">
          <button type="button" class="btn btn-ghost" @click="$emit('cancel')">
            Hủy Bỏ
          </button>
          <button
            type="button"
            class="btn btn-primary btn-save"
            :disabled="saving || !isFormValid"
            @click="handleSave"
          >
            <LoaderCircle v-if="saving" :size="13" class="spin" />
            <Save v-else :size="13" />
            <span>{{ isEditing ? 'Lưu Thay Đổi' : 'Thêm Máy Chủ' }}</span>
          </button>
        </div>
      </div>
    </div>
    <SshKeyModal :visible="showKeyModal" @close="showKeyModal = false" @saved="onKeyCreated" />
    <SshIdentityModal
      :visible="showIdentityModal"
      @close="showIdentityModal = false"
      @saved="onIdentityCreated"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import {
  Server,
  X,
  Eye,
  EyeOff,
  ChevronRight,
  Wifi,
  Save,
  LoaderCircle,
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  KeyRound,
  Trash2,
  Plus,
} from 'lucide-vue-next';
import type {
  SshIdentity,
  SshKey,
  SshHost,
  SshHostInput,
  SshHostView,
  SecretsInput,
  SshSecretStore,
  SshTestReport,
} from '@/types/ssh';
import { useSshHosts } from '@/composables/useSshHosts';
import { useKeychain } from '@/composables/useKeychain';
import SshKeyModal from '@/components/keychain/SshKeyModal.vue';
import SshIdentityModal from '@/components/keychain/SshIdentityModal.vue';
import { ipcClient } from '@/ipc/client';

const props = defineProps<{
  visible: boolean;
  hostToEdit?: SshHostView | null;
  existingHosts?: SshHostView[];
}>();

const emit = defineEmits<{
  (e: 'cancel'): void;
  (e: 'saved', host: SshHost): void;
}>();

const { createHost, updateHost, testConnection, forgetHostKey, forgetSecret } = useSshHosts();

const COLOR_PALETTE = [
  '#744791', // Brand Purple
  '#3b82f6', // Blue
  '#10b981', // Emerald
  '#f59e0b', // Amber
  '#ef4444', // Red
  '#06b6d4', // Cyan
  '#ec4899', // Pink
  '#64748b', // Slate
];

const isEditing = computed(() => Boolean(props.hostToEdit?.id));

const form = ref<SshHostInput>({
  name: '',
  host: '',
  port: 22,
  username: '',
  auth_method: 'auto',
  key_path: null,
  key_source: 'file',
  jump_host_id: null,
  use_system_ssh: false,
  agent_forwarding: false,
  default_dir: null,
  color: null,
  tags: [],
});

const tagsInput = ref('');
const rememberSecrets = ref(false);
const secretStore = ref<SshSecretStore>('keyring');
const passwordInput = ref('');
const passphraseInput = ref('');
const privateKeyInput = ref('');

const { keys: keychainKeys, identities, keyById, identityById, fetchKeychain } = useKeychain();
const showKeyModal = ref(false);
const showIdentityModal = ref(false);
/** Where the key comes from; 'keychain' maps to form.key_id. */
const keyMode = ref<'keychain' | 'file' | 'content'>('file');
const selectedKeyId = ref<string | null>(null);

const selectedIdentity = computed(() =>
  form.value.identity_id ? identityById.value.get(form.value.identity_id) ?? null : null
);
const identityKeyName = computed(() => {
  const keyId = selectedIdentity.value?.key_id;
  return keyId ? keyById.value.get(keyId)?.name ?? null : null;
});

const onKeyCreated = (key: SshKey) => {
  keyMode.value = 'keychain';
  selectedKeyId.value = key.id;
};

const onIdentityCreated = (identity: SshIdentity) => {
  form.value.identity_id = identity.id;
};
const keyringAvailable = ref(true);
const showPassword = ref(false);
const showPassphrase = ref(false);
const showAdvanced = ref(false);

const saving = ref(false);
const testingConnection = ref(false);
const forgettingHostKey = ref(false);
const forgettingSecret = ref(false);
const formError = ref<string | null>(null);
const testResult = ref<SshTestReport | null>(null);

const availableJumpHosts = computed(() => {
  return (props.existingHosts || []).filter(
    (h) => h.id !== props.hostToEdit?.id && !h.jump_host_id
  );
});

const usesKey = computed(() => form.value.auth_method === 'key' || form.value.auth_method === 'auto');
const usesKeyContent = computed(() => usesKey.value && keyMode.value === 'content');
/** Secrets this form saves on the host itself (not in the Keychain). */
const hasHostSecrets = computed(
  () =>
    (usesPassword.value && !selectedIdentity.value) ||
    (usesKey.value && keyMode.value !== 'keychain')
);
const usesPassword = computed(
  () => form.value.auth_method === 'password' || form.value.auth_method === 'auto'
);
const hasSavedSecrets = computed(
  () => isEditing.value && (props.hostToEdit?.secret_store ?? 'none') !== 'none'
);
const hasSavedKeyContent = computed(
  () => hasSavedSecrets.value && props.hostToEdit?.key_source === 'content'
);

// A pasted key lives only in the vault, so saving is not optional for it.
watch(usesKeyContent, (content) => {
  if (content) rememberSecrets.value = true;
});
watch(
  () => form.value.use_system_ssh,
  (system) => {
    if (system && keyMode.value !== 'file') keyMode.value = 'file';
  }
);

const isFormValid = computed(() => {
  if (usesKeyContent.value && !hasSavedKeyContent.value && !privateKeyInput.value.trim()) {
    return false;
  }
  if (usesKey.value && keyMode.value === 'keychain' && !selectedKeyId.value && !identityKeyName.value) {
    return false;
  }
  return (
    form.value.name.trim().length > 0 &&
    form.value.host.trim().length > 0 &&
    (form.value.username.trim().length > 0 || Boolean(selectedIdentity.value)) &&
    form.value.port &&
    form.value.port > 0 &&
    form.value.port <= 65535
  );
});

const canTest = computed(() => {
  return !form.value.use_system_ssh && isFormValid.value;
});

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    formError.value = null;
    testResult.value = null;
    testingConnection.value = false;
    saving.value = false;

    if (props.hostToEdit) {
      const h = props.hostToEdit;
      form.value = {
        name: h.name,
        host: h.host,
        port: h.port,
        username: h.username,
        auth_method: h.auth_method,
        key_path: h.key_path || null,
        key_source: h.key_source || 'file',
        identity_id: h.identity_id || null,
        jump_host_id: h.jump_host_id || null,
        use_system_ssh: h.use_system_ssh,
        agent_forwarding: h.agent_forwarding,
        default_dir: h.default_dir || null,
        color: h.color || null,
        tags: [...(h.tags || [])],
      };
      tagsInput.value = (h.tags || []).join(', ');
      rememberSecrets.value = h.secret_store !== 'none';
      secretStore.value = h.secret_store !== 'none' ? h.secret_store : 'keyring';
      passwordInput.value = '';
      passphraseInput.value = '';
      privateKeyInput.value = '';
      selectedKeyId.value = h.key_id || null;
      keyMode.value = h.key_id ? 'keychain' : h.key_source === 'content' ? 'content' : 'file';
    } else {
      form.value = {
        name: '',
        host: '',
        port: 22,
        username: '',
        auth_method: 'auto',
        key_path: null,
        key_source: 'file',
        identity_id: null,
        jump_host_id: null,
        use_system_ssh: false,
        agent_forwarding: false,
        default_dir: null,
        color: '#744791',
        tags: [],
      };
      tagsInput.value = '';
      rememberSecrets.value = false;
      secretStore.value = 'keyring';
      passwordInput.value = '';
      passphraseInput.value = '';
      privateKeyInput.value = '';
      selectedKeyId.value = null;
      keyMode.value = 'file';
    }
    void fetchKeychain();
    void ipcClient
      .sshSecretStatus()
      .then(({ keyring_available }) => {
        keyringAvailable.value = keyring_available;
        if (!keyring_available && secretStore.value === 'keyring') secretStore.value = 'sqlite';
      })
      .catch(() => {
        keyringAvailable.value = false;
      });
  },
  { immediate: true }
);

/** The form as the backend expects it, with the key choice mapped back. */
const buildHostInput = (): SshHostInput => {
  const keychain = usesKey.value && keyMode.value === 'keychain';
  return {
    ...form.value,
    port: Number(form.value.port) || 22,
    key_path: form.value.key_path?.trim() || null,
    key_source: usesKey.value && keyMode.value === 'content' ? 'content' : 'file',
    key_id: keychain ? selectedKeyId.value : null,
    default_dir: form.value.default_dir?.trim() || null,
  };
};

const handleTestConnection = async () => {
  if (!canTest.value) return;
  testingConnection.value = true;
  testResult.value = null;
  formError.value = null;
  try {
    const report = await testConnection(
      buildHostInput(),
      props.hostToEdit?.id,
      selectedIdentity.value ? undefined : passwordInput.value || undefined,
      passphraseInput.value || undefined,
      usesKeyContent.value ? privateKeyInput.value.trim() || undefined : undefined
    );
    testResult.value = report;
  } catch (e) {
    testResult.value = {
      ok: false,
      error: e instanceof Error ? e.message : String(e),
    };
  } finally {
    testingConnection.value = false;
  }
};

const handleForgetHostKey = async () => {
  if (!props.hostToEdit?.id) return;
  forgettingHostKey.value = true;
  try {
    await forgetHostKey(props.hostToEdit.id);
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e);
  } finally {
    forgettingHostKey.value = false;
  }
};

const handleForgetSecret = async () => {
  if (!props.hostToEdit?.id) return;
  forgettingSecret.value = true;
  try {
    await forgetSecret(props.hostToEdit.id);
    rememberSecrets.value = false;
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e);
  } finally {
    forgettingSecret.value = false;
  }
};

const handleSave = async () => {
  if (!isFormValid.value) return;
  saving.value = true;
  formError.value = null;

  try {
    const parsedTags = tagsInput.value
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean);

    const hostInput: SshHostInput = { ...buildHostInput(), tags: parsedTags };

    let secretsInput: SecretsInput | undefined;
    const keyText = privateKeyInput.value.trim();
    if (rememberSecrets.value) {
      secretsInput = {
        store: secretStore.value,
        password: passwordInput.value ? { action: 'set', value: passwordInput.value } : { action: 'keep' },
        passphrase: passphraseInput.value ? { action: 'set', value: passphraseInput.value } : { action: 'keep' },
        private_key: !usesKeyContent.value
          ? { action: 'clear' }
          : keyText
            ? { action: 'set', value: keyText + '\n' }
            : { action: 'keep' },
      };
    } else if (hasSavedSecrets.value) {
      secretsInput = {
        store: 'none',
        password: { action: 'clear' },
        passphrase: { action: 'clear' },
        private_key: { action: 'clear' },
      };
    }

    let savedHost: SshHost;
    if (isEditing.value && props.hostToEdit?.id) {
      savedHost = await updateHost(props.hostToEdit.id, hostInput, secretsInput);
    } else {
      savedHost = await createHost(hostInput, secretsInput);
    }

    emit('saved', savedHost);
    emit('cancel');
  } catch (e) {
    formError.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
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

.ssh-editor-modal {
  width: 100%;
  max-width: 580px;
  max-height: 90vh;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
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
  flex-shrink: 0;
}

.modal-header {
  height: 48px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-sidebar);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.icon-badge {
  display: flex;
  align-items: center;
  justify-content: center;
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
  gap: 16px;
}

.alert-banner {
  padding: 10px 12px;
  border-radius: var(--radius-md);
  font-size: 12px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.alert-error {
  background-color: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.3);
  color: var(--status-failed);
}

.alert-success {
  background-color: rgba(16, 185, 129, 0.12);
  border: 1px solid rgba(16, 185, 129, 0.3);
  color: var(--status-running);
}

.alert-icon {
  flex-shrink: 0;
  margin-top: 1px;
}

.form-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.section-title {
  font-size: 11.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  padding-bottom: 4px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.form-row {
  display: grid;
  gap: 12px;
}

.grid-host-port {
  grid-template-columns: 1fr 100px;
}

.grid-secret-store {
  grid-template-columns: 1fr;
}

.input-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}

.required-star {
  color: var(--status-failed);
}

.form-input, .form-select {
  height: 32px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  padding: 0 10px;
  font-size: 12.5px;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.form-input:focus, .form-select:focus {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.form-input::placeholder {
  color: var(--text-muted);
}

.field-hint {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.3;
}

.field-hint code {
  background-color: rgba(255, 255, 255, 0.06);
  padding: 1px 4px;
  border-radius: 3px;
  font-family: var(--font-mono);
}

.auth-method-tabs {
  display: flex;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 2px;
}

.method-tab {
  flex: 1;
  height: 28px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.12s ease;
}

.method-tab:hover {
  color: var(--text-primary);
  background-color: rgba(255, 255, 255, 0.04);
}

.method-tab.active {
  background-color: var(--primary-container);
  color: #ffffff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}

.secret-box {
  background-color: rgba(0, 0, 0, 0.2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  flex-direction: column;
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
  font-size: 12.5px;
  font-weight: 500;
  color: var(--text-primary);
}

.key-source-tabs .method-tab:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.form-textarea {
  height: auto;
  min-height: 110px;
  padding: 8px 10px;
  line-height: 1.45;
  font-size: 11.5px;
  resize: vertical;
  white-space: pre;
  overflow-x: auto;
}

.checkbox-row.disabled {
  cursor: default;
  opacity: 0.75;
}

.select-with-button {
  display: flex;
  gap: 6px;
}

.select-with-button .form-select {
  flex: 1;
  min-width: 0;
}

.password-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.password-input-wrapper .form-input {
  width: 100%;
  padding-right: 32px;
}

.btn-toggle-eye {
  position: absolute;
  right: 6px;
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 4px;
  border-radius: var(--radius-sm);
}

.btn-toggle-eye:hover {
  color: var(--text-primary);
}

.advanced-section {
  border-top: 1px solid var(--border-subtle);
  padding-top: 10px;
}

.advanced-toggle-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  padding: 4px 0;
}

.advanced-toggle-btn:hover {
  color: var(--text-primary);
}

.toggle-icon {
  transition: transform 0.15s ease;
}

.toggle-icon.open {
  transform: rotate(90deg);
}

.advanced-body {
  margin-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-left: 8px;
  border-left: 2px solid rgba(116, 71, 145, 0.3);
}

.color-palette {
  display: flex;
  gap: 8px;
}

.color-btn {
  width: 22px;
  height: 22px;
  border-radius: 9999px;
  border: 2px solid transparent;
  cursor: pointer;
  transition: transform 0.1s ease, border-color 0.1s ease;
}

.color-btn:hover {
  transform: scale(1.15);
}

.color-btn.selected {
  border-color: #ffffff;
  box-shadow: 0 0 6px rgba(255, 255, 255, 0.6);
}

.host-forget-actions {
  padding-top: 10px;
  border-top: 1px dashed rgba(255, 255, 255, 0.1);
}

.actions-label {
  font-size: 11px;
  color: var(--text-muted);
  margin-bottom: 6px;
}

.actions-row {
  display: flex;
  gap: 8px;
}

.text-danger {
  color: var(--status-failed) !important;
}

.modal-footer {
  height: 52px;
  padding: 0 16px;
  background-color: var(--bg-sidebar);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.footer-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-test {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-save {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
