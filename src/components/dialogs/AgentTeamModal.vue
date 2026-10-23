<template>
  <div v-if="visible" class="modal-backdrop" @keydown.esc="emit('close')">
    <div class="team-modal" role="dialog" aria-modal="true" aria-labelledby="team-modal-title">
      <div class="modal-accent-bar"></div>

      <div class="modal-header">
        <div class="header-left">
          <div class="icon-badge">
            <Users :size="16" />
          </div>
          <div class="header-titles">
            <h3 id="team-modal-title" class="modal-title">Đội & CLI</h3>
            <span class="modal-subtitle">Cấu hình từng CLI và thiết lập của dự án đang mở</span>
          </div>
        </div>
        <button class="btn btn-ghost btn-icon" title="Đóng (Esc)" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>

      <div class="tab-bar">
        <button type="button" class="tab" :class="{ active: tab === 'cli' }" @click="tab = 'cli'">
          <Bot :size="13" /> CLI
        </button>
        <button
          type="button"
          class="tab"
          :class="{ active: tab === 'project' }"
          :disabled="!project"
          @click="tab = 'project'"
        >
          <FolderGit2 :size="13" /> Dự án
        </button>
        <span class="tab-spacer" />
        <button v-if="tab === 'cli'" type="button" class="btn btn-ghost btn-xs" :disabled="detecting" @click="emit('refresh')">
          <RefreshCw :size="12" :class="{ spin: detecting }" /> Dò lại
        </button>
      </div>

      <div class="modal-body custom-scrollbar">
        <div v-if="error" class="banner banner-error">{{ error }}</div>
        <div v-if="saved" class="banner banner-ok">{{ saved }}</div>

        <!-- CLIs -->
        <template v-if="tab === 'cli'">
          <div v-for="draft in drafts" :key="draft.cli" class="cli-card" :class="{ missing: !infoOf(draft.cli)?.path }">
            <div class="cli-head">
              <span class="cli-name font-mono">{{ draft.cli }}</span>
              <span class="cli-label">{{ AGENT_CLI_LABELS[draft.cli] }}</span>
              <span v-if="infoOf(draft.cli)?.path" class="cli-version font-mono" :title="infoOf(draft.cli)?.path || ''">
                ✓ {{ infoOf(draft.cli)?.version || 'đã cài' }}
              </span>
              <span v-else class="cli-version missing-text">Chưa cài ({{ binaries[draft.cli] }})</span>
              <button
                v-if="infoOf(draft.cli)?.path"
                type="button"
                class="btn btn-ghost btn-xs models-btn"
                :disabled="loading[draft.cli]"
                :title="modelErrors[draft.cli] || 'Đọc lại danh sách model từ CLI'"
                @click="loadModels(draft.cli, true)"
              >
                <RefreshCw :size="11" :class="{ spin: loading[draft.cli] }" />
                {{ modelErrors[draft.cli] ? 'lỗi đọc model' : `${modelsOf(draft.cli).length} model` }}
              </button>
            </div>
            <datalist :id="`team-models-${draft.cli}`">
              <option v-for="item in modelsOf(draft.cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
            </datalist>
            <div class="cli-grid">
              <label class="field">
                <span class="field-label">Model</span>
                <input
                  v-model="draft.model"
                  class="field-input font-mono"
                  :list="`team-models-${draft.cli}`"
                  placeholder="mặc định của CLI"
                />
              </label>
              <label class="field">
                <span class="field-label">Effort</span>
                <select v-model="draft.effort" class="field-input" :disabled="draft.cli === 'cursor'">
                  <option value="">mặc định</option>
                  <option v-for="level in EFFORTS" :key="level" :value="level">{{ level }}</option>
                </select>
              </label>
              <label class="field field-wide">
                <span class="field-label">Tham số thêm (cách nhau bởi dấu cách)</span>
                <input v-model="draft.extraArgs" class="field-input font-mono" :placeholder="extraHint[draft.cli]" />
              </label>
            </div>
            <div class="steps">
              <span class="field-label">Theo bước — để trống thì dùng Model / Effort ở trên. Model rẻ hơn cho viết code và review tiết kiệm nhiều hạn mức.</span>
              <div v-for="phase in PHASES" :key="phase" class="step-row">
                <span class="step-name">{{ PHASE_LABELS[phase] }}</span>
                <input
                  v-model="draft.steps[phase].model"
                  :list="`team-models-${draft.cli}`"
                  class="field-input font-mono"
                  :placeholder="draft.model ? `như trên (${draft.model})` : 'mặc định của CLI'"
                />
                <select v-model="draft.steps[phase].effort" class="field-input" :disabled="draft.cli === 'cursor'">
                  <option value="">{{ draft.effort ? `như trên (${draft.effort})` : 'mặc định' }}</option>
                  <option v-for="level in EFFORTS" :key="level" :value="level">{{ level }}</option>
                </select>
              </div>
            </div>
            <label class="bypass-row" :class="{ on: draft.bypass }">
              <input v-model="draft.bypass" type="checkbox" />
              <span>
                <strong>Tự duyệt mọi thứ</strong> ({{ bypassFlag[draft.cli] }}) — agent chạy mọi lệnh không hỏi, chỉ dùng
                ở bước ghi code trong worktree. Bước thiết kế và review luôn chỉ đọc.
              </span>
            </label>
            <div class="cli-actions">
              <button type="button" class="btn btn-primary btn-xs" :disabled="busy" @click="saveCli(draft)">
                <Save :size="12" /> Lưu {{ draft.cli }}
              </button>
            </div>
          </div>
        </template>

        <!-- Project -->
        <template v-else-if="project">
          <div class="field">
            <span class="field-label">Thư mục</span>
            <span class="field-static font-mono">{{ project.path }}</span>
          </div>
          <div v-for="repo in repoDrafts" :key="repo.path" class="cli-card">
            <div class="cli-head">
              <span class="cli-name font-mono">{{ repoLabel(repo.path, project.path) }}</span>
              <span class="cli-label">{{ repo.path === '.' ? 'repo tại thư mục dự án' : `${project.path}/${repo.path}` }}</span>
              <span class="cli-version font-mono" :class="{ 'missing-text': !statusOf(repo.path)?.is_repo }">
                {{ statusOf(repo.path)?.branch || (statusOf(repo.path)?.is_repo ? 'detached' : 'không còn là git repo') }}
              </span>
            </div>
            <label class="field">
              <span class="field-label">Nhánh gốc (merge vào đây ở Cổng 2)</span>
              <input
                v-model="repo.base_branch"
                class="field-input font-mono"
                :placeholder="`nhánh đang checkout (${statusOf(repo.path)?.branch || 'detached'})`"
              />
            </label>
            <label class="field">
              <span class="field-label">Lệnh chuẩn bị worktree (chạy một lần cho mỗi task làm ở repo này)</span>
              <input v-model="repo.setup_cmd" class="field-input font-mono" placeholder="ví dụ: pnpm install --frozen-lockfile" />
            </label>
            <label class="field">
              <span class="field-label">Lệnh kiểm tra (mỗi dòng một lệnh; chạy trong repo này)</span>
              <textarea
                v-model="repo.checks"
                rows="3"
                class="field-input field-textarea font-mono"
                placeholder="cargo test&#10;pnpm build"
              ></textarea>
            </label>
            <label class="field field-narrow">
              <span class="field-label">Timeout mỗi lệnh chuẩn bị / kiểm tra (phút)</span>
              <input
                v-model.number="repo.check_timeout"
                type="number"
                min="1"
                :max="MAX_TIMEOUT_MINUTES"
                class="field-input font-mono"
                :placeholder="`${CHECK_TIMEOUT_MINUTES} (mặc định)`"
              />
            </label>
          </div>
          <label class="field">
            <span class="field-label">Ghi chú cho mọi agent (thêm vào prompt)</span>
            <textarea
              v-model="projectDraft.notes"
              rows="4"
              class="field-input field-textarea"
              placeholder="Quy ước code, thư mục cần tránh, cách chạy test..."
            ></textarea>
          </label>
          <div class="cli-actions">
            <button type="button" class="btn btn-primary btn-sm" :disabled="busy" @click="saveProject">
              <Save :size="13" /> Lưu thiết lập dự án
            </button>
          </div>
        </template>
      </div>
    </div>

    <!-- Inside the backdrop so it stacks above this modal. -->
    <ConfirmDialog
      :visible="!!pendingBypass"
      title="Bật Tự Duyệt Mọi Thứ"
      subtitle="Đội & CLI • Quyền của agent"
      :message="`Bật &quot;Tự duyệt mọi thứ&quot; cho ${pendingBypass?.cli ?? ''} (${pendingBypass ? bypassFlag[pendingBypass.cli] : ''})? Ở bước ghi code, agent sẽ chạy mọi lệnh shell mà không hỏi. Bước thiết kế và review vẫn chỉ đọc.`"
      confirm-text="Bật và lưu"
      danger
      @confirm="bypassConfirmed"
      @cancel="pendingBypass = null"
    />
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { Bot, FolderGit2, RefreshCw, Save, Users, X } from 'lucide-vue-next';
import type { AgentCli, AgentPhase, AgentProjectView, CliInfo, CliProfile, StepModel } from '@/types/agents';
import {
  AGENT_CLIS,
  AGENT_CLI_LABELS,
  CHECK_TIMEOUT_MINUTES,
  MAX_TIMEOUT_MINUTES,
  PHASE_LABELS,
  repoLabel,
} from '@/types/agents';
import { ipcClient } from '@/ipc/client';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { useAgentModels } from '@/composables/useAgentModels';

