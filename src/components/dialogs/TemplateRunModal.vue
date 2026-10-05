<template>
  <div v-if="visible && template" class="modal-backdrop" @click.self="$emit('close')">
    <div class="template-run-modal" @keydown.enter.exact="handleEnter">
      <!-- Modal Header -->
      <div class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge">
            <Puzzle :size="18" />
          </div>
          <div>
            <div class="modal-title">Thực Thi Mẫu Lệnh: {{ template.name }}</div>
            <div class="modal-subtitle">
              <span>Engine Tham Số v2.2</span>
              <span class="dot-separator">•</span>
              <span class="mono-id">#TPL-0{{ template.id }}</span>
              <span class="dot-separator">•</span>
              <span :class="template.is_shell ? 'mode-shell' : 'mode-argv'">
                {{ template.is_shell ? 'Shell Execution' : 'Direct Argv' }}
              </span>
            </div>
          </div>
        </div>
        <button class="close-btn" title="Đóng modal (Esc)" @click="$emit('close')">
          <X :size="18" />
        </button>
      </div>

      <!-- Presets Selector Toolbar -->
      <div class="preset-toolbar">
        <div class="preset-select-group">
          <Bookmark :size="15" class="preset-icon" />
          <span class="preset-label">Bộ giá trị đã lưu (Preset):</span>
          <select v-model="selectedPresetId" class="preset-select" @change="applySelectedPreset">
            <option :value="null">-- Chọn Preset có sẵn --</option>
            <option v-for="p in availablePresets" :key="p.id" :value="p.id">
              📌 {{ p.name }} (Lần cuối: {{ p.last_used_at || 'Mới lưu' }})
            </option>
          </select>
        </div>

        <div class="preset-actions">
          <button
            class="btn-secondary btn-sm"
            title="Lưu bộ tham số hiện tại thành Preset mới"
            @click="promptSavePreset"
          >
            <Save :size="13" />
            <span>Lưu Preset</span>
          </button>
          <button
            v-if="selectedPresetId"
            class="btn-danger-ghost btn-sm"
            title="Xóa preset đang chọn"
            @click="handleDeletePreset"
          >
            <Trash2 :size="13" />
          </button>
        </div>
      </div>

      <!-- Parameters Dynamic Form -->
      <div class="modal-body">
        <div class="section-title">
          <span>ĐIỀN THAM SỐ THỰC THI (PARAMETERS FORM)</span>
          <span class="param-count-badge">{{ template.params.length }} tham số</span>
        </div>

        <div class="params-grid">
          <div
            v-for="param in template.params"
            :key="param.name"
            class="param-field-card"
            :class="{ 'has-error': errors[param.name] }"
          >
            <div class="field-header">
              <label class="field-label">
                {{ param.label || param.name }}
                <span v-if="param.required" class="required-star">*</span>
              </label>
              <div class="field-meta">
                <code class="token-tag">&#123;&#123;{{ param.name }}&#125;&#125;</code>
                <span class="kind-badge">{{ param.kind.toUpperCase() }}</span>
                <span v-if="param.is_secret" class="secret-badge">
                  <Lock :size="10" /> Secret
                </span>
              </div>
            </div>

            <!-- Path Input -->
            <div v-if="param.kind === 'path'" class="input-with-button">
              <input
                v-model="paramValues[param.name]"
                type="text"
                class="form-input mono-input"
                :placeholder="param.default_value || 'Nhập đường dẫn file...'"
              />
            </div>

            <!-- Enum Select -->
            <select
              v-else-if="param.kind === 'enum'"
              v-model="paramValues[param.name]"
              class="form-input"
            >
              <option v-for="opt in (param.options || ['GET', 'POST', 'PUT', 'DELETE'])" :key="opt" :value="opt">
                {{ opt }}
              </option>
            </select>

            <!-- Secret Password Input with Eye Toggle -->
            <div v-else-if="param.is_secret" class="input-with-icon">
              <input
                v-model="paramValues[param.name]"
                :type="showSecrets[param.name] ? 'text' : 'password'"
                class="form-input mono-input"
                placeholder="••••••••••••••••"
              />
              <button
                class="icon-toggle-btn"
                type="button"
                title="Ẩn/Hiện mật khẩu"
                @click="showSecrets[param.name] = !showSecrets[param.name]"
              >
                <EyeOff v-if="showSecrets[param.name]" :size="14" />
                <Eye v-else :size="14" />
              </button>
            </div>

            <!-- Number Input -->
            <input
              v-else-if="param.kind === 'number'"
              v-model="paramValues[param.name]"
              type="number"
              class="form-input mono-input"
              :placeholder="param.default_value || '0'"
            />

            <!-- Standard String Input -->
            <input
              v-else
              v-model="paramValues[param.name]"
              type="text"
              class="form-input mono-input"
              :placeholder="param.default_value || 'Nhập giá trị...'"
            />

            <div v-if="param.is_secret" class="secret-notice">
              <ShieldAlert :size="12" />
              <span>Tham số bảo mật: Tự động loại trừ khỏi Preset và Lịch sử ghi DB.</span>
            </div>

            <div v-if="errors[param.name]" class="error-msg">
              {{ errors[param.name] }}
            </div>
          </div>
        </div>

        <!-- Rendered Live Command Preview -->
        <div class="preview-card">
          <div class="preview-header">
            <div class="preview-title">
              <Zap :size="14" class="zap-icon" />
              <span>XEM TRƯỚC LỆNH SẼ CHẠY (LIVE PREVIEW):</span>
            </div>
            <div class="preview-actions">
              <span class="preview-badge">{{ previewStatusLabel }}</span>
              <button
                type="button"
                class="copy-preview-btn"
                title="Sao chép lệnh preview đã che secret"
                @click="copyPreview"
              >
                <Check v-if="copiedPreview" :size="13" />
                <Copy v-else :size="13" />
                <span>{{ copiedPreview ? 'Đã copy' : 'Copy' }}</span>
              </button>
            </div>
          </div>

          <div class="code-preview-box">
            <code>$ {{ previewPayload.masked_rendered }}</code>
          </div>

          <div v-if="previewError" class="preview-error" role="alert">
            {{ previewError }}
          </div>
          <div v-if="previewPayload.warnings?.length" class="preview-warning" role="status">
            <span v-for="warning in previewPayload.warnings" :key="warning">{{ warning }}</span>
          </div>

          <div class="safety-indicator">
            <CheckCircle2 :size="13" class="check-icon" />
            <span>{{ previewSafetyMessage }}</span>
          </div>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="modal-footer">
        <button class="btn-ghost" :disabled="loading" @click="!loading && $emit('close')">
          Hủy Bỏ (Esc)
        </button>
        <div class="footer-target-hint">
          Đích đến: Mở tab Terminal PTY mới tại Workspace
        </div>
        <button class="btn-primary" :disabled="loading" @click="handleRun">
          <LoaderCircle v-if="loading" :size="15" class="spin" />
          <Play v-else :size="15" />
          <span>{{ loading ? loadingText : 'Thực Thi Lệnh (Enter)' }}</span>
        </button>
      </div>
    </div>

    <!-- Prompt Dialog Save Preset (MOD-15) -->
    <PromptDialog
      :visible="showSavePresetPrompt"
      title="Lưu Bộ Tham Số Preset"
      subtitle="MOD-15 • Parameter Snapshot Gate"
      hint="Lưu lại các giá trị tham số hiện tại thành Preset để tái sử dụng nhanh chóng cho các lần thực thi sau."
      label="Tên Preset Mới (*)"
      placeholder="Ví dụ: Production 1080p, Debug Mode..."
      :max-length="32"
      icon="bookmark"
      confirm-text="Lưu Preset"
      @confirm="handleConfirmSavePreset"
      @cancel="showSavePresetPrompt = false"
    />

    <!-- Confirm Dialog Delete Preset (MOD-13) -->
    <ConfirmDialog
      :visible="showDeletePresetConfirm"
      title="Xóa Bộ Tham Số Preset"
      subtitle="MOD-13 • Preset Removal Gate"
      :message="`Bạn có chắc chắn muốn xóa bộ tham số Preset '${selectedPresetName}'? Thao tác này không thể hoàn tác.`"
      confirm-text="Xóa Preset"
      :danger="true"
      @confirm="confirmDeletePreset"
      @cancel="showDeletePresetConfirm = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import {
  Puzzle,
  X,
  Bookmark,
  Save,
  Trash2,
  Lock,
  Eye,
  EyeOff,
  ShieldAlert,
  Zap,
  CheckCircle2,
  Check,
  Copy,
  Play,
  LoaderCircle,
} from 'lucide-vue-next';
import { useTemplates } from '@/composables/useTemplates';
import { ipcClient, isTauriRuntime } from '@/ipc/client';
import PromptDialog from '@/components/dialogs/PromptDialog.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import type { CommandTemplate, TemplatePreviewPayload } from '@/types/models';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    templateId: number | null;
    loading?: boolean;
    loadingText?: string;
  }>(),
  {
    loading: false,
    loadingText: 'Đang Khởi Chạy...',
  }
);

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'run', payload: {
    templateId: number;
    commandName: string;
    executionString: string;
    isShell: boolean;
    paramValues: Record<string, string>;
  }): void;
}>();

