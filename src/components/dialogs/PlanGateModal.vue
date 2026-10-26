<template>
  <div v-if="visible" class="modal-backdrop" @keydown.esc="handleClose">
    <div class="plan-gate-modal" role="dialog" aria-modal="true" aria-labelledby="plan-gate-title">
      <!-- Top Accent Bar -->
      <div class="modal-accent-bar"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge gate-badge">
            <span class="gate-icon">⛩</span>
          </div>
          <div class="header-titles">
            <h3 id="plan-gate-title" class="modal-title">
              Cổng 1: Duyệt Thiết Kế & Kế Hoạch — "{{ detail?.run.title }}"
            </h3>
            <span class="modal-subtitle">
              Leader {{ detail?.run.leader_cli }} đã xong Thiết kế & Kế hoạch. Sửa trực tiếp nếu cần, rồi duyệt để worker bắt đầu code.
            </span>
          </div>
        </div>
        <button class="btn btn-ghost btn-icon" title="Đóng (Esc)" @click="handleClose">
          <X :size="16" />
        </button>
      </div>

      <!-- Two-Column Workbench Body -->
      <div class="modal-body">
        <!-- Left Column: Design / API Contract Spec -->
        <div class="col-spec">
          <div class="spec-tab-bar">
            <button
              type="button"
              class="spec-tab"
              :class="{ active: activeSpecTab === 'design' }"
              @click="activeSpecTab = 'design'"
            >
              <FileText :size="13" />
              <span>Thiết kế Kiến trúc</span>
            </button>
            <button
              type="button"
              class="spec-tab"
              :class="{ active: activeSpecTab === 'contract' }"
              @click="activeSpecTab = 'contract'"
            >
              <Code2 :size="13" />
              <span>API Contract / Types</span>
            </button>
            <button
              v-if="checkDrafts.length > 0"
              type="button"
              class="spec-tab"
              :class="{ active: activeSpecTab === 'checks' }"
              @click="activeSpecTab = 'checks'"
            >
              <ShieldCheck :size="13" />
              <span>Lệnh kiểm tra ({{ checkDrafts.filter((c) => c.use).length }}/{{ checkDrafts.length }})</span>
            </button>
          </div>

          <div class="spec-editor-wrap">
            <textarea
              v-if="activeSpecTab === 'design'"
              v-model="designDoc"
              class="spec-textarea font-mono custom-scrollbar"
              placeholder="Tài liệu thiết kế kiến trúc..."
            ></textarea>
            <div v-else-if="activeSpecTab === 'checks'" class="checks-pane custom-scrollbar">
              <p class="checks-hint">
                Leader đề xuất từ quy tắc của dự án (AGENTS.md, skill, CI) cho repo chưa có lệnh kiểm tra. Lệnh được
                duyệt sẽ lưu vào thiết lập dự án và chạy trên máy này, trong worktree của từng task. Chỉ giữ lệnh bạn
                tin.
              </p>
              <div v-for="check in checkDrafts" :key="check.repo" class="check-card">
                <label class="check-head">
                  <input v-model="check.use" type="checkbox" />
                  <span class="font-mono">{{ check.repo === '.' ? 'repo' : check.repo }}</span>
                </label>
                <label class="check-field">
                  <span>Lệnh chuẩn bị worktree</span>
                  <input v-model="check.setup" class="check-input font-mono" :disabled="!check.use" placeholder="(không có)" />
                </label>
                <label class="check-field">
                  <span>Lệnh kiểm tra (mỗi dòng một lệnh)</span>
                  <textarea v-model="check.commands" rows="3" class="check-input font-mono" :disabled="!check.use"></textarea>
                </label>
              </div>
            </div>
            <textarea
              v-else
              v-model="apiContract"
              class="spec-textarea font-mono custom-scrollbar"
              placeholder="Kiểu dữ liệu và hợp đồng API dùng chung..."
            ></textarea>
          </div>
        </div>

        <!-- Right Column: Tasks Sequencer -->
        <div class="col-tasks">
          <div class="tasks-header">
            <div class="tasks-title">
              <span>Danh sách Task Triển Khai ({{ tasks.length }})</span>
              <span class="tasks-badge">Cô lập git worktree</span>
            </div>
            <div class="tasks-add">
              <button type="button" class="btn btn-xs btn-ghost text-primary" @click="addTask">
                <Plus :size="12" />
                <span>Thêm task</span>
              </button>
              <button
                type="button"
                class="btn btn-xs btn-ghost text-primary"
                title="Một người làm (vd. test UI): AI chuẩn bị theo mô tả, người test báo kết quả, lỗi thành task sửa"
                @click="addHumanTask"
              >
                <UserCheck :size="12" />
                <span>Thêm task test thủ công</span>
              </button>
            </div>
          </div>

          <div class="tasks-list custom-scrollbar">
            <div
              v-for="(task, idx) in tasks"
              :key="task.key"
              class="task-card"
              :class="{ 'is-selected': selectedTaskKey === task.key }"
              @click="selectedTaskKey = task.key"
            >
              <div class="task-drag-handle" title="Kéo để sắp xếp thứ tự thực thi">
                <GripVertical :size="14" />
              </div>

              <div class="task-info">
                <div class="task-top-row">
                  <span class="task-key font-mono">{{ task.key }}</span>
                  <span v-if="task.kind === 'human'" class="task-human-badge">👤 người test</span>
                  <span v-else class="task-role-badge" :class="'role-' + task.role">{{ task.role }}</span>
                  <span v-if="multi" class="task-repo-badge font-mono">{{ task.repos.join(', ') || '— chưa chọn repo' }}</span>
                  <div v-if="task.kind !== 'human'" class="task-cli-picker" @click.stop>
                    <select v-model="task.cli" class="task-cli-select font-mono">
                      <option v-for="cli in AGENT_CLIS" :key="cli" :value="cli" :disabled="!installed(cli)">
                        {{ cli }}{{ installed(cli) ? '' : ' (chưa cài)' }}
                      </option>
                    </select>
                  </div>
                </div>

                <div class="task-desc-row">
                  <span class="task-title-text">{{ task.title }}</span>
                </div>

                <div v-if="task.depends_on && task.depends_on.length > 0" class="task-deps-row font-mono">
                  <ArrowRight :size="10" />
                  <span>phụ thuộc: {{ task.depends_on.join(', ') }}</span>
                </div>
                <div v-if="taskErrors[idx]" class="task-error-row">{{ taskErrors[idx] }}</div>

                <!-- Editing the selected task -->
                <div v-if="selectedTaskKey === task.key" class="task-edit" @click.stop>
                  <label class="edit-label">Key</label>
                  <input v-model="task.key" class="edit-input font-mono" @input="selectedTaskKey = task.key" />
                  <label class="edit-label">Tiêu đề</label>
                  <input v-model="task.title" class="edit-input" />
                  <template v-if="task.kind !== 'human'">
                    <label class="edit-label">Vai</label>
                    <input v-model="task.role" class="edit-input font-mono" />
                  </template>
                  <template v-if="multi">
                    <label class="edit-label">Repo task thay đổi</label>
                    <div class="repo-checks">
                      <label v-for="repo in runRepos" :key="repo" class="repo-check font-mono">
                        <input
                          type="checkbox"
                          :checked="task.repos.includes(repo)"
                          @change="toggleRepo(task, repo, ($event.target as HTMLInputElement).checked)"
                        />
                        {{ repo }}
                      </label>
                    </div>
                  </template>
                  <template v-if="task.kind === 'human'">
                    <label class="edit-label">Mô tả cho AI: cần test gì, AI phải đưa ra gì trước</label>
                    <textarea
                      v-model="task.description"
                      rows="5"
                      class="edit-input edit-textarea"
                      placeholder="Ví dụ: Viết hướng dẫn test màn C-004 (đổi mật khẩu) trên desktop và mobile: các case validate, lỗi API, thông báo thành công."
                    ></textarea>
                    <label class="edit-label">Test sau các task (đến lượt khi chúng đã merge)</label>
                    <div class="repo-checks">
                      <label v-for="other in tasks.filter((t) => t.key !== task.key)" :key="other.key" class="repo-check font-mono">
                        <input
                          type="checkbox"
                          :checked="task.depends_on.includes(other.key)"
                          @change="toggleDep(task, other.key, ($event.target as HTMLInputElement).checked)"
                        />
                        {{ other.key }}
                      </label>
                    </div>
                  </template>
                  <template v-else>
                  <label class="edit-label">Mô tả</label>
                  <textarea v-model="task.description" rows="3" class="edit-input edit-textarea"></textarea>
                  <label class="edit-label">Tiêu chí chấp nhận (mỗi dòng một ý)</label>
                  <textarea
                    :value="task.acceptance.join('\n')"
                    rows="3"
                    class="edit-input edit-textarea"
                    @input="task.acceptance = lines($event)"
                  ></textarea>
                  <label class="edit-label">
                    Phạm vi tệp (glob, cách nhau bởi dấu phẩy{{ multi ? '; tính từ thư mục dự án, ví dụ be/src/**' : '' }})
                  </label>
                  <input
                    :value="task.scope.join(', ')"
                    class="edit-input font-mono"
                    @change="task.scope = commaList($event)"
                  />
                  </template>
                  <label v-if="task.kind !== 'human'" class="edit-label">Phụ thuộc (key, cách nhau bởi dấu phẩy)</label>
                  <input
                    v-if="task.kind !== 'human'"
                    :value="task.depends_on.join(', ')"
                    class="edit-input font-mono"
                    @change="task.depends_on = commaList($event)"
                  />
                </div>
              </div>

              <div class="task-actions" @click.stop>
                <button
                  type="button"
                  class="btn-task-action"
                  title="Xóa task"
                  @click="removeTask(idx)"
                >
                  <Trash2 :size="12" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Feedback & Action Footer -->
      <div class="modal-footer">
        <div v-if="error || planError" class="footer-error">{{ error || planError }}</div>
        <div class="feedback-row">
          <label class="feedback-label">Góp ý cho leader:</label>
          <input
            v-model="feedbackText"
            type="text"
            class="feedback-input font-mono"
            placeholder="Ví dụ: Tách coupon validation thành module riêng; đổi test e2e sang cypress..."
            @keydown.enter="handleRevise"
          />
        </div>

        <div class="actions-row">
          <button type="button" class="btn btn-danger-ghost" :disabled="busy" @click="handleDiscard">
            <XCircle :size="14" />
            <span>Bỏ lượt chạy</span>
          </button>

          <div class="right-actions">
            <button
              type="button"
              class="btn btn-secondary"
              :disabled="!feedbackText.trim() || busy"
              @click="handleRevise"
            >
              <RotateCcw :size="13" />
              <span>Gửi góp ý, làm lại</span>
            </button>

            <button
              type="button"
              class="btn btn-primary btn-approve"
              :disabled="busy || !!planError"
              @click="handleApprove"
            >
              <Check :size="14" />
              <span>Duyệt & Triển khai</span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Inside the backdrop so it stacks above this modal. -->
    <ConfirmDialog
      :visible="confirmDiscard"
      title="Bỏ Lượt Chạy"
      subtitle="MOD-22 • Cổng 1"
      :message="`Bỏ lượt chạy &quot;${detail?.run.title ?? ''}&quot;? Thiết kế, kế hoạch và mọi nhánh, worktree của lượt chạy sẽ bị xóa.`"
      confirm-text="Bỏ lượt chạy"
      loading-text="Đang dọn..."
      danger
      :loading="busy"
      @confirm="discardConfirmed"
      @cancel="confirmDiscard = false"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  X,
  FileText,
  Code2,
  Plus,
  GripVertical,
  ArrowRight,
  Trash2,
  XCircle,
  RotateCcw,
  Check,
  ShieldCheck,
  UserCheck,
} from 'lucide-vue-next';
import type { AgentCli, CheckProposal, CliInfo, RunDetail, TaskDraft } from '@/types/agents';
import { AGENT_CLIS, isMultiRepo } from '@/types/agents';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';

