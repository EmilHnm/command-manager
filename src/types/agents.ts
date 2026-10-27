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
  /** Model / effort per step; empty fields use the ones above. */
  steps: StepModels;
}

export interface StepModel {
  model?: string | null;
  effort?: string | null;
}

/** lead: design + plan; work: implement + fix; review: code review. */
export interface StepModels {
  lead: StepModel;
  work: StepModel;
  review: StepModel;
}

export type AgentPhase = keyof StepModels;

export const PHASE_LABELS: Record<AgentPhase, string> = {
  lead: 'Thiết kế & kế hoạch',
  work: 'Viết code & sửa',
  review: 'Review',
};

export interface QuotaWindow {
  /** 0..1 */
  utilization: number;
  /** Unix seconds. */
  resets_at: number;
}

/** A CLI account's usage limits (Claude's 5-hour / 7-day windows). */
export interface Quota {
  five_hour: QuotaWindow | null;
  seven_day: QuotaWindow | null;
  rejected: boolean;
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
  /** How long one setup or check command may run; empty: CHECK_TIMEOUT_MINUTES. */
  check_timeout_minutes?: number | null;
}

export const CHECK_TIMEOUT_MINUTES = 15;
export const MAX_TIMEOUT_MINUTES = 600;

/** A folder: one git repo, or a folder holding several (fe/, be/...). */
export interface AgentProject {
  id: string;
  path: string;
  repos: RepoConfig[];
  notes?: string | null;
  /** The last run's limits here: the next run starts from them. */
  limits?: RunLimits | null;
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

export interface PullRequest {
  repo: string;
  branch: string;
  base: string;
  url: string;
}

/** Gate 2's "Push & tạo PR". */
export interface PrRequest {
  repos: { repo: string; branch: string; base: string }[];
  title: string;
  body: string;
  draft: boolean;
}

/** What a finished run has not pushed to one of its pull requests. */
export interface PendingPush {
  repo: string;
  branch: string;
  url: string;
  /** null: origin's copy of the branch is not known here. */
  commits: number | null;
  /** Behind the PR's base or carrying merge commits: pushing rebases first. */
  needs_rebase: boolean;
  base: string;
}

/** A log tab that is not an agent (`agent:check-...`). */
export interface SideLog {
  id: string;
  task_id: string | null;
  kind: 'setup' | 'check' | 'context' | 'pr' | string;
  title: string;
  started_at: string;
  finished_at: string | null;
  ok: boolean | null;
}

export interface PrComment {
  author: string;
  path: string | null;
  line: number | null;
  body: string;
  url: string | null;
}

export interface PrCheck {
  repo: string;
  url: string;
  branch: string;
  base: string;
  state: string;
  review_decision: string;
  run_files: number;
  matched: number;
  pr_files: number;
  missing: string[];
  /** Fewer than half of the run's files are in the PR. */
  mismatch: boolean;
  open_comments: PrComment[];
  error: string | null;
}

/** What "Verify PR" found. */
export interface Verification {
  checked_at: string;
  prs: PrCheck[];
  review: Review | null;
  followed_up: boolean;
  note: string | null;
}

export interface CheckProposal {
  repo: string;
  setup: string;
  commands: string[];
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
  /** For this run; empty: the CLI profile's (Đội & CLI). */
  model?: string | null;
  effort?: string | null;
}

/** A model a CLI offers (`agents_models`). */
export interface ModelInfo {
  /** What `--model` takes. */
  id: string;
  label: string;
}

/** Until a CLI's own list is read: names it takes as aliases. Any name works. */
export const MODEL_SUGGESTIONS: Record<AgentCli, string[]> = {
  claude: ['opus', 'sonnet', 'haiku'],
  codex: [],
  agy: [],
  cursor: [],
};

export const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max'];

/** What a step uses when the run picks nothing: the profile's choice. */
export const profileModelHint = (profile: CliProfile | undefined, phase: AgentPhase) => {
  const model = profile?.steps?.[phase]?.model || profile?.model;
  return model ? `mặc định: ${model}` : 'mặc định của CLI';
};

export interface RunLimits {
  concurrency: number;
  max_rounds: number;
  max_invocations: number;
  timeout_minutes: number;
  /** Spending cap of the run, in list-price dollars the CLIs report. */
  max_cost_usd: number;
  /** Pause when an account usage window is this full (%); 0 = off. */
  quota_pause_percent: number;
  /** Go on by itself once the window resets. */
  auto_resume: boolean;
}

export const DEFAULT_LIMITS: RunLimits = {
  concurrency: 2,
  max_rounds: 3,
  max_invocations: 60,
  timeout_minutes: 20,
  max_cost_usd: 5,
  quota_pause_percent: 85,
  auto_resume: true,
};

export type RunStage =
  | 'research'
  | 'qa_gate'
  | 'design'
  | 'plan'
  | 'plan_gate'
  | 'implement'
  | 'integrate'
  | 'merge_gate'
  | 'done';
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
  | 'skipped'
  | 'awaiting_human'
  | 'triage'
  | 'passed';

/** Who does a task: an agent, or a person (a manual test). */
export type TaskKind = 'agent' | 'human';

export interface TestCase {
  id: string;
  title: string;
  steps: string[];
  expected: string;
}

/** What the AI prepared for a person's task. */
export interface TestGuide {
  summary: string;
  setup: string[];
  cases: TestCase[];
  notes: string;
}

export interface CaseResult {
  case_id: string;
  /** null: not tried. */
  pass: boolean | null;
  note: string;
}

export interface HumanRound {
  at: string;
  guide: TestGuide | null;
  results: CaseResult[];
  passed: boolean;
  note: string;
  attachments: string[];
  fix_tasks: string[];
}

export interface ChatMessage {
  role: 'user' | 'leader';
  text: string;
  at: string;
}

export interface HumanTest {
  guide: TestGuide | null;
  rounds: HumanRound[];
  chat: ChatMessage[];
  chat_session: string | null;
  leader_note: string | null;
}

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
  leader_model: string | null;
  leader_effort: string | null;
  /** Check commands the leader proposed for repos without any; gate 1 settles them. */
  suggested_checks: CheckProposal[];
  /** Opened from gate 2, one per repo; a follow-up's push updates them. */
  pull_requests: PullRequest[];
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
  /** Requests made after delivery (a code review's comments...); their tasks are batch 1, 2... */
  followups: { request: string; created_at: string }[];
  /** The leader's research before the design; null: the run skips it. */
  research: Research | null;
  /** Questions asked to the leader about the run, and its answers. */
  leader_chat: ChatMessage[];
}

