<template>
  <div v-if="visible" class="modal-backdrop">
    <div class="modal-content editor-modal">
      <div class="modal-header">
        <div class="modal-title">
          <Zap class="text-primary" :size="17" />
          <span>{{ form.id ? 'Chỉnh Sửa Câu Lệnh' : 'Tạo Câu Lệnh Mới' }}</span>
        </div>
        <button class="btn btn-ghost btn-icon" @click="$emit('close')">
          <X :size="15" />
        </button>
      </div>

      <div class="modal-body">
        <!-- Validation Error Banner -->
        <div v-if="wasSubmitted && hasErrors" class="validation-banner" role="alert">
          <AlertCircle :size="14" />
          <span>Vui lòng điền đầy đủ các thông tin bắt buộc bên dưới.</span>
        </div>

        <!-- Name Field -->
        <div class="form-group">
          <label class="form-label">Tên gợi nhớ của lệnh (*)</label>
          <input
            v-model="form.name"
            class="input"
            :class="{ 'has-error': isFieldInvalid('name') }"
            placeholder="Ví dụ: Vite Frontend Dev, Docker PostgreSQL..."
            @blur="touched.name = true"
            @input="touched.name = true"
          />
          <span v-if="isFieldInvalid('name')" class="field-error-msg">
            <AlertCircle :size="12" />
            <span>{{ errors.name }}</span>
          </span>
        </div>

        <!-- Where it may run: this machine and/or SSH hosts (SCR-07) -->
        <div class="form-group">
          <label class="form-label">Nơi chạy (*)</label>
          <div class="target-list">
            <label class="target-item" :class="{ selected: hasTarget(LOCAL_TARGET) }">
              <input type="checkbox" :checked="hasTarget(LOCAL_TARGET)" @change="toggleTarget(LOCAL_TARGET)" />
              <span class="target-name">💻 Máy này</span>
            </label>
            <label
              v-for="host in sshHosts"
              :key="host.id"
              class="target-item"
              :class="{ selected: hasTarget(host.id) }"
            >
              <input type="checkbox" :checked="hasTarget(host.id)" @change="toggleTarget(host.id)" />
              <span class="target-dot" :style="{ backgroundColor: host.color || '#744791' }" />
              <span class="target-name">{{ host.name }}</span>
              <span class="target-addr">{{ host.username }}@{{ host.host }}{{ host.port !== 22 ? `:${host.port}` : '' }}</span>
            </label>
          </div>
          <span v-if="isFieldInvalid('targets')" class="field-error-msg">
            <AlertCircle :size="12" />
            <span>{{ errors.targets }}</span>
          </span>
          <span v-else-if="selectedTargets.length > 1" class="field-hint">
            Khi chạy, app hỏi chạy ở đâu. Trên máy chủ, lệnh chạy qua shell đăng nhập của máy chủ (như
            <code>ssh host 'lệnh'</code>), trong thư mục mặc định của máy chủ nếu có.
          </span>
          <span v-else-if="selectedTargets[0] !== LOCAL_TARGET" class="field-hint">
            Lệnh chạy qua shell đăng nhập của máy chủ (như <code>ssh host 'lệnh'</code>); pipe, biến môi trường
            và <code>&amp;&amp;</code> dùng được.
          </span>
        </div>

        <!-- is_shell Execution Mode (only matters on this machine) -->
        <div v-if="hasTarget(LOCAL_TARGET)" class="form-group">
          <label class="form-label">Phương thức thực thi (*)</label>
          <div class="mode-selector">
            <label class="mode-card" :class="{ selected: !form.is_shell }">
              <input v-model="form.is_shell" type="radio" :value="false" />
              <div class="mode-text">
                <span class="mode-title">Chạy trực tiếp (Direct Argv) - Khuyên dùng</span>
                <span class="mode-desc">Phân tích mảng đối số tường minh, an toàn, không chạy qua trung gian shell.</span>
              </div>
            </label>

            <label class="mode-card" :class="{ selected: form.is_shell }">
              <input v-model="form.is_shell" type="radio" :value="true" />
              <div class="mode-text">
                <span class="mode-title">Chạy qua vỏ lệnh (Shell: bash/sh/cmd)</span>
                <span class="mode-desc">Hỗ trợ piping (|), chuyển hướng (>), và biến môi trường ($VAR).</span>
              </div>
            </label>
          </div>
        </div>

        <!-- Execution String -->
        <div class="form-group">
          <div class="label-row">
            <label class="form-label">Chuỗi dòng lệnh thực thi (Execution String) (*)</label>
            <span class="code-badge">{{ form.is_shell ? 'Shell Script' : 'Argv Array' }}</span>
          </div>
          <textarea
            v-model="form.execution_string"
            rows="4"
            class="textarea font-mono"
            :class="{ 'has-error': isFieldInvalid('execution_string') }"
            placeholder="Ví dụ: pnpm --filter web dev --port 3000"
            @blur="touched.execution_string = true"
            @input="touched.execution_string = true"
          />
          <div v-if="editorSuggestions.length" class="editor-suggestions" role="listbox" aria-label="Gợi ý câu lệnh">
            <button
              v-for="suggestion in editorSuggestions"
              :key="suggestion.id"
              type="button"
              class="editor-suggestion"
              @mousedown.prevent="applySuggestion(suggestion.command_line)"
            >
              <span>{{ suggestion.command_line }}</span>
              <small>{{ suggestion.source }} · {{ suggestion.run_count }}×</small>
            </button>
          </div>
          <span v-if="isFieldInvalid('execution_string')" class="field-error-msg">
            <AlertCircle :size="12" />
            <span>{{ errors.execution_string }}</span>
          </span>
        </div>

        <!-- Quick Access Toggle Card (SCR-01 Integration) -->
        <div class="form-group">
          <div class="quick-access-card" :class="{ active: form.quick_access }">
            <div class="quick-access-info">
              <div class="quick-access-header">
                <Zap :size="15" class="quick-access-icon" />
                <span class="quick-access-title">Hiển thị trong Quick Access (SCR-01)</span>
              </div>
              <span class="quick-access-desc">
                Ghim lệnh này vào thanh tác vụ nhanh bên dưới danh sách nhóm. Cho phép gửi trực tiếp vào Terminal đang active bằng nút Run [▷] hoặc Paste [⎘].
              </span>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="Boolean(form.quick_access)"
              class="quick-toggle"
              :class="{ active: Boolean(form.quick_access) }"
              @click="form.quick_access = !form.quick_access"
            >
              <span class="toggle-thumb" />
            </button>
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" :disabled="loading" @click="!loading && $emit('close')">
          Hủy bỏ
        </button>
        <button
          class="btn btn-primary"
          :disabled="loading"
          @click="handleSave"
        >
          <LoaderCircle v-if="loading" :size="14" class="spin" />
          <Save v-else :size="14" />
          <span>{{ loading ? loadingText : (form.id ? 'Cập Nhật Lệnh' : 'Lưu Câu Lệnh') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { Zap, Save, X, AlertCircle, LoaderCircle } from 'lucide-vue-next';
import type { CommandDefinition } from '@/types/models';
import { LOCAL_TARGET, allowedTargets } from '@/types/models';
import { useSshHosts } from '@/composables/useSshHosts';
import { useSuggestions } from '@/composables/useSuggestions';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    command?: CommandDefinition | null;
    loading?: boolean;
    loadingText?: string;
  }>(),
  {
    command: null,
    loading: false,
    loadingText: 'Đang Lưu...',
  }
);

