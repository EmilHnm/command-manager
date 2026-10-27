<template>
  <div class="lanes-wrapper">
    <div v-if="lanes.length === 0" class="lanes-empty">Chưa có agent nào chạy.</div>
    <div
      v-else
      class="lanes"
      :class="[`mode-${layout}`]"
    >
      <section
        v-for="lane in lanes"
        :key="lane.key"
        class="lane"
        :class="{
          running: lane.running,
          failed: lane.failed,
          wide: expanded === lane.key,
          collapsed: isCollapsed(lane.key),
        }"
      >
        <header
          class="lane-head"
          :title="isCollapsed(lane.key) ? 'Bấm để mở rộng terminal' : 'Bấm để thu gọn terminal'"
          @click="toggleCollapse(lane.key)"
        >
          <button
            type="button"
            class="lane-collapse-btn"
            :title="isCollapsed(lane.key) ? 'Mở rộng' : 'Thu gọn'"
            @click.stop="toggleCollapse(lane.key)"
          >
            <ChevronRight v-if="isCollapsed(lane.key)" :size="13" />
            <ChevronDown v-else :size="13" />
          </button>
          <span class="lane-dot" />
          <span class="lane-title" :title="lane.title">{{ lane.title }}</span>
          <span class="lane-cli font-mono">{{ lane.cli }}</span>
          <span class="lane-state">{{ lane.state }}</span>
          <span class="lane-spacer" />
          <select
            v-if="lane.items.length > 1"
            class="lane-select font-mono"
            :value="pinned[lane.key] ?? ''"
            title="Bước đang xem"
            @click.stop
            @change="pin(lane.key, ($event.target as HTMLSelectElement).value)"
          >
            <option value="">Mới nhất</option>
            <option v-for="(item, index) in lane.items" :key="item.id" :value="item.id">
              #{{ index + 1 }} {{ item.label }} · {{ clock(item.started_at) }} {{ mark(item) }}
            </option>
          </select>
          <button
            v-if="!isCollapsed(lane.key)"
            type="button"
            class="lane-btn"
            :title="expanded === lane.key ? 'Thu nhỏ' : 'Phóng to'"
            @click.stop="expanded = expanded === lane.key ? null : lane.key"
          >
            <Minimize2 v-if="expanded === lane.key" :size="13" />
            <Maximize2 v-else :size="13" />
          </button>
          <button
            type="button"
            class="lane-btn"
            title="Mở thành tab trong Workspace"
            @click.stop="emit('expand', { invocation: lane.shown.invocation, terminalId: lane.shown.id, title: lane.title })"
          >
            <ExternalLink :size="13" />
          </button>
        </header>
        <div v-show="!isCollapsed(lane.key)" class="lane-body">
          <XtermPane
            v-if="terminals[lane.shown.id]"
            :key="terminals[lane.shown.id]!.runEventId"
            :command-id="terminals[lane.shown.id]!.commandId"
            :command-name="lane.title"
            :run-event-id="terminals[lane.shown.id]!.runEventId"
            :process-status="lane.shown.finished_at ? (lane.shown.ok ? 'completed' : 'failed') : 'running'"
            shell-kind="agent"
            :font-size="12"
            :hide-toolbar="true"
          />
          <div v-else class="lane-wait">{{ errors[lane.shown.id] ?? 'Đang mở log…' }}</div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from 'vue';
import { ChevronDown, ChevronRight, ExternalLink, Maximize2, Minimize2 } from 'lucide-vue-next';
import XtermPane from '@/components/terminal/XtermPane.vue';
import { ipcClient } from '@/ipc/client';
import type { AgentCli, AgentInvocation, AgentTask, SideLog } from '@/types/agents';
import { TASK_STATUS_LABELS } from '@/types/agents';

const props = withDefaults(
  defineProps<{
    invocations: AgentInvocation[];
    /** Checks, setup, context and push logs (not agents). */
    sideLogs?: SideLog[];
    tasks: AgentTask[];
    leaderCli: AgentCli;
    layout?: 'grid' | 'list';
  }>(),
  {
    sideLogs: () => [],
    layout: 'grid',
  }
);

const emit = defineEmits<{
  (e: 'expand', item: { invocation: AgentInvocation | null; terminalId: string; title: string }): void;
  (e: 'collapse-change', allCollapsed: boolean): void;
}>();

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
  research: 'tìm hiểu',
  describe: 'viết PR',
  commit: 'commit message',
  chat: 'hỏi đáp',
};

