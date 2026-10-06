<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('close')">
    <div class="template-editor-modal">
      <!-- Modal Header -->
      <div class="modal-header">
        <div class="header-title-wrap">
          <div class="icon-badge">
            <Edit3 :size="18" />
          </div>
          <div>
            <div class="modal-title">
              {{ isEdit ? '📝 Chỉnh Sửa Mẫu Lệnh:' : '📝 Tạo Mẫu Lệnh Mới' }}
              <span v-if="isEdit" class="title-highlight">{{ formData.name }}</span>
            </div>
            <div class="modal-subtitle">
              <span>Cấu hình Template String {{ '{' + '{' + 'param' + '}' + '}' }} và Đặc tả Tham số</span>
            </div>
          </div>
        </div>
        <button class="close-btn" title="Đóng modal (Esc)" @click="$emit('close')">
          <X :size="18" />
        </button>
      </div>

      <div class="modal-body">
        <!-- Validation Error Banner -->
        <div v-if="wasSubmitted && hasErrors" class="validation-banner" role="alert">
          <AlertCircle :size="14" />
          <span>{{ validationSummaryMessage }}</span>
        </div>

        <!-- General Info Section -->
        <div class="form-section">
          <div class="form-row">
            <div class="form-group flex-2">
              <label class="form-label">Tên Mẫu Lệnh (*)</label>
              <input
                v-model="formData.name"
                type="text"
                class="form-input"
                :class="{ 'has-error': isFieldInvalid('name') }"
                placeholder="Ví dụ: FFmpeg Video Converter"
                @blur="touched.name = true"
                @input="touched.name = true"
              />
              <span v-if="isFieldInvalid('name')" class="field-error-msg">
                <AlertCircle :size="12" />
                <span>{{ errors.name }}</span>
              </span>
            </div>

            <div class="form-group flex-3">
              <label class="form-label">Mô Tả Vắn Tắt</label>
              <input
                v-model="formData.description"
                type="text"
                class="form-input"
                placeholder="Mô tả công dụng của template..."
              />
            </div>
          </div>

          <!-- Execution Mode Radio Group -->
          <div class="form-group">
            <label class="form-label">Phương Thức Thực Thi (*)</label>
            <div class="radio-cards-grid">
              <label
                class="radio-card"
                :class="{ active: !formData.is_shell }"
                @click="formData.is_shell = false"
              >
                <input type="radio" :checked="!formData.is_shell" name="exec_mode" />
                <div class="radio-content">
                  <div class="radio-title">Direct Argv (Phân Tách Mảng Token)</div>
                  <div class="radio-desc">
                    Khuyến nghị an toàn: Chạy trực tiếp file nhị phân, loại trừ hoàn toàn rủi ro Shell Injection.
                  </div>
                </div>
              </label>

              <label
                class="radio-card"
                :class="{ active: formData.is_shell }"
                @click="formData.is_shell = true"
              >
                <input type="radio" :checked="formData.is_shell" name="exec_mode" />
                <div class="radio-content">
                  <div class="radio-title">Shell Execution (bash / sh / cmd)</div>
                  <div class="radio-desc">
                    Hỗ trợ đường ống (pipe |), xuất file (>), biến môi trường ($VAR). Giá trị được bọc nháy an toàn.
                  </div>
                </div>
              </label>
            </div>
          </div>
        </div>

        <!-- Template String Section -->
        <div class="form-section">
          <div class="section-header">
            <label class="form-label">
              Chuỗi Cấu Trúc Mẫu (Template String) (*) — Dùng <code>&#123;&#123;param_name&#125;&#125;</code> cho tham số
            </label>
            <button class="btn-extract" type="button" @click="handleAutoExtract">
              <Wand2 :size="13" />
              <span>⚡ Tự Động Trích Xuất Placeholders</span>
            </button>
          </div>

          <div class="template-input-wrap">
            <textarea
              v-model="formData.template_string"
              rows="3"
              class="form-textarea mono-input"
              :class="{ 'has-error': isFieldInvalid('template_string') }"
              aria-label="Chuỗi template"
              placeholder="ffmpeg -i {{input}} -crf {{crf}} -key {{secret_key}} {{output}}"
              spellcheck="false"
              @blur="touched.template_string = true"
              @input="touched.template_string = true"
            ></textarea>
          </div>
          <span v-if="isFieldInvalid('template_string')" class="field-error-msg">
            <AlertCircle :size="12" />
            <span>{{ errors.template_string }}</span>
          </span>

          <div class="token-detected-strip">
            <span class="detected-label">Placeholders phát hiện:</span>
            <div class="tokens-list">
              <span v-for="ph in detectedPlaceholders" :key="ph" class="token-pill">
                &#123;&#123;{{ ph }}&#125;&#125;
              </span>
              <span v-if="detectedPlaceholders.length === 0" class="no-tokens">
                Chưa phát hiện biểu thức {{ '{' + '{' + 'name' + '}' + '}' }} nào.
              </span>
            </div>
          </div>
        </div>

        <!-- Parameters Spec Table Section -->
        <div class="form-section">
          <div class="section-header">
            <label class="form-label">
              DANH SÁCH THAM SỐ KHAI BÁO (TEMPLATE PARAMETERS)
            </label>
            <button class="btn-secondary btn-sm" type="button" @click="addParamRow">
              <Plus :size="13" />
              <span>Thêm Tham Số</span>
            </button>
          </div>

          <div class="params-table-wrap">
            <table class="params-table">
              <thead>
                <tr>
                  <th>TÊN THAM SỐ</th>
                  <th>NHÃN (LABEL)</th>
                  <th>KIỂU DỮ LIỆU</th>
                  <th>OPTIONS (ENUM)</th>
                  <th>MẶC ĐỊNH</th>
                  <th class="text-center">REQD</th>
                  <th class="text-center">SECRET</th>
                  <th class="text-center">THỨ TỰ</th>
                  <th class="text-center">XÓA</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(p, index) in formData.params" :key="index">
                  <td>
                    <input
                      v-model="p.name"
                      type="text"
                      class="table-input mono-input"
                      :class="{ 'has-error': (wasSubmitted || touched['param_' + index]) && errors.params?.[index] }"
                      placeholder="param_name"
                      @blur="touched['param_' + index] = true"
                      @input="touched['param_' + index] = true"
                    />
                    <span v-if="(wasSubmitted || touched['param_' + index]) && errors.params?.[index]" class="field-error-msg table-err">
                      {{ errors.params[index] }}
                    </span>
                  </td>
                  <td>
                    <input
                      v-model="p.label"
                      type="text"
                      class="table-input"
                      placeholder="Nhãn hiển thị..."
                    />
                  </td>
                  <td>
                    <select v-model="p.kind" class="table-select">
                      <option value="string">String</option>
                      <option value="number">Number</option>
                      <option value="path">Path</option>
                      <option value="enum">Enum</option>
                      <option value="bool">Bool</option>
                    </select>
                  </td>
                  <td>
                    <input
                      v-if="p.kind === 'enum'"
                      :value="(p.options || []).join(', ')"
                      type="text"
                      class="table-input mono-input"
                      placeholder="GET, POST, PUT"
                      @input="updateOptions(p, $event)"
                    />
                    <span v-else class="options-disabled">—</span>
                  </td>
                  <td>
                    <input
                      v-model="p.default_value"
                      type="text"
                      class="table-input mono-input"
                      placeholder="Mặc định..."
                    />
                  </td>
                  <td class="text-center">
                    <input v-model="p.required" type="checkbox" class="table-checkbox" />
                  </td>
                  <td class="text-center">
                    <input
                      v-model="p.is_secret"
                      type="checkbox"
                      class="table-checkbox secret-check"
                      title="Tham số bảo mật (ẩn ký tự, không ghi log DB/Preset)"
                    />
                  </td>
                  <td class="text-center order-cell">
                    <input v-model.number="p.param_order" type="number" min="1" class="order-input" />
                    <button type="button" class="order-btn" :disabled="index === 0" @click="moveParam(index, -1)">↑</button>
                    <button type="button" class="order-btn" :disabled="index === formData.params.length - 1" @click="moveParam(index, 1)">↓</button>
                  </td>
                  <td class="text-center">
                    <button class="btn-icon-danger" type="button" @click="removeParamRow(index)">
                      <Trash2 :size="14" />
                    </button>
                  </td>
                </tr>
                <tr v-if="formData.params.length === 0">
                  <td colspan="9" class="empty-params">
                    Chưa khai báo tham số nào. Nhấn "⚡ Tự Động Trích Xuất Placeholders" hoặc "Thêm Tham Số" để bắt đầu.
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="modal-footer">
        <button class="btn-ghost" :disabled="loading" @click="!loading && $emit('close')">
          Hủy Bỏ (Esc)
        </button>
        <button class="btn-primary" :disabled="loading" @click="handleSave">
          <LoaderCircle v-if="loading" :size="15" class="spin" />
          <Save v-else :size="15" />
          <span>{{ loading ? loadingText : '💾 Lưu Mẫu Lệnh' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { Edit3, X, Wand2, Plus, Trash2, Save, AlertCircle, LoaderCircle } from 'lucide-vue-next';
import { useTemplates } from '@/composables/useTemplates';
import type { CommandTemplate, TemplateParam, TemplateParamKind } from '@/types/models';

const props = withDefaults(
  defineProps<{
    visible: boolean;
    templateId: number | null;
    loading?: boolean;
    loadingText?: string;
  }>(),
  {
    loading: false,
    loadingText: 'Đang Lưu...',
  }
);

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'save'): void;
}>();