const {
  getTemplateById,
  fetchPresetsForTemplate,
  savePreset,
  deletePreset,
  previewTemplate,
} = useTemplates();

const selectedPresetId = ref<number | null>(null);
const paramValues = ref<Record<string, string>>({});
const showSecrets = ref<Record<string, boolean>>({});
const errors = ref<Record<string, string>>({});
const copiedPreview = ref(false);
const serverPreview = ref<TemplatePreviewPayload | null>(null);
const previewError = ref('');
const previewState = ref<'idle' | 'browser' | 'loading' | 'rust' | 'fallback'>('idle');
const previewRequestId = ref(0);

const template = computed<CommandTemplate | undefined>(() => {
  if (!props.templateId) return undefined;
  return getTemplateById(props.templateId);
});

const availablePresets = computed(() => {
  if (!props.templateId) return [];
  return fetchPresetsForTemplate(props.templateId);
});

// Initialize form values when template opens
watch(
  [() => props.visible, () => props.templateId],
  ([visible, newId]) => {
    if (visible && newId) {
      const tpl = getTemplateById(newId);
      if (tpl) {
        const initial: Record<string, string> = {};
        tpl.params.forEach(p => {
          initial[p.name] = p.default_value || '';
        });
        paramValues.value = initial;
        selectedPresetId.value = null;
        errors.value = {};
        previewError.value = '';
        serverPreview.value = null;
        previewState.value = isTauriRuntime() ? 'loading' : 'browser';

        const recentPreset = [...availablePresets.value]
          .filter(preset => Object.keys(preset.values).length > 0)
          .sort((a, b) => String(b.last_used_at || '').localeCompare(String(a.last_used_at || '')))[0];
        if (recentPreset) {
          selectedPresetId.value = recentPreset.id;
          tpl.params.forEach(param => {
            if (!param.is_secret && recentPreset.values[param.name] !== undefined) {
              initial[param.name] = recentPreset.values[param.name];
            }
          });
          paramValues.value = initial;
        }
      }
    }
  },
  { immediate: true }
);

