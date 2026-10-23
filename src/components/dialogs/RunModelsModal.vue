<template>
  <div v-if="visible && run" class="modal-backdrop">
    <div class="models-modal" role="dialog" aria-modal="true">
      <div class="modal-head">
        <Cpu :size="16" />
        <span class="modal-title">Thiết lập lượt chạy</span>
        <span class="modal-subtitle">{{ run.title }}</span>
        <button type="button" class="btn-close" title="Đóng" @click="emit('close')">
          <X :size="16" />
        </button>
      </div>
      <div class="modal-body">
        <p class="hint">
          Áp dụng từ bước tiếp theo; agent đang chạy giữ thiết lập lúc nó bắt đầu. Giới hạn được nhớ làm mặc định cho
          lượt chạy sau của dự án.
        </p>

        <div class="section-title">Giới hạn</div>
        <div class="limits">
          <label class="limit">
            <span>Timeout mỗi lượt agent (phút)</span>
            <input v-model.number="limits.timeout_minutes" type="number" min="1" :max="MAX_TIMEOUT_MINUTES" step="5" class="field font-mono" />
          </label>
          <label class="limit">
            <span>Ngân sách (USD)</span>
            <input v-model.number="limits.max_cost_usd" type="number" min="0.5" step="0.5" class="field font-mono" />
          </label>
          <label class="limit">
            <span>Dừng khi hạn mức ≥ (%)</span>
            <input v-model.number="limits.quota_pause_percent" type="number" min="0" max="100" step="5" class="field font-mono" />
          </label>
          <label class="limit">
            <span>Vòng review tối đa</span>
            <input v-model.number="limits.max_rounds" type="number" min="1" max="10" class="field font-mono" />
          </label>
          <label class="limit">
            <span>Tổng lượt gọi CLI</span>
            <input v-model.number="limits.max_invocations" type="number" min="1" max="500" class="field font-mono" />
          </label>
          <label class="limit check">
            <input v-model="limits.auto_resume" type="checkbox" />
            <span>Tự chạy tiếp khi hạn mức reset</span>
          </label>
        </div>
        <p class="hint small">
          Task nặng có thể đặt timeout riêng trong Task Inspector. Timeout của lệnh kiểm tra / chuẩn bị đặt theo repo
          trong Đội &amp; CLI → Dự án.
        </p>

        <div class="section-title">Model</div>
        <p v-if="driving" class="hint small">Đổi model khi lượt chạy đã dừng (Tạm dừng rồi mở lại).</p>
        <p v-else class="hint small">
          Để trống là dùng mặc định trong Đội &amp; CLI. Task đã "Giao lại" với model riêng giữ model của nó. Review dùng
          model Review của Đội &amp; CLI.
        </p>

        <div class="row head">
          <span>Agent</span>
          <span>CLI</span>
          <span>Model</span>
          <span>Effort</span>
        </div>
        <div class="row">
          <span class="who">Leader <small>thiết kế &amp; kế hoạch</small></span>
          <span class="cli font-mono">{{ run.leader_cli }}</span>
          <input
            v-model="leader.model"
            class="field font-mono"
            :list="`run-models-${run.leader_cli}`"
            :placeholder="profileModelHint(profileOf(run.leader_cli), 'lead')"
            :disabled="driving"
          />
          <select v-model="leader.effort" class="field" :disabled="driving || run.leader_cli === 'cursor'">
            <option value="">mặc định</option>
            <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
          </select>
        </div>
        <div v-for="role in roles" :key="role.role" class="row">
          <span class="who">{{ role.role }}</span>
          <span class="cli font-mono">{{ role.cli }}</span>
          <input
            v-model="role.model"
            class="field font-mono"
            :list="`run-models-${role.cli}`"
            :placeholder="profileModelHint(profileOf(role.cli), 'work')"
            :disabled="driving"
          />
          <select v-model="role.effort" class="field" :disabled="driving || role.cli === 'cursor'">
            <option value="">mặc định</option>
            <option v-for="level in EFFORT_LEVELS" :key="level" :value="level">{{ level }}</option>
          </select>
        </div>
        <datalist v-for="cli in AGENT_CLIS" :id="`run-models-${cli}`" :key="cli">
          <option v-for="item in modelsOf(cli)" :key="item.id" :value="item.id">{{ item.label }}</option>
        </datalist>
        <div v-if="error" class="error">{{ error }}</div>
      </div>
      <div class="modal-foot">
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('close')">Hủy</button>
        <button type="button" class="btn btn-primary btn-sm" :disabled="busy" @click="save">
          <Save :size="13" /> {{ busy ? 'Đang lưu…' : 'Lưu' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { Cpu, Save, X } from 'lucide-vue-next';
import { useAgentModels } from '@/composables/useAgentModels';
import type { AgentCli, AgentRun, CliInfo, RoleCli, RunLimits } from '@/types/agents';
import {
  AGENT_CLIS,
  DEFAULT_LIMITS,
  EFFORT_LEVELS,
  MAX_TIMEOUT_MINUTES,
  profileModelHint,
} from '@/types/agents';

const { modelsOf } = useAgentModels();

const props = defineProps<{
  visible: boolean;
  run: AgentRun | null;
  clis: CliInfo[];
  /** Models change only while the run is not running; limits always. */
  driving: boolean;
  save: (
    leader: { model: string | null; effort: string | null } | null,
    roles: RoleCli[] | null,
    limits: RunLimits
  ) => Promise<void>;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const leader = reactive({ model: '', effort: '' });
const limits = reactive<RunLimits>({ ...DEFAULT_LIMITS });
const roles = ref<{ role: string; cli: AgentCli; model: string; effort: string }[]>([]);
const busy = ref(false);
const error = ref<string | null>(null);

const profileOf = (cli: AgentCli) => props.clis.find((info) => info.cli === cli)?.profile;

watch(
  () => props.visible,
  (visible) => {
    if (!visible || !props.run) return;
    Object.assign(limits, DEFAULT_LIMITS, props.run.limits);
    leader.model = props.run.leader_model ?? '';
    leader.effort = props.run.leader_effort ?? '';
    roles.value = props.run.roles.map((role) => ({
      role: role.role,
      cli: role.cli,
      model: role.model ?? '',
      effort: role.effort ?? '',
    }));
    error.value = null;
  },
  { immediate: true }
);

async function save() {
  busy.value = true;
  error.value = null;
  try {
    await props.save(
      props.driving ? null : { model: leader.model.trim() || null, effort: leader.effort || null },
      props.driving
        ? null
        : roles.value.map((role) => ({
            role: role.role,
            cli: role.cli,
            model: role.model.trim() || null,
            effort: role.effort || null,
          })),
      { ...limits }
    );
    emit('close');
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
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
  padding: 16px;
  background-color: rgba(5, 7, 12, 0.7);
}

.models-modal {
  width: min(720px, 100%);
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  background-color: var(--surface-container, #1a1e2b);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 8px;
  color: var(--text-primary, #f1f5f9);
}

.modal-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-subtle, #242a3e);
  color: #e4b5ff;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary, #f1f5f9);
}

.modal-subtitle {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--text-muted, #64748b);
}

.btn-close {
  margin-left: auto;
  padding: 4px;
  color: var(--text-secondary, #94a3b8);
  background: none;
  border: none;
  cursor: pointer;
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  overflow-y: auto;
}

.hint {
  margin: 0 0 4px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary, #94a3b8);
}

.section-title {
  margin-top: 6px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #e4b5ff;
}

.hint.small {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.limits {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 8px;
}

.limit {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary, #94a3b8);
}

.limit.check {
  flex-direction: row;
  align-items: center;
  gap: 8px;
  align-self: end;
  font-size: 12px;
}

.row {
  display: grid;
  grid-template-columns: 1.2fr 0.7fr 1.5fr 0.9fr;
  gap: 8px;
  align-items: center;
}

.row.head {
  font-size: 11px;
  color: var(--text-muted, #64748b);
}

.who {
  font-size: 13px;
  font-weight: 600;
}

.who small {
  display: block;
  font-size: 10px;
  font-weight: 400;
  color: var(--text-muted, #64748b);
}

.cli {
  font-size: 12px;
  color: #e4b5ff;
}

.field {
  min-width: 0;
  height: 30px;
  padding: 0 8px;
  font-size: 12px;
  color: var(--text-primary, #f1f5f9);
  background-color: var(--bg-app-base, #0f1117);
  border: 1px solid var(--border-subtle, #242a3e);
  border-radius: 4px;
}

.error {
  font-size: 12px;
  color: var(--status-failed, #ef4444);
}

.modal-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 14px;
  border-top: 1px solid var(--border-subtle, #242a3e);
}

@media (max-width: 560px) {
  .row {
    grid-template-columns: 1fr 1fr;
  }

  .row.head {
    display: none;
  }
}
</style>
