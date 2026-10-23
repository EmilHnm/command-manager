<template>
  <div v-if="visible" class="modal-backdrop" @keydown.esc="handleClose">
    <div class="new-run-modal" role="dialog" aria-modal="true" aria-labelledby="new-run-title">
      <!-- Top Accent Bar -->
      <div class="modal-accent-bar"></div>

      <!-- Header -->
      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <Bot :size="18" />
          </div>
          <div class="header-titles">
            <h3 id="new-run-title" class="modal-title">
              Lượt Chạy Mới — {{ projectLabel }}
              <span v-if="branchTag" class="branch-tag">{{ branchTag }}</span>
            </h3>
            <span class="modal-subtitle">Khởi tạo phiên điều phối đa agent phân cấp (SCR-09 • MOD-21)</span>
          </div>
        </div>
        <button class="btn btn-ghost btn-icon" title="Đóng (Esc)" @click="handleClose">
          <X :size="16" />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body custom-scrollbar">
        <div v-if="error" class="warning-banner error-banner" role="alert">
          <AlertTriangle :size="15" class="warning-icon" />
          <div class="warning-text">{{ error }}</div>
        </div>

        <!-- Form Field: Requirement -->
        <div class="form-group">
          <label class="form-label required">
            Yêu cầu triển khai (Prompt nghiệp vụ)
            <span class="label-hint">Mô tả tính năng, tiêu chí chấp nhận hoặc lỗi cần sửa</span>
          </label>
          <textarea
            v-model="form.request"
            rows="3"
            class="form-textarea font-mono"
            placeholder="Ví dụ: Thêm giỏ hàng có mã giảm giá, áp dụng khi thanh toán; viết test e2e và đảm bảo cargo test pass..."
          ></textarea>
        </div>

        <!-- Team Preset & Leader -->
        <div class="form-grid-2">
          <div class="form-group">
            <label class="form-label">Đội mẫu (Team Preset)</label>
            <div class="select-wrapper">
              <select v-model="selectedTeam" class="form-select" @change="applyPreset">
                <option value="custom">Tùy chỉnh vai trò thủ công</option>
                <option v-for="preset in PRESETS" :key="preset.id" :value="preset.id">
                  {{ preset.label }}
                </option>
              </select>
            </div>
          </div>

          <div class="form-group">
            <label class="form-label required">Leader CLI (Thiết kế & Review)</label>
            <div class="select-wrapper">
              <select v-model="form.leader" class="form-select">
                <option v-for="cli in AGENT_CLIS" :key="cli" :value="cli" :disabled="!installed(cli)">
                  {{ cliOptionLabel(cli) }}
                </option>
              </select>
            </div>
            <div class="model-row" title="Model cho thiết kế & kế hoạch của lượt này; review dùng model Review trong Đội & CLI">
              <input
                v-model="form.leaderModel"
                class="form-input font-mono"
                :list="`models-${form.leader}`"
                :placeholder="profileModelHint(profileOf(form.leader), 'lead')"
              />
              <select v-model="form.leaderEffort" class="form-select effort-select" :disabled="form.leader === 'cursor'">
                <option value="">effort mặc định</option>
                <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
              </select>
            </div>
          </div>
        </div>

        <!-- Roles Allocation -->
        <div class="form-group">
          <div class="roles-header">
            <label class="form-label mb-0">Bảng phân bổ vai → CLI (Workers)</label>
            <button type="button" class="btn btn-xs btn-ghost text-primary" @click="addRole">
              <Plus :size="12" />
              <span>Thêm vai</span>
            </button>
          </div>

          <div class="roles-list">
            <div v-for="(item, idx) in form.roles" :key="idx" class="role-row">
              <div class="role-name-col">
                <input
                  v-model="item.role"
                  type="text"
                  class="form-input font-mono"
                  placeholder="Tên vai (backend, frontend...)"
                />
              </div>
              <div class="role-arrow">→</div>
              <div class="role-cli-col">
                <select v-model="item.cli" class="form-select" @change="item.model = ''">
                  <option v-for="cli in AGENT_CLIS" :key="cli" :value="cli" :disabled="!installed(cli)">
                    {{ cliOptionLabel(cli) }}
                  </option>
                </select>
              </div>
              <div class="role-model-col">
                <input
                  v-model="item.model"
                  class="form-input font-mono"
                  :list="`models-${item.cli}`"
                  :placeholder="profileModelHint(profileOf(item.cli), 'work')"
                  title="Model của vai này trong lượt chạy"
                />
              </div>
              <select v-model="item.effort" class="form-select effort-select" :disabled="item.cli === 'cursor'" title="Effort của vai này">
                <option value="">effort</option>
                <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
              </select>
              <button
                v-if="form.roles.length > 1"
                type="button"
                class="btn btn-ghost btn-icon btn-remove-role"
                title="Xóa vai"
                @click="removeRole(idx)"
              >
                <Trash2 :size="13" />
              </button>
            </div>
          </div>
        </div>

        <!-- Approval Gates -->
        <div class="form-group">
          <label class="form-label">Cổng kiểm soát duyệt người dùng (Safety Gates)</label>
          <div class="gates-row">
            <label class="checkbox-card">
              <input v-model="form.plan_gate" type="checkbox" class="form-checkbox" />
              <div class="checkbox-text">
                <span class="checkbox-title">⛩ Cổng 1 — Duyệt Thiết Kế & Kế Hoạch (MOD-22)</span>
                <span class="checkbox-desc">Dừng lại để bạn chỉnh sửa task, đổi agent trước khi worker bắt đầu code</span>
              </div>
            </label>

            <label class="checkbox-card">
              <input v-model="form.merge_gate" type="checkbox" class="form-checkbox" @change="confirmMergeGate" />
              <div class="checkbox-text">
                <span class="checkbox-title">⛩ Cổng 2 — Duyệt Merge (MOD-23)</span>
                <span class="checkbox-desc">Xem toàn bộ diff và test report trước khi merge vào nhánh gốc</span>
              </div>
            </label>
          </div>
        </div>

        <!-- Limits -->
        <div class="form-group">
          <label class="form-label">Giới hạn an toàn lượt chạy (Safety Limits)</label>
          <div class="limits-grid">
            <div class="limit-item">
              <span class="limit-label">Song song tối đa</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.concurrency" type="number" min="1" max="8" class="form-input font-mono text-center" />
                <span class="limit-unit">agents</span>
              </div>
            </div>

            <div class="limit-item">
              <span class="limit-label">Vòng review tối đa</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.max_rounds" type="number" min="1" max="10" class="form-input font-mono text-center" />
                <span class="limit-unit">vòng</span>
              </div>
            </div>

            <div class="limit-item">
              <span class="limit-label">Tổng lượt gọi CLI</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.max_invocations" type="number" min="5" max="200" class="form-input font-mono text-center" />
                <span class="limit-unit">lượt</span>
              </div>
            </div>

            <div class="limit-item">
              <span class="limit-label">Timeout mỗi lượt</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.timeout_minutes" type="number" min="5" :max="MAX_TIMEOUT_MINUTES" step="5" class="form-input font-mono text-center" />
                <span class="limit-unit">phút</span>
              </div>
            </div>

            <div class="limit-item" title="Dừng lượt chạy khi chi phí CLI báo về chạm mức này (Claude tự dừng giữa chừng; Codex không báo chi phí)">
              <span class="limit-label">Ngân sách</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.max_cost_usd" type="number" min="0.5" max="500" step="0.5" class="form-input font-mono text-center" />
                <span class="limit-unit">USD</span>
              </div>
            </div>

            <div class="limit-item" title="Claude báo % hạn mức 5 giờ / 7 ngày của tài khoản; chạm ngưỡng thì dừng agent đang chạy và tạm dừng lượt chạy (0 = tắt)">
              <span class="limit-label">Dừng khi hạn mức ≥</span>
              <div class="limit-input-wrap">
                <input v-model.number="form.limits.quota_pause_percent" type="number" min="0" max="100" step="5" class="form-input font-mono text-center" />
                <span class="limit-unit">%</span>
              </div>
            </div>
          </div>
          <label class="auto-resume">
            <input v-model="form.limits.auto_resume" type="checkbox" />
            <span>Tự chạy tiếp khi hạn mức reset (bước dở được làm tiếp trong phiên cũ, không làm lại từ đầu)</span>
          </label>
        </div>

        <div v-if="dirtyRepos.length > 0" class="warning-banner">
          <AlertTriangle :size="15" class="warning-icon" />
          <div class="warning-text">
            <strong>Có thay đổi chưa commit: {{ dirtyRepos.join(', ') }}.</strong>
            Lượt chạy dùng commit hiện tại của từng repo; phần chưa commit không được đưa cho agent.
          </div>
        </div>

        <div v-if="bypassing.length > 0" class="warning-banner error-banner">
          <AlertTriangle :size="15" class="warning-icon" />
          <div class="warning-text">
            <strong>{{ bypassing.join(', ') }}</strong> đang bật "Tự duyệt mọi thứ" (Đội & CLI):
            agent được chạy mọi lệnh mà không hỏi.
          </div>
        </div>

        <!-- Warning Ribbon -->
        <div class="warning-banner">
          <AlertTriangle :size="15" class="warning-icon" />
          <div class="warning-text">
            <strong>Bảo vệ nhánh gốc:</strong>
            Mỗi worker chạy trong một git worktree riêng biệt. Không có lệnh push hoặc can thiệp nhánh gốc nào được thực thi trước khi bạn bấm Duyệt ở Cổng 2.
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <button type="button" class="btn btn-secondary" @click="handleClose">
          Hủy
        </button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="!canSubmit || saving"
          @click="handleSubmit"
        >
          <LoaderCircle v-if="saving" :size="14" class="spin" />
          <Play v-else :size="14" />
          <span>Bắt đầu thiết kế</span>
        </button>
      </div>
    </div>

    <!-- Inside the backdrop so it stacks above this modal. -->
    <ConfirmDialog
      :visible="confirmNoMergeGate"
      title="Tắt Cổng 2"
      subtitle="MOD-21 • Cổng kiểm soát"
      message="Khi tắt Cổng 2, code của lượt chạy sẽ được merge vào nhánh gốc ngay khi xong mà không qua bạn duyệt (vẫn chỉ merge local, không push)."
      confirm-text="Tắt Cổng 2"
      danger
      @confirm="confirmNoMergeGate = false"
      @cancel="keepMergeGate"
    />
    <datalist v-for="cli in AGENT_CLIS" :id="`models-${cli}`" :key="cli">
      <option v-for="item in modelsOf(cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
    </datalist>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { Bot, X, Plus, Trash2, AlertTriangle, Play, LoaderCircle } from 'lucide-vue-next';
