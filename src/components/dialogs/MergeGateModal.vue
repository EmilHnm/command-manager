<template>
  <div v-if="visible && detail" class="modal-backdrop" @keydown.esc="handleClose">
    <div class="merge-gate-modal" role="dialog" aria-modal="true" aria-labelledby="merge-gate-title">
      <!-- Top Accent Bar -->
      <div class="modal-accent-bar"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge gate-badge">
            <span class="gate-icon">⛩</span>
          </div>
          <div class="header-titles">
            <h3 id="merge-gate-title" class="modal-title" :title="detail.run.title">
              Cổng 2: Duyệt Merge — "{{ detail.run.title }}"
            </h3>

            <!-- Dedicated Route Row (Wraps cleanly, never pushes close button out) -->
            <div class="branch-route-box custom-scrollbar">
              <span class="route-source font-mono">
                <GitBranch :size="12" />
                <code>{{ detail.run.integration_branch }}</code>
              </span>
              <span class="route-arrow">→</span>
              <div class="route-targets font-mono">
                <template v-if="multi">
                  <code
                    v-for="repo in detail.run.repos"
                    :key="repo.path"
                    class="repo-branch-pill"
                    :title="`${repo.path}:${repo.base_branch}`"
                  >
                    <span class="repo-name">{{ repo.path }}:</span><span class="branch-name">{{ repo.base_branch }}</span>
                  </code>
                </template>
                <code v-else class="repo-branch-pill">{{ detail.run.base_branch }}</code>
              </div>
            </div>

            <span class="modal-subtitle">
              Các task đã xong trên nhánh tích hợp. Chọn cách đưa vào nhánh gốc cục bộ.
            </span>
          </div>
        </div>
        <button class="btn btn-ghost btn-icon close-btn" title="Đóng (Esc)" @click="handleClose">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body custom-scrollbar">
        <div v-if="error" class="safety-note merge-error">
          <XCircle :size="14" />
          <span>{{ error }}</span>
        </div>

        <!-- Telemetry & Metrics Ribbon -->
        <div class="summary-card">
          <div class="metric-item">
            <span class="metric-value font-mono text-primary">{{ mergedCount }}</span>
            <span class="metric-label">tasks xong</span>
          </div>
          <div class="metric-divider"></div>
          <div class="metric-item">
            <span class="metric-value font-mono">{{ diffstat?.files ?? '…' }}</span>
            <span class="metric-label">tệp thay đổi</span>
          </div>
          <div class="metric-divider"></div>
          <div class="metric-item">
            <span class="metric-value font-mono text-status-running">
              +{{ diffstat?.added ?? 0 }} / -{{ diffstat?.removed ?? 0 }}
            </span>
            <span class="metric-label">dòng code</span>
          </div>
          <div class="metric-divider"></div>
          <div class="metric-item">
            <span class="metric-value font-mono">{{ reviewRounds }}</span>
            <span class="metric-label">vòng review</span>
          </div>
          <div class="metric-divider"></div>
          <div class="metric-item">
            <span class="metric-value font-mono text-amber-400">{{ formatTokens(totalTokens) }}</span>
            <span class="metric-label">
              tokens{{ detail.usage.cost_usd > 0 ? ` (~$${detail.usage.cost_usd.toFixed(2)})` : '' }}
            </span>
          </div>
        </div>

        <!-- Automated Check Suite -->
        <div class="suite-card">
          <div class="suite-title">
            <ShieldCheck :size="15" class="text-status-running" />
            <span>Kết quả kiểm tra tự động trên nhánh tích hợp:</span>
          </div>
          <div class="suite-badges font-mono">
            <span
              v-for="check in detail.run.integration_checks || []"
              :key="check.command"
              class="suite-badge"
              :class="check.ok ? 'pass' : 'fail'"
              :title="check.ok ? undefined : check.output"
            >
              {{ check.command }} ({{ (check.duration_ms / 1000).toFixed(1) }}s) {{ check.ok ? '✓' : '✗' }}
            </span>
            <span v-if="!(detail.run.integration_checks || []).length" class="suite-badge">
              Chưa khai báo lệnh kiểm tra (Đội & CLI → Dự án)
            </span>
          </div>
        </div>

        <!-- Task Breakdown Review List -->
        <div class="tasks-breakdown">
          <div class="breakdown-title">Chi tiết nghiệm thu từng Task:</div>

          <div class="breakdown-list">
            <div
              v-for="task in detail.tasks"
              :key="task.id"
              class="breakdown-item"
              :class="{ 'has-warning': shouldCount(task) > 0 || task.status !== 'merged' }"
            >
              <div class="item-status" :class="task.status === 'merged' ? 'pass' : 'skip'">
                {{ task.status === 'merged' ? '✓' : '–' }}
              </div>
              <div class="item-content">
                <div class="item-header">
                  <span class="item-key font-mono">{{ task.key }}</span>
                  <span class="item-text">{{ task.title }}</span>
                  <span class="item-diff font-mono text-status-running">+{{ task.added }} / -{{ task.removed }}</span>
                </div>
                <div v-if="task.status !== 'merged'" class="item-meta text-warning">
                  {{ TASK_STATUS_LABELS[task.status] }}{{ task.status_note ? ` — ${task.status_note}` : '' }}
                </div>
                <div v-else class="item-meta" :class="shouldCount(task) > 0 ? 'text-warning' : 'text-muted'">
                  {{ task.last_review?.verdict === 'approved' ? `Leader duyệt ở vòng ${task.last_review.round}` : 'Merge không qua review' }}
                  ({{ task.cli }})
                  <template v-if="shouldCount(task) > 0"> • ⚠ Còn {{ shouldCount(task) }} góp ý mức "should"</template>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Inspection Tools -->
        <div class="tools-bar">
          <button type="button" class="btn btn-secondary btn-sm" @click="inspectDiff">
            <FileDiff :size="13" />
            <span>Xem toàn bộ diff</span>
          </button>
          <button type="button" class="btn btn-secondary btn-sm" @click="openBranchTerminal">
            <Terminal :size="13" />
            <span>Mở nhánh trong Terminal</span>
          </button>
        </div>

        <!-- Safe Merge Notice -->
        <div class="safety-note">
          <GitMerge :size="14" class="text-primary" />
          <span>
            Merge chạy trong repo của bạn (local <code>--no-ff</code>), không bao giờ <code>git push</code>.
            <template v-if="multi">
              Mỗi repo có thay đổi phải đang ở nhánh gốc của nó và không có thay đổi chưa commit; app kiểm tra hết
              trước khi merge repo đầu tiên.
            </template>
            <template v-else>
              Repo phải đang ở nhánh <code>{{ detail.run.base_branch }}</code> và không có thay đổi chưa commit.
            </template>
            Worktree của các task được dọn sau khi merge.
          </span>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="modal-footer">
        <button type="button" class="btn btn-danger-ghost" :disabled="busy" @click="handleDiscard">
          <XCircle :size="14" />
          <span>Bỏ lượt chạy</span>
        </button>

        <div class="right-actions">
          <!-- Dropdown for other actions -->
          <div class="action-dropdown-container" ref="actionDropdownRef">
            <button
              type="button"
              class="btn btn-secondary action-dropdown-trigger"
              :disabled="busy"
              @click.stop="toggleActionMenu"
            >
              <MoreHorizontal :size="15" />
              <span>Tùy chọn khác</span>
              <span v-if="hasPr" class="dropdown-pr-badge font-mono">PR</span>
              <ChevronUp v-if="showActionMenu" :size="13" />
              <ChevronDown v-else :size="13" />
            </button>

            <!-- Dropdown Menu (opens upward) -->
            <div v-if="showActionMenu" class="action-dropdown-menu custom-scrollbar" @click.stop>
              <!-- Push & tạo PR / Push lên PR hiện tại -->
              <button
                type="button"
                class="dropdown-action-item"
                :disabled="busy"
                @click="handleAction(() => emit('open-pr'))"
              >
                <div class="item-icon-box text-primary">
                  <GitPullRequest :size="15" />
                </div>
                <div class="item-content">
                  <span class="item-title">{{ hasPr ? 'Push lên PR hiện tại' : 'Push & tạo PR' }}</span>
                  <span class="item-desc">
                    {{ hasPr ? 'Push các commit mới lên nhánh PR hiện tại' : 'Push nhánh tích hợp lên origin và mở PR' }}
                  </span>
                </div>
              </button>

              <!-- Đã có PR -->
              <button
                type="button"
                class="dropdown-action-item"
                :disabled="busy"
                @click="handleAction(() => emit('attach-pr'))"
              >
                <div class="item-icon-box text-amber-400">
                  <Link :size="15" />
                </div>
                <div class="item-content">
                  <span class="item-title">Đã có PR…</span>
                  <span class="item-desc">Gắn link PR đã tạo bằng tay vào lượt chạy</span>
                </div>
              </button>

              <!-- Giữ nhánh -->
              <button
                type="button"
                class="dropdown-action-item"
                :disabled="busy"
                @click="handleAction(handleKeepBranch)"
              >
                <div class="item-icon-box text-cyan-400">
                  <GitBranch :size="15" />
                </div>
                <div class="item-content">
                  <span class="item-title">Giữ nhánh, tự xử lý</span>
                  <span class="item-desc">Giữ nguyên nhánh tích hợp, không merge cục bộ</span>
                </div>
              </button>

              <div class="dropdown-sep"></div>

              <!-- Bỏ lượt chạy inside dropdown as secondary option -->
              <button
                type="button"
                class="dropdown-action-item item-danger"
                :disabled="busy"
                @click="handleAction(handleDiscard)"
              >
                <div class="item-icon-box text-red-400">
                  <XCircle :size="15" />
                </div>
                <div class="item-content">
                  <span class="item-title">Bỏ lượt chạy</span>
                  <span class="item-desc">Xóa nhánh tích hợp và dọn dẹp worktree</span>
                </div>
              </button>
            </div>
          </div>

          <!-- Primary Merge Button -->
          <button
            type="button"
            class="btn btn-primary btn-merge"
            :disabled="busy"
            @click="handleMerge"
          >
            <Check :size="15" />
            <span>{{ multi ? `Merge vào nhánh gốc của ${detail.run.repos.length} repo (local)` : `Merge vào ${detail.run.base_branch} (local)` }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Inside the backdrop so it stacks above this modal. -->
    <ConfirmDialog
      :visible="confirmDiscard"
      title="Bỏ Lượt Chạy"
      subtitle="MOD-23 • Cổng 2"
      :message="discardMessage"
      confirm-text="Bỏ lượt chạy"
      loading-text="Đang dọn nhánh và worktree..."
      danger
      :loading="busy"
      @confirm="discardConfirmed"
      @cancel="confirmDiscard = false"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import {
  X,
  ShieldCheck,
  FileDiff,
  Terminal,
  GitMerge,
  GitBranch,
  XCircle,
  Check,
  GitPullRequest,
  MoreHorizontal,
  ChevronDown,
  ChevronUp,
  Link,
} from 'lucide-vue-next';
import type { AgentTask, DiffStat, RunDetail } from '@/types/agents';
import { TASK_STATUS_LABELS, isMultiRepo } from '@/types/agents';
import { ipcClient } from '@/ipc/client';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';

const props = defineProps<{
  visible: boolean;
  detail: RunDetail | null;
  merge: () => Promise<void>;
  keep: () => Promise<void>;
  discard: () => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'inspect-diff'): void;
  (e: 'open-pr'): void;
  (e: 'attach-pr'): void;
  (e: 'open-terminal'): void;
}>();

