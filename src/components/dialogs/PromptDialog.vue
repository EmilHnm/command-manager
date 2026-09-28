<template>
  <div v-if="visible" class="modal-backdrop" @click.self="handleCancel">
    <div class="prompt-modal-container animate-fade-in" role="dialog" aria-modal="true">
      <!-- Gradient Header Line -->
      <div class="header-gradient-line"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge">
            <Bookmark v-if="icon === 'bookmark'" :size="17" />
            <Terminal v-else-if="icon === 'terminal'" :size="17" />
            <Edit3 v-else :size="17" />
          </div>
          <div>
            <h2 class="modal-title">{{ title }}</h2>
            <div class="modal-subtitle">{{ subtitle }}</div>
          </div>
        </div>
        <button class="close-btn" :disabled="loading" title="Hủy bỏ (Esc)" @click="handleCancel">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- Hint Box -->
        <div v-if="hint" class="hint-box">
          <Lightbulb :size="16" class="hint-icon" />
          <p class="hint-text">{{ hint }}</p>
        </div>

        <!-- Command Trace Preview (Optional) -->
        <div v-if="commandPreview" class="command-preview-wrap">
          <div class="preview-header">
            <span class="preview-label">CHUYỂN ĐỔI TỪ LỆNH RUNTIME:</span>
            <span v-if="commandBadge" class="preview-badge">{{ commandBadge }}</span>
          </div>
          <div class="preview-box">
            <div class="preview-cmd truncate">
              <span class="cmd-prompt">$</span>
              <span>{{ commandPreview }}</span>
            </div>
            <button
              class="copy-btn"
              :title="copied ? 'Đã sao chép!' : 'Sao chép lệnh'"
              @click="handleCopyPreview"
            >
              <Check v-if="copied" :size="14" class="copied-icon" />
              <Copy v-else :size="14" />
            </button>
          </div>
        </div>

        <!-- Input Field Container -->
        <div class="field-wrap">
          <div class="field-header">
            <label for="prompt-input" class="field-label">
              {{ label }} <span class="required-pip">*</span>
            </label>
            <span class="char-counter" :class="{ 'over-limit': inputValue.length > maxLength }">
              {{ inputValue.length }} / {{ maxLength }}
            </span>
          </div>

          <div class="input-container" :class="{ 'has-error': !isValid && touched }">
            <span class="input-icon">
              <Terminal :size="15" />
            </span>
            <input
              id="prompt-input"
              ref="inputRef"
              v-model="inputValue"
              type="text"
              class="text-input"
              :placeholder="placeholder"
              :maxlength="maxLength"
              autocomplete="off"
              spellcheck="false"
              @input="touched = true"
              @keydown.enter.prevent="handleConfirm"
              @keydown.esc.prevent="handleCancel"
            />
            <button
              v-if="inputValue"
              type="button"
              class="clear-btn"
              title="Xóa nhanh"
              @click="clearInput"
            >
              <XCircle :size="15" />
            </button>
          </div>

          <!-- Validation Feedback -->
          <div v-if="!isValid && touched" class="feedback-msg error-msg">
            <AlertCircle :size="13" />
            <span>Tên không được để trống hoặc chỉ chứa khoảng trắng.</span>
          </div>
          <div v-else-if="isValid && inputValue.trim()" class="feedback-msg success-msg">
            <CheckCircle2 :size="13" />
            <span>Tên hợp lệ. Nhấn Enter để xác nhận lưu.</span>
          </div>
        </div>

        <!-- Quick Presets / Tag Chips (Optional) -->
        <div v-if="presets && presets.length > 0" class="presets-wrap">
          <div class="presets-header">
            <span>Gợi ý nhãn nhanh:</span>
            <span class="presets-sub">Nhấn để điền</span>
          </div>
          <div class="chips-list">
            <button
              v-for="chip in presets"
              :key="chip"
              type="button"
              class="preset-chip"
              @click="applyPreset(chip)"
            >
              <span>+</span>
              <span>[{{ chip }}]</span>
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <div class="keyboard-hints">
          <span>⌨️</span>
          <span>Esc: Hủy</span>
          <span class="hint-divider">|</span>
          <span>Enter: Lưu</span>
        </div>

        <div class="action-buttons">
          <button type="button" class="btn-ghost" :disabled="loading" @click="handleCancel">
            {{ cancelText }}
          </button>
          <button
            type="button"
            class="btn-primary"
            :disabled="!isValid || loading"
            @click="handleConfirm"
          >
            <LoaderCircle v-if="loading" :size="14" class="spin" />
            <Save v-else :size="14" />
            <span>{{ loading ? loadingText : confirmText }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import {
  Edit3,
  Bookmark,
  Terminal,
  X,
  XCircle,
  Lightbulb,
  Copy,
  Check,
  CheckCircle2,
  AlertCircle,
  Save,
  LoaderCircle,
} from 'lucide-vue-next';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    title?: string;
    subtitle?: string;
    hint?: string;
    label?: string;
    placeholder?: string;
    defaultValue?: string;
    commandPreview?: string;
    commandBadge?: string;
    confirmText?: string;
    loadingText?: string;
    cancelText?: string;
    maxLength?: number;
    presets?: string[];
    icon?: 'edit' | 'bookmark' | 'terminal';
    loading?: boolean;
  }>(),
  {
    title: 'Lưu Dữ Liệu',
    subtitle: 'MOD-15 • Technical Input Gate',
    hint: '',
    label: 'Tên (*)',
    placeholder: 'Nhập tên...',
    defaultValue: '',
    commandPreview: '',
    commandBadge: '',
    confirmText: 'Lưu Lại',
    loadingText: 'Đang lưu...',
    cancelText: 'Hủy Bỏ',
    maxLength: 64,
    presets: () => [],
    icon: 'edit',
    loading: false,
  }
);

