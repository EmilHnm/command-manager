<template>
  <div v-if="visible" class="modal-backdrop" @click.self="handleCancel">
    <div class="keybinding-modal-container">
      <!-- Violet Accent Hazard Ribbon -->
      <div class="hazard-stripe"></div>

      <!-- Header -->
      <header class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge">
            <Keyboard :size="16" />
          </div>
          <div>
            <div class="modal-title">
              {{ isNewBinding ? 'Thêm phím tắt:' : 'Đổi phím tắt:' }} {{ title }}
            </div>
            <div class="header-meta">
              <span class="command-id-mono">{{ commandId }}</span>
              <span class="meta-dot">•</span>
              <span class="badge-mod">MOD-16</span>
            </div>
          </div>
        </div>
        <button class="close-btn" title="Đóng (Esc)" @click="handleCancel">
          <X :size="16" />
        </button>
      </header>

      <!-- Body -->
      <div class="modal-body">
        <div class="instructions-row">
          <span class="instruction-text">
            <Touchpad :size="14" class="text-primary" />
            <span>Nhấn tổ hợp phím mong muốn, rồi nhấn Enter để lưu.</span>
          </span>
          <span class="rec-badge">REC MODE</span>
        </div>

        <!-- Recording Keystroke Box -->
        <div
          ref="captureBoxRef"
          tabindex="0"
          class="capture-box"
          :class="{ 'has-error': hasError, 'is-focused': isRecordingFocused }"
          @keydown="handleCaptureKeydown"
          @focus="isRecordingFocused = true"
          @blur="isRecordingFocused = false"
        >
          <!-- Display Keycaps -->
          <div v-if="capturedChord" class="keycaps-row">
            <template v-for="(keyPart, idx) in formattedCapturedChord" :key="idx">
              <kbd class="keycap">{{ keyPart }}</kbd>
              <span v-if="idx < formattedCapturedChord.length - 1" class="keycap-plus">+</span>
            </template>
          </div>

          <!-- Pending Modifier Display -->
          <div v-else-if="pendingModifiers.length > 0" class="keycaps-row">
            <template v-for="(mod, idx) in pendingModifiers" :key="idx">
              <kbd class="keycap">{{ mod }}</kbd>
              <span class="keycap-plus">+</span>
            </template>
            <span class="text-text-muted text-xs font-mono">…</span>
          </div>

          <!-- Empty Listening Prompt -->
          <div v-else class="empty-prompt">
            <span class="listening-dot"></span>
            <span>Đang lắng nghe phím... (Nhấn tổ hợp phím để gán)</span>
          </div>

          <div v-if="capturedChord" class="listening-indicator">
            <span class="listening-dot"></span>
            <span>Đang lắng nghe... (Nhấn phím mới để thay đổi)</span>
          </div>

          <!-- Reset/Clear Button -->
          <button
            v-if="capturedChord"
            class="clear-chord-btn"
            title="Xóa phím đã bấm"
            @click.stop="clearCaptured"
          >
            <Delete :size="14" />
          </button>
        </div>

        <!-- Scope Selector Group -->
        <div class="scope-group">
          <div class="scope-label-row">
            <label class="scope-label">
              <Layers :size="14" class="text-text-muted" />
              <span>Phạm vi tác dụng:</span>
            </label>
            <span class="scope-hint-mono">Keybinding Scope</span>
          </div>

          <div class="select-wrapper">
            <select v-model="selectedScope" class="scope-select">
              <option
                v-for="sc in availableScopes"
                :key="sc"
                :value="sc"
              >
                {{ ALL_SCOPE_LABELS[sc] || sc }}
              </option>
            </select>
          </div>
          <p class="scope-desc">
            Phím tắt chỉ có hiệu lực khi focus nằm trong phạm vi được chọn.
          </p>
        </div>

        <!-- Error Banner (Locked or Steals Typing) -->
        <div v-if="hasError" class="alert-banner error-banner">
          <AlertCircle :size="16" class="banner-icon" />
          <div class="banner-content">
            <div class="banner-title">Phím không thể gán</div>
            <p class="banner-desc">{{ errorMessage }}</p>
          </div>
        </div>

        <!-- Shell Clash Warning Banner -->
        <div v-else-if="shellWarning" class="alert-banner warning-banner">
          <AlertTriangle :size="16" class="banner-icon" />
          <div class="banner-content">
            <div class="banner-title">Cảnh báo phím dành cho shell</div>
            <p class="banner-desc">{{ shellWarning }}</p>
          </div>
        </div>

        <!-- Conflict Warning Banner -->
        <div v-if="conflictCommands.length > 0 && !hasError" class="alert-banner conflict-banner">
          <AlertTriangle :size="16" class="banner-icon" />
          <div class="banner-content">
            <div class="banner-title">Phím này đang được dùng bởi:</div>
            <ul class="conflict-list">
              <li v-for="(cTitle, idx) in conflictCommands" :key="idx">
                • {{ cTitle }}
              </li>
            </ul>
            <p class="conflict-desc">
              Lưu sẽ tạo xung đột trong phạm vi này (ưu tiên Người dùng & phạm vi hẹp hơn khi thực thi).
            </p>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <footer class="modal-footer">
        <button
          v-if="!isNewBinding && initialKey"
          class="btn-unbind"
          title="Gỡ bỏ phím tắt này"
          @click="handleRemove"
        >
          <Unlink :size="13" />
          <span>Gỡ bỏ phím</span>
        </button>
        <div v-else></div>

        <div class="footer-actions">
          <button class="btn btn-secondary btn-sm" @click="handleCancel">
            Hủy (Esc)
          </button>
          <button
            class="btn btn-primary btn-sm"
            :disabled="hasError || (!capturedChord && !isUnchanged)"
            @click="handleSave"
          >
            <span>{{ isUnchanged ? 'Không đổi' : 'Lưu (Enter)' }}</span>
          </button>
        </div>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount, nextTick } from 'vue';