const { getTemplateById, createTemplate, updateTemplate, extractPlaceholders } = useTemplates();

const isEdit = computed(() => props.templateId !== null);

const formData = ref<{
  name: string;
  description: string;
  template_string: string;
  is_shell: boolean;
  params: TemplateParam[];
}>({
  name: '',
  description: '',
  template_string: '',
  is_shell: true,
  params: [],
});



const touched = ref<Record<string, boolean>>({});
const wasSubmitted = ref(false);

const errors = computed(() => {
  const errs: { name?: string; template_string?: string; params?: Record<number, string> } = {};
  if (!formData.value.name.trim()) {
    errs.name = 'Tên mẫu lệnh không được để trống.';
  }
  if (!formData.value.template_string.trim()) {
    errs.template_string = 'Chuỗi mẫu lệnh không được để trống.';
  }

  const paramErrs: Record<number, string> = {};
  const duplicateNames = new Set<string>();

  formData.value.params.forEach((param, index) => {
    const trimmedName = param.name.trim();
    if (!trimmedName) {
      paramErrs[index] = 'Tên tham số không được để trống.';
    } else if (!/^[A-Za-z0-9_]+$/.test(trimmedName)) {
      paramErrs[index] = `Tên "${trimmedName}" không hợp lệ (chỉ được dùng chữ cái, số và dấu gạch dưới).`;
    } else if (duplicateNames.has(trimmedName)) {
      paramErrs[index] = `Tên tham số "${trimmedName}" bị trùng lặp.`;
    } else if (param.kind === 'enum' && !(param.options || []).length) {
      paramErrs[index] = `Enum "${trimmedName}" cần có ít nhất một option (ví dụ: GET, POST).`;
    } else {
      duplicateNames.add(trimmedName);
    }
  });

  if (Object.keys(paramErrs).length > 0) {
    errs.params = paramErrs;
  }

  return errs;
});