const diffstat = ref<DiffStat | null>(null);
const busy = ref(false);
const error = ref<string | null>(null);

const showActionMenu = ref(false);
const actionDropdownRef = ref<HTMLElement | null>(null);

function toggleActionMenu() {
  showActionMenu.value = !showActionMenu.value;
}

function handleAction(actionFn: () => void) {
  showActionMenu.value = false;
  actionFn();
}

function handleClickOutside(e: MouseEvent) {
  if (actionDropdownRef.value && !actionDropdownRef.value.contains(e.target as Node)) {
    showActionMenu.value = false;
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});

watch(
  () => props.visible,
  (visible) => {
    if (!visible || !props.detail) {
      showActionMenu.value = false;
      return;
    }
    showActionMenu.value = false;
    error.value = null;
    diffstat.value = null;
    confirmDiscard.value = false;
    void ipcClient
      .agentsRunDiffstat(props.detail.run.id)
      .then((stat) => (diffstat.value = stat))
      .catch(() => (diffstat.value = null));
  },
  { immediate: true }
);

const multi = computed(() => !!props.detail && isMultiRepo(props.detail.run.repos));

const mergedCount = computed(() => props.detail?.tasks.filter((t) => t.status === 'merged').length ?? 0);

/** Every review a task went through, approvals included. */
const reviewRounds = computed(
  () => props.detail?.invocations.filter((inv) => inv.step === 'review').length ?? 0
);