/** What the leader found before designing, and the QA it raised. */
export interface Research {
  report: ResearchReport | null;
  /** Gate 0 settled. */
  approved: boolean;
}

export interface ResearchReport {
  summary: string;
  findings: Finding[];
  qa: QaItem[];
  at: string;
}

export interface Finding {
  title: string;
  status: 'ready' | 'missing' | 'unclear' | string;
  detail: string;
  source: string;
}

export interface QaItem {
  id: string;
  title: string;
  /** blocker: no design without an answer; question: the leader goes on with its assumption. */
  severity: 'blocker' | 'question' | string;
  detail: string;
  question: string;
  assumption: string;
  source: string;
  answer: string | null;
}

export interface QaAnswer {
  id: string;
  answer: string;
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
  /** 0: the run's request; n: its n-th follow-up. */
  batch: number;
  /** What the worker left for the tasks after it. */
  handoff: string | null;
  /** Picked when the task was given to another CLI; null: the role's. */
  model: string | null;
  effort: string | null;
  /** This task's agent timeout; null: the run's. */
  timeout_minutes: number | null;
  kind: TaskKind;
  /** A person's task: guide, rounds, chat. */
  human: HumanTest | null;
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
  kind?: TaskKind;
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
  /** Cut short: the next try continues its session. */
  resumable: boolean;
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
  /** Last usage limits seen, per CLI. */
  quotas: Partial<Record<AgentCli, Quota>>;
  /** Paused for a usage limit: when it goes on (unix seconds). */
  resume_at: number | null;
  /** The last "Verify PR". */
  verification: Verification | null;
  /** Checks, setup, context and push logs of this session. */
  side_logs: SideLog[];
  /** Worktrees of the run are still on disk. */
  worktrees: boolean;
  /** The last push that failed, until one goes through. */
  push_failure: PushFailure | null;
}

/** A refused push, for the leader to look into. */
export interface PushFailure {
  at: string;
  repo: string;
  error: string;
  /** The end of the push's output (the pre-push hook's checks). */
  output: string;
  request: PrRequest;
  diagnosis: string | null;
  /** What the leader chose: retry, self, tasks, none. */
  action: string | null;
  fix_tasks: string[];
  /** Fix tasks are on it: pushed again once they are merged. */
  retry: boolean;
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
  leader_model?: string | null;
  leader_effort?: string | null;
  roles: RoleCli[];
  plan_gate: boolean;
  merge_gate: boolean;
  limits: RunLimits;
  /** The leader looks for blockers and raises QA before designing. */
  research?: boolean;
}

export interface DiffStat {
  files: number;
  added: number;
  removed: number;
}

export const STAGE_LABELS: Record<RunStage, string> = {
  research: 'Tìm hiểu',
  qa_gate: 'Cổng 0',
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
  awaiting_human: 'Chờ người test',
  triage: 'Leader xác nhận lỗi',
  passed: 'Đã test ✓',
};

/** Statuses that let dependent tasks start. */
export const isSettled = (status: TaskStatus) =>
  status === 'merged' || status === 'skipped' || status === 'passed';

/** Run statuses after which nothing more happens. */
export const isFinished = (run: Pick<AgentRun, 'stage'>) => run.stage === 'done';