const hasErrors = computed(() => {
  return !!(errors.value.name || errors.value.template_string || errors.value.params);
});

const isFieldInvalid = (field: 'name' | 'template_string') => {
  return (wasSubmitted.value || touched.value[field]) && !!errors.value[field];
};

const validationSummaryMessage = computed(() => {
  if (errors.value.name) return errors.value.name;
  if (errors.value.template_string) return errors.value.template_string;
  if (errors.value.params) {
    const firstKey = Object.keys(errors.value.params)[0];
    return errors.value.params[Number(firstKey)];
  }
  return 'Vui lòng kiểm tra và sửa các thông tin không hợp lệ bên dưới.';
});

watch(
  [() => props.visible, () => props.templateId],
  ([visible, newId]) => {
    wasSubmitted.value = false;
    touched.value = {};
    if (visible) {
      if (newId) {
        const existing = getTemplateById(newId);
        if (existing) {
          formData.value = {
            name: existing.name,
            description: existing.description || '',
            template_string: existing.template_string,
            is_shell: existing.is_shell,
            params: JSON.parse(JSON.stringify(existing.params || [])),
          };
          return;
        }
      }
      // Default form for new template
      formData.value = {
        name: '',
        description: '',
        template_string: 'ffmpeg -i {{input}} -crf {{crf}} {{output}}',
        is_shell: true,
        params: [
          {
            name: 'input',
            label: 'File Video Input',
            kind: 'path',
            default_value: '',
            required: true,
            is_secret: false,
            param_order: 1,
          },
          {
            name: 'crf',
            label: 'CRF Compression Level',
            kind: 'number',
            default_value: '23',
            required: false,
            is_secret: false,
            param_order: 2,
          },
          {
            name: 'output',
            label: 'Output File Path',
            kind: 'string',
            default_value: '',
            required: true,
            is_secret: false,
            param_order: 3,
          },
        ],
      };
    } else {
      formData.value = {
        name: '',
        description: '',
        template_string: 'ffmpeg -i {{input}} -crf {{crf}} {{output}}',
        is_shell: true,
        params: [],
      };
    }
  },
  { immediate: true }
);