watch(
  [() => props.visible, () => props.templateId, paramValues],
  (_, __, onCleanup) => {
    const requestId = ++previewRequestId.value;
    previewError.value = '';
    serverPreview.value = null;
    if (!props.visible || !template.value) {
      previewState.value = 'idle';
      return;
    }
    if (!isTauriRuntime()) {
      previewState.value = 'browser';
      return;
    }
    previewState.value = 'loading';
    const timer = window.setTimeout(async () => {
      try {
        const preview = await ipcClient.previewTemplate(template.value!.id, paramValues.value);
        if (requestId !== previewRequestId.value) return;
        serverPreview.value = preview;
        previewState.value = 'rust';
      } catch (error) {
        if (requestId !== previewRequestId.value) return;
        previewError.value = error instanceof Error ? error.message : String(error);
        serverPreview.value = null;
        previewState.value = 'fallback';
      }
    }, 120);
    onCleanup(() => window.clearTimeout(timer));
  },
  { deep: true, immediate: true },
);

const previewPayload = computed(() => {
  if (serverPreview.value) return serverPreview.value;
  if (!template.value) {
    return { rendered: '', masked_rendered: '', is_shell: false };
  }
  return previewTemplate(template.value, paramValues.value);
});

const previewStatusLabel = computed(() => {
  switch (previewState.value) {
    case 'rust':
      return 'Rust Safe-Quoted';
    case 'loading':
      return 'Đang xác thực Rust...';
    case 'fallback':
      return 'JS fallback — chưa xác thực';
    case 'browser':
      return 'Browser Mock Preview';
    default:
      return 'Chưa có preview';
  }
});