const emit = defineEmits<{
  (e: 'save', command: Partial<CommandDefinition>): void;
  (e: 'close'): void;
}>();

const form = ref<Partial<CommandDefinition>>({
  name: '',
  execution_string: '',
  is_shell: false,
  quick_access: false,
});

const { hosts: sshHosts, fetchHosts: fetchSshHosts } = useSshHosts();
// Stored as given; no entries means this machine only.
const selectedTargets = computed(() => allowedTargets(form.value));
const hasTarget = (target: string) => selectedTargets.value.includes(target);
const toggleTarget = (target: string) => {
  const next = hasTarget(target)
    ? selectedTargets.value.filter((item) => item !== target)
    : [...selectedTargets.value, target];
  // Unticking everything is caught by validation, not silently "local".
  form.value.targets = next.length > 0 ? next : ['__none__'];
  touched.value.targets = true;
};

const touched = ref<Record<string, boolean>>({});
const wasSubmitted = ref(false);
const { loadHistory, findSuggestions } = useSuggestions();

const editorSuggestions = computed(() => {
  const input = form.value.execution_string?.trim() || '';
  return input ? findSuggestions(input).slice(0, 5) : [];
});

const errors = computed(() => {
  const errs: { name?: string; execution_string?: string; targets?: string } = {};
  if (form.value.targets?.includes('__none__')) {
    errs.targets = 'Chọn ít nhất một nơi chạy.';
  }
  if (!form.value.name?.trim()) {
    errs.name = 'Tên gợi nhớ không được để trống.';
  }
  if (!form.value.execution_string?.trim()) {
    errs.execution_string = 'Chuỗi dòng lệnh thực thi không được để trống.';
  }
  return errs;
});