const emit = defineEmits<{
  (e: 'confirm', value: string): void;
  (e: 'cancel'): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const inputValue = ref('');
const touched = ref(false);
const copied = ref(false);

const isValid = computed(() => {
  const trimmed = inputValue.value.trim();
  return trimmed.length > 0 && trimmed.length <= props.maxLength;
});

const focusAndSelect = async () => {
  await nextTick();
  if (inputRef.value) {
    inputRef.value.focus();
    inputRef.value.select();
  }
};

watch(
  () => props.visible,
  (val) => {
    if (val) {
      inputValue.value = props.defaultValue || '';
      touched.value = false;
      copied.value = false;
      focusAndSelect();
    }
  },
  { immediate: true }
);

const handleGlobalKeydown = (event: KeyboardEvent) => {
  if (!props.visible || props.loading) return;
  if (event.key === 'Escape') {
    handleCancel();
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalKeydown);
});

const clearInput = () => {
  inputValue.value = '';
  touched.value = true;
  inputRef.value?.focus();
};

const applyPreset = (chip: string) => {
  if (!inputValue.value.trim()) {
    inputValue.value = chip;
  } else {
    inputValue.value = `${inputValue.value.trim()} [${chip}]`;
  }
  touched.value = true;
  inputRef.value?.focus();
};

const handleCopyPreview = async () => {
  if (!props.commandPreview) return;
  try {
    await navigator.clipboard.writeText(props.commandPreview);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1800);
  } catch {
    // fallback ignore
  }
};

const handleConfirm = () => {
  if (props.loading) return;
  touched.value = true;
  if (!isValid.value) return;
  emit('confirm', inputValue.value.trim());
};