const totalTokens = computed(() => (props.detail ? props.detail.usage.input_tokens + props.detail.usage.output_tokens : 0));

const formatTokens = (n: number) => (n >= 1000 ? `${Math.round(n / 1000)}k` : String(n));

const shouldCount = (task: AgentTask) =>
  task.last_review?.comments.filter((c) => c.severity === 'should').length ?? 0;

function handleClose() {
  emit('close');
}

async function act(action: () => Promise<void>) {
  busy.value = true;
  error.value = null;
  try {
    await action();
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

function handleMerge() {
  void act(props.merge);
}

function handleKeepBranch() {
  void act(props.keep);
}

/** The run already has pull requests: updating them is the way on. */
const hasPr = computed(() => (props.detail?.run.pull_requests.length ?? 0) > 0);

const confirmDiscard = ref(false);

const discardMessage = computed(() => {
  const run = props.detail?.run;
  if (!run) return '';
  const repos = run.repos.length > 1 ? ` trong ${run.repos.length} repo` : '';
  return `Bỏ lượt chạy "${run.title}"? Nhánh tích hợp ${run.integration_branch}, các nhánh task và mọi worktree${repos} sẽ bị xóa. Nhánh gốc của bạn không bị đụng tới.`;
});

function handleDiscard() {
  confirmDiscard.value = true;
}

async function discardConfirmed() {
  await act(props.discard);
  confirmDiscard.value = false;
}

function inspectDiff() {
  emit('inspect-diff');
}

function openBranchTerminal() {
  emit('open-terminal');
}
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(12, 14, 20, 0.75);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 1rem;
}

.merge-gate-modal {
  width: 100%;
  max-width: 760px;
  max-height: 85vh;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: var(--radius-lg, 8px);
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalIn 0.15s ease-out;
}

@keyframes modalIn {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.modal-accent-bar {
  height: 3px;
  background: linear-gradient(90deg, #10b981 0%, #4cd7f6 40%, #744791 100%);
}

.modal-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
}

.header-left {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 0;
  flex: 1;
}

.gate-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background-color: rgba(16, 185, 129, 0.15);
  border: 1px solid rgba(16, 185, 129, 0.35);
  flex-shrink: 0;
  margin-top: 2px;
}

.gate-icon {
  font-size: 16px;
  color: #10b981;
}

.header-titles {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
  flex: 1;
}

.modal-title {
  font-size: 13.5px;
  font-weight: 600;
  line-height: 1.35;
  color: var(--text-primary, #f1f5f9);
  word-break: break-word;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.branch-route-box {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  padding: 4px 8px;
  background-color: rgba(15, 17, 23, 0.6);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 5px;
  max-width: 100%;
  max-height: 60px;
  overflow-y: auto;
}

.route-source {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: #e4b5ff;
}

.route-source code {
  font-size: 11px;
  background-color: rgba(116, 71, 145, 0.2);
  border: 1px solid rgba(116, 71, 145, 0.4);
  padding: 1px 6px;
  border-radius: 3px;
  color: #e4b5ff;
  white-space: nowrap;
}

.route-arrow {
  color: var(--text-muted, #64748b);
  font-size: 12px;
  user-select: none;
}

.route-targets {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
}

.repo-branch-pill {
  display: inline-flex;
  align-items: center;
  font-size: 11px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 3px;
  padding: 1px 6px;
  white-space: nowrap;
}

.repo-branch-pill .repo-name {
  color: var(--text-secondary, #94a3b8);
  margin-right: 2px;
}

.repo-branch-pill .branch-name {
  color: #34d399;
  font-weight: 500;
}

.close-btn {
  flex-shrink: 0;
  margin-top: 2px;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  line-height: 1.35;
}

.modal-body {
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.summary-card {
  display: flex;
  align-items: center;
  justify-content: space-around;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  padding: 10px 12px;
}

.metric-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}

.metric-value {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary, #f1f5f9);
}

.metric-label {
  font-size: 10px;
  color: var(--text-muted, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.02em;
}

.metric-divider {
  width: 1px;
  height: 24px;
  background-color: var(--border-subtle, #242a3e);
}

.suite-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 12px;
  background-color: rgba(16, 185, 129, 0.06);
  border: 1px solid rgba(16, 185, 129, 0.2);
  border-radius: 6px;
}

.suite-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
}

.suite-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.suite-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
}

.suite-badge.pass {
  background-color: rgba(16, 185, 129, 0.12);
  color: #34d399;
  border: 1px solid rgba(16, 185, 129, 0.25);
}

.tasks-breakdown {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.breakdown-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.breakdown-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.breakdown-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.breakdown-item.has-warning {
  border-color: rgba(245, 158, 11, 0.3);
}

.item-status.pass {
  color: #10b981;
  font-weight: 700;
  font-size: 13px;
  margin-top: 1px;
}

.item-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.item-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.item-key {
  font-weight: 700;
  color: var(--text-primary, #f1f5f9);
}

.item-text {
  color: var(--text-secondary, #94a3b8);
}

.item-diff {
  margin-left: auto;
  font-size: 11px;
}

.item-meta {
  font-size: 11px;
}

.item-meta.text-muted {
  color: var(--text-muted, #64748b);
}

.item-meta.text-warning {
  color: #fbbf24;
}

.tools-bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.safety-note {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  padding: 6px 10px;
  background-color: rgba(116, 71, 145, 0.08);
  border: 1px solid rgba(116, 71, 145, 0.2);
  border-radius: 4px;
  line-height: 15px;
}

.safety-note code {
  color: #e4b5ff;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
}

.right-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Dropdown Container */
.action-dropdown-container {
  position: relative;
}

.action-dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
}

.dropdown-pr-badge {
  font-size: 9px;
  padding: 1px 4px;
  background-color: rgba(76, 215, 246, 0.15);
  border: 1px solid rgba(76, 215, 246, 0.3);
  color: #4cd7f6;
  border-radius: 3px;
  font-weight: 600;
  line-height: 1;
}

/* Action Dropdown Menu - opens UPWARD from footer */
.action-dropdown-menu {
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  min-width: 290px;
  max-width: 360px;
  max-height: 320px;
  background-color: var(--surface-container-high, #1e2230);
  border: 1px solid var(--border-subtle, #2e354a);
  border-radius: 8px;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5), 0 8px 10px -6px rgba(0, 0, 0, 0.4);
  padding: 5px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  z-index: 50;
  animation: dropdownUp 0.15s ease-out;
}

@keyframes dropdownUp {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.dropdown-action-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  text-align: left;
  width: 100%;
  color: var(--text-primary, #f1f5f9);
  transition: background-color 0.15s;
}

.dropdown-action-item:hover:not(:disabled) {
  background-color: rgba(255, 255, 255, 0.06);
}

.dropdown-action-item:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.item-icon-box {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 5px;
  background-color: rgba(255, 255, 255, 0.04);
  flex-shrink: 0;
  margin-top: 1px;
}

.dropdown-action-item .item-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}

.dropdown-action-item .item-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
  line-height: 1.25;
}

.dropdown-action-item .item-desc {
  font-size: 10.5px;
  color: var(--text-secondary, #94a3b8);
  line-height: 1.3;
}

.dropdown-sep {
  height: 1px;
  background-color: var(--border-subtle, #242a3e);
  margin: 4px 6px;
}

.dropdown-action-item.item-danger:hover:not(:disabled) {
  background-color: rgba(239, 68, 68, 0.1);
}

.dropdown-action-item.item-danger .item-title {
  color: #ef4444;
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 500;
  border-radius: 4px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-sm {
  font-size: 11px;
  padding: 4px 8px;
}

.btn-primary {
  background-color: #744791;
  color: #ffffff;
}

.btn-primary:hover {
  background-color: #8956aa;
}

.btn-merge {
  padding-left: 16px;
  padding-right: 16px;
  font-weight: 600;
}

.btn-secondary {
  background-color: var(--bg-app-base, #0f1117);
  border-color: var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
}

.btn-secondary:hover {
  background-color: var(--surface-container-high, #23283a);
}

.btn-danger-ghost {
  background: transparent;
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.2);
}

.btn-danger-ghost:hover {
  background-color: rgba(239, 68, 68, 0.1);
  border-color: rgba(239, 68, 68, 0.4);
}

.btn-ghost {
  background: transparent;
  color: var(--text-secondary, #94a3b8);
}

.btn-ghost:hover {
  background-color: rgba(255, 255, 255, 0.05);
  color: var(--text-primary, #f1f5f9);
}

.btn-icon {
  width: 28px;
  height: 28px;
  padding: 0;
}

.suite-badge.fail {
  color: var(--status-failed, #ef4444);
  border-color: rgba(239, 68, 68, 0.4);
  background-color: rgba(239, 68, 68, 0.08);
}

.item-status.skip {
  color: var(--text-muted, #64748b);
}

.merge-error {
  color: var(--status-failed, #ef4444);
  border-color: rgba(239, 68, 68, 0.4);
}
</style>