const { modelsOf, load: loadModels, loading, errors: modelErrors } = useAgentModels();

const props = defineProps<{
  visible: boolean;
  clis: CliInfo[];
  detecting: boolean;
  project: AgentProjectView | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'refresh'): void;
  (e: 'saved-cli', profile: CliProfile): void;
  (e: 'saved-project', project: AgentProjectView): void;
}>();

const EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max'];
const PHASES: AgentPhase[] = ['lead', 'work', 'review'];

const binaries: Record<AgentCli, string> = {
  claude: 'claude',
  codex: 'codex',
  agy: 'agy',
  cursor: 'cursor-agent / agent',
};

const bypassFlag: Record<AgentCli, string> = {
  claude: '--permission-mode bypassPermissions',
  codex: '--dangerously-bypass-approvals-and-sandbox',
  agy: '--dangerously-skip-permissions',
  cursor: '--force',
};

const extraHint: Record<AgentCli, string> = {
  claude: '--allowedTools "Bash(cargo test:*)"',
  codex: '-c sandbox_workspace_write.network_access=true',
  agy: '--add-dir ../shared',
  cursor: '--add-dir ../shared',
};

interface CliDraft {
  cli: AgentCli;
  model: string;
  effort: string;
  extraArgs: string;
  bypass: boolean;
  steps: Record<AgentPhase, { model: string; effort: string }>;
}