import type { AgentCli, AgentProjectView, CliInfo, NewRunInput, RoleCli } from '@/types/agents';
import {
  AGENT_CLIS,
  AGENT_CLI_LABELS,
  DEFAULT_LIMITS,
  EFFORT_LEVELS,
  MAX_TIMEOUT_MINUTES,
  isMultiRepo,
  profileModelHint,
  repoLabel,
} from '@/types/agents';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useAgentModels } from '@/composables/useAgentModels';

const { modelsOf } = useAgentModels();

const props = defineProps<{
  visible: boolean;
  project: AgentProjectView | null;
  clis: CliInfo[];
  initialPrompt?: string;
  /** Starts the run; a rejection is shown in the modal. */
  submit: (input: Omit<NewRunInput, 'project_id'>) => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

interface Preset {
  id: string;
  label: string;
  leader: AgentCli;
  roles: RoleCli[];
}

const PRESETS: Preset[] = [
  {
    id: 'claude-lead',
    label: 'Claude lãnh đạo · Codex BE · Agy FE',
    leader: 'claude',
    roles: [
      { role: 'backend', cli: 'codex' },
      { role: 'frontend', cli: 'agy' },
      { role: 'test', cli: 'claude' },
    ],
  },
  {
    id: 'codex-lead',
    label: 'Codex lãnh đạo · Claude fullstack · Agy test',
    leader: 'codex',
    roles: [
      { role: 'fullstack', cli: 'claude' },
      { role: 'test', cli: 'agy' },
    ],
  },
  {
    id: 'agy-lead',
    label: 'Agy lãnh đạo · Codex worker',
    leader: 'agy',
    roles: [
      { role: 'backend', cli: 'codex' },
      { role: 'frontend', cli: 'agy' },
    ],
  },
  {
    id: 'cursor-lead',
    label: 'Cursor lãnh đạo · Codex BE · Claude FE',
    leader: 'cursor',
    roles: [
      { role: 'backend', cli: 'codex' },
      { role: 'frontend', cli: 'claude' },
    ],
  },
];

const selectedTeam = ref('claude-lead');
const saving = ref(false);
const error = ref<string | null>(null);

interface RoleDraft {
  role: string;
  cli: AgentCli;
  model: string;
  effort: string;
}

const roleDraft = (role: RoleCli): RoleDraft => ({
  role: role.role,
  cli: role.cli,
  model: role.model ?? '',
  effort: role.effort ?? '',
});

const form = reactive({
  request: '',
  leader: 'claude' as AgentCli,
  leaderModel: '',
  leaderEffort: '',
  roles: PRESETS[0].roles.map(roleDraft),
  plan_gate: true,
  merge_gate: true,
  limits: { ...DEFAULT_LIMITS },
});

const projectLabel = computed(() => {
  const path = props.project?.path ?? '';
  return path.replace(/^\/home\/[^/]+/, '~').replace(/^\/Users\/[^/]+/, '~');
});

/** The base branch, or how many repos for a folder of repos. */
const branchTag = computed(() => {
  const project = props.project;
  if (!project) return '';
  if (isMultiRepo(project.repos)) return `${project.repos.length} repo`;
  return project.repos[0]?.base_branch || project.status[0]?.branch || '';
});

const dirtyRepos = computed(() =>
  (props.project?.status ?? [])
    .filter((repo) => repo.dirty > 0)
    .map((repo) => `${repoLabel(repo.path, props.project?.path)} (${repo.dirty})`)
);

const installed = (cli: AgentCli) => props.clis.some((info) => info.cli === cli && info.path);

const cliOptionLabel = (cli: AgentCli) => {
  const info = props.clis.find((item) => item.cli === cli);
  if (!info?.path) return `${cli} — chưa cài`;
  return `${cli} · ${AGENT_CLI_LABELS[cli]}${info.version ? ` (${info.version})` : ''}`;
};

/** CLIs of this team that skip every permission prompt. */
const bypassing = computed(() => {
  const used = new Set<AgentCli>([form.leader, ...form.roles.map((r) => r.cli)]);
  return props.clis.filter((info) => used.has(info.cli) && info.profile.bypass_permissions).map((info) => info.cli);
});

const canSubmit = computed(
  () =>
    form.request.trim().length > 0 &&
    installed(form.leader) &&
    form.roles.some((r) => r.role.trim()) &&
    form.roles.every((r) => !r.role.trim() || installed(r.cli))
);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    error.value = null;
    saving.value = false;
    if (props.initialPrompt) form.request = props.initialPrompt;
    // The project's last run set these; it knows how long its work takes.
    form.limits = { ...DEFAULT_LIMITS, ...(props.project?.limits ?? {}) };
  },
  { immediate: true }
);