const previewSafetyMessage = computed(() => {
  if (previewState.value === 'rust') {
    return 'Đã render và xác thực bởi Rust backend; giá trị được bọc/ phân tách theo kiểu thực thi.';
  }
  if (previewState.value === 'browser') {
    return 'Preview trình duyệt mock; chưa đại diện cho kết quả render của Rust backend.';
  }
  return 'Preview tạm thời từ frontend; chưa thể xác nhận an toàn cho tới khi Rust backend phản hồi.';
});

const copyPreview = async () => {
  const text = previewPayload.value.masked_rendered;
  if (!text) return;

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.focus();
      textarea.select();
      document.execCommand('copy');
      textarea.remove();
    }
    copiedPreview.value = true;
    window.setTimeout(() => {
      copiedPreview.value = false;
    }, 1600);
  } catch (error) {
    console.error('[TemplateRunModal] Không thể sao chép preview:', error);
  }
};

const applySelectedPreset = () => {
  if (!selectedPresetId.value) return;
  const p = availablePresets.value.find(item => item.id === selectedPresetId.value);
  if (p && template.value) {
    // Fill values from preset (retaining current secret field values)
    template.value.params.forEach(param => {
      if (!param.is_secret && p.values[param.name] !== undefined) {
        paramValues.value[param.name] = p.values[param.name];
      }
    });
  }
};

const showSavePresetPrompt = ref(false);
const showDeletePresetConfirm = ref(false);

const selectedPresetName = computed(() => {
  if (!selectedPresetId.value) return '';
  const p = availablePresets.value.find(pr => pr.id === selectedPresetId.value);
  return p ? p.name : '';
});

const promptSavePreset = () => {
  if (!template.value) return;
  showSavePresetPrompt.value = true;
};

const handleConfirmSavePreset = async (name: string) => {
  if (!template.value || !name.trim()) return;
  showSavePresetPrompt.value = false;
  const newP = await savePreset(template.value.id, name.trim(), paramValues.value);
  if (newP) {
    selectedPresetId.value = newP.id;
  }
};

const handleDeletePreset = () => {
  if (!selectedPresetId.value) return;
  showDeletePresetConfirm.value = true;
};

const confirmDeletePreset = async () => {
  if (!selectedPresetId.value) return;
  try {
    await deletePreset(selectedPresetId.value);
    selectedPresetId.value = null;
  } finally {
    showDeletePresetConfirm.value = false;
  }
};

const handleEnter = (event: KeyboardEvent) => {
  if (event.isComposing || event.key === 'Process' || event.keyCode === 229) return;
  const target = event.target as HTMLElement | null;
  if (!target || target.tagName !== 'INPUT') return;
  event.preventDefault();
  handleRun();
};