import {
  Keyboard,
  X,
  Touchpad,
  Delete,
  Layers,
  AlertCircle,
  AlertTriangle,
  Unlink,
} from 'lucide-vue-next';
import type { KeyScope, ResolvedKeybinding } from '@/types/keybindings';
import {
  eventToChord,
  formatChordForDisplay,
  getLockedReason,
  stealsTyping,
  getShellWarning,
  normalizeKey,
  isImeComposing,
} from '@/keybindings/normalize';
import { pauseDispatcher, resumeDispatcher } from '@/keybindings/dispatcher';

const props = defineProps<{
  visible: boolean;
  commandId: string;
  title: string;
  initialKey?: string;
  initialWhen?: KeyScope;
  isNewBinding?: boolean;
  existingBindings: ResolvedKeybinding[];
  allowedScopes?: KeyScope[];
}>();

const emit = defineEmits<{
  (e: 'save', payload: {
    commandId: string;
    newKey: string;
    when: KeyScope;
    oldKey?: string;
    oldWhen?: KeyScope;
  }): void;
  (e: 'remove', payload: {
    commandId: string;
    key: string;
    when?: KeyScope;
  }): void;
  (e: 'close'): void;
}>();

const captureBoxRef = ref<HTMLDivElement | null>(null);
const isRecordingFocused = ref(false);
const capturedChord = ref<string>('');
const pendingModifiers = ref<string[]>([]);
const selectedScope = ref<KeyScope>('workspace');

const ALL_SCOPE_LABELS: Record<KeyScope, string> = {
  workspace: 'Workspace (Không gian làm việc)',
  global: 'Toàn ứng dụng (Global Navigation)',
  terminal: 'Terminal đang gõ (Terminal Active)',
  explorer: 'Cây nhóm lệnh (Explorer)',
  files: 'Trình quản lý file (Files)',
  suggest: 'Gợi ý lệnh (Suggest)',
  modal: 'Hộp thoại (Modal)',
};

const availableScopes = computed<KeyScope[]>(() => {
  if (props.allowedScopes && props.allowedScopes.length > 0) {
    return props.allowedScopes;
  }
  return ['workspace', 'global', 'terminal', 'explorer', 'files', 'suggest'];
});

watch(
  () => props.visible,
  (val) => {
    if (val) {
      pauseDispatcher();
      capturedChord.value = props.initialKey ? normalizeKey(props.initialKey) : '';
      const validScopes = availableScopes.value;
      if (props.initialWhen && validScopes.includes(props.initialWhen)) {
        selectedScope.value = props.initialWhen;
      } else {
        selectedScope.value = validScopes[0] || 'workspace';
      }
      pendingModifiers.value = [];
      nextTick(() => {
        captureBoxRef.value?.focus();
      });
    } else {
      resumeDispatcher();
    }
  },
  { immediate: true }
);

onMounted(() => {
  if (props.visible) {
    pauseDispatcher();
  }
});

onBeforeUnmount(() => {
  resumeDispatcher();
});

const formattedCapturedChord = computed(() => {
  return formatChordForDisplay(capturedChord.value);
});

const isUnchanged = computed(() => {
  if (!props.initialKey || props.isNewBinding) return false;
  return (
    capturedChord.value.toLowerCase() === props.initialKey.toLowerCase() &&
    selectedScope.value === props.initialWhen
  );
});