const props = defineProps<{
  visible: boolean;
  detail: RunDetail | null;
  clis: CliInfo[];
  approve: (data: { design: string; contract: string; tasks: TaskDraft[]; checks: CheckProposal[] }) => Promise<void>;
  revise: (data: { design: string; feedback: string }) => Promise<void>;
  discard: () => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const activeSpecTab = ref<'design' | 'contract' | 'checks'>('design');

/** The leader's check proposals as the user edits them. */
const checkDrafts = ref<{ repo: string; use: boolean; setup: string; commands: string }[]>([]);

const approvedChecks = (): CheckProposal[] =>
  checkDrafts.value
    .filter((check) => check.use)
    .map((check) => ({
      repo: check.repo,
      setup: check.setup.trim(),
      commands: check.commands
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean),
    }))
    .filter((check) => check.commands.length > 0);
const selectedTaskKey = ref('');
const feedbackText = ref('');
const designDoc = ref('');
const apiContract = ref('');
const tasks = ref<TaskDraft[]>([]);
const busy = ref(false);
const error = ref<string | null>(null);

/** Repos of the run's project. */
const runRepos = computed(() => props.detail?.run.repos.map((repo) => repo.path) ?? []);
const multi = computed(() => !!props.detail && isMultiRepo(props.detail.run.repos));