const SIDE_LABELS: Record<string, string> = {
  check: 'kiểm tra',
  setup: 'chuẩn bị',
  context: 'ngữ cảnh',
  pr: 'push & PR',
};

/** One step in a lane: an agent's invocation or a side log (checks...). */
interface LaneItem {
  /** Invocation id, or the side log's terminal id. */
  id: string;
  started_at: string;
  finished_at: string | null;
  ok: boolean | null;
  label: string;
  invocation: AgentInvocation | null;
}

interface Lane {
  key: string;
  title: string;
  cli: string;
  state: string;
  items: LaneItem[];
  /** The step in the terminal. */
  shown: LaneItem;
  running: boolean;
  failed: boolean;
}

/** Lane → invocation id the user picked; none: follow the newest. */
const pinned = reactive<Record<string, string>>({});
const expanded = ref<string | null>(null);
const terminals = reactive<Record<string, { commandId: number; runEventId: string }>>({});
const errors = reactive<Record<string, string>>({});

/** Map of collapsed status per lane key */
const collapsed = reactive<Record<string, boolean>>({});

const isCollapsed = (key: string) => !!collapsed[key];

const toggleCollapse = (key: string) => {
  collapsed[key] = !collapsed[key];
  checkCollapseChange();
};

const checkCollapseChange = () => {
  if (lanes.value.length === 0) return;
  const all = lanes.value.every((l) => collapsed[l.key]);
  emit('collapse-change', all);
};

const setAllCollapsed = (target: boolean) => {
  for (const lane of lanes.value) {
    collapsed[lane.key] = target;
  }
  emit('collapse-change', target);
};

defineExpose({
  setAllCollapsed,
});

const taskOf = (id: string | null) => (id ? props.tasks.find((task) => task.id === id) : undefined);

const stepLabel = (inv: AgentInvocation) => {
  const step = STEP_LABELS[inv.step] ?? inv.step;
  const task = inv.step === 'review' ? taskOf(inv.task_id) : undefined;
  return task ? `${step} ${task.key}` : step;
};

const clock = (unix: string) =>
  new Date(Number(unix) * 1000).toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' });

const mark = (item: { finished_at: string | null; ok: boolean | null }) =>
  !item.finished_at ? '…' : item.ok ? '✓' : '✗';

/**
 * The leader designs, plans and reviews; each task has its worker, and its
 * setup and checks. Run-wide logs (integration checks, context, pushes) go
 * with the leader.
 */
const laneKey = (inv: AgentInvocation) =>
  inv.step === 'review' || inv.step === 'verify' || !inv.task_id ? 'leader' : `task:${inv.task_id}`;

const lanes = computed<Lane[]>(() => {
  const groups = new Map<string, LaneItem[]>();
  const add = (key: string, item: LaneItem) => {
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(item);
  };
  for (const inv of props.invocations) {
    add(laneKey(inv), {
      id: inv.id,
      started_at: inv.started_at,
      finished_at: inv.finished_at,
      ok: inv.ok,
      label: stepLabel(inv),
      invocation: inv,
    });
  }
  for (const log of props.sideLogs) {
    add(log.task_id ? `task:${log.task_id}` : 'leader', {
      id: log.id,
      started_at: log.started_at,
      finished_at: log.finished_at,
      ok: log.ok,
      label: log.task_id ? (SIDE_LABELS[log.kind] ?? log.kind) : log.title,
      invocation: null,
    });
  }
  for (const items of groups.values()) {
    items.sort((a, b) => Number(a.started_at) - Number(b.started_at));
  }
  const order = (key: string) =>
    key === 'leader' ? -1 : (taskOf(key.slice('task:'.length))?.sort_order ?? Number.MAX_SAFE_INTEGER);
  return [...groups.entries()]
    .sort(([a], [b]) => order(a) - order(b))
    .map(([key, items]) => {
      const latest = items[items.length - 1];
      const shown = items.find((item) => item.id === pinned[key]) ?? latest;
      const task = key === 'leader' ? undefined : taskOf(key.slice('task:'.length));
      const running = !latest.finished_at;
      return {
        key,
        title: task ? `${task.key} · ${task.title}` : 'Leader',
        cli: task ? `${task.role} · ${task.cli}` : props.leaderCli,
        state: running
          ? `đang ${latest.label}…`
          : task
            ? TASK_STATUS_LABELS[task.status]
            : `${latest.label} ${mark(latest)}`,
        items,
        shown,
        running,
        failed: latest.ok === false,
      };
    });
});