const stepDraft = (step?: StepModel) => ({ model: step?.model ?? '', effort: step?.effort ?? '' });

const stepValue = (step: { model: string; effort: string }): StepModel => ({
  model: step.model.trim() || null,
  effort: step.effort || null,
});

const tab = ref<'cli' | 'project'>('cli');
const drafts = ref<CliDraft[]>([]);
interface RepoDraft {
  path: string;
  base_branch: string;
  setup_cmd: string;
  checks: string;
  /** '' (empty input): the default. */
  check_timeout: number | '';
}

const projectDraft = reactive({ notes: '' });
const repoDrafts = ref<RepoDraft[]>([]);
const busy = ref(false);
const error = ref<string | null>(null);
const saved = ref<string | null>(null);

const statusOf = (path: string) => props.project?.status.find((repo) => repo.path === path);

const infoOf = (cli: AgentCli) => props.clis.find((info) => info.cli === cli);

/** Splits on spaces, keeping "quoted parts" together. */
const splitArgs = (text: string) =>
  (text.match(/"[^"]*"|'[^']*'|\S+/g) ?? []).map((part) => part.replace(/^(["'])(.*)\1$/, '$2'));

const joinArgs = (args: string[]) => args.map((arg) => (/\s/.test(arg) ? `"${arg}"` : arg)).join(' ');

const load = () => {
  drafts.value = AGENT_CLIS.map((cli) => {
    const profile = infoOf(cli)?.profile;
    return {
      cli,
      model: profile?.model ?? '',
      effort: profile?.effort ?? '',
      extraArgs: joinArgs(profile?.extra_args ?? []),
      bypass: profile?.bypass_permissions ?? false,
      steps: {
        lead: stepDraft(profile?.steps?.lead),
        work: stepDraft(profile?.steps?.work),
        review: stepDraft(profile?.steps?.review),
      },
    };
  });
  const project = props.project;
  repoDrafts.value = (project?.repos ?? []).map((repo) => ({
    path: repo.path,
    base_branch: repo.base_branch ?? '',
    setup_cmd: repo.setup_cmd ?? '',
    checks: repo.check_cmds.join('\n'),
    check_timeout: repo.check_timeout_minutes ?? '',
  }));
  projectDraft.notes = project?.notes ?? '';
};

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    error.value = null;
    saved.value = null;
    load();
  },
  { immediate: true }
);

// A new detection result refreshes what is shown.
watch(
  () => props.clis,
  () => {
    if (props.visible) load();
  }
);