const toggleRepo = (task: TaskDraft, repo: string, on: boolean) => {
  task.repos = on
    ? runRepos.value.filter((path) => path === repo || task.repos.includes(path))
    : task.repos.filter((path) => path !== repo);
};

const installed = (cli: AgentCli) => props.clis.some((info) => info.cli === cli && info.path);

watch(
  () => props.visible,
  (visible) => {
    if (!visible || !props.detail) return;
    const { run } = props.detail;
    // Delivered batches are done; the gate is about the current one.
    const planned = props.detail.tasks.filter((task) => task.batch === run.followups.length);
    designDoc.value = run.design_md ?? '';
    checkDrafts.value = run.suggested_checks.map((check) => ({
      repo: check.repo,
      use: true,
      setup: check.setup,
      commands: check.commands.join('\n'),
    }));
    activeSpecTab.value = 'design';
    apiContract.value = run.contract ?? '';
    tasks.value = planned.map((task) => ({
      key: task.key,
      title: task.title,
      role: task.role,
      cli: task.cli,
      description: task.description,
      acceptance: [...task.acceptance],
      scope: [...task.scope],
      depends_on: [...task.depends_on],
      repos: [...task.repos],
      kind: task.kind,
    }));
    selectedTaskKey.value = '';
    feedbackText.value = '';
    error.value = null;
    confirmDiscard.value = false;
  },
  { immediate: true }
);