const handleRun = () => {
  if (!template.value) return;

  if (isTauriRuntime() && previewState.value !== 'rust') {
    previewError.value =
      previewState.value === 'loading'
        ? 'Đang chờ Rust backend xác thực preview. Vui lòng thử lại sau một chút.'
        : 'Không thể chạy khi preview Rust chưa xác thực thành công.';
    return;
  }

  const preview = previewPayload.value;
  if (preview.validation_errors) {
    const nextErrors = { ...preview.validation_errors };
    previewError.value = nextErrors._template || '';
    delete nextErrors._template;
    errors.value = nextErrors;
    return;
  }

  errors.value = {};
  previewError.value = '';
  emit('run', {
    templateId: template.value.id,
    commandName: `[Template] ${template.value.name}`,
    executionString: preview.rendered,
    isShell: template.value.is_shell,
    paramValues: { ...paramValues.value },
  });
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: var(--surface-overlay, #0b0d13bf);
  backdrop-filter: blur(6px);
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.template-run-modal {
  width: 100%;
  max-width: 680px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 30px #00000099, 0 0 20px var(--primary-alpha-25, #74479140);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  max-height: 90vh;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
  border-top: 3px solid var(--primary);
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 34px;
  height: 34px;
  border-radius: var(--radius-md);
  background-color: var(--primary-subtle);
  color: #cda8ee;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
}

.dot-separator {
  color: var(--text-muted);
}

.mono-id {
  font-family: var(--font-mono);
  color: #4cd7f6;
}

.mode-shell {
  color: var(--status-starting);
}

.mode-argv {
  color: var(--status-running);
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
}

.close-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.preset-toolbar {
  padding: 10px 18px;
  background-color: #111319;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.preset-select-group {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
}

.preset-icon {
  color: #4cd7f6;
}

.preset-label {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.preset-select {
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  color-scheme: dark;
  flex: 1;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  padding: 5px 28px 5px 8px;
  border-radius: var(--radius-sm);
  outline: none;
  cursor: pointer;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 8px center;
  background-size: 14px 14px;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.preset-select:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.preset-select option {
  background-color: var(--surface-container);
  color: var(--text-primary);
}

.preset-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.modal-body {
  padding: 16px 18px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.section-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
}

.param-count-badge {
  font-size: 10px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  color: #4cd7f6;
}

.params-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.param-field-card {
  background-color: #141721;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.param-field-card.has-error {
  border-color: var(--status-failed);
}

.field-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.field-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.required-star {
  color: var(--status-failed);
  margin-left: 2px;
}

.field-meta {
  display: flex;
  align-items: center;
  gap: 6px;
}

.token-tag {
  font-family: var(--font-mono);
  font-size: 11px;
  background-color: var(--primary-alpha-20, #74479133);
  color: #e4b5ff;
  padding: 1px 5px;
  border-radius: 3px;
}

.kind-badge {
  font-size: 9px;
  font-weight: 700;
  background-color: var(--bg-app-base);
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
  padding: 1px 4px;
  border-radius: 3px;
}

.secret-badge {
  font-size: 9px;
  color: #f59e0b;
  display: flex;
  align-items: center;
  gap: 3px;
}

.form-input {
  width: 100%;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  outline: none;
}

.form-input:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

select.form-input {
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  color-scheme: dark;
  padding-right: 28px;
  cursor: pointer;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 8px center;
  background-size: 14px 14px;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

select.form-input option {
  background-color: var(--surface-container);
  color: var(--text-primary);
}

.mono-input {
  font-family: var(--font-mono);
}

.input-with-button,
.input-with-icon {
  display: flex;
  align-items: center;
  gap: 6px;
  position: relative;
}

.btn-browse {
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 11px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}

.btn-browse:hover {
  background-color: var(--primary);
}

.icon-toggle-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  position: absolute;
  right: 8px;
}

.icon-toggle-btn:hover {
  color: var(--text-primary);
}

.secret-notice {
  font-size: 10px;
  color: #f59e0b;
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 2px;
}

.error-msg {
  font-size: 11px;
  color: var(--status-failed);
}

.preview-card {
  background-color: #0b0d13;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.preview-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.preview-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.preview-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 700;
  color: #4cd7f6;
}

.zap-icon {
  color: #f59e0b;
}

.preview-badge {
  font-size: 10px;
  color: var(--text-muted);
}

.copy-preview-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 7px;
  color: var(--text-secondary);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  font-size: 10px;
  cursor: pointer;
}

.copy-preview-btn:hover {
  color: var(--text-primary);
  border-color: var(--primary);
}

.code-preview-box {
  background-color: #050608;
  padding: 10px;
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: 12px;
  color: #4edea3;
  word-break: break-all;
  border: 1px solid #1a1e2b;
}

.preview-error {
  padding: 7px 9px;
  color: #fecaca;
  background: #7f1d1d59;
  border: 1px solid #f8717159;
  border-radius: var(--radius-sm);
  font-size: 11px;
}

.preview-warning {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-top: 8px;
  padding: 8px 10px;
  border: 1px solid #f5be5059;
  border-radius: 6px;
  color: #f5be50;
  font-size: 11px;
}

.safety-indicator {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  color: var(--status-running);
}

.check-icon {
  flex-shrink: 0;
}

.modal-footer {
  padding: 12px 18px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.footer-target-hint {
  font-size: 11px;
  color: var(--text-muted);
}

.btn-primary {
  background-color: var(--primary);
  color: #ffffff;
  border: none;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  box-shadow: 0 0 10px var(--primary-glow);
}

.btn-primary:hover {
  background-color: var(--primary-hover);
}

.btn-secondary {
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  padding: 5px 10px;
  font-size: 11px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
}

.btn-secondary:hover {
  background-color: var(--primary-subtle);
  border-color: var(--primary);
}

.btn-danger-ghost {
  background: transparent;
  border: 1px solid var(--status-failed);
  color: var(--status-failed);
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
}

.btn-danger-ghost:hover {
  background-color: var(--status-failed);
  color: #ffffff;
}

.btn-ghost {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.btn-ghost:hover {
  color: var(--text-primary);
}

.btn-ghost:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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