const hasErrors = computed(() => Object.keys(errors.value).length > 0);

const isFieldInvalid = (field: 'name' | 'execution_string' | 'targets') => {
  return (wasSubmitted.value || touched.value[field]) && !!errors.value[field];
};

watch(
  [() => props.visible, () => props.command],
  ([visible, val]) => {
    wasSubmitted.value = false;
    touched.value = {};
    if (visible) {
      if (val) {
        form.value = {
          ...val,
          quick_access: Boolean(val.quick_access),
          targets: [...(val.targets ?? [])],
        };
      } else {
        form.value = {
          name: '',
          execution_string: '',
          is_shell: false,
          quick_access: false,
          targets: [],
        };
      }
    } else {
      form.value = {
        name: '',
        execution_string: '',
        is_shell: false,
        quick_access: false,
      };
    }
  },
  { immediate: true }
);

onMounted(() => {
  void loadHistory();
  void fetchSshHosts();
});

const applySuggestion = (commandLine: string) => {
  form.value.execution_string = commandLine;
  touched.value.execution_string = true;
};

const handleSave = () => {
  wasSubmitted.value = true;
  if (hasErrors.value) return;
  emit('save', { ...form.value });
};
</script>

<style scoped>
.editor-modal {
  max-width: 560px;
}

.text-primary {
  color: var(--primary);
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.target-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 180px;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background-color: var(--bg-app-base);
}

.target-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 4px;
  font-size: 12.5px;
  cursor: pointer;
}

.target-item.selected {
  background-color: var(--primary-subtle, #7447911f);
}

.target-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.target-name {
  font-weight: 500;
}

.target-addr {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.field-hint {
  font-size: 11px;
  line-height: 1.4;
  color: var(--text-muted);
}

.field-hint code {
  font-family: var(--font-mono);
  background-color: rgba(255, 255, 255, 0.06);
  padding: 0 3px;
  border-radius: 3px;
}

.label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.form-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.code-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
}

.mode-selector {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mode-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
  background-color: var(--bg-app-base);
  cursor: pointer;
  transition: all 0.15s ease;
}

.mode-card:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-medium);
}

.mode-card.selected {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.mode-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.mode-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.mode-desc {
  font-size: 11px;
  color: var(--text-muted);
}

.input.has-error,
.textarea.has-error {
  border-color: var(--status-failed, #ef4444) !important;
  background-color: #ef44440d;
}

.field-error-msg {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--status-failed, #ef4444);
  margin-top: 2px;
}

.editor-suggestions {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px;
  background: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.editor-suggestion {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  padding: 6px 8px;
  color: var(--text-primary);
  background: transparent;
  border: 0;
  text-align: left;
  font: 11px var(--font-mono);
  cursor: pointer;
}

.editor-suggestion:hover {
  background: var(--bg-surface-hover);
}

.editor-suggestion small {
  color: var(--text-muted);
  white-space: nowrap;
}

.validation-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: var(--status-failed-bg, #ef44441a);
  border: 1px solid #ef44444d;
  border-radius: var(--radius-md);
  color: var(--status-failed, #ef4444);
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 12px;
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

/* Quick Access Toggle Card */
.quick-access-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 14px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  transition: all 0.15s ease;
}

.quick-access-card.active {
  border-color: var(--primary-alpha-45, #74479173);
  background-color: var(--primary-alpha-08, #74479114);
}

.quick-access-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.quick-access-header {
  display: flex;
  align-items: center;
  gap: 7px;
}

.quick-access-icon {
  color: #fbbf24;
}

.quick-access-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-primary);
}

.quick-access-desc {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.4;
}

.quick-toggle {
  position: relative;
  width: 38px;
  height: 20px;
  border-radius: 10px;
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-medium);
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 0.2s ease, border-color 0.2s ease;
  padding: 0;
}

.quick-toggle.active {
  background-color: var(--primary);
  border-color: var(--primary);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background-color: #ffffff;
  transition: transform 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.quick-toggle.active .toggle-thumb {
  transform: translateX(18px);
}
</style>
