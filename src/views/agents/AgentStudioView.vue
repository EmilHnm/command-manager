<template>
  <div class="agent-studio-view">
    <!-- Top Action Header (Toolbar) -->
    <header class="studio-header">
      <div class="header-left">
        <div class="header-icon-box">
          <Bot :size="18" class="header-icon" />
        </div>
        <div class="header-texts">
          <h1 class="page-title">Agent Studio</h1>
          <span class="page-subtitle">
            Điều phối đa agent phân cấp (Hierarchical Multi-Agent Orchestration • SCR-09)
          </span>
        </div>
      </div>

      <div class="header-right">
        <!-- Project Context Tag -->
        <div v-if="project" class="project-pill" :title="projectTooltip">
          <FolderGit2 :size="13" class="text-primary" />
          <span class="project-path">{{ shortPath(project.path) }}</span>
          <template v-if="multiRepo">
            <span :class="projectProblems ? 'branch-dirty' : 'branch-clean'">
              {{ project.status.length }} repo{{ projectProblems ? ` · ${projectProblems} cần chú ý` : ' ✓ sạch' }}
            </span>
          </template>
          <template v-else-if="project.status[0]">
            <span v-if="!project.status[0].is_repo" class="branch-dirty">không còn là git repo</span>
            <span v-else-if="project.status[0].dirty > 0" class="branch-dirty">
              {{ project.status[0].branch || 'detached' }} · {{ project.status[0].dirty }} thay đổi
            </span>
            <span v-else class="branch-clean">{{ project.status[0].branch || 'detached' }} ✓ sạch</span>
          </template>
        </div>

        <div class="project-menu-wrap">
          <button type="button" class="btn btn-secondary btn-sm" @click.stop="projectMenuOpen = !projectMenuOpen">
            <FolderOpen :size="13" />
            <span>Đổi thư mục</span>
          </button>
          <div v-if="projectMenuOpen" class="project-menu" @click.stop>
            <button
              v-for="item in recentProjects"
              :key="item.id"
              type="button"
              class="project-menu-item font-mono"
              :class="{ current: item.id === project?.id }"
              @click="openProject(item.path)"
            >
              {{ shortPath(item.path) }}
            </button>
            <div v-if="recentProjects.length > 0" class="project-menu-divider" />
            <button type="button" class="project-menu-item" @click="askFolder">
              <FolderOpen :size="12" /> Mở thư mục khác…
            </button>
          </div>
        </div>

        <button type="button" class="btn btn-secondary btn-sm" @click="openTeamModal">
          <Users :size="13" />
          <span>Đội & CLI</span>
        </button>

        <button
          type="button"
          class="btn btn-primary btn-sm btn-new-run"
          :disabled="!project || !project.status.some((repo) => repo.is_repo)"
          @click="openNewRun"
        >
          <Plus :size="14" />
          <span>Lượt mới</span>
        </button>
      </div>
    </header>

    <div v-if="notice" class="studio-notice" :class="'notice-' + notice.kind" @click="notice = null">
      {{ notice.text }}
    </div>

    <!-- No project yet -->
    <div v-if="!project" class="studio-empty">
      <FolderGit2 :size="36" class="text-primary" />
      <h2>Chọn thư mục dự án</h2>
      <p>
        Mở một git repo, hoặc một thư mục chứa nhiều repo (fe/, be/, docs/…). Mỗi task chạy trong worktree riêng,
        không đụng nhánh của bạn.
      </p>
      <div class="empty-open-row">
        <input
          v-model="folderInput"
          class="empty-input font-mono"
          placeholder="/đường/dẫn/tới/repo"
          @keydown.enter="openProject(folderInput)"
        />
        <button type="button" class="btn btn-primary btn-sm" :disabled="!folderInput.trim()" @click="openProject(folderInput)">
          Mở
        </button>
      </div>
      <div v-if="recentProjects.length > 0" class="empty-recent">
        <span class="text-muted">Gần đây:</span>
        <button
          v-for="item in recentProjects"
          :key="item.id"
          type="button"
          class="btn btn-secondary btn-xs font-mono"
          @click="openProject(item.path)"
        >
          {{ shortPath(item.path) }}
        </button>
      </div>
    </div>

    <!-- Main Workspace Body (2-Column) -->
    <div v-else class="studio-body">
      <!-- Left Sidebar: LƯỢT CHẠY / RUNS -->
      <aside class="runs-sidebar">
        <div class="runs-sidebar-header">
          <span class="runs-header-title">LƯỢT CHẠY / RUNS</span>
          <span class="runs-count font-mono">{{ activeRunCount }} active</span>
        </div>

        <div class="runs-list custom-scrollbar">
          <div v-if="runs.length === 0" class="runs-empty text-muted">
            Chưa có lượt chạy. Bấm <strong>Lượt mới</strong> để giao việc cho đội agent.
          </div>
          <div
            v-for="run in runs"
            :key="run.id"
            class="run-item"
            :class="{ active: selectedRunId === run.id }"
            @click="selectRun(run.id)"
          >
            <div class="run-item-top">
              <span class="run-name">
                <span class="run-status-dot" :class="'dot-' + runTone(run)"></span>
                <span>{{ run.title }}</span>
              </span>
              <span class="run-stage-badge" :class="'badge-' + runTone(run)">
                {{ runBadge(run) }}
              </span>
            </div>

            <div class="run-item-desc font-mono">
              {{ run.request }}
            </div>

            <div class="run-item-footer font-mono">
              <span>{{ run.leader_cli }} • {{ duration(run) }}</span>
              <span class="run-progress-text">{{ run.tasks_done }}/{{ run.tasks_total }} tasks</span>
            </div>
          </div>
        </div>
      </aside>

      <!-- Center / Right: Active Run Dashboard -->
      <main class="run-dashboard custom-scrollbar">
        <div v-if="!detail" class="studio-empty small">
          <GitFork :size="28" class="text-primary" />
          <p>{{ runs.length > 0 ? 'Chọn một lượt chạy bên trái.' : 'Dự án chưa có lượt chạy nào.' }}</p>
        </div>

        <template v-else>
          <!-- Active Run Workflow Header -->
          <div class="card run-hero-card">
            <div class="hero-top-row">
              <div class="hero-title-wrap">
                <div class="hero-icon-wrap">
                  <GitFork :size="16" class="text-primary" />
                </div>
                <h2 class="hero-title" :title="detail.run.request">{{ detail.run.title }}</h2>
                <span class="hero-meta-badge font-mono">Leader: {{ detail.run.leader_cli }}</span>
                <span class="hero-meta-badge font-mono" :title="runReposTooltip">
                  <template v-if="isMultiRepo(detail.run.repos)">
                    {{ detail.run.repos.length }} repo ← {{ detail.run.integration_branch }}
                  </template>
                  <template v-else>Nhánh: {{ detail.run.base_branch }} ← {{ detail.run.integration_branch }}</template>
                </span>
                <span class="hero-timer font-mono">⏱ {{ duration(detail.run) }}</span>
              </div>

              <div class="hero-actions">
                <template v-if="detail.driving">
                  <button type="button" class="btn btn-secondary btn-sm" @click="act(() => ipcClient.agentsRunPause(detail!.run.id))">
                    <Pause :size="13" />
                    <span>Tạm dừng</span>
                  </button>
                  <button type="button" class="btn btn-danger-ghost btn-sm" @click="stopNow">
                    <Square :size="13" />
                    <span>Dừng ngay</span>
                  </button>
                </template>
                <template v-else-if="detail.run.stage === 'plan_gate'">
                  <button type="button" class="btn btn-primary btn-sm" @click="showPlanGateModal = true">
                    <span>⛩ Mở Cổng 1</span>
                  </button>
                </template>
                <template v-else-if="detail.run.stage === 'merge_gate'">
                  <button type="button" class="btn btn-primary btn-sm" @click="showMergeGateModal = true">
                    <span>⛩ Mở Cổng 2</span>
                  </button>
                </template>
                <button
                  v-else-if="detail.run.stage !== 'done'"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  @click="act(() => ipcClient.agentsRunResume(detail!.run.id))"
                >
                  <Play :size="13" />
                  <span>Tiếp tục</span>
                </button>
                <button
                  v-if="detail.run.stage !== 'done' && !detail.driving"
                  type="button"
                  class="btn btn-danger-ghost btn-sm"
                  @click="discardRun"
                >
                  <XCircle :size="13" />
                  <span>Bỏ lượt</span>
                </button>
                <button
                  v-if="detail.run.stage === 'done'"
                  type="button"
                  class="btn btn-danger-ghost btn-sm"
                  @click="deleteRun"
                >
                  <Trash2 :size="13" />
                  <span>Xóa</span>
                </button>
              </div>
            </div>

            <!-- Pipeline Stepper Ribbon -->
            <div class="pipeline-stepper font-mono">
              <template v-for="(step, index) in STEPS" :key="step.stage">
                <span v-if="index > 0" class="step-separator">─</span>
                <component
                  :is="step.gate ? 'button' : 'div'"
                  :type="step.gate ? 'button' : undefined"
                  class="step-item"
                  :class="[stepState(step.stage), { gate: step.gate, clickable: step.gate && stepState(step.stage) === 'active' }]"
                  @click="step.gate && stepState(step.stage) === 'active' ? openGate(step.stage) : undefined"
                >
                  <span v-if="step.gate" class="gate-icon-mini">⛩</span>
                  <Check v-else-if="stepState(step.stage) === 'pass'" :size="12" />
                  <span v-else-if="stepState(step.stage) === 'active' && detail.driving" class="pulse-dot"></span>
                  <span>{{ step.label }}{{ step.gate && stepState(step.stage) === 'pass' ? ' ✓' : '' }}</span>
                </component>
              </template>
            </div>

            <div v-if="detail.run.status_note" class="run-note" :class="'note-' + runTone(detail.run)">
              <AlertTriangle v-if="detail.run.status === 'failed'" :size="13" />
              <Clock v-else :size="13" />
              <span>{{ detail.run.status_note }}</span>
            </div>
          </div>

          <!-- Task Allocation Matrix Table -->
          <div class="card matrix-card">
            <div class="matrix-header">
              <div class="matrix-title">
                <Layers :size="15" class="text-primary" />
                <span>MA TRẬN PHÂN CHIA VÀ THỰC THI NHIỆM VỤ (TASK ALLOCATION)</span>
              </div>
              <div class="matrix-meta font-mono">
                <span class="matrix-concurrency">
                  {{ detail.run.roles.map((r) => `${r.role}→${r.cli}`).join(' · ') }}
                </span>
              </div>
            </div>

            <div class="table-wrap">
              <table class="tasks-table font-mono">
                <thead>
                  <tr>
                    <th class="col-task">Task</th>
                    <th class="col-role">Vai (Role)</th>
                    <th class="col-cli">Agent CLI</th>
                    <th class="col-status">Trạng thái (Status)</th>
                    <th class="col-round">Vòng Review</th>
                    <th class="col-diff text-right">± Diff</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-if="detail.tasks.length === 0">
                    <td colspan="6" class="text-muted">
                      {{ detail.run.stage === 'design' || detail.run.stage === 'plan' ? 'Leader đang thiết kế và lập kế hoạch…' : 'Chưa có task.' }}
                    </td>
                  </tr>
                  <tr
                    v-for="task in detail.tasks"
                    :key="task.id"
                    class="task-row"
                    :class="{ selected: selectedTaskId === task.id }"
                    @click="selectTask(task.id)"
                  >
                    <td class="col-task font-bold" :title="task.title">
                      <span class="task-key-tag">{{ task.key }}</span>
                      <span v-if="isMultiRepo(detail.run.repos)" class="task-repos">{{ task.repos.join(', ') }}</span>
                    </td>
                    <td class="col-role">
                      <span class="role-pill" :class="'role-' + task.role">{{ task.role }}</span>
                    </td>
                    <td class="col-cli">
                      <span class="cli-tag">{{ task.cli }}</span>
                    </td>
                    <td class="col-status">
                      <span class="status-indicator" :class="'status-' + taskTone(task)">
                        <span v-if="task.status === 'running' || task.status === 'fixing'" class="pulse-dot-sm"></span>
                        <Check v-else-if="task.status === 'merged'" :size="12" />
                        <AlertCircle v-else-if="taskTone(task) === 'failed' || task.status === 'review'" :size="12" />
                        <Clock v-else :size="12" />
                        <span>{{ taskStatusText(task) }}</span>
                      </span>
                    </td>
                    <td class="col-round text-muted">
                      {{ roundText(task) }}
                    </td>
                    <td class="col-diff text-right font-bold" :class="task.added + task.removed > 0 ? 'text-status-running' : 'text-muted'">
                      {{ task.added + task.removed > 0 ? `+${task.added} / -${task.removed}` : '–' }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- Selected Task Inspector & Diagnostics Panel -->
          <div v-if="selectedTask" class="card inspector-card">
            <div class="inspector-header">
              <div class="inspector-title-row">
                <div class="inspector-icon">
                  <SearchCode :size="16" class="text-primary" />
                </div>
                <h3 class="inspector-title">Task Inspector: {{ selectedTask.key }}</h3>
                <span class="inspector-badge font-mono">Agent: {{ selectedTask.cli }}</span>
                <span v-if="selectedTask.branch" class="inspector-badge font-mono">{{ selectedTask.branch }}</span>
                <span class="inspector-badge warning font-mono">{{ roundText(selectedTask) }}</span>
              </div>

              <div class="inspector-toolbar">
                <button
                  type="button"
                  class="btn btn-secondary btn-xs"
                  :disabled="!latestInvocation(selectedTask.id)"
                  @click="openLog(latestInvocation(selectedTask.id)!)"
                >
                  <Terminal :size="12" />
                  <span>Xem log trực tiếp</span>
                </button>
                <button type="button" class="btn btn-secondary btn-xs" :disabled="!selectedTask.branch" @click="showTaskDiff">
                  <FileCode :size="12" />
                  <span>Xem diff</span>
                </button>
                <button type="button" class="btn btn-secondary btn-xs" :disabled="!selectedTask.worktree" @click="openWorktreeFiles">
                  <Folder :size="12" />
                  <span>Mở trong Files</span>
                </button>
                <button type="button" class="btn btn-secondary btn-xs" :disabled="!selectedTask.worktree" @click="openWorktreeTerminal">
                  <Code2 :size="12" />
                  <span>Terminal worktree</span>
                </button>
              </div>
            </div>

            <p class="task-description">{{ selectedTask.title }}<template v-if="selectedTask.description"> — {{ selectedTask.description }}</template></p>

            <div v-if="selectedTask.status_note" class="review-alert-box">
              <AlertTriangle :size="15" class="alert-icon" />
              <div class="alert-content font-mono">
                <strong class="alert-verdict">{{ TASK_STATUS_LABELS[selectedTask.status] }}</strong>
                <span> — {{ selectedTask.status_note }}</span>
              </div>
            </div>

            <!-- Review Warning Box -->
            <div v-if="selectedTask.last_review" class="review-alert-box" :class="{ approved: selectedTask.last_review.verdict === 'approved' }">
              <AlertTriangle v-if="selectedTask.last_review.verdict !== 'approved'" :size="15" class="alert-icon" />
              <CheckCircle2 v-else :size="15" class="alert-icon" />
              <div class="alert-content font-mono">
                <strong class="alert-verdict">
                  Review vòng {{ selectedTask.last_review.round }}:
                  {{ selectedTask.last_review.verdict === 'approved' ? '✓ Đạt' : '✗ Cần sửa' }}
                </strong>
                <span> — {{ selectedTask.last_review.summary }}</span>
                <div v-for="(comment, index) in selectedTask.last_review.comments" :key="index" class="alert-file">
                  [{{ comment.severity }}] {{ comment.file }}{{ comment.line ? `:${comment.line}` : '' }} — {{ comment.issue }}
                </div>
              </div>
            </div>

            <!-- Automated Verification Guard Strip -->
            <div v-if="(selectedTask.checks && selectedTask.checks.length) || selectedTask.out_of_scope.length" class="guard-strip font-mono">
              <div
                v-for="check in selectedTask.checks || []"
                :key="check.command"
                class="guard-item"
                :class="check.ok ? 'pass' : 'fail'"
                :title="check.ok ? undefined : check.output"
              >
                <CheckCircle2 v-if="check.ok" :size="13" />
                <AlertCircle v-else :size="13" />
                <span>{{ check.command }} ({{ (check.duration_ms / 1000).toFixed(1) }}s) {{ check.ok ? '✓' : '✗' }}</span>
              </div>
              <div v-if="selectedTask.out_of_scope.length" class="guard-item warning">
                <AlertCircle :size="13" />
                <span>Sửa ngoài scope: {{ selectedTask.out_of_scope.join(', ') }}</span>
              </div>
            </div>

            <!-- Workflow Controls -->
            <div class="inspector-footer">
              <div class="footer-left">
                <select
                  class="btn btn-secondary btn-sm reassign-select"
                  :disabled="!canEditTasks || selectedTask.status === 'merged'"
                  :title="canEditTasks ? 'Giao task cho CLI khác (phiên mới, cùng worktree)' : editHint"
                  :value="''"
                  @change="reassign($event)"
                >
                  <option value="" disabled>Giao lại cho agent khác ▾</option>
                  <option v-for="cli in AGENT_CLIS" :key="cli" :value="cli" :disabled="cli === selectedTask.cli || !installed(cli)">
                    {{ cli }}
                  </option>
                </select>

                <button
                  v-if="['failed', 'conflict', 'needs_human'].includes(selectedTask.status)"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canEditTasks"
                  :title="canEditTasks ? undefined : editHint"
                  @click="act(() => ipcClient.agentsTaskRetry(selectedTask!.id))"
                >
                  <RotateCcw :size="12" />
                  <span>Thử lại</span>
                </button>

                <button
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canEditTasks || !selectedTask.worktree"
                  :title="canEditTasks ? 'Merge task như hiện tại, không chờ leader duyệt' : editHint"
                  @click="approveTask"
                >
                  <span>Bỏ qua review</span>
                </button>

                <button
                  type="button"
                  class="btn btn-secondary btn-sm text-warning"
                  :disabled="!canEditTasks || selectedTask.status === 'merged' || selectedTask.status === 'skipped'"
                  :title="canEditTasks ? 'Bỏ qua task; các task phụ thuộc vẫn được chạy' : editHint"
                  @click="skipTask"
                >
                  <span>Bỏ qua task</span>
                </button>
              </div>

              <button
                type="button"
                class="btn btn-primary btn-sm btn-retry-agent"
                :disabled="!canEditTasks || !selectedTask.worktree"
                :title="canEditTasks ? undefined : editHint"
                @click="fixPromptVisible = true"
              >
                <Zap :size="13" />
                <span>Yêu cầu agent sửa lại</span>
              </button>
            </div>
          </div>

          <!-- Agent Activity -->
          <div class="card pty-monitor-card">
            <div class="pty-header font-mono">
              <div class="pty-title">
                <span class="stream-dot" :class="{ idle: !runningInvocation }"></span>
                <span class="font-bold text-text-primary">Hoạt động agent</span>
              </div>
              <div class="pty-stats text-muted">
                <span>{{ detail.invocations.length }} lượt gọi</span>
                <template v-if="runningInvocation">
                  <span>•</span>
                  <span class="text-status-running">{{ runningInvocation.cli }} đang chạy…</span>
                </template>
              </div>
            </div>

            <div class="pty-viewport font-mono custom-scrollbar">
              <div v-if="detail.invocations.length === 0" class="pty-line text-muted">Chưa có agent nào chạy.</div>
              <button
                v-for="inv in [...detail.invocations].reverse()"
                :key="inv.id"
                type="button"
                class="pty-line activity-line"
                :class="inv.ok === false ? 'text-warning' : inv.finished_at ? 'text-muted' : 'text-status-running'"
                title="Mở log của lượt gọi này"
                @click="openLog(inv)"
              >
                [{{ clock(inv.started_at) }}] [{{ inv.cli }}] {{ STEP_LABELS[inv.step] || inv.step }}{{ taskKeyOf(inv) ? ` · ${taskKeyOf(inv)}` : '' }}
                <template v-if="!inv.finished_at"> — đang chạy…</template>
                <template v-else-if="inv.ok"> — ✓ {{ formatTokens(inv.input_tokens + inv.output_tokens) }} token</template>
                <template v-else> — ✗ {{ inv.error }}</template>
              </button>
            </div>
          </div>
        </template>
      </main>
    </div>

    <!-- Bottom Telemetry Footer Strip -->
    <footer v-if="detail" class="studio-footer font-mono">
      <div class="footer-metrics">
        <span class="metric-pill">
          Chi phí lượt này:
          <strong>
            {{ formatTokens(detail.usage.input_tokens + detail.usage.output_tokens) }} token{{ detail.usage.cost_usd > 0 ? ` (~$${detail.usage.cost_usd.toFixed(2)})` : '' }}
          </strong>
        </span>
        <span class="metric-separator">•</span>
        <span class="metric-pill">Agent đang chạy: <strong>{{ runningInvocation ? 1 : 0 }}</strong></span>
        <span class="metric-separator">•</span>
        <span class="metric-pill">Đã gọi: <strong>{{ detail.usage.invocations }}/{{ detail.run.limits.max_invocations }} lượt</strong></span>
        <span class="metric-separator">•</span>
        <span class="metric-pill text-muted">Base: {{ detail.run.base_commit.slice(0, 8) }}</span>
      </div>

      <div class="footer-telemetry text-muted">
        <span>{{ RUN_STATUS_LABELS[detail.run.status] }}</span>
        <span>•</span>
        <span>{{ STAGE_LABELS[detail.run.stage] }}</span>
      </div>
    </footer>

    <!-- Modals -->
    <NewAgentRunModal
      :visible="showNewRunModal"
      :project="project"
      :clis="clis"
      :submit="startRun"
      @close="showNewRunModal = false"
    />

    <PlanGateModal
      :visible="showPlanGateModal"
      :detail="detail"
      :clis="clis"
      :approve="approvePlan"
      :revise="revisePlan"
      :discard="discardFromGate"
      @close="showPlanGateModal = false"
    />

    <MergeGateModal
      :visible="showMergeGateModal"
      :detail="detail"
      :merge="mergeRun"
      :keep="keepRun"
      :discard="discardFromGate"
      @close="showMergeGateModal = false"
      @inspect-diff="showRunDiff"
      @open-terminal="openIntegrationTerminal"
    />

    <AgentTeamModal
      :visible="showTeamModal"
      :clis="clis"
      :detecting="detecting"
      :project="project"
      @close="showTeamModal = false"
      @refresh="detectClis"
      @saved-cli="detectClis"
      @saved-project="(view) => (project = view)"
    />

    <AgentDiffModal
      :visible="diff.visible"
      :title="diff.title"
      :text="diff.text"
      :loading="diff.loading"
      :error="diff.error"
      @close="diff.visible = false"
    />

    <ConfirmDialog
      :visible="confirmBox.visible"
      :title="confirmBox.title"
      subtitle="SCR-09 • Agent Studio"
      :message="confirmBox.message"
      :confirm-text="confirmBox.confirmText"
      loading-text="Đang xử lý..."
      :danger="confirmBox.danger"
      :loading="confirmBox.loading"
      @confirm="confirmAccepted"
      @cancel="confirmBox.visible = false"
    />

    <PromptDialog
      :visible="folderPromptVisible"
      title="Mở Thư Mục Dự Án"
      label="Đường dẫn tới git repo"
      :default-value="project?.path || ''"
      confirm-text="Mở"
      icon="edit"
      @confirm="(path: string) => { folderPromptVisible = false; openProject(path); }"
      @cancel="folderPromptVisible = false"
    />

    <PromptDialog
      :visible="fixPromptVisible"
      title="Yêu Cầu Agent Sửa Lại"
      :subtitle="selectedTask ? `${selectedTask.cli} · ${selectedTask.key}` : ''"
      label="Cần sửa gì?"
      placeholder="Ví dụ: Thiếu kiểm tra coupon hết hạn ở apply_coupon"
      default-value=""
      confirm-text="Gửi cho agent"
      icon="edit"
      @confirm="requestFix"
      @cancel="fixPromptVisible = false"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import {
  Bot,
  FolderGit2,
  FolderOpen,
  Users,
  Plus,
  GitFork,
  Pause,
  Play,
  Square,
  Check,
  Layers,
  SearchCode,
  Terminal,
  FileCode,
  Folder,
  Code2,
  AlertTriangle,
  CheckCircle2,
  AlertCircle,
  Clock,
  Zap,
  XCircle,
  Trash2,
  RotateCcw,
} from 'lucide-vue-next';
import NewAgentRunModal from '@/components/dialogs/NewAgentRunModal.vue';
import PlanGateModal from '@/components/dialogs/PlanGateModal.vue';
import MergeGateModal from '@/components/dialogs/MergeGateModal.vue';
import AgentTeamModal from '@/components/dialogs/AgentTeamModal.vue';
import AgentDiffModal from '@/components/dialogs/AgentDiffModal.vue';
import PromptDialog from '@/components/dialogs/PromptDialog.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';
import { ipcClient } from '@/ipc/client';
import type {
  AgentCli,
  AgentInvocation,
  AgentProject,
  AgentProjectView,
  AgentRun,
  AgentTask,
  CliInfo,
  NewRunInput,
  RunDetail,
  RunStage,
  RunSummary,
  TaskDraft,
} from '@/types/agents';
import {
  AGENT_CLIS,
  RUN_STATUS_LABELS,
  STAGE_LABELS,
  TASK_STATUS_LABELS,
  isMultiRepo,
  repoLabel,
} from '@/types/agents';

const LAST_PROJECT_KEY = 'agents.lastProjectPath';

const STEPS: { stage: RunStage; label: string; gate?: boolean }[] = [
  { stage: 'design', label: '① Thiết kế' },
  { stage: 'plan', label: '② Kế hoạch' },
  { stage: 'plan_gate', label: 'Cổng 1', gate: true },
  { stage: 'implement', label: '③ Triển khai' },
  { stage: 'integrate', label: '④ Tích hợp' },
  { stage: 'merge_gate', label: 'Cổng 2', gate: true },
];

const STEP_LABELS: Record<string, string> = {
  design: 'thiết kế',
  plan: 'kế hoạch',
  implement: 'triển khai',
  fix: 'sửa',
  review: 'review',
};

const route = useRoute();
const router = useRouter();

const project = ref<AgentProjectView | null>(null);
const recentProjects = ref<AgentProject[]>([]);
const runs = ref<RunSummary[]>([]);
const selectedRunId = ref<string | null>(null);
const detail = ref<RunDetail | null>(null);
const selectedTaskId = ref<string | null>(null);
const clis = ref<CliInfo[]>([]);
const detecting = ref(false);
const folderInput = ref('');
const now = ref(Math.floor(Date.now() / 1000));
const notice = ref<{ kind: 'error' | 'info'; text: string } | null>(null);

const showNewRunModal = ref(false);
const showPlanGateModal = ref(false);
const showMergeGateModal = ref(false);
const showTeamModal = ref(false);
const projectMenuOpen = ref(false);
const folderPromptVisible = ref(false);
const fixPromptVisible = ref(false);
const diff = reactive({ visible: false, title: '', text: '', loading: false, error: null as string | null });

const errorText = (error: unknown) => (error instanceof Error ? error.message : String(error));

const notify = (text: string, kind: 'error' | 'info' = 'error') => {
  notice.value = { kind, text };
  window.setTimeout(() => {
    if (notice.value?.text === text) notice.value = null;
  }, 6000);
};

/** Runs an action, showing its error; the change events reload the view. */
const act = async (action: () => Promise<unknown>) => {
  try {
    await action();
    await reload();
  } catch (error) {
    notify(errorText(error));
  }
};

// ---- formatting -----------------------------------------------------------

const shortPath = (path: string) => path.replace(/^\/home\/[^/]+/, '~').replace(/^\/Users\/[^/]+/, '~');

const duration = (run: Pick<AgentRun, 'created_at' | 'finished_at'>) => {
  const end = run.finished_at ? Number(run.finished_at) : now.value;
  const secs = Math.max(0, end - Number(run.created_at));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}h ${pad(m)}m` : `${pad(m)}m ${pad(s)}s`;
};

const clock = (unix: string) =>
  new Date(Number(unix) * 1000).toLocaleTimeString('vi-VN', { hour12: false });

const formatTokens = (n: number) => (n >= 10000 ? `${Math.round(n / 1000)}k` : String(n));

/** Colour family of a run, for the dot and badge classes. */
const runTone = (run: Pick<AgentRun, 'status'>) => {
  switch (run.status) {
    case 'running':
      return 'running';
    case 'waiting':
      return 'review';
    case 'failed':
      return 'failed';
    case 'merged':
    case 'kept':
      return 'merged';
    default:
      return 'paused';
  }
};

const runBadge = (run: RunSummary) =>
  run.stage === 'done' || run.status === 'paused' || run.status === 'failed'
    ? RUN_STATUS_LABELS[run.status]
    : STAGE_LABELS[run.stage];

const taskTone = (task: AgentTask) => {
  switch (task.status) {
    case 'merged':
      return 'merged';
    case 'review':
    case 'checking':
      return 'review';
    case 'running':
    case 'fixing':
      return 'running';
    case 'failed':
    case 'conflict':
    case 'needs_human':
      return 'failed';
    default:
      return 'waiting';
  }
};

const taskStatusText = (task: AgentTask) => {
  if (task.status === 'waiting' && detail.value) {
    const pending = task.depends_on.filter((dep) => {
      const other = detail.value!.tasks.find((t) => t.key === dep);
      return other && other.status !== 'merged' && other.status !== 'skipped';
    });
    if (pending.length > 0) return `Chờ ${pending.join(', ')}`;
  }
  if (task.status === 'review') return `Review (${task.round + 1}/${detail.value?.run.limits.max_rounds ?? 3})`;
  return TASK_STATUS_LABELS[task.status];
};

const roundText = (task: AgentTask) => {
  if (!task.branch) return '–';
  const verdict = task.last_review
    ? task.last_review.verdict === 'approved'
      ? ' (Đạt)'
      : ' (Cần sửa)'
    : '';
  return `Vòng ${task.round + 1}${verdict}`;
};

const installed = (cli: AgentCli) => clis.value.some((info) => info.cli === cli && info.path);

// ---- derived state --------------------------------------------------------

const multiRepo = computed(() => !!project.value && isMultiRepo(project.value.repos));

/** Repos not usable as they are: gone, detached or with uncommitted files. */
const projectProblems = computed(
  () => project.value?.status.filter((repo) => !repo.is_repo || !repo.branch || repo.dirty > 0).length ?? 0
);

const projectTooltip = computed(() => {
  const view = project.value;
  if (!view) return '';
  return [
    view.path,
    ...view.status.map((repo) => {
      const name = repoLabel(repo.path, view.path);
      if (!repo.is_repo) return `${name}: không còn là git repo`;
      const dirty = repo.dirty > 0 ? ` · ${repo.dirty} thay đổi chưa commit` : '';
      return `${name}: ${repo.branch || 'detached'}${dirty}`;
    }),
  ].join('\n');
});

const runReposTooltip = computed(() =>
  (detail.value?.run.repos ?? [])
    .map((repo) => `${repoLabel(repo.path, project.value?.path)}: ${repo.base_branch} @ ${repo.base_commit.slice(0, 8)}`)
    .join('\n')
);

const activeRunCount = computed(() => runs.value.filter((run) => run.stage !== 'done').length);

const selectedTask = computed(() => detail.value?.tasks.find((t) => t.id === selectedTaskId.value) ?? null);

const runningInvocation = computed(() => detail.value?.invocations.find((inv) => !inv.finished_at) ?? null);

/** Task actions need the driver stopped (it might be working on the task). */
const canEditTasks = computed(
  () => !!detail.value && !detail.value.driving && detail.value.run.stage === 'implement'
);

const editHint = computed(() =>
  detail.value?.driving ? 'Tạm dừng lượt chạy trước' : 'Chỉ dùng được ở bước triển khai'
);

const stepState = (stage: RunStage) => {
  const run = detail.value?.run;
  if (!run) return 'pending';
  if (run.stage === 'done') return run.status === 'discarded' ? 'pending' : 'pass';
  const current = STEPS.findIndex((step) => step.stage === run.stage);
  const index = STEPS.findIndex((step) => step.stage === stage);
  if (index < current) return 'pass';
  return index === current ? 'active' : 'pending';
};

const latestInvocation = (taskId: string) =>
  [...(detail.value?.invocations ?? [])].reverse().find((inv) => inv.task_id === taskId) ?? null;

const taskKeyOf = (inv: AgentInvocation) =>
  inv.task_id ? detail.value?.tasks.find((t) => t.id === inv.task_id)?.key : undefined;

// ---- loading --------------------------------------------------------------

const loadRuns = async () => {
  if (!project.value) {
    runs.value = [];
    return;
  }
  runs.value = await ipcClient.agentsRunList(project.value.id);
  if (selectedRunId.value && !runs.value.some((run) => run.id === selectedRunId.value)) {
    selectedRunId.value = null;
    detail.value = null;
  }
  if (!selectedRunId.value && runs.value.length > 0) {
    selectedRunId.value = runs.value[0].id;
  }
};

const loadDetail = async () => {
  if (!selectedRunId.value) {
    detail.value = null;
    return;
  }
  const next = await ipcClient.agentsRunGet(selectedRunId.value);
  detail.value = next;
  const tasks = next.tasks;
  if (!tasks.some((t) => t.id === selectedTaskId.value)) {
    // Follow the work: the task being worked on, else the first one.
    const active = tasks.find((t) => ['running', 'checking', 'review', 'fixing'].includes(t.status));
    selectedTaskId.value = (active ?? tasks[0])?.id ?? null;
  }
};

const reload = async () => {
  await loadRuns();
  await loadDetail();
};

let reloadTimer: number | undefined;
const scheduleReload = () => {
  window.clearTimeout(reloadTimer);
  reloadTimer = window.setTimeout(() => {
    void reload().catch((error) => console.error('[AgentStudio] reload failed:', error));
  }, 150);
};

const detectClis = async () => {
  detecting.value = true;
  try {
    clis.value = await ipcClient.agentsDetect();
  } catch (error) {
    notify(errorText(error));
  } finally {
    detecting.value = false;
  }
};

const openProject = async (path: string) => {
  projectMenuOpen.value = false;
  const trimmed = path.trim();
  if (!trimmed) return;
  try {
    project.value = await ipcClient.agentsProjectOpen(trimmed);
    try {
      localStorage.setItem(LAST_PROJECT_KEY, project.value.path);
    } catch {
      // Storage may be unavailable; the project just is not remembered.
    }
    selectedRunId.value = null;
    detail.value = null;
    recentProjects.value = await ipcClient.agentsProjectList();
    await reload();
  } catch (error) {
    notify(errorText(error));
  }
};

const askFolder = () => {
  projectMenuOpen.value = false;
  folderPromptVisible.value = true;
};

const selectRun = (id: string) => {
  selectedRunId.value = id;
  selectedTaskId.value = null;
  void loadDetail().catch((error) => notify(errorText(error)));
};

const selectTask = (id: string) => {
  selectedTaskId.value = id;
};

// ---- actions --------------------------------------------------------------

const openTeamModal = () => {
  showTeamModal.value = true;
  if (clis.value.length === 0) void detectClis();
};

const openNewRun = async () => {
  if (clis.value.length === 0) await detectClis();
  if (project.value) {
    // Branch and uncommitted files may have changed since it was opened.
    project.value = await ipcClient.agentsProjectGet(project.value.id).catch(() => project.value);
  }
  showNewRunModal.value = true;
};

const startRun = async (input: Omit<NewRunInput, 'project_id'>) => {
  if (!project.value) throw new Error('Chưa chọn dự án');
  const run = await ipcClient.agentsRunStart({ ...input, project_id: project.value.id });
  selectedRunId.value = run.id;
  selectedTaskId.value = null;
  await reload();
};

const openGate = (stage: RunStage) => {
  if (stage === 'plan_gate') showPlanGateModal.value = true;
  if (stage === 'merge_gate') showMergeGateModal.value = true;
};

const approvePlan = async (data: { design: string; contract: string; tasks: TaskDraft[] }) => {
  await ipcClient.agentsPlanApprove(detail.value!.run.id, data.design, data.contract, data.tasks);
  await reload();
};

const revisePlan = async (data: { design: string; feedback: string }) => {
  await ipcClient.agentsPlanRevise(detail.value!.run.id, data.design, data.feedback);
  await reload();
};

const mergeRun = async () => {
  await ipcClient.agentsRunMerge(detail.value!.run.id);
  notify(`Đã merge vào ${detail.value!.run.base_branch}`, 'info');
  await reload();
};

const keepRun = async () => {
  await ipcClient.agentsRunKeep(detail.value!.run.id);
  await reload();
};

const discardFromGate = async () => {
  await ipcClient.agentsRunDiscard(detail.value!.run.id);
  await reload();
};

/** MOD-13 for the actions of this screen. */
const confirmBox = reactive({
  visible: false,
  title: '',
  message: '',
  confirmText: '',
  danger: false,
  loading: false,
  action: null as null | (() => Promise<unknown>),
});

const ask = (
  options: { title: string; message: string; confirmText: string; danger?: boolean },
  action: () => Promise<unknown>
) => {
  Object.assign(confirmBox, { ...options, danger: !!options.danger, loading: false, visible: true, action });
};

const confirmAccepted = async () => {
  const action = confirmBox.action;
  if (!action) return;
  confirmBox.loading = true;
  await act(action);
  Object.assign(confirmBox, { loading: false, visible: false, action: null });
};

const discardRun = () => {
  const run = detail.value?.run;
  if (!run) return;
  ask(
    {
      title: 'Bỏ Lượt Chạy',
      message: `Bỏ lượt chạy "${run.title}"? Các worktree, nhánh task và nhánh tích hợp ${run.integration_branch} sẽ bị xóa. Nhánh gốc của bạn không bị đụng tới.`,
      confirmText: 'Bỏ lượt chạy',
      danger: true,
    },
    () => ipcClient.agentsRunDiscard(run.id)
  );
};

const deleteRun = () => {
  const run = detail.value?.run;
  if (!run) return;
  ask(
    {
      title: 'Xóa Lượt Chạy',
      message: `Xóa "${run.title}" khỏi danh sách cùng toàn bộ log của nó? Code đã merge không bị ảnh hưởng.`,
      confirmText: 'Xóa',
      danger: true,
    },
    async () => {
      await ipcClient.agentsRunDelete(run.id);
      selectedRunId.value = null;
      detail.value = null;
    }
  );
};

const stopNow = () => {
  const run = detail.value?.run;
  if (!run) return;
  ask(
    {
      title: 'Dừng Agent Ngay',
      message: 'Dừng ngay agent đang chạy? Lượt chạy chuyển sang tạm dừng; bước đang làm sẽ được làm lại khi bạn bấm Tiếp tục.',
      confirmText: 'Dừng ngay',
      danger: true,
    },
    () => ipcClient.agentsRunStop(run.id)
  );
};

const reassign = (event: Event) => {
  const select = event.target as HTMLSelectElement;
  const cli = select.value as AgentCli;
  select.value = '';
  if (!cli || !selectedTask.value) return;
  void act(() => ipcClient.agentsTaskReassign(selectedTask.value!.id, cli));
};

const approveTask = () => {
  const task = selectedTask.value;
  if (!task) return;
  ask(
    {
      title: 'Bỏ Qua Review',
      message: `Merge task ${task.key} vào nhánh tích hợp như hiện tại, không chờ leader duyệt?`,
      confirmText: 'Merge task',
    },
    () => ipcClient.agentsTaskApprove(task.id)
  );
};

const skipTask = () => {
  const task = selectedTask.value;
  if (!task) return;
  ask(
    {
      title: 'Bỏ Qua Task',
      message: `Bỏ qua task ${task.key}? Code của nó sẽ không được merge; các task phụ thuộc vẫn được chạy.`,
      confirmText: 'Bỏ qua task',
      danger: true,
    },
    () => ipcClient.agentsTaskSkip(task.id)
  );
};

const requestFix = (feedback: string) => {
  fixPromptVisible.value = false;
  if (!feedback || !selectedTask.value) return;
  void act(() => ipcClient.agentsTaskRequestFix(selectedTask.value!.id, feedback));
};

const openLog = async (inv: AgentInvocation) => {
  const key = taskKeyOf(inv);
  const title = `🤖 ${inv.cli} · ${key ?? STEP_LABELS[inv.step] ?? inv.step}`;
  try {
    const { commandId, runEventId } = await ipcClient.agentsOpenLog(inv.id, title);
    await router.push({
      path: '/workspace',
      query: { runEventId, runCommandId: String(commandId), runCommandName: title, shellKind: 'agent' },
    });
  } catch (error) {
    notify(errorText(error));
  }
};

const openTerminalAt = async (cwd: string, title: string) => {
  try {
    const terminal = await ipcClient.openTerminal(cwd);
    await router.push({
      path: '/workspace',
      query: {
        runEventId: terminal.runEventId,
        runCommandId: String(terminal.commandId),
        runCommandName: title,
      },
    });
  } catch (error) {
    notify(errorText(error));
  }
};

const openWorktreeTerminal = () => {
  const task = selectedTask.value;
  if (task?.worktree) void openTerminalAt(task.worktree, `worktree · ${task.key}`);
};

const openIntegrationTerminal = () => {
  if (project.value && detail.value) {
    // The integration worktree is gone after a merge; the repo is always there.
    void openTerminalAt(project.value.path, `repo · ${detail.value.run.integration_branch}`);
  }
};

const openWorktreeFiles = () => {
  const task = selectedTask.value;
  if (!task?.worktree) return;
  void router.push({
    path: '/workspace',
    query: { openFiles: 'local', filesLabel: `worktree · ${task.key}`, filesPath: task.worktree },
  });
};

const showDiff = async (title: string, load: () => Promise<string>) => {
  Object.assign(diff, { visible: true, title, text: '', loading: true, error: null });
  try {
    diff.text = await load();
  } catch (error) {
    diff.error = errorText(error);
  } finally {
    diff.loading = false;
  }
};

const showTaskDiff = () => {
  const task = selectedTask.value;
  if (task) void showDiff(`Diff · ${task.key}`, () => ipcClient.agentsTaskDiff(task.id));
};

const showRunDiff = () => {
  const run = detail.value?.run;
  if (run) void showDiff(`Diff · ${run.integration_branch} → ${run.base_branch}`, () => ipcClient.agentsRunDiff(run.id));
};

// ---- lifecycle ------------------------------------------------------------

let unlisten: (() => void) | undefined;
let ticker: number | undefined;

const closeMenus = () => {
  projectMenuOpen.value = false;
};

// "Mở Agent Studio tại đây" while the view is already open.
watch(
  () => route.query.path,
  (path) => {
    if (typeof path === 'string' && path && path !== project.value?.path) void openProject(path);
  }
);

// Gates open by themselves when a run reaches them while it is shown.
watch(
  () => [detail.value?.run.id, detail.value?.run.stage, detail.value?.driving] as const,
  ([id, stage, driving], [oldId, oldStage]) => {
    if (!id || driving || id !== oldId || stage === oldStage) return;
    if (stage === 'plan_gate') showPlanGateModal.value = true;
    if (stage === 'merge_gate') showMergeGateModal.value = true;
  }
);

onMounted(async () => {
  window.addEventListener('click', closeMenus);
  ticker = window.setInterval(() => (now.value = Math.floor(Date.now() / 1000)), 1000);
  unlisten = await ipcClient.onAgentsChanged(() => scheduleReload());
  try {
    recentProjects.value = await ipcClient.agentsProjectList();
  } catch (error) {
    notify(errorText(error));
  }
  let initial = typeof route.query.path === 'string' ? route.query.path : '';
  if (!initial) {
    try {
      initial = localStorage.getItem(LAST_PROJECT_KEY) ?? '';
    } catch {
      initial = '';
    }
  }
  if (initial) await openProject(initial);
  void detectClis();
});

onUnmounted(() => {
  window.removeEventListener('click', closeMenus);
  window.clearInterval(ticker);
  window.clearTimeout(reloadTimer);
  unlisten?.();
});
</script>

<style scoped>
.agent-studio-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--bg-app-base, #0f1117);
  color: var(--text-primary, #f1f5f9);
  overflow: hidden;
}

/* Header */
.studio-header {
  height: 52px;
  min-height: 52px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  background-color: var(--surface-container-high, #23283a);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.header-icon-box {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background-color: rgba(116, 71, 145, 0.2);
  color: #e4b5ff;
  border: 1px solid rgba(116, 71, 145, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
}

.page-title {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  line-height: 18px;
}

.page-subtitle {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  line-height: 14px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.project-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  font-size: 11px;
  font-family: var(--font-mono, monospace);
}

.branch-clean {
  color: #4edea3;
  background-color: rgba(78, 222, 163, 0.1);
  padding: 1px 5px;
  border-radius: 3px;
}

/* Body */
.studio-body {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

/* Runs Sidebar */
.runs-sidebar {
  width: 270px;
  min-width: 270px;
  background-color: var(--surface-container-low, #141721);
  border-right: 1px solid var(--border-subtle, #242a3e);
  display: flex;
  flex-direction: column;
}

.runs-sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.runs-header-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary, #94a3b8);
  letter-spacing: 0.04em;
}

.runs-count {
  font-size: 10px;
  background-color: var(--surface-container, #1a1e2b);
  padding: 2px 6px;
  border-radius: 4px;
  color: var(--text-muted, #64748b);
}

.runs-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.run-item {
  padding: 8px 10px;
  border-radius: 6px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  transition: all 0.15s;
}

.run-item:hover {
  background-color: var(--surface-container, #1a1e2b);
}

.run-item.active {
  border-color: #744791;
  background-color: rgba(116, 71, 145, 0.15);
}

.run-item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.run-name {
  font-size: 12px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}

.run-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}
.dot-review {
  background-color: #f59e0b;
}
.dot-merged {
  background-color: #10b981;
}
.dot-paused {
  background-color: #64748b;
}

.run-stage-badge {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
}
.badge-review {
  background-color: rgba(245, 158, 11, 0.15);
  color: #fbbf24;
}
.badge-merged {
  background-color: rgba(16, 185, 129, 0.15);
  color: #34d399;
}
.badge-paused {
  background-color: rgba(100, 116, 139, 0.2);
  color: #94a3b8;
}

.run-item-desc {
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.run-item-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10px;
  color: var(--text-muted, #64748b);
  padding-top: 4px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

/* Dashboard Area */
.run-dashboard {
  flex: 1;
  padding: 14px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.card {
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  overflow: hidden;
}

/* Hero card */
.run-hero-card {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.hero-top-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.hero-title-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.hero-icon-wrap {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background-color: rgba(116, 71, 145, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
}

.hero-title {
  font-size: 15px;
  font-weight: 600;
  margin: 0;
}

.hero-meta-badge {
  font-size: 11px;
  padding: 2px 7px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  color: var(--text-secondary, #94a3b8);
}

.hero-timer {
  font-size: 11px;
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.2);
  padding: 2px 7px;
  border-radius: 4px;
}

.hero-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Pipeline Stepper */
.pipeline-stepper {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  font-size: 11px;
  overflow-x: auto;
}

.step-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 9px;
  border-radius: 4px;
  white-space: nowrap;
}

.step-item.clickable {
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
}

.step-item.clickable:hover {
  filter: brightness(1.2);
  border-color: rgba(255, 255, 255, 0.2);
}

.step-item.pass {
  background-color: rgba(16, 185, 129, 0.12);
  color: #34d399;
  border: 1px solid rgba(16, 185, 129, 0.25);
}

.step-item.active {
  background-color: rgba(116, 71, 145, 0.25);
  color: #e4b5ff;
  border: 1px solid #744791;
  font-weight: 700;
}

.step-item.pending {
  background-color: var(--bg-app-base, #0f1117);
  color: var(--text-muted, #64748b);
  border: 1px solid var(--border-subtle, #242a3e);
}

.step-separator {
  color: var(--border-subtle, #242a3e);
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: #e4b5ff;
  animation: pulse 1.2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}

/* Matrix Table Card */
.matrix-card {
  display: flex;
  flex-direction: column;
}

.matrix-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  background-color: var(--surface-container-high, #23283a);
}

.matrix-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-primary, #f1f5f9);
}

.matrix-concurrency {
  font-size: 10px;
  color: var(--text-secondary, #94a3b8);
  background-color: var(--bg-app-base, #0f1117);
  padding: 2px 7px;
  border-radius: 4px;
  border: 1px solid var(--border-subtle, #242a3e);
}

.table-wrap {
  overflow-x: auto;
}

.tasks-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}

.tasks-table th {
  padding: 8px 12px;
  text-align: left;
  color: var(--text-muted, #64748b);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  font-weight: 600;
}

.tasks-table td {
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.task-row {
  cursor: pointer;
  transition: background-color 0.15s;
}

.task-row:hover {
  background-color: var(--surface-container-high, #23283a);
}

.task-row.selected {
  background-color: rgba(116, 71, 145, 0.18);
  border-left: 3px solid #744791;
}

.task-key-tag {
  color: var(--text-primary, #f1f5f9);
}

.role-pill {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  text-transform: uppercase;
}

.role-backend {
  background-color: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
}

.role-frontend {
  background-color: rgba(168, 85, 247, 0.15);
  color: #c084fc;
}

.role-test {
  background-color: rgba(16, 185, 129, 0.15);
  color: #34d399;
}

.cli-tag {
  background-color: var(--bg-app-base, #0f1117);
  padding: 2px 6px;
  border-radius: 3px;
  border: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-primary, #f1f5f9);
}

.status-indicator {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.status-merged {
  color: #34d399;
}
.status-review {
  color: #fbbf24;
}
.status-running {
  color: #e4b5ff;
}
.status-waiting {
  color: var(--text-muted, #64748b);
}

.pulse-dot-sm {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background-color: #e4b5ff;
  animation: pulse 1s infinite;
}

/* Inspector Card */
.inspector-card {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.inspector-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  padding-bottom: 8px;
}

.inspector-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.inspector-title {
  font-size: 13px;
  font-weight: 600;
  margin: 0;
}

.inspector-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 3px;
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-secondary, #94a3b8);
}

.inspector-badge.warning {
  background-color: rgba(245, 158, 11, 0.12);
  color: #fbbf24;
  border-color: rgba(245, 158, 11, 0.3);
}

.inspector-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
}

.review-alert-box {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
  border-radius: 6px;
  font-size: 11px;
}

.alert-icon {
  color: #ef4444;
  flex-shrink: 0;
}

.alert-verdict {
  color: #ef4444;
}

.alert-file {
  color: var(--text-muted, #64748b);
  margin-left: 4px;
}

.guard-strip {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.guard-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 4px;
  font-size: 11px;
}

.guard-item.pass {
  background-color: rgba(16, 185, 129, 0.08);
  border: 1px solid rgba(16, 185, 129, 0.2);
  color: #34d399;
}

.guard-item.warning {
  background-color: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.2);
  color: #fbbf24;
}

/* Diff Snippet */
.diff-preview {
  background-color: var(--bg-terminal, #0b0d13);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  overflow: hidden;
  font-size: 11px;
}

.diff-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 10px;
  background-color: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  color: var(--text-muted, #64748b);
  font-size: 10px;
}

.diff-code {
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.diff-code .line {
  line-height: 16px;
}

.diff-code .line.muted {
  color: var(--text-muted, #64748b);
}

.diff-code .line.highlight {
  background-color: rgba(245, 158, 11, 0.12);
  color: #fef08a;
  padding: 1px 4px;
  border-left: 2px solid #f59e0b;
}

/* Inspector Footer */
.inspector-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 6px;
}

.footer-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* PTY Monitor */
.pty-monitor-card {
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.pty-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
}

.pty-title {
  display: flex;
  align-items: center;
  gap: 6px;
}

.stream-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: #10b981;
  animation: pulse 1s infinite;
}

.pty-stats {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
}

.pty-viewport {
  background-color: var(--bg-terminal, #0b0d13);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  padding: 8px 10px;
  height: 90px;
  overflow-y: auto;
  font-size: 11px;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

/* Studio Footer */
.studio-footer {
  height: 28px;
  min-height: 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  background-color: var(--surface-container-high, #23283a);
  border-top: 1px solid var(--border-subtle, #242a3e);
  font-size: 10px;
  color: var(--text-secondary, #94a3b8);
}

.footer-metrics,
.footer-telemetry {
  display: flex;
  align-items: center;
  gap: 8px;
}

.metric-separator {
  color: var(--text-muted, #64748b);
}

/* Generic Buttons */
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

.btn-xs {
  font-size: 10px;
  padding: 3px 6px;
}

.btn-primary {
  background-color: #744791;
  color: #ffffff;
}

.btn-primary:hover {
  background-color: #8956aa;
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

.text-status-running {
  color: #34d399;
}

.text-warning {
  color: #fbbf24;
}

.text-muted {
  color: var(--text-muted, #64748b);
}

/* Live data additions */
.branch-dirty {
  color: #fbbf24;
  background-color: rgba(245, 158, 11, 0.1);
  padding: 1px 5px;
  border-radius: 3px;
}

.project-menu-wrap {
  position: relative;
}

.project-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 50;
  min-width: 260px;
  padding: 4px;
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}

.project-menu-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  font-size: 12px;
  text-align: left;
  color: var(--text-primary, #f1f5f9);
  background: none;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.project-menu-item:hover {
  background-color: rgba(116, 71, 145, 0.2);
}

.project-menu-item.current {
  color: #e4b5ff;
}

.project-menu-divider {
  height: 1px;
  margin: 4px 0;
  background-color: var(--border-subtle, #242a3e);
}

.studio-notice {
  padding: 6px 16px;
  font-size: 12px;
  cursor: pointer;
}

.notice-error {
  color: #fecaca;
  background-color: rgba(239, 68, 68, 0.15);
}

.notice-info {
  color: #a7f3d0;
  background-color: rgba(16, 185, 129, 0.15);
}

.studio-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  text-align: center;
  color: var(--text-secondary, #94a3b8);
}

.studio-empty h2 {
  margin: 0;
  font-size: 16px;
  color: var(--text-primary, #f1f5f9);
}

.studio-empty p {
  max-width: 460px;
  margin: 0;
  font-size: 12px;
}

.studio-empty.small {
  height: 100%;
}

.empty-open-row {
  display: flex;
  gap: 6px;
  width: min(520px, 100%);
}

.empty-input {
  flex: 1;
  padding: 6px 10px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--surface-container-low, #141721);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.empty-recent {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 12px;
}

.runs-empty {
  padding: 12px 6px;
  font-size: 12px;
  line-height: 1.5;
}

.dot-running {
  background-color: #a78bfa;
  box-shadow: 0 0 6px #a78bfa;
}

.dot-failed {
  background-color: #ef4444;
}

.badge-running {
  background-color: rgba(167, 139, 250, 0.15);
  color: #c4b5fd;
}

.badge-failed {
  background-color: rgba(239, 68, 68, 0.15);
  color: #fca5a5;
}

.run-note {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
  padding: 6px 10px;
  font-size: 12px;
  border-radius: 4px;
  color: var(--text-secondary, #94a3b8);
  background-color: var(--bg-app-base, #0f1117);
}

.note-review {
  color: #fbbf24;
}

.note-failed {
  color: #fca5a5;
}

.status-failed {
  color: #f87171;
}

.task-repos {
  margin-left: 6px;
  font-size: 10px;
  font-weight: 400;
  color: var(--text-muted, #64748b);
}

.task-description {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--text-secondary, #94a3b8);
}

.review-alert-box.approved {
  border-color: rgba(16, 185, 129, 0.4);
  background-color: rgba(16, 185, 129, 0.08);
}

.guard-item.fail {
  color: #f87171;
  border-color: rgba(239, 68, 68, 0.4);
}

.reassign-select {
  appearance: none;
}

.stream-dot.idle {
  background-color: #64748b;
  animation: none;
}

.activity-line {
  display: block;
  width: 100%;
  text-align: left;
  background: none;
  border: none;
  cursor: pointer;
  font: inherit;
}

.activity-line:hover {
  background-color: rgba(116, 71, 145, 0.12);
}

.btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