// Validation
const lockedReason = computed(() => {
  if (!capturedChord.value) return null;
  return getLockedReason(capturedChord.value);
});

const doesStealTyping = computed(() => {
  if (!capturedChord.value) return false;
  return stealsTyping(capturedChord.value);
});

const hasError = computed(() => {
  return Boolean(lockedReason.value || doesStealTyping.value);
});

const errorMessage = computed(() => {
  if (lockedReason.value) return lockedReason.value;
  if (doesStealTyping.value) {
    return 'Phím cần chứa Ctrl, Alt hoặc Cmd (trừ phím chức năng F1–F24).';
  }
  return '';
});

const shellWarning = computed(() => {
  if (!capturedChord.value || hasError.value) return null;
  return getShellWarning(capturedChord.value, selectedScope.value);
});

const conflictCommands = computed(() => {
  if (!capturedChord.value || hasError.value) return [];
  const normalized = capturedChord.value.toLowerCase();
  const conflicts: string[] = [];

  for (const b of props.existingBindings) {
    if (b.isRemoved) continue;
    if (b.key.toLowerCase() === normalized) {
      const overlap =
        b.when === selectedScope.value ||
        b.when === 'global' ||
        selectedScope.value === 'global' ||
        (b.when === 'workspace' && selectedScope.value === 'terminal') ||
        (b.when === 'terminal' && selectedScope.value === 'workspace');

      if (overlap && b.commandId !== props.commandId) {
        conflicts.push(`${b.title} (${b.when})`);
      }
    }
  }

  return conflicts;
});

const handleCaptureKeydown = (event: KeyboardEvent) => {
  if (isImeComposing(event)) return;

  // Single Escape: Cancel
  if (event.key === 'Escape' && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
    event.preventDefault();
    event.stopPropagation();
    handleCancel();
    return;
  }

  // Single Enter: Save
  if (event.key === 'Enter' && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
    event.preventDefault();
    event.stopPropagation();
    handleSave();
    return;
  }

  event.preventDefault();
  event.stopPropagation();

  const parsed = eventToChord(event);
  if (!parsed) return;

  if (parsed.isModifierOnly) {
    pendingModifiers.value = formatChordForDisplay(parsed.chord);
  } else {
    try {
      capturedChord.value = normalizeKey(parsed.chord);
      pendingModifiers.value = [];
    } catch {
      // Incomplete or invalid chord
    }
  }
};

const clearCaptured = () => {
  capturedChord.value = '';
  pendingModifiers.value = [];
  captureBoxRef.value?.focus();
};

const handleCancel = () => {
  emit('close');
};

const handleSave = () => {
  if (isUnchanged.value) {
    emit('close');
    return;
  }
  if (hasError.value || !capturedChord.value) return;

  emit('save', {
    commandId: props.commandId,
    newKey: capturedChord.value,
    when: selectedScope.value,
    oldKey: props.initialKey,
    oldWhen: props.initialWhen,
  });
  emit('close');
};

const handleRemove = () => {
  if (props.initialKey) {
    emit('remove', {
      commandId: props.commandId,
      key: props.initialKey,
      when: props.initialWhen,
    });
    emit('close');
  }
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background-color: rgba(11, 13, 19, 0.78);
  backdrop-filter: blur(8px);
}

.keybinding-modal-container {
  width: 100%;
  max-width: 460px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  box-shadow: 0 0 0 1px var(--border-subtle), 0 24px 48px -12px rgba(0, 0, 0, 0.85),
    0 0 32px 0 var(--primary-glow);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  animation: fadeInModal 0.15s ease-out;
}

@keyframes fadeInModal {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.hazard-stripe {
  height: 2.5px;
  width: 100%;
  background: linear-gradient(
    90deg,
    var(--primary) 0%,
    var(--primary-accent) 50%,
    var(--primary) 100%
  );
  flex-shrink: 0;
}

.modal-header {
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.header-title-wrap {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.icon-badge {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: var(--primary-alpha-20);
  border: 1px solid var(--primary-alpha-35);
  color: var(--primary-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-top: 2px;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.3;
}

.header-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
}

.command-id-mono {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: var(--text-muted);
}

.meta-dot {
  color: var(--border-subtle);
  font-size: 12px;
}

.badge-mod {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--primary-accent);
  background: var(--primary-alpha-20);
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid var(--primary-alpha-25);
}

.close-btn {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s ease;
}

.close-btn:hover {
  background: var(--bg-surface-hover);
  color: var(--text-primary);
  border-color: var(--border-subtle);
}

.modal-body {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.instructions-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-secondary);
}

.instruction-text {
  display: flex;
  align-items: center;
  gap: 6px;
}

.rec-badge {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
  font-weight: 600;
  letter-spacing: 0.5px;
}

/* Capture Box */
.capture-box {
  width: 100%;
  min-height: 64px;
  border-radius: 6px;
  background: var(--bg-terminal);
  border: 1.5px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  position: relative;
  padding: 10px 16px;
  outline: none;
  cursor: pointer;
  transition: all 0.15s ease;
}

.capture-box.is-focused {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary), 0 0 16px 2px var(--primary-glow);
}