const detectedPlaceholders = computed(() => {
  return extractPlaceholders(formData.value.template_string);
});

const handleAutoExtract = () => {
  const phs = detectedPlaceholders.value;
  const existingNames = new Set(formData.value.params.map(p => p.name));

  phs.forEach(ph => {
    if (!existingNames.has(ph)) {
      let guessedKind: TemplateParamKind = 'string';
      if (ph.includes('file') || ph.includes('path') || ph === 'input' || ph === 'out' || ph === 'output') {
        guessedKind = 'path';
      } else if (ph.includes('port') || ph.includes('crf') || ph.includes('count') || ph.includes('num')) {
        guessedKind = 'number';
      }

      formData.value.params.push({
        name: ph,
        label: ph.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase()),
        kind: guessedKind,
        default_value: '',
        required: true,
        is_secret: ph.includes('secret') || ph.includes('password') || ph.includes('token') || ph.includes('key'),
        param_order: formData.value.params.length + 1,
      });
    }
  });
};

const addParamRow = () => {
  formData.value.params.push({
    name: `param_${formData.value.params.length + 1}`,
    label: `Tham số ${formData.value.params.length + 1}`,
    kind: 'string',
    default_value: '',
    required: false,
    is_secret: false,
    param_order: formData.value.params.length + 1,
  });
};

const removeParamRow = (index: number) => {
  formData.value.params.splice(index, 1);
  formData.value.params.forEach((param, order) => { param.param_order = order + 1; });
};

const moveParam = (index: number, direction: -1 | 1) => {
  const target = index + direction;
  if (target < 0 || target >= formData.value.params.length) return;
  const [param] = formData.value.params.splice(index, 1);
  formData.value.params.splice(target, 0, param);
  formData.value.params.forEach((item, order) => { item.param_order = order + 1; });
};

const updateOptions = (param: TemplateParam, event: Event) => {
  const value = (event.target as HTMLInputElement).value;
  param.options = value.split(',').map(item => item.trim()).filter(Boolean);
};

