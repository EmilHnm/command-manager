/**
 * SCR-09 Agent Studio types. Mirror src-tauri/src/db/repos/agents.rs,
 * src-tauri/src/agents/engine.rs and src-tauri/src/ipc/agents.rs.
 */

export type AgentCli = 'claude' | 'codex' | 'agy' | 'cursor';

export const AGENT_CLIS: AgentCli[] = ['claude', 'codex', 'agy', 'cursor'];

export const AGENT_CLI_LABELS: Record<AgentCli, string> = {
  claude: 'Claude Code',
  codex: 'Codex CLI',
  agy: 'Antigravity (agy)',
  cursor: 'Cursor CLI',
};

export interface CliProfile {
  cli: AgentCli;
  model?: string | null;
  effort?: string | null;
  /** "Tự duyệt mọi thứ": the CLI's own bypass flag. */
  bypass_permissions: boolean;
  extra_args: string[];
}

export interface CliInfo {
  cli: AgentCli;
  /** Null when not installed. */
  path: string | null;
  version: string | null;
  profile: CliProfile;
}

/** The repo at the project folder itself. */
export const ROOT_REPO = '.';

export interface RepoConfig {
  /** Relative to the project folder (`fe`, `apps/web`), or `.`. */
  path: string;
  base_branch?: string | null;
  setup_cmd?: string | null;
  check_cmds: string[];
}

/** A folder: one git repo, or a folder holding several (fe/, be/...). */
export interface AgentProject {
  id: string;
  path: string;
  repos: RepoConfig[];
  notes?: string | null;
  created_at: string;
  last_opened_at?: string | null;
}

export interface RepoStatus {
  path: string;
  branch: string | null;
  dirty: number;
  is_repo: boolean;
}

export interface AgentProjectView extends AgentProject {
  status: RepoStatus[];
}

export interface ProjectSettings {
  repos: RepoConfig[];
  notes?: string | null;
}

export interface RunRepo {
  path: string;
  base_branch: string;
  base_commit: string;
}

export const isMultiRepo = (repos: { path: string }[]) =>
  !(repos.length === 1 && repos[0].path === ROOT_REPO);

/** Display name of a repo: its folder, or the project folder's name. */
export const repoLabel = (path: string, projectPath = '') =>
  path === ROOT_REPO ? projectPath.split(/[\\/]/).filter(Boolean).pop() || '.' : path;

export interface RoleCli {
  role: string;
  cli: AgentCli;
}

export interface RunLimits {
  concurrency: number;
  max_rounds: number;
  max_invocations: number;
  timeout_minutes: number;
}

export const DEFAULT_LIMITS: RunLimits = {
  concurrency: 2,
  max_rounds: 3,
  max_invocations: 60,
  timeout_minutes: 20,
};

export type RunStage = 'design' | 'plan' | 'plan_gate' | 'implement' | 'integrate' | 'merge_gate' | 'done';
export type RunStatus = 'running' | 'waiting' | 'paused' | 'failed' | 'merged' | 'kept' | 'discarded';
export type TaskStatus =
  | 'waiting'
  | 'ready'
  | 'running'
  | 'checking'
  | 'review'
  | 'fixing'
  | 'merged'
  | 'failed'
  | 'conflict'
  | 'needs_human'
  | 'skipped';

export interface CheckResult {
  command: string;
  ok: boolean;
  exit_code: number | null;
  output: string;
  duration_ms: number;
}

export interface ReviewComment {
  file: string;
  line: number | null;
  issue: string;
  severity: 'must' | 'should';
}

export interface Review {
  verdict: 'approved' | 'changes_requested';
  summary: string;
  comments: ReviewComment[];
  round: number;
}

export interface AgentRun {
  id: string;
  project_id: string;
  title: string;
  request: string;
  leader_cli: AgentCli;
  roles: RoleCli[];
  plan_gate: boolean;
  merge_gate: boolean;
  limits: RunLimits;
  stage: RunStage;
  status: RunStatus;
  status_note: string | null;
  design_md: string | null;
  contract: string | null;
  leader_session: string | null;
  pending_feedback: string | null;
  base_branch: string;
  base_commit: string;
  repos: RunRepo[];
  integration_branch: string;
  integration_checks: CheckResult[] | null;
  created_at: string;
  updated_at: string;
  finished_at: string | null;
}

export interface AgentTask {
  id: string;
  run_id: string;
  key: string;
  title: string;
  role: string;
  cli: AgentCli;
  description: string;
  acceptance: string[];
  scope: string[];
  depends_on: string[];
  repos: string[];
  status: TaskStatus;
  status_note: string | null;
  round: number;
  worker_session: string | null;
  branch: string | null;
  worktree: string | null;
  checks: CheckResult[] | null;
  last_review: Review | null;
  added: number;
  removed: number;
  out_of_scope: string[];
  sort_order: number;
}

/** A task as gate 1 (MOD-22) edits it. */
export interface TaskDraft {
  key: string;
  title: string;
  role: string;
  cli: AgentCli;
  description: string;
  acceptance: string[];
  scope: string[];
  depends_on: string[];
  repos: string[];
}

export interface AgentInvocation {
  id: string;
  run_id: string;
  task_id: string | null;
  cli: AgentCli;
  step: 'design' | 'plan' | 'implement' | 'fix' | 'review' | string;
  session_id: string | null;
  terminal_id: string;
  started_at: string;
  finished_at: string | null;
  ok: boolean | null;
  error: string | null;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number | null;
}

export interface RunUsage {
  invocations: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
}

export interface RunDetail {
  run: AgentRun;
  tasks: AgentTask[];
  invocations: AgentInvocation[];
  usage: RunUsage;
  driving: boolean;
}

export interface RunSummary extends AgentRun {
  tasks_total: number;
  tasks_done: number;
  usage: RunUsage;
  driving: boolean;
}

export interface NewRunInput {
  project_id: string;
  request: string;
  title?: string | null;
  leader_cli: AgentCli;
  roles: RoleCli[];
  plan_gate: boolean;
  merge_gate: boolean;
  limits: RunLimits;
}

export interface DiffStat {
  files: number;
  added: number;
  removed: number;
}

export const STAGE_LABELS: Record<RunStage, string> = {
  design: 'Thiết kế',
  plan: 'Kế hoạch',
  plan_gate: 'Cổng 1',
  implement: 'Triển khai',
  integrate: 'Tích hợp',
  merge_gate: 'Cổng 2',
  done: 'Kết thúc',
};

export const RUN_STATUS_LABELS: Record<RunStatus, string> = {
  running: 'Đang chạy',
  waiting: 'Chờ',
  paused: 'Tạm dừng',
  failed: 'Lỗi',
  merged: 'Đã merge',
  kept: 'Giữ nhánh',
  discarded: 'Đã bỏ',
};

export const TASK_STATUS_LABELS: Record<TaskStatus, string> = {
  waiting: 'Chờ',
  ready: 'Sẵn sàng',
  running: 'Đang làm',
  checking: 'Kiểm tra',
  review: 'Review',
  fixing: 'Đang sửa',
  merged: 'Đã merge',
  failed: 'Lỗi',
  conflict: 'Xung đột merge',
  needs_human: 'Cần người xem',
  skipped: 'Bỏ qua',
};

/** Statuses that let dependent tasks start. */
export const isSettled = (status: TaskStatus) => status === 'merged' || status === 'skipped';

/** Run statuses after which nothing more happens. */
export const isFinished = (run: Pick<AgentRun, 'stage'>) => run.stage === 'done';