.capture-box.has-error {
  border-color: var(--status-failed) !important;
  box-shadow: 0 0 0 1px var(--status-failed), 0 0 12px 2px var(--status-failed-bg) !important;
}

.keycaps-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.keycap {
  background: linear-gradient(180deg, var(--surface-container-high) 0%, var(--surface-container-low) 100%);
  box-shadow: 0 2px 0 0 var(--surface-container-lowest), 0 0 0 1px var(--outline), inset 0 1px 0 rgba(255, 255, 255, 0.15);
  border-radius: 4px;
  padding: 3px 8px;
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  font-weight: 600;
  color: var(--text-inverse);
  letter-spacing: 0.5px;
}

.keycap-plus {
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  font-weight: bold;
}

.empty-prompt {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--primary-accent);
  font-family: var(--font-mono, monospace);
}

.listening-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  font-size: 10px;
  color: var(--primary-accent);
  font-family: var(--font-mono, monospace);
}

.listening-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--primary-accent);
  animation: pulseDot 1.5s infinite;
}

@keyframes pulseDot {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.4;
    transform: scale(0.8);
  }
}

.clear-chord-btn {
  position: absolute;
  right: 10px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 4px;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  transition: all 0.15s ease;
}

.clear-chord-btn:hover {
  color: var(--text-inverse);
  background: var(--bg-surface);
}

/* Scope Selector */
.scope-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.scope-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.scope-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: 6px;
}

.scope-hint-mono {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
}

.select-wrapper {
  position: relative;
}

.scope-select {
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border-radius: 4px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  outline: none;
  cursor: pointer;
  transition: border-color 0.15s ease;
}

.scope-select:focus {
  border-color: var(--primary);
}

.scope-desc {
  font-size: 10px;
  color: var(--text-muted);
  margin: 0;
}

/* Alert Banners */
.alert-banner {
  border-radius: 6px;
  padding: 10px 12px;
  display: flex;
  gap: 10px;
  font-size: 11px;
}

.error-banner {
  background: var(--status-failed-bg);
  border: 1px solid var(--status-failed);
  color: var(--text-primary);
}

.error-banner .banner-icon {
  color: var(--status-failed);
  flex-shrink: 0;
  margin-top: 1px;
}

.warning-banner {
  background: var(--status-starting-bg);
  border: 1px solid var(--status-starting);
  color: var(--text-primary);
}

.warning-banner .banner-icon {
  color: var(--status-starting);
  flex-shrink: 0;
  margin-top: 1px;
}

.conflict-banner {
  background: var(--status-starting-bg);
  border: 1px solid var(--status-starting);
  color: var(--text-primary);
}

.conflict-banner .banner-icon {
  color: var(--status-starting);
  flex-shrink: 0;
  margin-top: 1px;
}

.banner-content {
  flex: 1;
}

.banner-title {
  font-weight: 600;
  font-size: 12px;
  margin-bottom: 2px;
}

.banner-desc {
  margin: 0;
  line-height: 1.4;
  opacity: 0.9;
}

.conflict-list {
  list-style: none;
  padding: 0;
  margin: 4px 0 6px 0;
  font-weight: 500;
}

.conflict-desc {
  margin: 0;
  font-size: 10px;
  opacity: 0.8;
}

/* Modal Footer */
.modal-footer {
  padding: 12px 20px;
  background: var(--bg-surface);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.btn-unbind {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 11px;
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  padding: 4px 6px;
  border-radius: 4px;
  transition: all 0.15s ease;
}

.btn-unbind:hover {
  color: var(--status-failed);
  background: var(--status-failed-bg);
}

.footer-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
  border: 1px solid transparent;
}

.btn-sm {
  height: 30px;
}

.btn-secondary {
  background: transparent;
  border-color: var(--border-subtle);
  color: var(--text-secondary);
}

.btn-secondary:hover {
  background: var(--bg-surface-hover);
  color: var(--text-primary);
}

.btn-primary {
  background: var(--primary);
  color: var(--text-inverse);
}

.btn-primary:hover:not(:disabled) {
  background: var(--primary-hover);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
