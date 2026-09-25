<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('close')">
    <div class="modal-content editor-modal">
      <div class="modal-header">
        <div class="modal-title">
          <Zap class="text-primary" :size="17" />
          <span>{{ form.id ? 'Chỉnh Sửa Câu Lệnh' : 'Tạo Câu Lệnh Mới' }}</span>
        </div>
        <button class="btn btn-ghost btn-icon" @click="$emit('close')">
          <X :size="15" />
        </button>
      </div>

      <div class="modal-body">
        <!-- Name Field -->
        <div class="form-group">
          <label class="form-label">Tên gợi nhớ của lệnh (*)</label>
          <input
            v-model="form.name"
            class="input"
            placeholder="Ví dụ: Vite Frontend Dev, Docker PostgreSQL..."
          />
        </div>

        <!-- is_shell Execution Mode -->
        <div class="form-group">
          <label class="form-label">Phương thức thực thi (*)</label>
          <div class="mode-selector">
            <label class="mode-card" :class="{ selected: !form.is_shell }">
              <input v-model="form.is_shell" type="radio" :value="false" />
              <div class="mode-text">
                <span class="mode-title">Chạy trực tiếp (Direct Argv) - Khuyên dùng</span>
                <span class="mode-desc">Phân tích mảng đối số tường minh, an toàn, không chạy qua trung gian shell.</span>
              </div>
            </label>

            <label class="mode-card" :class="{ selected: form.is_shell }">
              <input v-model="form.is_shell" type="radio" :value="true" />
              <div class="mode-text">
                <span class="mode-title">Chạy qua vỏ lệnh (Shell: bash/sh/cmd)</span>
                <span class="mode-desc">Hỗ trợ piping (|), chuyển hướng (>), và biến môi trường ($VAR).</span>
              </div>
            </label>
          </div>
        </div>

        <!-- Execution String -->
        <div class="form-group">
          <div class="label-row">
            <label class="form-label">Chuỗi dòng lệnh thực thi (Execution String) (*)</label>
            <span class="code-badge">{{ form.is_shell ? 'Shell Script' : 'Argv Array' }}</span>
          </div>
          <textarea
            v-model="form.execution_string"
            rows="4"
            class="textarea font-mono"
            placeholder="Ví dụ: pnpm --filter web dev --port 3000"
          />
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" @click="$emit('close')">
          Hủy bỏ
        </button>
        <button
          class="btn btn-primary"
          :disabled="!canSave"
          @click="handleSave"
        >
          <Save :size="14" />
          <span>{{ form.id ? 'Cập Nhật Lệnh' : 'Lưu Câu Lệnh' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { Zap, Save, X } from 'lucide-vue-next';
import type { CommandDefinition } from '@/types/models';

const props = defineProps<{
  visible: boolean;
  command?: CommandDefinition | null;
}>();

const emit = defineEmits<{
  (e: 'save', command: Partial<CommandDefinition>): void;
  (e: 'close'): void;
}>();

const form = ref<Partial<CommandDefinition>>({
  name: '',
  execution_string: '',
  is_shell: false,
});

watch(
  () => props.command,
  (val) => {
    if (val) {
      form.value = { ...val };
    } else {
      form.value = {
        name: '',
        execution_string: '',
        is_shell: false,
      };
    }
  },
  { immediate: true }
);

const canSave = computed(() => {
  return form.value.name?.trim() && form.value.execution_string?.trim();
});

const handleSave = () => {
  if (!canSave.value) return;
  emit('save', { ...form.value });
};
</script>

<style scoped>
.editor-modal {
  max-width: 560px;
}

.text-primary {
  color: var(--primary);
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.form-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.code-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
}

.mode-selector {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mode-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
  background-color: var(--bg-app-base);
  cursor: pointer;
  transition: all 0.15s ease;
}

.mode-card:hover {
  background-color: var(--bg-surface-hover);
  border-color: var(--border-medium);
}

.mode-card.selected {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.mode-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.mode-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.mode-desc {
  font-size: 11px;
  color: var(--text-muted);
}
</style>