async function run(action: () => Promise<void>, message: string) {
  busy.value = true;
  error.value = null;
  saved.value = null;
  try {
    await action();
    saved.value = message;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}

/** A save that turns bypass on, waiting for the confirmation. */
const pendingBypass = ref<CliDraft | null>(null);

function saveCli(draft: CliDraft) {
  if (draft.bypass && !infoOf(draft.cli)?.profile.bypass_permissions) {
    pendingBypass.value = draft;
    return;
  }
  writeCli(draft);
}

function bypassConfirmed() {
  const draft = pendingBypass.value;
  pendingBypass.value = null;
  if (draft) writeCli(draft);
}

function writeCli(draft: CliDraft) {
  void run(async () => {
    const profile = await ipcClient.agentsProfileSave({
      cli: draft.cli,
      model: draft.model.trim() || null,
      effort: draft.effort || null,
      bypass_permissions: draft.bypass,
      extra_args: splitArgs(draft.extraArgs),
      steps: {
        lead: stepValue(draft.steps.lead),
        work: stepValue(draft.steps.work),
        review: stepValue(draft.steps.review),
      },
    });
    emit('saved-cli', profile);
  }, `Đã lưu ${draft.cli}`);
}

function saveProject() {
  const project = props.project;
  if (!project) return;
  void run(async () => {
    const view = await ipcClient.agentsProjectSave(project.id, {
      repos: repoDrafts.value.map((repo) => ({
        path: repo.path,
        base_branch: repo.base_branch.trim() || null,
        setup_cmd: repo.setup_cmd.trim() || null,
        check_cmds: repo.checks.split('\n').map((line) => line.trim()).filter(Boolean),
        check_timeout_minutes: typeof repo.check_timeout === 'number' && repo.check_timeout > 0 ? repo.check_timeout : null,
      })),
      notes: projectDraft.notes.trim() || null,
    });
    emit('saved-project', view);
  }, 'Đã lưu thiết lập dự án');
}
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(5, 7, 12, 0.7);
}

.team-modal {
  width: min(720px, calc(100vw - 32px));
  max-height: calc(100vh - 64px);
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  overflow: hidden;
  color: var(--text-primary, #f1f5f9);
}

.modal-accent-bar {
  height: 3px;
  background-color: #744791;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.icon-badge {
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.2);
}

.header-titles {
  display: flex;
  flex-direction: column;
}

.modal-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.tab-bar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: 1px solid transparent;
  border-radius: 4px;
  cursor: pointer;
}

.tab.active {
  color: #e4b5ff;
  border-color: rgba(116, 71, 145, 0.5);
  background-color: rgba(116, 71, 145, 0.15);
}

.tab:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.tab-spacer {
  flex: 1;
}

.modal-body {
  padding: 14px 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.banner {
  padding: 6px 10px;
  font-size: 12px;
  border-radius: 4px;
}

.banner-error {
  color: var(--status-failed, #ef4444);
  background-color: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.3);
}

.banner-ok {
  color: #4edea3;
  background-color: rgba(78, 222, 163, 0.08);
  border: 1px solid rgba(78, 222, 163, 0.3);
}

.cli-card {
  padding: 10px 12px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  background-color: var(--bg-app-base, #0f1117);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.cli-card.missing {
  opacity: 0.6;
}

.cli-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.cli-name {
  font-weight: 600;
  color: #e4b5ff;
}

.cli-label {
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
}

.cli-version {
  margin-left: auto;
  font-size: 11px;
  color: #4edea3;
}

.missing-text {
  color: var(--text-muted, #64748b);
}

.cli-grid {
  display: grid;
  grid-template-columns: 1fr 140px;
  gap: 8px;
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 10px;
}

.models-btn {
  margin-left: auto;
  font-size: 11px;
}

.field-narrow {
  max-width: 280px;
}

.step-row {
  display: grid;
  grid-template-columns: 140px 1fr 140px;
  gap: 8px;
  align-items: center;
}

.step-name {
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field-wide {
  grid-column: 1 / -1;
}

.field-label {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.field-static {
  font-size: 12px;
}

.field-input {
  padding: 5px 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--surface-container-low, #141721);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.field-textarea {
  resize: vertical;
}

.bypass-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.bypass-row.on {
  color: var(--status-failed, #ef4444);
}

.cli-actions {
  display: flex;
  justify-content: flex-end;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 10px;
  font-size: 12px;
  border-radius: 4px;
  border: 1px solid transparent;
  cursor: pointer;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-xs {
  padding: 3px 8px;
  font-size: 11px;
}

.btn-sm {
  padding: 4px 10px;
}

.btn-primary {
  color: #fff;
  background-color: #744791;
}

.btn-ghost {
  color: var(--text-secondary, #94a3b8);
  background: none;
}

.btn-icon {
  padding: 4px;
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
