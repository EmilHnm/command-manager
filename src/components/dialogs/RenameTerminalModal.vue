<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('cancel')">
    <div class="rename-modal-container" role="dialog" aria-modal="true" aria-labelledby="rename-modal-title">
      <!-- Top Accent Ribbon -->
      <div class="modal-accent-bar" />

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <Terminal :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="rename-modal-title" class="modal-title">Đổi Tên Terminal</h3>
            <div class="modal-badges">
              <span class="badge-pty">
                <span class="dot-live" />
                <span>Shell PTY</span>
              </span>
              <span v-if="pid" class="badge-meta">PID: {{ pid }}</span>
              <span v-if="shellKind" class="badge-meta">{{ shellKind }}</span>
            </div>
          </div>
        </div>
        <button class="btn-close" title="Hủy bỏ (Esc)" @click="$emit('cancel')">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- Guidance Note -->
        <div class="info-banner">
          <Sparkles :size="14" class="info-icon" />
          <div class="info-text">
            Nháy đúp vào tab thủ công để đổi tên hiển thị. Tên tùy chỉnh giúp phân biệt nhanh các phiên PTY độc lập trong không gian làm việc.
          </div>
        </div>

        <!-- Form Input Field -->
        <div class="form-group">
          <div class="label-row">
            <label for="terminal-name-input" class="input-label">
              Tên Terminal Mới <span class="required-star">*</span>
            </label>
            <span class="char-counter" :class="{ 'at-limit': nameInput.length >= 32 }">
              {{ nameInput.length }} / 32
            </span>
          </div>

          <div class="input-wrapper" :class="{ 'is-focused': isInputFocused, 'has-error': errorMessage }">
            <Pencil :size="14" class="input-prefix-icon" />
            <input
              id="terminal-name-input"
              ref="inputRef"
              v-model="nameInput"
              type="text"
              class="terminal-input"
              maxlength="32"
              placeholder="Ví dụ: Dev Server, Worker Debug, API Test..."
              autocomplete="off"
              spellcheck="false"
              @focus="isInputFocused = true"
              @blur="isInputFocused = false"
              @input="onInput"
              @keydown.enter.prevent="handleSave"
              @keydown.esc.prevent="$emit('cancel')"
            />
            <button
              v-if="nameInput.length > 0"
              type="button"
              class="btn-clear"
              title="Xóa nội dung"
              @click="clearInput"
            >
              <X :size="12" />
            </button>
          </div>

          <div v-if="errorMessage" class="error-msg">
            {{ errorMessage }}
          </div>
        </div>

        <!-- Quick Preset Suggestions -->
        <div class="presets-section">
          <div class="presets-label">Gợi ý nhanh:</div>
          <div class="presets-chips">
            <button
              v-for="preset in PRESET_NAMES"
              :key="preset"
              type="button"
              class="preset-chip"
              :class="{ active: nameInput.trim() === preset }"
              @click="selectPreset(preset)"
            >
              {{ preset }}
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <div class="footer-hint">
          <span>Esc</span> để hủy • <span>Enter</span> để lưu
        </div>
        <div class="footer-actions">
          <button type="button" class="btn btn-secondary" :disabled="loading" @click="!loading && $emit('cancel')">
            Hủy Bỏ
          </button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="!isValid || loading"
            @click="handleSave"
          >
            <LoaderCircle v-if="loading" :size="14" class="spin" />
            <Check v-else :size="14" />
            <span>{{ loading ? loadingText : 'Lưu Tên Mới' }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { Terminal, Pencil, X, Check, Sparkles, LoaderCircle } from 'lucide-vue-next';

const PRESET_NAMES = [
  'Terminal',
  'Dev Server',
  'Worker Debug',
  'Build & Watch',
  'API Test',
  'Logs',
  'Scratchpad',
];

const props = withDefaults(
  defineProps<{
    visible: boolean;
    tabId: string;
    currentName: string;
    pid?: number;
    shellKind?: string;
    loading?: boolean;
    loadingText?: string;
  }>(),
  {
    loading: false,
    loadingText: 'Đang Lưu...',
  }
);

const emit = defineEmits<{
  (e: 'save', tabId: string, newName: string): void;
  (e: 'cancel'): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const nameInput = ref('');
const isInputFocused = ref(false);
const touched = ref(false);

const isValid = computed(() => {
  const trimmed = nameInput.value.trim();
  return trimmed.length > 0 && trimmed.length <= 32;
});

const errorMessage = computed(() => {
  if (!touched.value) return '';
  const trimmed = nameInput.value.trim();
  if (trimmed.length === 0) {
    return 'Tên terminal không được để trống hoặc chỉ chứa khoảng trắng.';
  }
  return '';
});

watch(
  () => props.visible,
  (newVal) => {
    if (newVal) {
      nameInput.value = props.currentName || 'Terminal';
      touched.value = false;
      nextTick(() => {
        if (inputRef.value) {
          inputRef.value.focus();
          inputRef.value.select();
        }
      });
    }
  },
  { immediate: true }
);

const onInput = () => {
  touched.value = true;
};

const clearInput = () => {
  nameInput.value = '';
  touched.value = true;
  nextTick(() => {
    inputRef.value?.focus();
  });
};

const selectPreset = (preset: string) => {
  nameInput.value = preset;
  touched.value = true;
  nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
};

const handleSave = () => {
  touched.value = true;
  const trimmed = nameInput.value.trim();
  if (!trimmed) return;
  emit('save', props.tabId, trimmed);
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1050;
  background-color: var(--surface-overlay, #0b0d13c7);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  animation: modalFadeIn 0.18s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modalFadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.rename-modal-container {
  position: relative;
  width: 100%;
  max-width: 480px;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-medium, #2d344d);
  border-radius: 8px;
  box-shadow: 0 16px 36px #000000a6, 0 0 24px var(--primary-alpha-25, #74479140);
  overflow: hidden;
  animation: modalScaleIn 0.22s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modalScaleIn {
  from {
    opacity: 0.6;
    transform: scale(0.96) translateY(-4px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.modal-accent-bar {
  height: 2px;
  width: 100%;
  background: linear-gradient(90deg, var(--primary, #744791), var(--primary-accent, #e4b5ff), #4cd7f6);
}

/* Header */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px 12px;
  border-bottom: 1px solid var(--border-subtle, #1e2436);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background: var(--primary-alpha-20, #74479133);
  border: 1px solid var(--primary-alpha-40, #74479166);
  color: var(--primary-accent, #e4b5ff);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 10px var(--primary-alpha-30, #7447914d);
}

.header-titles {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.modal-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
  letter-spacing: -0.01em;
}

.modal-badges {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: var(--font-mono, monospace);
  font-size: 11px;
}

.badge-pty {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: #4edea3;
  font-weight: 500;
}

.dot-live {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background-color: #10b981;
  box-shadow: 0 0 6px #10b981;
}

.badge-meta {
  color: var(--text-muted, #64748b);
  background-color: var(--surface-container-high, #23283a);
  padding: 1px 5px;
  border-radius: 3px;
}

.btn-close {
  background: transparent;
  border: none;
  color: var(--text-muted, #64748b);
  padding: 6px;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.btn-close:hover {
  background-color: var(--surface-container-high, #23283a);
  color: var(--text-primary, #f1f5f9);
}

/* Body */
.modal-body {
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.info-banner {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background-color: var(--primary-alpha-12, #7447911f);
  border: 1px solid var(--primary-alpha-25, #74479140);
  border-radius: 6px;
  padding: 10px 12px;
}

.info-icon {
  color: var(--primary-accent, #e4b5ff);
  flex-shrink: 0;
  margin-top: 2px;
}

.info-text {
  font-size: 12px;
  line-height: 17px;
  color: var(--text-secondary, #94a3b8);
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.input-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
}

.required-star {
  color: var(--status-failed, #ef4444);
}

.char-counter {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: var(--text-muted, #64748b);
  transition: color 0.15s ease;
}

.char-counter.at-limit {
  color: var(--status-warning, #f59e0b);
  font-weight: 600;
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  background-color: var(--bg-input, #12151f);
  border: 1px solid var(--border-medium, #2d344d);
  border-radius: 6px;
  padding: 0 10px;
  transition: border-color 0.18s ease, box-shadow 0.18s ease;
}

.input-wrapper.is-focused {
  border-color: var(--primary, #744791);
  box-shadow: 0 0 0 3px var(--primary-glow, #74479159);
}

.input-wrapper.has-error {
  border-color: var(--status-failed, #ef4444);
  box-shadow: 0 0 0 3px #ef444440;
}

.input-prefix-icon {
  color: var(--text-muted, #64748b);
  flex-shrink: 0;
  margin-right: 8px;
}

.terminal-input {
  flex: 1;
  height: 34px;
  background: transparent;
  border: none;
  outline: none;
  font-family: var(--font-mono, monospace);
  font-size: 13px;
  color: var(--text-primary, #f1f5f9);
}

.terminal-input::placeholder {
  color: var(--text-muted, #64748b);
  font-style: italic;
  font-size: 12px;
}

.btn-clear {
  background: transparent;
  border: none;
  color: var(--text-muted, #64748b);
  padding: 4px;
  border-radius: 9999px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}

.btn-clear:hover {
  color: var(--text-primary, #f1f5f9);
  background-color: var(--surface-container-high, #23283a);
}

.error-msg {
  font-size: 11px;
  color: var(--status-failed, #ef4444);
  margin-top: 2px;
}

/* Presets */
.presets-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.presets-label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 500;
}

.presets-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.preset-chip {
  background-color: var(--surface-container-high, #23283a);
  border: 1px solid var(--border-subtle, #1e2436);
  border-radius: 4px;
  color: var(--text-secondary, #94a3b8);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  padding: 3px 8px;
  cursor: pointer;
  transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

.preset-chip:hover {
  background-color: var(--primary-alpha-22, #74479138);
  border-color: var(--primary-alpha-40, #74479166);
  color: var(--primary-accent, #e4b5ff);
  transform: translateY(-1px);
}

.preset-chip.active {
  background-color: var(--primary, #744791);
  border-color: var(--primary-accent, #e4b5ff);
  color: #ffffff;
  box-shadow: 0 0 8px var(--primary-alpha-40, #74479166);
}

/* Footer */
.modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 18px;
  border-top: 1px solid var(--border-subtle, #1e2436);
  background-color: #12151f66;
}

.footer-hint {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.footer-hint span {
  display: inline-block;
  padding: 1px 4px;
  background-color: var(--surface-container-high, #23283a);
  border: 1px solid var(--border-subtle, #1e2436);
  border-radius: 3px;
  color: var(--text-secondary, #94a3b8);
}

.footer-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
  border: 1px solid transparent;
}

.btn-secondary {
  background-color: transparent;
  color: var(--text-secondary, #94a3b8);
}

.btn-secondary:hover {
  background-color: var(--surface-container-high, #23283a);
  color: var(--text-primary, #f1f5f9);
}

.btn-primary {
  background-color: var(--primary, #744791);
  border-color: var(--primary-accent-alpha-25, #e4b5ff40);
  color: #ffffff;
  box-shadow: 0 2px 6px #00000059;
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-hover, #8956aa);
  box-shadow: 0 0 12px var(--primary-glow, #74479173);
}

.btn-primary:active:not(:disabled) {
  background-color: var(--primary-active, #5f3977);
}

.btn-primary:disabled {
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