function applyPreset() {
  const preset = PRESETS.find((p) => p.id === selectedTeam.value);
  if (!preset) return;
  form.leader = preset.leader;
  form.leaderModel = '';
  form.leaderEffort = '';
  form.roles = preset.roles.map(roleDraft);
}

function addRole() {
  form.roles.push(roleDraft({ role: '', cli: 'codex' }));
}

const profileOf = (cli: AgentCli) => props.clis.find((info) => info.cli === cli)?.profile;

function removeRole(idx: number) {
  if (form.roles.length > 1) {
    form.roles.splice(idx, 1);
  }
}

const confirmNoMergeGate = ref(false);

function confirmMergeGate() {
  if (!form.merge_gate) confirmNoMergeGate.value = true;
}

function keepMergeGate() {
  form.merge_gate = true;
  confirmNoMergeGate.value = false;
}

function handleClose() {
  emit('close');
}

async function handleSubmit() {
  if (!canSubmit.value || saving.value) return;
  saving.value = true;
  error.value = null;
  try {
    await props.submit({
      request: form.request.trim(),
      leader_cli: form.leader,
      leader_model: form.leaderModel.trim() || null,
      leader_effort: form.leaderEffort || null,
      roles: form.roles
        .filter((r) => r.role.trim())
        .map((r) => ({ role: r.role.trim(), cli: r.cli, model: r.model.trim() || null, effort: r.effort || null })),
      plan_gate: form.plan_gate,
      merge_gate: form.merge_gate,
      limits: { ...form.limits },
    });
    form.request = '';
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
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

.new-run-modal {
  width: 100%;
  max-width: 660px;
  max-height: 90vh;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: var(--radius-lg, 8px);
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.5);
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
  background: linear-gradient(90deg, #744791 0%, #e4b5ff 50%, #4edea3 100%);
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

.icon-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background-color: rgba(116, 71, 145, 0.2);
  color: #e4b5ff;
  border: 1px solid rgba(116, 71, 145, 0.4);
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
  display: flex;
  align-items: center;
  gap: 8px;
}

.branch-tag {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: #4edea3;
  background-color: rgba(78, 222, 163, 0.12);
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(78, 222, 163, 0.25);
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.modal-body {
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.form-label.required::after {
  content: '*';
  color: #ef4444;
  margin-left: 4px;
}

.label-hint {
  font-size: 11px;
  font-weight: 400;
  color: var(--text-muted, #64748b);
}

.form-grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.form-input,
.form-textarea,
.form-select {
  width: 100%;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
  border-radius: 4px;
  padding: 7px 10px;
  font-size: 12px;
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.form-input:focus,
.form-textarea:focus,
.form-select:focus {
  border-color: #744791;
  box-shadow: 0 0 0 2px rgba(116, 71, 145, 0.35);
}

.form-textarea {
  resize: vertical;
  min-height: 72px;
}

.roles-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 2px;
}

.roles-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.role-row {
  display: flex;
  align-items: center;
  gap: 8px;
  background-color: rgba(26, 30, 43, 0.4);
  padding: 4px;
  border-radius: 4px;
  border: 1px solid var(--border-subtle, #242a3e);
}

.role-name-col {
  flex: 1;
}

.role-arrow {
  color: var(--text-muted, #64748b);
  font-size: 13px;
  user-select: none;
}

.role-cli-col {
  flex: 1;
}

.role-model-col {
  flex: 1;
  min-width: 0;
}

.model-row {
  display: flex;
  gap: 6px;
  margin-top: 6px;
}

.model-row .form-input {
  flex: 1;
  min-width: 0;
}

.effort-select {
  flex: none;
  width: 110px;
}

.btn-remove-role {
  color: var(--text-muted, #64748b);
}
.btn-remove-role:hover {
  color: #ef4444;
}

.gates-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.checkbox-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 10px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  cursor: pointer;
}

.form-checkbox {
  margin-top: 3px;
  accent-color: #744791;
}

.checkbox-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.checkbox-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary, #f1f5f9);
}

.checkbox-desc {
  font-size: 11px;
  color: var(--text-muted, #64748b);
  line-height: 14px;
}

.auto-resume {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
  cursor: pointer;
}

.limits-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
}

.limit-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  padding: 6px 8px;
}

.limit-label {
  font-size: 10px;
  color: var(--text-muted, #64748b);
  text-transform: uppercase;
  letter-spacing: 0.02em;
}

.limit-input-wrap {
  display: flex;
  align-items: center;
  gap: 4px;
}

.limit-input-wrap .form-input {
  padding: 3px 6px;
  font-weight: 600;
}

.limit-unit {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.warning-banner {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 12px;
  background-color: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  border-radius: 6px;
  font-size: 11px;
  color: #fbbf24;
  line-height: 16px;
}

.warning-icon {
  flex-shrink: 0;
  margin-top: 1px;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 10px 16px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
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

.btn-primary:hover:not(:disabled) {
  background-color: #8956aa;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background-color: var(--bg-app-base, #0f1117);
  border-color: var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
}

.btn-secondary:hover {
  background-color: var(--surface-container-high, #23283a);
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

.error-banner {
  border-color: rgba(239, 68, 68, 0.4);
  background-color: rgba(239, 68, 68, 0.08);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
