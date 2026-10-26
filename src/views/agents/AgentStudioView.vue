<template>
  <div class="agent-studio-view">
    <!-- Top Action Header (Toolbar) -->
    <header class="studio-header">
      <div class="header-left">
        <div class="header-icon-box">
          <Bot :size="16" class="header-icon text-primary" />
        </div>
        <h1 class="page-title">Agent Studio</h1>

        <!-- Project Context Pill -->
        <div v-if="project" class="project-pill font-mono" :title="projectTooltip">
          <FolderGit2 :size="13" class="text-primary" />
          <span class="project-path">{{ shortPath(project.path) }}</span>
          <template v-if="multiRepo">
            <span :class="projectProblems ? 'branch-dirty' : 'branch-clean'">
              ({{ project.status.length }} repo{{ projectProblems ? ` · ${projectProblems} cần chú ý` : ' ✓ sạch' }})
            </span>
          </template>
          <template v-else-if="project.status[0]">
            <span v-if="!project.status[0].is_repo" class="branch-dirty">(không còn là git repo)</span>
            <span v-else-if="project.status[0].dirty > 0" class="branch-dirty">
              ({{ project.status[0].branch || 'detached' }} · {{ project.status[0].dirty }} thay đổi)
            </span>
            <span v-else class="branch-clean">({{ project.status[0].branch || 'detached' }} ✓ sạch)</span>
          </template>
        </div>
      </div>

      <div class="header-right">
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
                <span class="run-title-text">{{ run.title }}</span>
              </span>
              <span class="run-stage-badge font-mono" :class="'badge-' + runTone(run)">
                {{ runBadge(run) }}
              </span>
            </div>

            <p class="run-item-desc font-mono" :title="run.request">
              {{ run.request }}
            </p>

            <div class="run-item-footer font-mono">
              <span>{{ run.leader_cli }} • {{ duration(run) }}</span>
              <span class="run-status-text" :class="'text-' + runTone(run)">{{ runStatusLabel(run) }}</span>
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
                <h2 class="hero-title" :title="detail.run.title">{{ detail.run.title }}</h2>
                <button
                  type="button"
                  class="btn-edit-run"
                  title="Sửa tiêu đề & mô tả"
                  @click="openEditRunModal"
                >
                  <Pencil :size="12" />
                </button>
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
                  v-if="detail.run.status !== 'discarded'"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  title="Timeout, ngân sách, model của leader và các vai (áp dụng từ bước tiếp theo)"
                  @click="showModels = true"
                >
                  <Cpu :size="13" />
                  <span>Thiết lập</span>
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
                  v-if="
                    !detail.driving &&
                    (detail.run.stage === 'merge_gate' ||
                      (detail.run.stage === 'done' && (detail.run.status === 'merged' || detail.run.status === 'kept')))
                  "
                  type="button"
                  class="btn btn-secondary btn-sm"
                  title="Gắn PR bạn đã tạo bằng tay; lần làm tiếp sẽ làm trên nhánh của PR"
                  @click="showAttachPr = true"
                >
                  <GitFork :size="13" />
                  <span>{{ detail.run.pull_requests.length ? 'Đổi PR' : 'Gắn PR' }}</span>
                </button>
                <button
                  v-if="detail.run.stage === 'done' && (detail.run.status === 'merged' || detail.run.status === 'kept')"
                  type="button"
                  class="btn btn-primary btn-sm"
                  title="Yêu cầu tiếp cho lượt chạy này (ví dụ comment review trên PR), giữ ngữ cảnh của leader"
                  @click="showFollowUp = true"
                >
                  <MessageSquarePlus :size="13" />
                  <span>Làm tiếp</span>
                </button>
                <button
                  v-if="detail.run.stage === 'done' && detail.worktrees && !detail.driving"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  title="Xóa worktree (cả dependency đã cài) của lượt chạy để giải phóng ổ đĩa; nhánh giữ nguyên, lần làm tiếp / push sẽ tạo và cài lại"
                  @click="cleanWorktrees"
                >
                  <HardDrive :size="13" />
                  <span>Dọn worktree</span>
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

            <!-- Run Description / Prompt -->
            <div v-if="detail.run.request" class="hero-desc-row">
              <div class="hero-desc-header">
                <div class="hero-desc-label">
                  <FileText :size="12" />
                  <span>Mô tả yêu cầu</span>
                </div>
                <div class="hero-desc-actions">
                  <button
                    type="button"
                    class="btn-edit-desc"
                    title="Sửa tiêu đề & mô tả"
                    @click="openEditRunModal"
                  >
                    <Pencil :size="11" />
                    <span>Sửa</span>
                  </button>
                  <button
                    v-if="detail.run.request.length > 140 || detail.run.request.includes('\n')"
                    type="button"
                    class="btn-toggle-desc"
                    @click="isDescExpanded = !isDescExpanded"
                  >
                    <span>{{ isDescExpanded ? 'Thu gọn' : 'Xem toàn bộ' }}</span>
                    <ChevronUp v-if="isDescExpanded" :size="12" />
                    <ChevronDown v-else :size="12" />
                  </button>
                </div>
              </div>
              <p class="hero-desc-text font-mono" :class="{ 'is-collapsed': !isDescExpanded }">
                {{ detail.run.request }}
              </p>
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
              <span v-if="detail.resume_at" class="text-muted">— tự chạy tiếp lúc {{ formatClock(detail.resume_at) }}</span>
            </div>
            <div v-if="detail.run.pull_requests.length" class="pr-links">
              <GitFork :size="13" />
              <a
                v-for="pr in detail.run.pull_requests"
                :key="pr.repo + pr.branch"
                href="#"
                class="pr-link font-mono"
                :title="`${pr.branch} → ${pr.base}`"
                @click.prevent="ipcClient.openUrl(pr.url)"
              >
                {{ pr.url.replace(/^https:\/\/github\.com\//, '') || pr.branch }}
              </a>
              <button
                v-if="detail.run.stage === 'done' && !detail.driving && unpushed !== 0"
                type="button"
                class="btn btn-primary btn-xs"
                :disabled="pushing || verifying"
                title="Push các commit mới của lượt chạy lên nhánh PR (fast-forward, hook pre-push vẫn chạy, không tạo PR mới)"
                @click="pushToPrs"
              >
                <GitFork :size="12" :class="{ spin: pushing }" />
                <span>
                  {{
                    pushing
                      ? 'Đang push…'
                      : `${needsRebase ? 'Rebase & push' : 'Push'} lên PR${unpushed ? ` (${unpushed} commit)` : ''}`
                  }}
                </span>
              </button>
              <button
                v-if="(detail.run.stage === 'done' || detail.run.stage === 'merge_gate') && !detail.driving"
                type="button"
                class="btn btn-secondary btn-xs"
                :disabled="pushing || verifying"
                title="Giao cho một agent rebase, gỡ xung đột, sửa lỗi kiểm tra và push lên PR"
                @click="showAiPush = true"
              >
                <Sparkles :size="12" />
                <span>Push bằng AI</span>
              </button>
              <button
                v-if="detail.run.stage === 'done' && !detail.driving"
                type="button"
                class="btn btn-secondary btn-xs"
                :disabled="verifying"
                title="Leader review cả PR, kiểm tra PR có đúng của lượt chạy, đọc comment review chưa xử lý; còn comment thì tự làm tiếp"
                @click="verifyPrs"
              >
                <ShieldCheck :size="12" :class="{ spin: verifying }" />
                <span>{{ verifying ? 'Đang verify…' : 'Verify PR' }}</span>
              </button>
            </div>

            <div v-if="detail.verification" class="verify-box">
              <div class="verify-head">
                <ShieldCheck :size="13" />
                <strong>Verify PR</strong>
                <span class="text-muted">lúc {{ formatClock(Number(detail.verification.checked_at)) }}</span>
                <span v-if="detail.verification.followed_up" class="verify-tag ok">đã tạo vòng làm tiếp từ các điểm còn mở</span>
              </div>
              <div v-if="detail.verification.note" class="verify-warn">{{ detail.verification.note }}</div>
              <div v-for="pr in detail.verification.prs" :key="pr.url" class="verify-pr">
                <div class="verify-line">
                  <a href="#" class="pr-link font-mono" @click.prevent="ipcClient.openUrl(pr.url)">
                    {{ pr.url.replace(/^https:\/\/github\.com\//, '') }}
                  </a>
                  <span v-if="pr.error" class="verify-warn-text">{{ pr.error }}</span>
                  <template v-else>
                    <span class="verify-tag">{{ pr.state || '?' }}{{ pr.review_decision ? ` · ${pr.review_decision}` : '' }}</span>
                    <span class="verify-tag" :class="pr.mismatch ? 'bad' : 'ok'">
                      {{ pr.matched }}/{{ pr.run_files }} tệp của lượt chạy có trong PR ({{ pr.pr_files }} tệp)
                    </span>
                  </template>
                </div>
                <div v-if="pr.mismatch && pr.missing.length" class="verify-detail font-mono">
                  Thiếu: {{ pr.missing.join(', ') }}
                </div>
                <div v-for="(comment, index) in pr.open_comments" :key="index" class="verify-comment">
                  <span class="font-mono">{{ comment.path ? `${comment.path}${comment.line ? `:${comment.line}` : ''}` : 'review' }}</span>
                  — {{ comment.body.split('\n')[0] }}
                  <a v-if="comment.url" href="#" class="pr-link" @click.prevent="ipcClient.openUrl(comment.url!)">mở</a>
                </div>
                <div v-if="!pr.error && !pr.open_comments.length" class="verify-detail">Không còn comment chưa xử lý.</div>
              </div>
              <div v-if="detail.verification.review" class="verify-review" :class="{ approved: detail.verification.review.verdict === 'approved' }">
                <strong>Leader: {{ detail.verification.review.verdict === 'approved' ? '✓ Đạt' : '✗ Cần sửa' }}</strong>
                — {{ detail.verification.review.summary }}
                <div v-for="(comment, index) in detail.verification.review.comments" :key="index" class="verify-detail">
                  [{{ comment.severity }}] {{ comment.file }}{{ comment.line ? `:${comment.line}` : '' }} — {{ comment.issue }}
                </div>
              </div>
            </div>
          </div>

          <!-- Task Allocation Matrix Table -->
          <div class="card matrix-card">
            <div class="matrix-header">
              <div class="matrix-title">
                <Layers :size="15" class="text-primary" />
                <span>MÃ TRẬN PHÂN CHIA VÀ THỰC THI NHIỆM VỤ</span>
              </div>
              <div class="matrix-meta font-mono">
                <span class="matrix-concurrency">
                  Song song tối đa: {{ detail.run.limits.concurrency }} worker
                </span>
                <button
                  v-if="failedTasks.length > 0"
                  type="button"
                  class="btn btn-danger-ghost btn-xs"
                  :disabled="!canEditTasks"
                  :title="canEditTasks ? 'Giao các task lỗi cho một agent khác rồi chạy tiếp' : editHint"
                  @click="showBulkReassign = true"
                >
                  <Users :size="12" />
                  <span>Giao lại {{ failedTasks.length }} task lỗi</span>
                </button>
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
                      <span class="task-key-tag">{{ task.kind === 'human' ? '👤 ' : '' }}{{ task.key }}</span>
                      <span v-if="task.batch > 0" class="task-batch" :title="detail.run.followups[task.batch - 1]?.request">tiếp #{{ task.batch }}</span>
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
                        <span
                          v-if="(task.status === 'running' || task.status === 'fixing') && !isInterrupted(task)"
                          class="pulse-dot-sm"
                        ></span>
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
                <span class="inspector-badge font-mono">Agent: {{ selectedTask.cli }}{{ taskModel(selectedTask) ? ` · ${taskModel(selectedTask)}` : '' }}</span>
                <span v-if="selectedTask.worktree" class="inspector-badge font-mono" :title="selectedTask.worktree">
                  Worktree: {{ selectedTask.worktree.split('/').slice(-2).join('/') }}
                </span>
                <span class="inspector-badge warning font-mono">{{ roundText(selectedTask) }}</span>
              </div>

              <div class="inspector-toolbar">
                <button
                  type="button"
                  class="btn btn-secondary btn-xs"
                  :disabled="!latestTaskLog(selectedTask.id)"
                  :title="
                    selectedTask.status === 'review'
                      ? 'Terminal của reviewer'
                      : 'Terminal của bước mới nhất (worker, chuẩn bị hoặc kiểm tra)'
                  "
                  @click="openTaskLog(selectedTask.id)"
                >
                  <Terminal :size="12" />
                  <span>{{ selectedTask.status === 'checking' ? 'Xem kiểm tra' : selectedTask.status === 'review' ? 'Xem reviewer' : 'Xem log trực tiếp' }}</span>
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
                  <span>Terminal tại worktree</span>
                </button>
              </div>
            </div>

            <p class="task-description">{{ selectedTask.title }}<template v-if="selectedTask.description"> — {{ selectedTask.description }}</template></p>

            <HumanTestPanel v-if="selectedTask.kind === 'human'" :task="selectedTask" @changed="reload" />

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

            <div v-if="selectedTask.handoff" class="handoff-box">
              <strong>Ghi chú bàn giao</strong> <span class="text-muted">(đưa cho các task sau)</span>
              <pre class="handoff-text">{{ selectedTask.handoff }}</pre>
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
                <span>Phạm vi: ⚠ Sửa ngoài scope: {{ selectedTask.out_of_scope.join(', ') }}</span>
              </div>
            </div>

            <!-- Diff Snippet Preview -->
            <div v-if="diffSnippet" class="diff-preview font-mono">
              <div class="diff-header">
                <div class="diff-header-left">
                  <span class="diff-file-icon">📄</span>
                  <span class="diff-file-name">{{ diffSnippetFile }}</span>
                </div>
                <div class="diff-header-right">
                  <span class="diff-status-badge">Modified</span>
                  <button type="button" class="btn-diff-full" @click="showTaskDiff">Xem toàn bộ diff →</button>
                </div>
              </div>
              <div class="diff-code custom-scrollbar">
                <div
                  v-for="(line, idx) in diffSnippet.split('\n')"
                  :key="idx"
                  class="line"
                  :class="{
                    'line-add': line.startsWith('+'),
                    'line-del': line.startsWith('-'),
                    'line-hunk': line.startsWith('@@'),
                    'muted': !line.startsWith('+') && !line.startsWith('-') && !line.startsWith('@@')
                  }"
                >
                  {{ line }}
                </div>
              </div>
            </div>

            <!-- Workflow Controls -->
            <div class="inspector-footer">
              <div class="footer-left">
                <button
                  v-if="selectedTask.kind !== 'human'"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canReassign"
                  :title="canEditTasks ? 'Giao lại task này cho một agent khác' : editHint"
                  @click="showReassignModal = true"
                >
                  <Users :size="13" />
                  <span>Giao lại cho agent khác</span>
                  <ChevronDown :size="12" />
                </button>

                <button
                  v-if="selectedTask.kind !== 'human'"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canEditTasks || !selectedTask.worktree"
                  :title="canEditTasks ? 'Merge task như hiện tại, không chờ leader duyệt' : editHint"
                  @click="approveTask"
                >
                  <span>Bỏ qua review</span>
                </button>

                <button
                  v-if="selectedTask.kind !== 'human'"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canEditTasks || !selectedTask.worktree || ['merged', 'skipped'].includes(selectedTask.status)"
                  :title="
                    canEditTasks
                      ? 'Leader review lại code hiện tại của task (gồm phần bạn sửa tay), sau khi chạy lại lệnh kiểm tra'
                      : editHint
                  "
                  @click="rereviewTask"
                >
                  <SearchCode :size="12" />
                  <span>Review lại</span>
                </button>

                <button
                  v-if="['failed', 'conflict', 'needs_human'].includes(selectedTask.status) || isInterrupted(selectedTask)"
                  type="button"
                  class="btn btn-secondary btn-sm"
                  :disabled="!canEditTasks"
                  :title="
                    !canEditTasks
                      ? editHint
                      : isInterrupted(selectedTask)
                        ? 'Bước đang làm bị ngắt (tạm dừng hoặc ứng dụng đóng); giao lại cho agent, nối tiếp phiên bị ngắt nếu còn'
                        : undefined
                  "
                  @click="act(() => ipcClient.agentsTaskRetry(selectedTask!.id))"
                >
                  <RotateCcw :size="12" />
                  <span>Thử lại</span>
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
                @click="showFeedbackModal = true"
              >
                <Zap :size="13" />
                <span>Yêu cầu agent sửa lại ngay</span>
              </button>
            </div>
          </div>

          <!-- Agent Activity -->
          <div class="card pty-monitor-card" :class="{ collapsed: activityCardCollapsed }">
            <div class="pty-header font-mono">
              <div
                class="pty-title"
                :title="activityCardCollapsed ? 'Bấm để mở rộng Hoạt động agent' : 'Bấm để thu gọn Hoạt động agent'"
                @click="activityCardCollapsed = !activityCardCollapsed"
              >
                <button type="button" class="card-collapse-btn" :title="activityCardCollapsed ? 'Mở rộng' : 'Thu gọn'">
                  <ChevronRight v-if="activityCardCollapsed" :size="13" />
                  <ChevronDown v-else :size="13" />
                </button>
                <span class="stream-dot" :class="{ idle: !runningInvocation }"></span>
                <span class="font-bold text-text-primary">Hoạt động agent</span>
              </div>
              <div class="pty-stats text-muted">
                <span>{{ detail.invocations.length }} lượt gọi</span>
                <template v-if="runningInvocation">
                  <span>•</span>
                  <span class="text-status-running">{{ runningInvocation.cli }} đang chạy…</span>
                </template>
                <div v-show="!activityCardCollapsed" class="activity-toggle">
                  <template v-if="activityView === 'terminals'">
                    <button
                      type="button"
                      :class="{ active: lanesLayout === 'grid' }"
                      title="Bố cục dạng lưới (Grid)"
                      @click="setLanesLayout('grid')"
                    >
                      <LayoutGrid :size="12" /> Lưới
                    </button>
                    <button
                      type="button"
                      :class="{ active: lanesLayout === 'list' }"
                      title="Bố cục dạng danh sách (List)"
                      @click="setLanesLayout('list')"
                    >
                      <Rows3 :size="12" /> Danh sách
                    </button>
                    <button
                      type="button"
                      class="collapse-all-btn"
                      :title="allLanesCollapsed ? 'Mở rộng tất cả agent' : 'Thu gọn tất cả agent'"
                      @click="toggleAllLanesCollapse"
                    >
                      <ChevronsDownUp v-if="!allLanesCollapsed" :size="12" />
                      <ChevronsUpDown v-else :size="12" />
                      <span>{{ allLanesCollapsed ? 'Mở hết' : 'Thu gọn' }}</span>
                    </button>
                  </template>
                  <button
                    type="button"
                    :class="{ active: activityView === 'list' }"
                    title="Mọi lượt gọi theo thời gian"
                    @click="setActivityView(activityView === 'list' ? 'terminals' : 'list')"
                  >
                    <ScrollText :size="12" /> {{ activityView === 'list' ? 'Terminal' : 'Nhật ký' }}
                  </button>
                </div>
              </div>
            </div>

            <template v-if="!activityCardCollapsed">
              <AgentLanes
                v-if="activityView === 'terminals'"
                ref="agentLanesRef"
                :invocations="detail.invocations"
                :side-logs="detail.side_logs"
                :tasks="detail.tasks"
                :leader-cli="detail.run.leader_cli"
                :layout="lanesLayout"
                @expand="openLaneItem"
                @collapse-change="allLanesCollapsed = $event"
              />
              <div v-else class="pty-viewport font-mono custom-scrollbar">
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
            </template>
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
            {{ formatTokens(detail.usage.input_tokens + detail.usage.output_tokens) }} token{{ ` (~$${detail.usage.cost_usd.toFixed(2)} / $${detail.run.limits.max_cost_usd})` }}
          </strong>
        </span>
        <span class="metric-separator">•</span>
        <span class="metric-pill">Agent đang chạy: <strong>{{ runningInvocation ? 1 : 0 }}</strong></span>
        <span class="metric-separator">•</span>
        <span class="metric-pill">Đã gọi: <strong>{{ detail.usage.invocations }}/{{ detail.run.limits.max_invocations }} lượt</strong></span>
        <span class="metric-separator">•</span>
        <template v-for="item in quotaPills" :key="item.cli">
          <span class="metric-separator">•</span>
          <span class="metric-pill" :class="{ 'quota-hot': item.hot }" :title="item.title">
            Hạn mức {{ item.cli }}: <strong>{{ item.text }}</strong>
          </span>
        </template>
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

    <AiPushModal
      :visible="showAiPush"
      :run="detail?.run ?? null"
      :clis="clis"
      @started="notify('Agent đang push lên PR — theo dõi ở ô Leader trong lưới terminal', 'info')"
      @close="showAiPush = false"
    />

    <AttachPrModal
      :visible="showAttachPr"
      :run="detail?.run ?? null"
      @attached="(run) => notify(`Đã gắn PR: ${run.pull_requests.map((pr) => pr.url).join(' · ')}`, 'info')"
      @close="showAttachPr = false"
    />

    <OpenPrModal
      :visible="showOpenPr"
      :run-id="detail?.run.id ?? null"
      :run-title="detail?.run.title ?? ''"
      @opened="(urls: string[]) => notify(`Đã mở PR: ${urls.join(' · ')}`, 'info')"
      @close="showOpenPr = false"
    />

    <BulkReassignModal
      :visible="showBulkReassign"
      :tasks="failedTasks"
      :clis="clis"
      :submit="bulkReassign"
      @close="showBulkReassign = false"
    />

    <RunModelsModal
      :visible="showModels"
      :run="detail?.run ?? null"
      :clis="clis"
      :driving="!!detail?.driving"
      :save="saveModels"
      @close="showModels = false"
    />

    <FollowUpModal
      :visible="showFollowUp"
      :subtitle="detail?.run.title ?? ''"
      :merged="detail?.run.status === 'merged'"
      :pull-requests="detail?.run.pull_requests ?? []"
      :submit="followUp"
      @close="showFollowUp = false"
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
      @open-pr="
        showMergeGateModal = false;
        showOpenPr = true;
      "
      @attach-pr="
        showMergeGateModal = false;
        showAttachPr = true;
      "
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

    <ReassignTaskModal
      :visible="showReassignModal"
      :task="selectedTask"
      :clis="clis"
      :default-timeout="detail?.run.limits.timeout_minutes"
      :submit="reassignSingleTask"
      @close="showReassignModal = false"
    />

    <TaskFeedbackModal
      :visible="showFeedbackModal"
      :task="selectedTask"
      :submit="requestFix"
      @close="showFeedbackModal = false"
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

    <!-- Edit Run Modal -->
    <div v-if="showEditRunModal" class="modal-backdrop" @keydown.esc="showEditRunModal = false">
      <div class="edit-run-modal" role="dialog" aria-modal="true">
        <div class="modal-accent-bar"></div>
        <div class="modal-header">
          <div class="header-left">
            <div class="icon-badge">
              <Pencil :size="16" />
            </div>
            <div class="header-titles">
              <h3 class="modal-title">Sửa Thông Tin Lượt Chạy</h3>
              <span class="modal-subtitle">Cập nhật tiêu đề và mô tả yêu cầu cho lượt chạy</span>
            </div>
          </div>
          <button class="btn btn-ghost btn-icon" title="Đóng (Esc)" @click="showEditRunModal = false">
            <X :size="16" />
          </button>
        </div>

        <div class="modal-body custom-scrollbar">
          <div class="form-group">
            <label class="form-label required">Tiêu đề lượt chạy (Title)</label>
            <input
              v-model="editRunForm.title"
              type="text"
              class="form-input"
              placeholder="Nhập tiêu đề lượt chạy..."
            />
          </div>

          <div class="form-group">
            <label class="form-label required">Mô tả yêu cầu (Description / Prompt)</label>
            <textarea
              v-model="editRunForm.request"
              rows="5"
              class="form-textarea font-mono"
              placeholder="Nhập mô tả yêu cầu chi tiết..."
            ></textarea>
          </div>
        </div>

        <div class="modal-footer">
          <button type="button" class="btn btn-secondary btn-sm" @click="showEditRunModal = false">
            Hủy
          </button>
          <button
            type="button"
            class="btn btn-primary btn-sm"
            :disabled="savingEditRun || !editRunForm.title.trim() || !editRunForm.request.trim()"
            @click="saveEditRun"
          >
            {{ savingEditRun ? 'Đang lưu...' : 'Lưu thay đổi' }}
          </button>
        </div>
      </div>
    </div>
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
  LayoutGrid,
  List,
  MessageSquarePlus,
  Cpu,
  ShieldCheck,
  Sparkles,
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
  ChevronDown,
  ChevronUp,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  Rows3,
  ScrollText,
  HardDrive,
  FileText,
  Pencil,
  X,
} from 'lucide-vue-next';
import NewAgentRunModal from '@/components/dialogs/NewAgentRunModal.vue';
import PlanGateModal from '@/components/dialogs/PlanGateModal.vue';
import MergeGateModal from '@/components/dialogs/MergeGateModal.vue';
import AgentTeamModal from '@/components/dialogs/AgentTeamModal.vue';
import AgentDiffModal from '@/components/dialogs/AgentDiffModal.vue';
import ReassignTaskModal from '@/components/dialogs/ReassignTaskModal.vue';
import TaskFeedbackModal from '@/components/dialogs/TaskFeedbackModal.vue';
import AgentLanes from '@/views/agents/AgentLanes.vue';
import { useAgentModels } from '@/composables/useAgentModels';
import FollowUpModal from '@/components/dialogs/FollowUpModal.vue';
import RunModelsModal from '@/components/dialogs/RunModelsModal.vue';
import BulkReassignModal from '@/components/dialogs/BulkReassignModal.vue';
import OpenPrModal from '@/components/dialogs/OpenPrModal.vue';
import AttachPrModal from '@/components/dialogs/AttachPrModal.vue';
import AiPushModal from '@/components/dialogs/AiPushModal.vue';
import HumanTestPanel from '@/views/agents/HumanTestPanel.vue';
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
  RunLimits,
  PendingPush,
  RoleCli,
  CheckProposal,
  TaskDraft,
} from '@/types/agents';
import {
  AGENT_CLIS,
  MAX_TIMEOUT_MINUTES,
  profileModelHint,
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
  verify: 'verify PR',
  push: 'push lên PR (AI)',
  prepare: 'chuẩn bị test',
  triage: 'xác nhận lỗi',
  chat: 'hỏi đáp',
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
const isDescExpanded = ref(false);

const showNewRunModal = ref(false);
const showPlanGateModal = ref(false);
const showMergeGateModal = ref(false);
const showTeamModal = ref(false);
const showReassignModal = ref(false);
const showFeedbackModal = ref(false);
const projectMenuOpen = ref(false);
const folderPromptVisible = ref(false);
const diff = reactive({ visible: false, title: '', text: '', loading: false, error: null as string | null });

const showEditRunModal = ref(false);
const savingEditRun = ref(false);
const editRunForm = reactive({
  title: '',
  request: '',
});

const openEditRunModal = () => {
  if (!detail.value) return;
  editRunForm.title = detail.value.run.title;
  editRunForm.request = detail.value.run.request;
  showEditRunModal.value = true;
};

const saveEditRun = async () => {
  if (!detail.value || savingEditRun.value) return;
  savingEditRun.value = true;
  try {
    const updated = await ipcClient.agentsRunUpdate(
      detail.value.run.id,
      editRunForm.title.trim() || null,
      editRunForm.request.trim() || null
    );
    detail.value.run.title = updated.title;
    detail.value.run.request = updated.request;
    const found = runs.value.find((r) => r.id === updated.id);
    if (found) {
      found.title = updated.title;
      found.request = updated.request;
    }
    showEditRunModal.value = false;
    notify('Đã cập nhật thông tin lượt chạy', 'info');
  } catch (err) {
    notify(errorText(err));
  } finally {
    savingEditRun.value = false;
  }
};

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

const runStatusLabel = (run: RunSummary) => {
  if (run.status === 'running') return 'Đang chạy';
  if (run.status === 'merged') return 'Đã merge';
  if (run.status === 'paused') return 'Tạm dừng';
  if (run.status === 'failed') return 'Lỗi';
  if (run.status === 'kept') return 'Giữ nhánh';
  if (run.status === 'discarded') return 'Đã bỏ';
  return RUN_STATUS_LABELS[run.status] || run.status;
};

/** How agent activity is shown; remembered on this machine. */
const ACTIVITY_VIEW_KEY = 'agent-studio.activity-view';
const readActivityView = (): 'terminals' | 'list' => {
  try {
    return localStorage.getItem(ACTIVITY_VIEW_KEY) === 'list' ? 'list' : 'terminals';
  } catch {
    return 'terminals';
  }
};
const activityView = ref(readActivityView());
const setActivityView = (view: 'terminals' | 'list') => {
  activityView.value = view;
  try {
    localStorage.setItem(ACTIVITY_VIEW_KEY, view);
  } catch {
    // Not remembered; the view still switches.
  }
};

/** Terminal lanes layout mode (grid or list); remembered on this machine. */
const LANES_LAYOUT_KEY = 'agent-studio.lanes-layout';
const readLanesLayout = (): 'grid' | 'list' => {
  try {
    return localStorage.getItem(LANES_LAYOUT_KEY) === 'list' ? 'list' : 'grid';
  } catch {
    return 'grid';
  }
};
const lanesLayout = ref<'grid' | 'list'>(readLanesLayout());
const setLanesLayout = (layout: 'grid' | 'list') => {
  lanesLayout.value = layout;
  try {
    localStorage.setItem(LANES_LAYOUT_KEY, layout);
  } catch {
    // ignore
  }
};

const activityCardCollapsed = ref(false);
const agentLanesRef = ref<InstanceType<typeof AgentLanes> | null>(null);
const allLanesCollapsed = ref(false);

const toggleAllLanesCollapse = () => {
  allLanesCollapsed.value = !allLanesCollapsed.value;
  agentLanesRef.value?.setAllCollapsed(allLanesCollapsed.value);
};

const formatTokens = (n: number) => (n >= 10000 ? `${Math.round(n / 1000)}k` : String(n));

/** Unix seconds as a local time, with the day when it is not today. */
const formatClock = (unix: number) => {
  const date = new Date(unix * 1000);
  const time = date.toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' });
  return date.toDateString() === new Date().toDateString()
    ? time
    : `${time} ${date.toLocaleDateString('vi-VN', { day: '2-digit', month: '2-digit' })}`;
};

/** Account usage of the run's CLIs, as they last reported it. */
const quotaPills = computed(() => {
  const run = detail.value?.run;
  if (!run) return [];
  const used = new Set<AgentCli>([run.leader_cli, ...run.roles.map((role) => role.cli)]);
  const pills = [];
  for (const cli of AGENT_CLIS) {
    const quota = detail.value?.quotas[cli];
    if (!quota || !used.has(cli)) continue;
    const parts: string[] = [];
    const titles: string[] = [];
    let worst = 0;
    for (const [name, window] of [
      ['5h', quota.five_hour],
      ['7 ngày', quota.seven_day],
    ] as const) {
      if (!window) continue;
      worst = Math.max(worst, window.utilization);
      parts.push(`${name} ${Math.round(window.utilization * 100)}%`);
      titles.push(`${name}: reset lúc ${formatClock(window.resets_at)}`);
    }
    const threshold = run.limits.quota_pause_percent / 100;
    pills.push({
      cli,
      text: quota.rejected ? 'đã hết' : parts.join(' · '),
      title: titles.join('\n'),
      hot: quota.rejected || (threshold > 0 && worst >= threshold - 0.1),
    });
  }
  return pills;
});

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

/** Left mid-step by a run that stopped (paused, or the app closed): nothing works on it. */
const isInterrupted = (task: AgentTask) =>
  !!detail.value && !detail.value.driving && ['running', 'checking', 'review', 'fixing'].includes(task.status);

const taskTone = (task: AgentTask) => {
  if (isInterrupted(task)) return 'waiting';
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
  const label =
    task.status === 'review'
      ? `Review (${task.round + 1}/${detail.value?.run.limits.max_rounds ?? 3})`
      : TASK_STATUS_LABELS[task.status];
  return isInterrupted(task) ? `${label} · bị ngắt` : label;
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

/** A task's newest step: its worker or reviewer, or its setup / checks. */
const latestTaskLog = (taskId: string) => {
  const inv = latestInvocation(taskId);
  const side = [...(detail.value?.side_logs ?? [])].reverse().find((log) => log.task_id === taskId) ?? null;
  if (side && (!inv || Number(side.started_at) >= Number(inv.started_at))) {
    return { invocation: null, terminalId: side.id, title: side.title };
  }
  return inv ? { invocation: inv, terminalId: inv.terminal_id, title: '' } : null;
};

const openTaskLog = (taskId: string) => {
  const item = latestTaskLog(taskId);
  if (item) void openLaneItem(item);
};

/** A terminal of the lanes as a Workspace tab. */
const openLaneItem = async (item: { invocation: AgentInvocation | null; terminalId: string; title: string }) => {
  if (item.invocation) return openLog(item.invocation);
  const { commandId, runEventId } = ipcClient.agentsAttachLog(item.terminalId, `🤖 ${item.title}`);
  await router.push({
    path: '/workspace',
    query: { runEventId, runCommandId: String(commandId), runCommandName: `🤖 ${item.title}`, shellKind: 'agent' },
  });
};

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
  isDescExpanded.value = false;
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

const approvePlan = async (data: {
  design: string;
  contract: string;
  tasks: TaskDraft[];
  checks: CheckProposal[];
}) => {
  await ipcClient.agentsPlanApprove(detail.value!.run.id, data.design, data.contract, data.tasks, data.checks);
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

const cleanWorktrees = () => {
  const run = detail.value?.run;
  if (!run) return;
  ask(
    {
      title: 'Dọn Worktree',
      message: `Xóa các worktree của "${run.title}" (worktree của leader và các task, cả dependency đã cài)? Nhánh ${run.integration_branch} và PR giữ nguyên; lần làm tiếp hay push sau sẽ tạo worktree và cài lại dependency.`,
      confirmText: 'Dọn worktree',
    },
    () => ipcClient.agentsRunCleanWorktrees(run.id)
  );
};

const deleteRun = () => {
  const run = detail.value?.run;
  if (!run) return;
  ask(
    {
      title: 'Xóa Lượt Chạy',
      message: `Xóa "${run.title}" khỏi danh sách cùng toàn bộ log và worktree của nó? Code đã merge và các nhánh không bị ảnh hưởng.`,
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

/** The model a task's worker runs with: its own, its role's, the profile's. */
const taskModel = (task: AgentTask) => {
  const role = detail.value?.run.roles.find((r) => r.role === task.role && r.cli === task.cli);
  const profile = clis.value.find((info) => info.cli === task.cli)?.profile;
  return task.model || role?.model || profile?.steps?.work?.model || profile?.model || '';
};

const canReassign = computed(() => canEditTasks.value && selectedTask.value?.status !== 'merged');

const reassignSingleTask = async (
  cli: AgentCli,
  model: string | null,
  effort: string | null,
  timeoutMinutes: number | null
) => {
  const task = selectedTask.value;
  if (!task) return;
  await ipcClient.agentsTaskReassign(task.id, cli, model, effort);
  if (timeoutMinutes !== null) {
    await ipcClient.agentsTaskSetTimeout(task.id, timeoutMinutes);
  }
  notify(`Đã giao lại task ${task.key} cho ${cli}`, 'info');
  await reload();
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

const rereviewTask = () => {
  const task = selectedTask.value;
  if (!task) return;
  ask(
    {
      title: 'Leader Review Lại',
      message: `Leader review lại ${task.key} với code hiện tại trong worktree (phần sửa tay được commit trước), sau khi chạy lại lệnh kiểm tra. Đạt thì merge; chưa đạt thì nhận xét mới hiện ra để bạn chọn Thử lại hoặc Yêu cầu sửa.`,
      confirmText: 'Review lại',
    },
    () => ipcClient.agentsTaskRereview(task.id)
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
  showFeedbackModal.value = false;
  if (!feedback || !selectedTask.value) return;
  void act(() => ipcClient.agentsTaskRequestFix(selectedTask.value!.id, feedback));
};

// ---- Diff Snippet Preview for Selected Task ------------------------------
const diffSnippet = ref<string>('');
const diffSnippetFile = ref<string>('');
const loadingDiffSnippet = ref(false);

const loadTaskDiffSnippet = async (taskId?: string | null) => {
  const id = taskId ?? selectedTaskId.value;
  if (!id) {
    diffSnippet.value = '';
    diffSnippetFile.value = '';
    return;
  }
  const task = detail.value?.tasks.find((t) => t.id === id);
  if (!task || !task.branch || !task.worktree) {
    diffSnippet.value = '';
    diffSnippetFile.value = '';
    return;
  }
  loadingDiffSnippet.value = true;
  try {
    const raw = await ipcClient.agentsTaskDiff(id);
    if (!raw || !raw.trim()) {
      diffSnippet.value = '';
      diffSnippetFile.value = '';
      return;
    }
    const lines = raw.split('\n');
    let firstFile = '';
    for (const line of lines) {
      if (line.startsWith('diff --git')) {
        const parts = line.split(' ');
        firstFile = parts[parts.length - 1]?.replace(/^[ab]\//, '') || '';
        break;
      }
    }
    diffSnippetFile.value = firstFile || (task.key ? `${task.key} changes` : 'Tệp thay đổi');
    const meaningful = lines.filter((l) => !l.startsWith('diff --git') && !l.startsWith('index ')).slice(0, 10);
    diffSnippet.value = meaningful.join('\n');
  } catch {
    diffSnippet.value = '';
    diffSnippetFile.value = '';
  } finally {
    loadingDiffSnippet.value = false;
  }
};

watch(
  () => selectedTaskId.value,
  (id) => {
    if (id) void loadTaskDiffSnippet(id);
    else {
      diffSnippet.value = '';
      diffSnippetFile.value = '';
    }
  },
  { immediate: true }
);

const showFollowUp = ref(false);
const showModels = ref(false);
const showBulkReassign = ref(false);
const showOpenPr = ref(false);
const showAttachPr = ref(false);
const showAiPush = ref(false);
const verifying = ref(false);
const pushing = ref(false);
const pending = ref<PendingPush[]>([]);

/** Commits a finished run has not pushed: a number, 0, or null (unknown). */
const unpushed = computed(() => {
  if (!pending.value.length) return 0;
  if (pending.value.some((item) => item.commits === null)) return null;
  return pending.value.reduce((sum, item) => sum + (item.commits ?? 0), 0);
});

/** Pushing will rebase onto the PR's base first (and rewrite the branch). */
const needsRebase = computed(() => pending.value.some((item) => item.needs_rebase));

const loadPending = async () => {
  const run = detail.value?.run;
  if (!run || run.stage !== 'done' || !run.pull_requests.length) {
    pending.value = [];
    return;
  }
  pending.value = await ipcClient.agentsPrPending(run.id).catch(() => []);
};

watch(
  () => [detail.value?.run.id, detail.value?.run.stage, detail.value?.run.updated_at] as const,
  () => void loadPending(),
  { immediate: true }
);

const pushToPrs = () => {
  const run = detail.value?.run;
  if (!run) return;
  const targets = pending.value
    .map((item) => `${item.branch}${item.commits ? ` (${item.commits} commit)` : ''}`)
    .join(', ');
  const rebaseNote = pending.value
    .filter((item) => item.needs_rebase)
    .map((item) => `${item.branch} → origin/${item.base}`)
    .join(', ');
  ask(
    {
      title: needsRebase.value ? 'Rebase & Push Lên PR' : 'Push Lên PR',
      message: needsRebase.value
        ? `Nhánh chậm so với nhánh đích hoặc có commit merge (${rebaseNote}): app rebase trong worktree tạm (làm phẳng các commit merge), rồi push lên ${targets} bằng --force-with-lease — lịch sử nhánh PR được viết lại, nhưng chỉ khi nhánh trên GitHub vẫn đúng như app vừa fetch. Xung đột thì dừng, không push gì. Hook pre-push vẫn chạy.`
        : `Push công việc mới của lượt chạy lên ${targets}? Chỉ fast-forward, hook pre-push của dự án vẫn chạy (có thể mất vài phút); không tạo PR mới.`,
      confirmText: needsRebase.value ? 'Rebase & push' : 'Push',
      danger: needsRebase.value,
    },
    async () => {
      pushing.value = true;
      try {
        await ipcClient.agentsRunPushPrs(run.id);
        notify(`Đã push lên PR: ${run.pull_requests.map((pr) => pr.url).join(' · ')}`, 'info');
      } finally {
        pushing.value = false;
        void loadPending();
      }
    }
  );
};

/** Verify PR: may take a minute (fetch, leader review); may start a follow-up. */
const verifyPrs = async () => {
  const run = detail.value?.run;
  if (!run || verifying.value) return;
  verifying.value = true;
  try {
    const found = await ipcClient.agentsRunVerifyPrs(run.id);
    const open = found.prs.reduce((sum, pr) => sum + pr.open_comments.length, 0);
    notify(
      found.followed_up
        ? `Verify PR: còn ${open} comment / điểm cần sửa — đã tạo vòng làm tiếp`
        : found.note ?? `Verify PR xong: ${found.review?.verdict === 'approved' ? 'leader đạt' : 'xem kết quả'}`,
      'info'
    );
    await reload();
  } catch (error) {
    notify(errorText(error));
  } finally {
    verifying.value = false;
  }
};

/** Tasks that stopped and wait for a person: failed or out of review rounds. */
const failedTasks = computed(
  () => detail.value?.tasks.filter((task) => task.status === 'failed' || task.status === 'needs_human') ?? []
);

/** Errors stay in the modal; the change events reload the view. */
const bulkReassign = async (
  taskIds: string[],
  cli: AgentCli,
  model: string | null,
  effort: string | null,
  timeoutMinutes: number | null
) => {
  if (detail.value) {
    await ipcClient.agentsTasksReassign(detail.value.run.id, taskIds, cli, model, effort, timeoutMinutes);
  }
};

const saveModels = async (
  leader: { model: string | null; effort: string | null } | null,
  roles: RoleCli[] | null,
  limits: RunLimits
) => {
  const run = detail.value?.run;
  if (!run) return;
  await ipcClient.agentsRunSetLimits(run.id, limits);
  if (leader && roles) await ipcClient.agentsRunSetModels(run.id, leader, roles);
};



/** Errors stay in the modal; the change events reload the view. */
const followUp = async (request: string, pushPr: boolean) => {
  if (detail.value) await ipcClient.agentsRunFollowUp(detail.value.run.id, request, pushPr);
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
  min-width: 0;
}

.run-title-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
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
  flex-shrink: 0;
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
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
  margin: 2px 0 0;
  word-break: break-word;
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
  min-width: 0;
  min-height: 0;
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
  flex-shrink: 0;
}

/* Hero card */
.run-hero-card {
  flex-shrink: 0;
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

/* Hero Description */
.hero-desc-row {
  background-color: rgba(15, 17, 23, 0.45);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.hero-desc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.hero-desc-label {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-muted, #64748b);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-size: 10px;
}

.hero-desc-text {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-primary, #e2e8f0);
  white-space: pre-wrap;
  word-break: break-word;
}

.hero-desc-text.is-collapsed {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.btn-edit-run {
  background: none;
  border: none;
  color: var(--text-muted, #64748b);
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.btn-edit-run:hover {
  color: var(--text-primary, #e2e8f0);
  background-color: rgba(255, 255, 255, 0.08);
}

.hero-desc-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-edit-desc {
  background: none;
  border: none;
  color: var(--text-muted, #64748b);
  font-size: 11px;
  cursor: pointer;
  padding: 1px 5px;
  border-radius: 3px;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  transition: all 0.15s;
}

.btn-edit-desc:hover {
  color: var(--color-primary, #818cf8);
  background-color: rgba(129, 140, 248, 0.1);
}

.btn-toggle-desc {
  background: none;
  border: none;
  color: var(--color-primary, #818cf8);
  font-size: 11px;
  cursor: pointer;
  padding: 0;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  transition: opacity 0.15s;
}

.btn-toggle-desc:hover {
  text-decoration: underline;
  opacity: 0.85;
}

.edit-run-modal {
  width: 100%;
  max-width: 540px;
  background-color: var(--surface-container-high, #1e2230);
  border: 1px solid var(--border-subtle, #2d3348);
  border-radius: 10px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.5);
  position: relative;
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
  flex-shrink: 0;
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

.matrix-meta {
  display: flex;
  align-items: center;
  gap: 8px;
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
  flex-shrink: 0;
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

.spin {
  animation: verify-spin 1s linear infinite;
}

@keyframes verify-spin {
  to {
    transform: rotate(360deg);
  }
}

.verify-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
  padding: 8px 12px;
  font-size: 11px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
}

.verify-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  color: #e4b5ff;
}

.verify-pr {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.verify-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.verify-tag {
  padding: 1px 6px;
  border-radius: 4px;
  color: var(--text-secondary, #94a3b8);
  background-color: rgba(255, 255, 255, 0.05);
}

.verify-tag.ok {
  color: #4edea3;
}

.verify-tag.bad,
.verify-warn,
.verify-warn-text {
  color: var(--status-failed, #ef4444);
}

.verify-detail,
.verify-comment {
  color: var(--text-secondary, #94a3b8);
}

.verify-review {
  padding-top: 6px;
  border-top: 1px solid var(--border-subtle, #242a3e);
  color: #f87171;
}

.verify-review.approved {
  color: #4edea3;
}

.pr-links {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
  font-size: 12px;
  color: #e4b5ff;
}

.pr-link {
  color: #60a5fa;
  text-decoration: none;
}

.pr-link:hover {
  text-decoration: underline;
}

.task-batch {
  margin-left: 6px;
  padding: 0 5px;
  font-size: 10px;
  font-weight: 500;
  border-radius: 3px;
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.25);
}

.handoff-box {
  padding: 8px 12px;
  font-size: 11px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  background-color: rgba(96, 165, 250, 0.05);
}

.handoff-text {
  max-height: 180px;
  margin: 6px 0 0;
  overflow: auto;
  font-size: 11px;
  line-height: 1.5;
  white-space: pre-wrap;
  color: var(--text-secondary, #94a3b8);
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
  padding: 6px 12px;
  background-color: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  font-size: 11px;
}

.diff-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-primary, #e2e2eb);
  font-weight: 500;
}

.diff-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.diff-status-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  background-color: rgba(245, 158, 11, 0.15);
  color: #fbbf24;
  border: 1px solid rgba(245, 158, 11, 0.3);
}

.btn-diff-full {
  background: transparent;
  border: none;
  color: #38bdf8;
  font-size: 11px;
  cursor: pointer;
  padding: 0;
  transition: color 0.15s;
}

.btn-diff-full:hover {
  color: #7dd3fc;
  text-decoration: underline;
}

.diff-code {
  padding: 8px 12px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 180px;
  overflow-y: auto;
  background-color: #0b0d13;
}

.diff-code .line {
  line-height: 18px;
  white-space: pre;
}

.diff-code .line.muted {
  color: var(--text-muted, #64748b);
}

.diff-code .line.line-add {
  color: #34d399;
  background-color: rgba(16, 185, 129, 0.08);
}

.diff-code .line.line-del {
  color: #f87171;
  background-color: rgba(239, 68, 68, 0.08);
}

.diff-code .line.line-hunk {
  color: #818cf8;
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
  /* Terminals keep their height; the dashboard scrolls. */
  flex-shrink: 0;
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
  cursor: pointer;
  user-select: none;
}

.card-collapse-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 1px;
  color: var(--text-muted, #64748b);
  background: none;
  border: none;
  cursor: pointer;
  transition: color 0.15s;
}

.pty-title:hover .card-collapse-btn {
  color: var(--text-primary, #f1f5f9);
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

.activity-toggle {
  display: flex;
  margin-left: 6px;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
  overflow: hidden;
}

.activity-toggle button {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  font-size: 11px;
  color: var(--text-muted, #64748b);
  background: none;
  border: none;
  cursor: pointer;
  transition: color 0.15s, background-color 0.15s;
}

.activity-toggle button:hover {
  color: var(--text-primary, #f1f5f9);
}

.activity-toggle button.active {
  color: var(--text-primary, #f1f5f9);
  background-color: rgba(116, 71, 145, 0.3);
}

.collapse-all-btn {
  border-left: 1px solid var(--border-subtle, #242a3e) !important;
  border-right: 1px solid var(--border-subtle, #242a3e) !important;
}

.pty-monitor-card.collapsed {
  gap: 0;
}

.pty-monitor-card.collapsed .pty-header {
  border-bottom: none;
  padding-bottom: 0;
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
.quota-hot strong {
  color: var(--status-failed, #ef4444);
}

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

.reassign-group {
  display: flex;
  align-items: center;
  gap: 4px;
}

.reassign-input {
  height: 28px;
  padding: 0 6px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.reassign-model {
  width: 150px;
}

.timeout-field {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.timeout-input {
  width: 64px;
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