const lines = (event: Event) =>
  (event.target as HTMLTextAreaElement).value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);

const commaList = (event: Event) =>
  (event.target as HTMLInputElement).value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);

/** Per-task problems, same rules as the backend's check_plan. */
const taskErrors = computed(() =>
  tasks.value.map((task, index) => {
    if (!/^[A-Za-z0-9_-]{1,40}$/.test(task.key)) return 'Key chỉ gồm chữ, số, - và _';
    if (tasks.value.findIndex((other) => other.key === task.key) !== index) return 'Key bị trùng';
    if (!task.title.trim()) return 'Chưa có tiêu đề';
    if (task.kind === 'human' && !task.description.trim()) return 'Cần mô tả cho AI (cần test gì)';
    if (!installed(task.cli)) return `${task.cli} chưa cài`;
    if (task.repos.length === 0) return 'Chưa chọn repo';
    const unknown = task.repos.filter((repo) => !runRepos.value.includes(repo));
    if (unknown.length > 0) return `Repo không có trong dự án: ${unknown.join(', ')}`;
    const missing = task.depends_on.filter((dep) => dep === task.key || !tasks.value.some((t) => t.key === dep));
    if (missing.length > 0) return `Phụ thuộc không hợp lệ: ${missing.join(', ')}`;
    return '';
  })
);