const handleCancel = () => {
  if (props.loading) return;
  emit('cancel');
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(11, 13, 19, 0.8);
  backdrop-filter: blur(6px);
  z-index: 120;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.prompt-modal-container {
  width: 100%;
  max-width: 480px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 35px -5px rgba(0, 0, 0, 0.8), 0 0 20px rgba(116, 71, 145, 0.28);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

.header-gradient-line {
  height: 3px;
  background: linear-gradient(90deg, #744791 0%, #a855f7 50%, #744791 100%);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.icon-badge {
  width: 30px;
  height: 30px;
  border-radius: var(--radius-md);
  background-color: rgba(116, 71, 145, 0.2);
  border: 1px solid rgba(116, 71, 145, 0.4);
  color: var(--primary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
  line-height: 1.3;
}

.modal-subtitle {
  font-size: 10px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  margin-top: 1px;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.close-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.modal-body {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.hint-box {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  background-color: rgba(116, 71, 145, 0.1);
  border: 1px solid rgba(116, 71, 145, 0.25);
  border-radius: var(--radius-sm);
  padding: 8px 10px;
}

.hint-icon {
  color: var(--primary);
  margin-top: 2px;
  flex-shrink: 0;
}

.hint-text {
  font-size: 11.5px;
  color: var(--text-secondary);
  line-height: 1.5;
  margin: 0;
}

.command-preview-wrap {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.preview-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 10.5px;
  font-family: var(--font-mono);
}

.preview-label {
  color: var(--text-muted);
}

.preview-badge {
  font-size: 10px;
  color: var(--secondary);
  background-color: rgba(78, 222, 163, 0.12);
  border: 1px solid rgba(78, 222, 163, 0.3);
  border-radius: var(--radius-sm);
  padding: 1px 6px;
}

.preview-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 6px 10px;
  gap: 8px;
}

.preview-cmd {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--primary);
  user-select: text;
}

.cmd-prompt {
  color: var(--text-muted);
  margin-right: 6px;
}

.copy-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 3px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all 0.15s ease;
}

.copy-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.copied-icon {
  color: var(--status-running);
}

.field-wrap {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.field-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}

.required-pip {
  color: var(--status-warning);
}

.char-counter {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.char-counter.over-limit {
  color: var(--status-failed);
}

.input-container {
  height: 36px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  padding: 0 10px;
  gap: 8px;
  transition: all 0.15s ease;
}

.input-container:focus-within {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.input-container.has-error {
  border-color: var(--status-failed);
  box-shadow: 0 0 0 2px rgba(239, 68, 68, 0.25);
}

.input-icon {
  color: var(--primary);
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.text-input {
  width: 100%;
  background: transparent;
  border: none;
  outline: none;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
  padding: 0;
}

.text-input::placeholder {
  color: var(--text-muted);
  font-family: var(--font-sans);
}

.clear-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s ease;
}

.clear-btn:hover {
  color: var(--text-primary);
}

.feedback-msg {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  margin-top: 2px;
}

.feedback-msg.error-msg {
  color: var(--status-failed);
}

.feedback-msg.success-msg {
  color: var(--status-running);
}

.presets-wrap {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 6px;
  border-top: 1px solid var(--border-subtle);
}

.presets-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  color: var(--text-muted);
}

.presets-sub {
  font-family: var(--font-mono);
  font-size: 10px;
}

.chips-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.preset-chip {
  display: flex;
  align-items: center;
  gap: 4px;
  background-color: var(--bg-sidebar);
  border: 1px solid var(--border-subtle);
  border-radius: 9999px;
  padding: 3px 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s ease;
}

.preset-chip:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-focus);
  color: var(--text-primary);
}

.modal-footer {
  padding: 10px 16px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.keyboard-hints {
  display: flex;
  align-items: center;
  gap: 4px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.hint-divider {
  margin: 0 2px;
  color: var(--border-subtle);
}

.action-buttons {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-ghost {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  padding: 6px 12px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-ghost:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.btn-primary {
  background-color: var(--primary);
  color: #ffffff;
  border: none;
  padding: 6px 14px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  box-shadow: 0 0 10px var(--primary-glow);
  transition: all 0.15s ease;
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-hover);
}

.btn-primary:active:not(:disabled) {
  background-color: var(--primary-active);
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