const pin = (lane: string, id: string) => {
  if (id) pinned[lane] = id;
  else delete pinned[lane];
};

// A log tab is opened (or rebuilt from its saved output) once per invocation.
const opening = new Set<string>();
let retry: number | undefined;

const ensureTerminals = async () => {
  for (const lane of lanes.value) {
    const item = lane.shown;
    if (terminals[item.id] || opening.has(item.id)) continue;
    opening.add(item.id);
    const title = `🤖 ${lane.cli} · ${lane.title}`;
    try {
      terminals[item.id] = item.invocation
        ? await ipcClient.agentsOpenLog(item.id, title)
        : ipcClient.agentsAttachLog(item.id, `${title} · ${item.label}`);
      delete errors[item.id];
    } catch (error) {
      errors[item.id] = error instanceof Error ? error.message : String(error);
      // A process just starting has no tab yet: try again shortly.
      if (!item.finished_at) {
        window.clearTimeout(retry);
        retry = window.setTimeout(() => void ensureTerminals(), 500);
      }
    } finally {
      opening.delete(item.id);
    }
  }
};

watch(() => lanes.value.map((lane) => lane.shown.id).join(), () => void ensureTerminals(), {
  immediate: true,
});

onUnmounted(() => window.clearTimeout(retry));
</script>

<style scoped>
.lanes-wrapper {
  display: flex;
  flex-direction: column;
}

.lanes-empty {
  padding: 12px;
  font-size: 12px;
  color: var(--text-muted, #64748b);
}

/* Grid layout mode */
.lanes.mode-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(460px, 1fr));
  gap: 10px;
}

/* List layout mode */
.lanes.mode-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.lane {
  display: flex;
  flex-direction: column;
  height: 320px;
  min-width: 0;
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 6px;
  overflow: hidden;
  background-color: var(--bg-terminal, #0b0d13);
  transition: border-color 0.15s;
}

.lane.collapsed {
  height: auto;
  min-height: unset;
}

.lane.wide {
  grid-column: 1 / -1;
  height: 560px;
}

.lanes.mode-list .lane.wide {
  height: 560px;
}

.lane.running {
  border-color: rgba(96, 165, 250, 0.6);
}

.lane.failed {
  border-color: rgba(248, 113, 113, 0.5);
}

.lane-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px 6px 6px;
  font-size: 12px;
  background-color: var(--surface-container, #1a1e2b);
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  cursor: pointer;
  user-select: none;
  transition: background-color 0.15s;
}

.lane-head:hover {
  background-color: var(--surface-container-high, #23283a);
}

.lane.collapsed .lane-head {
  border-bottom: none;
}

.lane-collapse-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px 4px;
  color: var(--text-muted, #64748b);
  background: none;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  transition: color 0.15s, background-color 0.15s;
}

.lane-collapse-btn:hover {
  color: var(--text-primary, #f1f5f9);
  background-color: rgba(255, 255, 255, 0.08);
}

.lane-dot {
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: var(--text-muted, #64748b);
}

.lane.running .lane-dot {
  background-color: #60a5fa;
  box-shadow: 0 0 6px #60a5fa;
}

.lane.failed .lane-dot {
  background-color: #f87171;
}

.lane-title {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-primary, #f1f5f9);
}

.lane-cli {
  flex: none;
  padding: 1px 6px;
  font-size: 11px;
  border-radius: 4px;
  color: #e4b5ff;
  background-color: rgba(116, 71, 145, 0.2);
}

.lane-state {
  flex: none;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.lane.running .lane-state {
  color: #60a5fa;
}

.lane-spacer {
  flex: 1;
}

.lane-select {
  max-width: 190px;
  padding: 2px 4px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.lane-btn {
  display: flex;
  padding: 3px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.lane-btn:hover {
  color: var(--text-primary, #f1f5f9);
  background-color: rgba(255, 255, 255, 0.06);
}

.lane-body {
  flex: 1;
  min-height: 0;
}

.lane-wait {
  padding: 12px;
  font-size: 12px;
  color: var(--text-muted, #64748b);
}

@media (max-width: 720px) {
  .lanes.mode-grid {
    grid-template-columns: 1fr;
  }
}
</style>