/** A loop in the dependencies, or the first per-task problem. */
const planError = computed(() => {
  if (tasks.value.length === 0) return 'Kế hoạch chưa có task nào';
  const first = taskErrors.value.findIndex(Boolean);
  if (first >= 0) return `Task ${tasks.value[first].key}: ${taskErrors.value[first]}`;
  const placed = new Set<string>();
  let left = [...tasks.value];
  while (left.length > 0) {
    const ready = left.filter((task) => task.depends_on.every((dep) => placed.has(dep)));
    if (ready.length === 0) return `Phụ thuộc vòng lặp: ${left.map((t) => t.key).join(', ')}`;
    ready.forEach((task) => placed.add(task.key));
    left = left.filter((task) => !placed.has(task.key));
  }
  return '';
});

/** A person's task, tested after every agent task by default. */
function addHumanTask() {
  let n = 1;
  while (tasks.value.some((task) => task.key === `test-${n}`)) n += 1;
  const key = `test-${n}`;
  tasks.value.push({
    key,
    title: 'Test thủ công',
    role: 'human',
    cli: props.detail?.run.leader_cli ?? 'claude',
    description: '',
    acceptance: [],
    scope: [],
    depends_on: tasks.value.filter((task) => task.kind !== 'human').map((task) => task.key),
    repos: [...runRepos.value],
    kind: 'human',
  });
  selectedTaskKey.value = key;
}

function toggleDep(task: TaskDraft, key: string, on: boolean) {
  task.depends_on = on ? [...new Set([...task.depends_on, key])] : task.depends_on.filter((dep) => dep !== key);
}

function addTask() {
  let n = tasks.value.length + 1;
  while (tasks.value.some((task) => task.key === `task-${n}`)) n += 1;
  const role = props.detail?.run.roles[0];
  tasks.value.push({
    key: `task-${n}`,
    title: 'Nhiệm vụ mới',
    role: role?.role ?? 'backend',
    cli: role?.cli ?? 'codex',
    description: '',
    acceptance: [],
    scope: [],
    depends_on: [],
    repos: runRepos.value.slice(0, 1),
    kind: 'agent',
  });
  selectedTaskKey.value = `task-${n}`;
}

function removeTask(idx: number) {
  if (tasks.value.length <= 1) return;
  const [removed] = tasks.value.splice(idx, 1);
  for (const task of tasks.value) {
    task.depends_on = task.depends_on.filter((dep) => dep !== removed.key);
  }
}

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

function handleApprove() {
  if (planError.value) return;
  void act(() =>
    props.approve({
      design: designDoc.value,
      contract: apiContract.value,
      tasks: tasks.value,
      checks: approvedChecks(),
    })
  );
}

function handleRevise() {
  if (!feedbackText.value.trim()) return;
  void act(() => props.revise({ design: designDoc.value, feedback: feedbackText.value }));
}

const confirmDiscard = ref(false);

function handleDiscard() {
  confirmDiscard.value = true;
}

