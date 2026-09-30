<template>
  <div class="quick-access-panel" :class="{ collapsed }">
    <!-- Panel Header (always visible, 28px height when collapsed) -->
    <div class="qa-header" @click="$emit('toggle-collapse')">
      <div class="qa-header-left">
        <Zap :size="13" class="qa-icon" />
        <span class="qa-title">QUICK ACCESS</span>
        <span class="qa-count-badge">({{ commands.length }})</span>
      </div>
      <button
        type="button"
        class="qa-collapse-btn"
        :title="collapsed ? 'Mở rộng Quick Access' : 'Thu gọn Quick Access'"
        @click.stop="$emit('toggle-collapse')"
      >
        <ChevronDown :size="14" class="collapse-icon" :class="{ rotated: collapsed }" />
      </button>
    </div>

    <!-- Panel Content (hidden when collapsed) -->
    <div v-if="!collapsed" class="qa-body">
      <!-- Target Terminal Indicator Strip -->
      <div v-if="target" class="qa-target-strip">
        <div class="target-label-group">
          <span class="target-arrow">→</span>
          <span class="target-name" :title="target.label">{{ target.label }}</span>
          <span class="target-dot">·</span>
          <span class="target-pane">{{ target.pane === 'paneB' ? 'Khung B' : 'Khung A' }}</span>
        </div>
        <div class="target-status-group">
          <template v-if="target.status !== 'running'">
            <span class="target-status stopped">(đã dừng)</span>
          </template>
          <template v-else-if="target.busy">
            <span class="status-pulse-dot warning" />
            <span class="target-status busy">đang bận</span>
          </template>
          <template v-else>
            <span class="status-pulse-dot running" />
            <span class="target-status running">Active</span>
          </template>
        </div>
      </div>

      <!-- Quick Filter Search (shown when commands count > 8) -->
      <div v-if="commands.length > 8" class="qa-search-box">
        <Search :size="12" class="qa-search-icon" />
        <input
          v-model="searchQuery"
          type="text"
          class="qa-search-input"
          placeholder="Lọc lệnh nhanh..."
        />
      </div>

      <!-- Command Cards List -->
      <div class="qa-cards-container">
        <!-- Empty State -->
        <div v-if="commands.length === 0" class="qa-empty-state">
          <Zap :size="24" class="empty-icon" />
          <p class="empty-text">Chưa có lệnh Quick Access.</p>
          <p class="empty-subtext">Bật switch ⚡ trong màn Lệnh để ghim chạy nhanh.</p>
          <button type="button" class="btn btn-secondary btn-sm mt-2" @click="goToCommandsLibrary">
            <ExternalLink :size="12" />
            <span>Mở màn Lệnh</span>
          </button>
        </div>

        <!-- Filter Empty State -->
        <div v-else-if="filteredCommands.length === 0" class="qa-no-results">
          <span>Không tìm thấy lệnh phù hợp</span>
        </div>

        <!-- Card Items -->
        <div
          v-for="cmd in filteredCommands"
          v-else
          :key="cmd.id"
          class="qa-card"
          :title="itemTooltip(cmd)"
          tabindex="0"
          @dblclick="$emit('paste', cmd)"
          @keydown.enter.self="$emit('paste', cmd)"
        >
          <!-- Card Row 1: Name, Badges & Action Buttons -->
          <div class="qa-card-row-top">
            <div class="qa-card-name-group">
              <span class="qa-card-name" :title="cmd.name">{{ cmd.name }}</span>
              <span
                v-if="hasWarning(cmd)"
                class="qa-warning-badge"
                :title="warningTooltip(cmd)"
              >
                <AlertTriangle :size="11" />
              </span>
            </div>

            <div class="qa-actions">
              <button
                type="button"
                class="qa-btn qa-run-btn"
                :disabled="target?.status !== 'running'"
                :title="runButtonTooltip(cmd)"
                :aria-label="runButtonTooltip(cmd)"
                @click.stop="$emit('run', cmd)"
              >
                <Play :size="12" />
              </button>
              <button
                type="button"
                class="qa-btn qa-paste-btn"
                :disabled="target?.status !== 'running'"
                :title="pasteButtonTooltip(cmd)"
                :aria-label="pasteButtonTooltip(cmd)"
                @click.stop="$emit('paste', cmd)"
              >
                <ClipboardPaste :size="12" />
              </button>
            </div>
          </div>

          <!-- Card Row 2: Code Preview -->
          <div class="qa-card-row-bottom">
            <code class="qa-code-line">{{ getFirstLine(cmd.execution_string) }}</code>
            <span v-if="getExtraLinesCount(cmd.execution_string) > 0" class="qa-multiline-tag">
              ↵ +{{ getExtraLinesCount(cmd.execution_string) }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router';
import {
  Zap,
  ChevronDown,
  Search,
  Play,
  ClipboardPaste,
  AlertTriangle,
  ExternalLink,
} from 'lucide-vue-next';
import type { CommandDefinition } from '@/types/models';
import type { QuickAccessTarget } from '@/components/terminal/DockHost.vue';
import { useQuickAccess } from '@/composables/useQuickAccess';

const props = withDefaults(
  defineProps<{
    target: QuickAccessTarget | null;
    collapsed?: boolean;
  }>(),
  {
    collapsed: false,
  }
);

defineEmits<{
  (e: 'run', cmd: CommandDefinition): void;
  (e: 'paste', cmd: CommandDefinition): void;
  (e: 'toggle-collapse'): void;
}>();

const router = useRouter();
const { commands, searchQuery, filteredCommands } = useQuickAccess();

const goToCommandsLibrary = () => {
  router.push('/commands');
};

const getFirstLine = (text: string) => {
  if (!text) return '';
  const first = text.split(/\r?\n/)[0];
  return first.trim();
};

const getExtraLinesCount = (text: string) => {
  if (!text) return 0;
  const lines = text.split(/\r?\n/).filter((l) => l.trim().length > 0);
  return Math.max(0, lines.length - 1);
};

const KNOWN_SHELLS = new Set(['bash', 'zsh', 'sh', 'fish', 'pwsh', 'powershell', 'cmd', 'nu']);

const isShellMismatch = (cmd: CommandDefinition): boolean => {
  if (!props.target?.shellKind || !cmd.shell_kind) return false;
  const targetShell = props.target.shellKind.toLowerCase();
  const cmdShell = cmd.shell_kind.toLowerCase();
  if (targetShell === 'command') return false;
  if (!KNOWN_SHELLS.has(targetShell) || !KNOWN_SHELLS.has(cmdShell)) return false;
  return cmdShell !== targetShell;
};

const hasWarning = (cmd: CommandDefinition): boolean => {
  if (!props.target) return false;
  // Shell mismatch (only between known shell types, ignore 'command')
  if (isShellMismatch(cmd)) {
    return true;
  }
  // Multiline command on shell without bracketed paste
  const isMultiline = cmd.execution_string.includes('\n') || cmd.execution_string.includes('\r');
  const targetShell = (props.target.shellKind || '').toLowerCase();
  if (isMultiline && (targetShell === 'sh' || targetShell === 'cmd')) {
    return true;
  }
  return false;
};

const warningTooltip = (cmd: CommandDefinition): string => {
  if (!props.target) return '';
  if (isShellMismatch(cmd)) {
    return `Lệnh được lưu cho ${cmd.shell_kind}, terminal đích là ${props.target.shellKind}`;
  }
  const isMultiline = cmd.execution_string.includes('\n') || cmd.execution_string.includes('\r');
  const targetShell = (props.target.shellKind || '').toLowerCase();
  if (isMultiline && (targetShell === 'sh' || targetShell === 'cmd')) {
    return 'Paste sẽ chạy từng dòng do shell thiếu bracketed paste';
  }
  return '';
};

const itemTooltip = (cmd: CommandDefinition): string => {
  const typeStr = cmd.is_shell ? `Shell: ${cmd.shell_kind || 'mặc định'}` : 'Direct Argv';
  return `${cmd.name} (${typeStr})\n${cmd.execution_string}\n(Nháy đúp để dán vào terminal)`;
};

const runButtonTooltip = (cmd: CommandDefinition): string => {
  if (props.target?.status !== 'running') {
    return 'Tiến trình của tab đích đã dừng';
  }
  const targetName = props.target?.label || 'terminal đích';
  return `Run: Chạy "${cmd.name}" trong ${targetName}`;
};

const pasteButtonTooltip = (cmd: CommandDefinition): string => {
  if (props.target?.status !== 'running') {
    return 'Tiến trình của tab đích đã dừng';
  }
  const targetName = props.target?.label || 'terminal đích';
  return `Paste: Dán "${cmd.name}" vào ${targetName}`;
};
</script>

<style scoped>
.quick-access-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-surface-container-low, #191b22);
  border-top: 1px solid var(--border-subtle, #242a3e);
  overflow: hidden;
  user-select: none;
}

.quick-access-panel.collapsed {
  height: 28px !important;
  min-height: 28px !important;
}

/* Header */
.qa-header {
  height: 28px;
  min-height: 28px;
  background-color: var(--bg-surface-container, #1e1f26);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  cursor: pointer;
}

.qa-header:hover {
  background-color: var(--bg-surface-hover, #23283a);
}

.qa-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.qa-icon {
  color: var(--status-warning, #f59e0b);
}

.qa-title {
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--text-primary, #f1f5f9);
}

.qa-count-badge {
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 10px;
  font-weight: 600;
  color: var(--primary, #e4b5ff);
  background-color: var(--primary-subtle, rgba(116, 71, 145, 0.2));
  padding: 1px 5px;
  border-radius: 4px;
}

.qa-collapse-btn {
  background: transparent;
  border: none;
  color: var(--text-muted, #64748b);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 2px;
  border-radius: 3px;
  transition: all 0.15s ease;
}

.qa-collapse-btn:hover {
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-surface-hover, #23283a);
}

.collapse-icon {
  transition: transform 0.2s ease;
}

.collapse-icon.rotated {
  transform: rotate(-90deg);
}

/* Body */
.qa-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* Target Strip */
.qa-target-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 10px;
  background-color: var(--bg-terminal, #0b0d13);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 10.5px;
  flex-shrink: 0;
}

.target-label-group {
  display: flex;
  align-items: center;
  gap: 5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.target-arrow {
  color: var(--primary, #e4b5ff);
  font-weight: bold;
}

.target-name {
  color: var(--text-primary, #f1f5f9);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.target-dot {
  color: var(--text-muted, #64748b);
}

.target-pane {
  color: var(--text-secondary, #94a3b8);
  font-size: 10px;
}

.target-status-group {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.status-pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.status-pulse-dot.running {
  background-color: var(--status-running, #10b981);
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
}

.status-pulse-dot.warning {
  background-color: var(--status-warning, #f59e0b);
  box-shadow: 0 0 6px rgba(245, 158, 11, 0.6);
}

.target-status {
  font-size: 9.5px;
  font-weight: 500;
}

.target-status.running {
  color: var(--status-running, #10b981);
}

.target-status.busy {
  color: var(--status-warning, #f59e0b);
}

.target-status.stopped {
  color: var(--status-failed, #ef4444);
}

/* Search Box */
.qa-search-box {
  position: relative;
  padding: 4px 8px;
  background-color: var(--bg-surface-container-low, #191b22);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  flex-shrink: 0;
}

.qa-search-icon {
  position: absolute;
  left: 14px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-muted, #64748b);
  pointer-events: none;
}

.qa-search-input {
  width: 100%;
  background-color: var(--bg-terminal, #0b0d13);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  color: var(--text-primary, #f1f5f9);
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 10px;
  padding: 3px 6px 3px 24px;
  outline: none;
  transition: border-color 0.15s ease;
}

.qa-search-input:focus {
  border-color: var(--border-focus, #744791);
}

/* Cards Container */
.qa-cards-container {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

/* Card */
.qa-card {
  background-color: var(--bg-surface-container, #1e1f26);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  padding: 5px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.qa-card:hover {
  background-color: var(--bg-surface-hover, #23283a);
  border-color: rgba(116, 71, 145, 0.45);
}

.qa-card:focus-visible {
  outline: 1px solid var(--border-focus, #744791);
}

.qa-card-row-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.qa-card-name-group {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  flex: 1;
}

.qa-card-name {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qa-warning-badge {
  display: inline-flex;
  align-items: center;
  color: var(--status-warning, #f59e0b);
  background-color: rgba(245, 158, 11, 0.15);
  border-radius: 3px;
  padding: 1px 3px;
}

.qa-actions {
  display: flex;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
}

.qa-btn {
  width: 22px;
  height: 22px;
  border-radius: 3px;
  border: 1px solid transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: transparent;
  color: var(--text-secondary, #94a3b8);
  cursor: pointer;
  transition: all 0.15s ease;
}

.qa-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.qa-run-btn:not(:disabled):hover {
  color: #fff;
  background-color: var(--primary, #744791);
  border-color: var(--primary-glow, rgba(116, 71, 145, 0.5));
}

.qa-paste-btn:not(:disabled):hover {
  color: var(--primary, #e4b5ff);
  background-color: var(--primary-subtle, rgba(116, 71, 145, 0.2));
  border-color: rgba(116, 71, 145, 0.4);
}

.qa-card-row-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  min-width: 0;
}

.qa-code-line {
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 10px;
  color: var(--text-muted, #64748b);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.qa-multiline-tag {
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  font-size: 9px;
  font-weight: 600;
  color: var(--tertiary, #4cd7f6);
  background-color: rgba(76, 215, 246, 0.15);
  border-radius: 3px;
  padding: 1px 4px;
  flex-shrink: 0;
}

/* Empty State */
.qa-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 12px;
  text-align: center;
  color: var(--text-muted, #64748b);
}

.empty-icon {
  color: var(--status-warning, #f59e0b);
  opacity: 0.7;
  margin-bottom: 8px;
}

.empty-text {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
  margin: 0;
}

.empty-subtext {
  font-size: 10.5px;
  color: var(--text-muted, #64748b);
  margin: 4px 0 0 0;
}

.qa-no-results {
  padding: 16px;
  text-align: center;
  font-size: 11px;
  color: var(--text-muted, #64748b);
}
</style>