const handleSave = async () => {
  wasSubmitted.value = true;
  if (hasErrors.value) return;

  if (isEdit.value && props.templateId) {
    await updateTemplate(props.templateId, formData.value);
  } else {
    await createTemplate(formData.value);
  }

  emit('save');
  emit('close');
};
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: var(--surface-overlay, #0b0d13bf);
  backdrop-filter: blur(6px);
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.template-editor-modal {
  width: 100%;
  max-width: 740px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 30px #00000099, 0 0 20px var(--primary-alpha-25, #74479140);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  max-height: 90vh;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
  border-top: 3px solid var(--primary);
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-badge {
  width: 34px;
  height: 34px;
  border-radius: var(--radius-md);
  background-color: var(--primary-subtle);
  color: #cda8ee;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.title-highlight {
  color: #e4b5ff;
  margin-left: 4px;
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
}

.close-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.modal-body {
  padding: 16px 18px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.form-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.flex-2 { flex: 2; }
.flex-3 { flex: 3; }

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.form-input {
  width: 100%;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  padding: 7px 10px;
  border-radius: var(--radius-sm);
  outline: none;
}

.form-input:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.radio-cards-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.radio-card {
  background-color: #141721;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  cursor: pointer;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  transition: all 0.15s ease;
}

.radio-card.active {
  border-color: var(--primary);
  background-color: var(--primary-subtle);
}

.radio-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.radio-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
}

.radio-desc {
  font-size: 11px;
  color: var(--text-secondary);

  line-height: 1.3;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-extract {
  background-color: var(--primary-subtle);
  border: 1px solid var(--primary);
  color: #e4b5ff;
  font-size: 11px;
  font-weight: 600;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-extract:hover {
  background-color: var(--primary);
  color: #ffffff;
}

.template-input-wrap {
  position: relative;
  width: 100%;
}

.form-textarea {
  width: 100%;
  min-height: 80px;
  margin: 0;
  padding: 8px 10px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow: auto;
  box-sizing: border-box;
  outline: none;
  resize: vertical;
}

.form-textarea:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-glow);
}

.form-textarea.has-error {
  border-color: var(--status-failed, #ef4444) !important;
  background-color: rgba(239, 68, 68, 0.05);
}

.mono-input {
  font-family: var(--font-mono);
}

.token-detected-strip {
  display: flex;
  align-items: center;
  gap: 8px;
  background-color: #141721;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.detected-label {
  font-size: 11px;
  color: var(--text-secondary);
}

.tokens-list {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.token-pill {
  font-family: var(--font-mono);
  font-size: 11px;
  background-color: var(--primary-alpha-25, #74479140);
  color: #e4b5ff;
  padding: 1px 6px;
  border-radius: 3px;
  border: 1px solid var(--primary-alpha-40, #74479166);
}

.no-tokens {
  font-size: 11px;
  color: var(--text-muted);
}

.params-table-wrap {
  background-color: #141721;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.params-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.params-table th {
  background-color: #0f1117;
  color: var(--text-secondary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  padding: 8px 10px;
  text-align: left;
  border-bottom: 1px solid var(--border-subtle);
}

.params-table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--border-subtle);
}

.params-table tr:last-child td {
  border-bottom: none;
}

.table-input {
  width: 100%;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 11px;
  padding: 4px 6px;
  border-radius: 3px;
}

.options-disabled {
  color: var(--text-muted);
  padding: 0 8px;
}

.order-cell {
  white-space: nowrap;
}

.order-input {
  width: 38px;
  padding: 4px;
  color: var(--text-primary);
  background: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: 3px;
}

.order-btn {
  padding: 2px 4px;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: 3px;
  cursor: pointer;
}

.order-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.table-select {
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  color-scheme: dark;
  width: 100%;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 11px;
  padding: 4px 20px 4px 6px;
  border-radius: 3px;
  outline: none;
  cursor: pointer;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 4px center;
  background-size: 12px 12px;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.table-select:focus {
  border-color: var(--border-focus);
}

.table-select option {
  background-color: var(--surface-container);
  color: var(--text-primary);
}

.table-checkbox {
  cursor: pointer;
}

.secret-check:checked {
  accent-color: #f59e0b;
}

.text-center {
  text-align: center;
}

.btn-icon-danger {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
}

.btn-icon-danger:hover {
  color: var(--status-failed);
}

.empty-params {
  text-align: center;
  color: var(--text-muted);
  padding: 16px;
  font-size: 12px;
}

.modal-footer {
  padding: 12px 18px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.btn-primary {
  background-color: var(--primary);
  color: #ffffff;
  border: none;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;
  box-shadow: 0 0 10px var(--primary-glow);
}

.btn-primary:hover {
  background-color: var(--primary-hover);
}

.btn-secondary {
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  padding: 5px 10px;
  font-size: 11px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
}

.btn-secondary:hover {
  background-color: var(--primary-subtle);
  border-color: var(--primary);
}

.btn-ghost {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.btn-ghost:hover {
  color: var(--text-primary);
}

.form-input.has-error,
.form-textarea.has-error,
.table-input.has-error {
  border-color: var(--status-failed, #ef4444) !important;
  background-color: #ef44440d !important;
}

.field-error-msg {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--status-failed, #ef4444);
  margin-top: 4px;
}

.table-err {
  font-size: 10px;
  margin-top: 2px;
}

.validation-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background-color: var(--status-failed-bg, #ef44441a);
  border: 1px solid #ef44444d;
  border-radius: var(--radius-md);
  color: var(--status-failed, #ef4444);
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 12px;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