async function discardConfirmed() {
  await act(() => props.discard());
  confirmDiscard.value = false;
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

.plan-gate-modal {
  width: 100%;
  max-width: 960px;
  height: 85vh;
  max-height: 800px;
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
  background: linear-gradient(90deg, #f59e0b 0%, #e4b5ff 60%, #744791 100%);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.gate-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background-color: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.35);
}

.gate-icon {
  font-size: 16px;
  color: #fbbf24;
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.modal-body {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0;
  min-height: 0;
  background-color: var(--bg-app-base, #0f1117);
}

.col-spec {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-subtle, #242a3e);
  min-height: 0;
}

.spec-tab-bar {
  display: flex;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-low, #141721);
}

.spec-tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary, #94a3b8);
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: all 0.15s;
}

.spec-tab:hover {
  color: var(--text-primary, #f1f5f9);
}

.spec-tab.active {
  color: #e4b5ff;
  border-bottom-color: #744791;
  background-color: rgba(116, 71, 145, 0.08);
}

.spec-editor-wrap {
  flex: 1;
  min-height: 0;
  padding: 10px;
}

.checks-pane {
  height: 100%;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.checks-hint {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.check-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.check-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
}

.check-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.check-input {
  padding: 6px 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  resize: vertical;
}

.spec-textarea {
  width: 100%;
  height: 100%;
  background-color: transparent;
  border: none;
  color: var(--text-primary, #f1f5f9);
  font-size: 12px;
  line-height: 18px;
  resize: none;
  outline: none;
}

.col-tasks {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background-color: var(--surface-container-low, #141721);
}

.tasks-add {
  display: flex;
  gap: 4px;
}

.task-human-badge {
  padding: 1px 6px;
  font-size: 10px;
  border-radius: 4px;
  color: #fbbf24;
  background-color: rgba(251, 191, 36, 0.12);
}

.tasks-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  background-color: rgba(26, 30, 43, 0.4);
}

.tasks-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.tasks-badge {
  font-size: 10px;
  color: #4edea3;
  background-color: rgba(78, 222, 163, 0.1);
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(78, 222, 163, 0.2);
}

.tasks-list {
  flex: 1;
  padding: 10px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.task-card:hover {
  background-color: var(--surface-container-high, #23283a);
  border-color: rgba(116, 71, 145, 0.4);
}

.task-card.is-selected {
  border-color: #744791;
  background-color: rgba(116, 71, 145, 0.12);
}

.task-drag-handle {
  color: var(--text-muted, #64748b);
  cursor: grab;
  margin-top: 3px;
}

.task-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.task-top-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-key {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-primary, #f1f5f9);
}

.task-role-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
  text-transform: uppercase;
}

.role-backend {
  background-color: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
  border: 1px solid rgba(59, 130, 246, 0.3);
}

.role-frontend {
  background-color: rgba(168, 85, 247, 0.15);
  color: #c084fc;
  border: 1px solid rgba(168, 85, 247, 0.3);
}

.role-test {
  background-color: rgba(16, 185, 129, 0.15);
  color: #34d399;
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.task-cli-picker {
  margin-left: auto;
}

.task-cli-select {
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
  border-radius: 3px;
  font-size: 11px;
  padding: 2px 4px;
  outline: none;
}

.task-desc-row {
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
}

.task-deps-row {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: #f59e0b;
}

.btn-task-action {
  background: transparent;
  border: none;
  color: var(--text-muted, #64748b);
  cursor: pointer;
  padding: 2px;
  border-radius: 3px;
}

.btn-task-action:hover {
  color: #ef4444;
}

.modal-footer {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
}

.feedback-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.feedback-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary, #94a3b8);
  white-space: nowrap;
}

.feedback-input {
  flex: 1;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
  border-radius: 4px;
  padding: 6px 10px;
  font-size: 12px;
  outline: none;
}

.feedback-input:focus {
  border-color: #744791;
  box-shadow: 0 0 0 2px rgba(116, 71, 145, 0.35);
}

.actions-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.right-actions {
  display: flex;
  align-items: center;
  gap: 8px;
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

.btn-primary {
  background-color: #744791;
  color: #ffffff;
}

.btn-primary:hover {
  background-color: #8956aa;
}

.btn-approve {
  padding-left: 16px;
  padding-right: 16px;
  font-weight: 600;
}

.btn-secondary {
  background-color: var(--bg-app-base, #0f1117);
  border-color: var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--surface-container-high, #23283a);
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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

.btn-xs {
  font-size: 11px;
  padding: 2px 6px;
}

.task-repo-badge {
  font-size: 10px;
  padding: 0 5px;
  border-radius: 3px;
  color: #93c5fd;
  background-color: rgba(59, 130, 246, 0.12);
}

.repo-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
}

.repo-check {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
}

.task-error-row {
  margin-top: 4px;
  font-size: 11px;
  color: var(--status-failed, #ef4444);
}

.task-edit {
  display: grid;
  gap: 4px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed var(--border-subtle, #242a3e);
}

.edit-label {
  font-size: 10px;
  color: var(--text-muted, #64748b);
  margin-top: 4px;
}

.edit-input {
  width: 100%;
  padding: 4px 6px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.edit-textarea {
  resize: vertical;
  font-family: inherit;
}

.footer-error {
  width: 100%;
  font-size: 12px;
  color: var(--status-failed, #ef4444);
}
</style>
