<template>
  <div v-if="visible" class="modal-backdrop">
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
                  <th>KIỂU NHẬP</th>
                  <th>OPTIONS (LỰA CHỌN)</th>
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
                    <select v-model="p.kind" class="table-select" @change="handleKindChange(p, index)">
                      <option value="string">🔤 Nhập Text</option>
                      <option value="enum">📋 Selection (Lựa chọn)</option>
                      <option value="path">📁 Đường dẫn (Path)</option>
                      <option value="number">🔢 Số (Number)</option>
                      <option value="bool">☑️ Boolean</option>
                    </select>
                  </td>
                  <td class="options-cell">
                    <div v-if="p.kind === 'enum'" class="selection-cell-content">
                      <!-- Mode Switcher: Tự nhập vs Chọn Folder -->
                      <div class="selection-mode-toggle">
                        <button
                          type="button"
                          class="mode-btn"
                          :class="{ active: getSelectionMode(index) === 'manual' }"
                          title="Tự nhập danh sách options"
                          @click="setSelectionMode(index, 'manual')"
                        >
                          <Edit3 :size="10" />
                          <span>Tự nhập</span>
                        </button>
                        <button
                          type="button"
                          class="mode-btn"
                          :class="{ active: getSelectionMode(index) === 'folder' }"
                          title="Chọn thư mục và load danh sách file"
                          @click="setSelectionMode(index, 'folder')"
                        >
                          <Folder :size="10" />
                          <span>Từ Folder</span>
                        </button>
                      </div>

                      <!-- Mode 1: Tự nhập -->
                      <div v-if="getSelectionMode(index) === 'manual'" class="manual-input-box">
                        <input
                          :value="(p.options || []).join(', ')"
                          type="text"
                          class="table-input mono-input"
                          placeholder="GET, POST, PUT hoặc val1, val2..."
                          @input="updateOptions(p, $event)"
                        />
                      </div>

                      <!-- Mode 2: Chọn Folder và load files -->
                      <div v-else class="folder-input-box">
                        <input
                          :ref="el => setFolderInputRef(el, index)"
                          type="file"
                          webkitdirectory
                          directory
                          class="hidden-file-input"
                          @change="handleFolderSelect($event, index)"
                        />
                        <div class="folder-control-row">
                          <button
                            type="button"
                            class="btn-folder-pick"
                            :title="getFolderMeta(index).folderPath ? `Thư mục: ${getFolderMeta(index).folderPath}. Nhấp để chọn lại` : 'Nhấp để chọn thư mục'"
                            @click="triggerFolderPick(index)"
                          >
                            <FolderOpen :size="12" />
                            <span>{{ getFolderMeta(index).folderPath ? 'Đổi Folder' : 'Chọn Folder' }}</span>
                          </button>

                          <label class="deep-checkbox-label" title="Tích chọn: quét cả thư mục con; Bỏ chọn: chỉ lấy file trong thư mục gốc, không lấy thư mục con">
                            <input
                              type="checkbox"
                              :checked="getFolderMeta(index).loadDeep"
                              @change="toggleDeep(index)"
                            />
                            <span>Deep file</span>
                          </label>

                          <button
                            v-if="getFolderMeta(index).folderPath"
                            type="button"
                            class="btn-folder-refresh"
                            title="Quét lại file từ thư mục này"
                            @click="loadFilesFromPath(index, getFolderMeta(index).folderPath)"
                          >
                            <RefreshCw :size="11" />
                          </button>
                        </div>

                        <div class="folder-path-input-row">
                          <input
                            :value="getFolderMeta(index).folderPath"
                            type="text"
                            class="table-input font-mono folder-path-input"
                            placeholder="Nhập hoặc dán đường dẫn thư mục..."
                            @change="onFolderPathManualChange(index, ($event.target as HTMLInputElement).value)"
                          />
                        </div>

                        <div v-if="p.options && p.options.length > 0" class="folder-info-row">
                          <span class="folder-summary" :title="getFolderMeta(index).folderPath || getFolderMeta(index).folderName">
                            📁 <strong>{{ p.options.length }}</strong> files
                          </span>
                          <button
                            type="button"
                            class="btn-view-files"
                            title="Xem chi tiết danh sách file"
                            @click="openFilesPreview(index)"
                          >
                            <Eye :size="11" />
                            <span>Xem ({{ p.options.length }})</span>
                          </button>
                        </div>
                        <div v-else class="folder-empty-tip">
                          Chưa nạp file từ folder
                        </div>
                      </div>
                    </div>
                    <span v-else class="options-disabled">—</span>
                  </td>
                  <td>
                    <select
                      v-if="p.kind === 'enum' && p.options && p.options.length > 0"
                      v-model="p.default_value"
                      class="table-select mono-input"
                    >
                      <option value="">(Không chọn)</option>
                      <option v-for="opt in p.options" :key="opt" :value="opt">
                        {{ opt }}
                      </option>
                    </select>
                    <input
                      v-else
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

    <!-- Sub-modal Xem trước danh sách Files (Modal Preview Files) -->
    <div v-if="previewModalVisible && previewParam" class="sub-modal-backdrop" @click.self="closeFilesPreview">
      <div class="sub-modal-card">
        <div class="sub-modal-header">
          <div class="sub-modal-title">
            <FolderOpen :size="16" class="sub-modal-icon" />
            <span>Danh Sách File: {{ currentFolderMeta?.folderName || previewParam.label || previewParam.name }}</span>
            <span class="badge-total">{{ previewParam.options?.length || 0 }} files</span>
          </div>
          <button class="close-btn" @click="closeFilesPreview">
            <X :size="16" />
          </button>
        </div>

        <div class="sub-modal-body">
          <div class="sub-modal-toolbar">
            <div class="search-input-wrap">
              <Search :size="13" class="search-icon" />
              <input
                v-model="previewSearch"
                type="text"
                class="form-input search-input"
                placeholder="Tìm kiếm file theo tên..."
              />
            </div>

            <div v-if="currentFolderMeta && currentFolderMeta.rawFiles.length > 0" class="format-toggle-wrap">
              <label class="format-label">Định dạng:</label>
              <select
                :value="currentFolderMeta.valueFormat"
                class="format-select"
                @change="handleFormatChange(previewingParamIndex!, ($event.target as HTMLSelectElement).value as any)"
              >
                <option value="name">Tên file (filename.ext)</option>
                <option value="relative">Đường dẫn tương đối (sub/file.ext)</option>
              </select>
            </div>
          </div>

          <div class="files-list-container">
            <div
              v-for="(opt, optIdx) in filteredPreviewOptions"
              :key="opt + optIdx"
              class="file-item-chip"
            >
              <FileText :size="12" class="file-icon" />
              <span class="file-text" :title="opt">{{ opt }}</span>
              <button
                type="button"
                class="btn-remove-file"
                title="Loại bỏ file này khỏi options"
                @click="removeOptionFromPreview(opt)"
              >
                <X :size="11" />
              </button>
            </div>
            <div v-if="filteredPreviewOptions.length === 0" class="empty-files-msg">
              Không tìm thấy file nào khớp với tìm kiếm.
            </div>
          </div>
        </div>

        <div class="sub-modal-footer">
          <span class="files-hint">
            * Danh sách trên sẽ được nạp vào lựa chọn của tham số <code>{{ previewParam.name }}</code>.
          </span>
          <button class="btn-primary btn-sm" @click="closeFilesPreview">
            <Check :size="13" />
            <span>Hoàn Tất</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import {
  Edit3,
  X,
  Wand2,
  Plus,
  Trash2,
  Save,
  AlertCircle,
  LoaderCircle,
  Folder,
  FolderOpen,
  Eye,
  Check,
  Search,
  FileText,
  RefreshCw,
} from 'lucide-vue-next';
import { useTemplates } from '@/composables/useTemplates';
import { ipcClient } from '@/ipc/client';
import { parseFolderConfigFromOptions, buildOptionsWithFolderConfig } from '@/utils/templateFolder';
import type { CommandTemplate, TemplateParam, TemplateParamKind } from '@/types/models';

interface FolderMeta {
  folderName: string;
  folderPath: string;
  loadDeep: boolean;
  valueFormat: 'name' | 'relative';
  rawFiles: File[];
}

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

const selectionModes = ref<Record<number, 'manual' | 'folder'>>({});
const folderMetas = ref<Record<number, FolderMeta>>({});
const folderInputRefs = ref<Record<number, HTMLInputElement | null>>({});

const previewModalVisible = ref(false);
const previewingParamIndex = ref<number | null>(null);
const previewSearch = ref('');

const previewParam = computed(() => {
  if (previewingParamIndex.value === null) return null;
  return formData.value.params[previewingParamIndex.value] || null;
});

const currentFolderMeta = computed(() => {
  if (previewingParamIndex.value === null) return null;
  return getFolderMeta(previewingParamIndex.value);
});

const filteredPreviewOptions = computed(() => {
  if (!previewParam.value || !previewParam.value.options) return [];
  const q = previewSearch.value.trim().toLowerCase();
  if (!q) return previewParam.value.options;
  return previewParam.value.options.filter(opt => opt.toLowerCase().includes(q));
});

const getSelectionMode = (index: number): 'manual' | 'folder' => {
  return selectionModes.value[index] || 'manual';
};

const setSelectionMode = (index: number, mode: 'manual' | 'folder') => {
  selectionModes.value[index] = mode;
  if (mode === 'folder' && !folderMetas.value[index]) {
    folderMetas.value[index] = {
      folderName: '',
      folderPath: '',
      loadDeep: false,
      valueFormat: 'name',
      rawFiles: [],
    };
  }
};

const getFolderMeta = (index: number): FolderMeta => {
  if (!folderMetas.value[index]) {
    folderMetas.value[index] = {
      folderName: '',
      folderPath: '',
      loadDeep: false,
      valueFormat: 'name',
      rawFiles: [],
    };
  }
  return folderMetas.value[index];
};

const setFolderInputRef = (el: any, index: number) => {
  if (el) {
    folderInputRefs.value[index] = el as HTMLInputElement;
  }
};

const triggerFolderPick = async (index: number) => {
  try {
    const picked = await ipcClient.pickFolder();
    if (picked) {
      await loadFilesFromPath(index, picked);
      return;
    }
  } catch (e) {
    console.warn('Native folder picker failed or canceled, falling back to input:', e);
  }
  folderInputRefs.value[index]?.click();
};

const onFolderPathManualChange = async (index: number, path: string) => {
  const meta = getFolderMeta(index);
  meta.folderPath = path.trim();
  await loadFilesFromPath(index, path.trim());
};

const loadFilesFromPath = async (index: number, folderPath: string) => {
  if (!folderPath || !folderPath.trim()) return;
  const meta = getFolderMeta(index);
  meta.folderPath = folderPath.trim();
  meta.folderName = folderPath.trim().split(/[/\\]/).pop() || folderPath.trim();
  try {
    const files = await ipcClient.readFolderFiles(meta.folderPath, meta.loadDeep);
    const param = formData.value.params[index];
    if (param) {
      param.options = files;
      if (!param.default_value && files.length > 0) {
        param.default_value = files[0];
      }
    }
  } catch (err) {
    console.error('Failed to read folder files:', err);
  }
};

const handleKindChange = (param: TemplateParam, index: number) => {
  if (param.kind === 'enum') {
    if (!param.options) {
      param.options = [];
    }
  }
};

const handleFolderSelect = (event: Event, index: number) => {
  const input = event.target as HTMLInputElement;
  const fileList = input.files;
  if (!fileList || fileList.length === 0) return;

  const rawFiles = Array.from(fileList);
  const meta = getFolderMeta(index);
  meta.rawFiles = rawFiles;

  const firstPath = rawFiles[0].webkitRelativePath || '';
  if (firstPath) {
    meta.folderName = firstPath.split('/')[0] || 'Thư mục đã chọn';
  } else {
    meta.folderName = 'Thư mục đã chọn';
  }

  // Nếu file object có .path trên desktop
  const fullPath = (rawFiles[0] as any).path;
  if (fullPath) {
    // Trích xuất folder path
    const normalized = fullPath.replace(/\\/g, '/');
    const folderIdx = normalized.lastIndexOf('/');
    if (folderIdx !== -1) {
      meta.folderPath = normalized.substring(0, folderIdx);
    }
  }

  applyFolderFiles(index);
  input.value = '';
};

const applyFolderFiles = (index: number) => {
  const meta = getFolderMeta(index);
  const param = formData.value.params[index];
  if (!param || !meta.rawFiles.length) return;

  const results: string[] = [];
  for (const file of meta.rawFiles) {
    const relPath = file.webkitRelativePath || file.name;
    const parts = relPath.split('/');

    // Nếu loadDeep = false:
    // parts[0] là folder gốc, parts[1] là file ở folder gốc (parts.length === 2)
    // Nếu parts.length > 2: file nằm trong thư mục con -> bỏ qua
    if (!meta.loadDeep && parts.length > 2) {
      continue;
    }

    if (meta.loadDeep && meta.valueFormat === 'relative') {
      results.push(parts.slice(1).join('/'));
    } else {
      results.push(file.name);
    }
  }

  const unique = Array.from(new Set(results)).sort();
  param.options = unique;
  if (!param.default_value && unique.length > 0) {
    param.default_value = unique[0];
  }
};

const toggleDeep = async (index: number) => {
  const meta = getFolderMeta(index);
  meta.loadDeep = !meta.loadDeep;
  if (meta.folderPath) {
    await loadFilesFromPath(index, meta.folderPath);
  } else if (meta.rawFiles.length > 0) {
    applyFolderFiles(index);
  }
};

const handleFormatChange = (index: number, format: 'name' | 'relative') => {
  const meta = getFolderMeta(index);
  meta.valueFormat = format;
  if (meta.rawFiles.length > 0) {
    applyFolderFiles(index);
  }
};

const openFilesPreview = (index: number) => {
  previewingParamIndex.value = index;
  previewSearch.value = '';
  previewModalVisible.value = true;
};

const closeFilesPreview = () => {
  previewModalVisible.value = false;
  previewingParamIndex.value = null;
};

const removeOptionFromPreview = (optToRemove: string) => {
  if (previewingParamIndex.value === null) return;
  const param = formData.value.params[previewingParamIndex.value];
  if (!param || !param.options) return;
  param.options = param.options.filter(opt => opt !== optToRemove);
};

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
      paramErrs[index] = `Selection "${trimmedName}" cần có ít nhất một option (tự nhập hoặc chọn từ folder).`;
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
    selectionModes.value = {};
    folderMetas.value = {};
    folderInputRefs.value = {};
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
          formData.value.params.forEach((param, index) => {
            if (param.kind === 'enum') {
              const { isFolder, config, files } = parseFolderConfigFromOptions(param.options);
              if (isFolder && config) {
                selectionModes.value[index] = 'folder';
                folderMetas.value[index] = {
                  folderName: config.path.split(/[/\\]/).pop() || config.path,
                  folderPath: config.path,
                  loadDeep: config.deep,
                  valueFormat: 'name',
                  rawFiles: [],
                };
                param.options = files;
              } else {
                selectionModes.value[index] = 'manual';
              }
            }
          });
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
  delete selectionModes.value[index];
  delete folderMetas.value[index];
  delete folderInputRefs.value[index];
};

const moveParam = (index: number, direction: -1 | 1) => {
  const target = index + direction;
  if (target < 0 || target >= formData.value.params.length) return;
  const [param] = formData.value.params.splice(index, 1);
  formData.value.params.splice(target, 0, param);
  formData.value.params.forEach((item, order) => { item.param_order = order + 1; });

  const tempMode = selectionModes.value[index];
  selectionModes.value[index] = selectionModes.value[target];
  selectionModes.value[target] = tempMode;

  const tempMeta = folderMetas.value[index];
  folderMetas.value[index] = folderMetas.value[target];
  folderMetas.value[target] = tempMeta;
};

const updateOptions = (param: TemplateParam, event: Event) => {
  const value = (event.target as HTMLInputElement).value;
  param.options = value.split(',').map(item => item.trim()).filter(Boolean);
};

const handleSave = async () => {
  wasSubmitted.value = true;
  if (hasErrors.value) return;

  const payload = JSON.parse(JSON.stringify(formData.value));
  payload.params.forEach((param: TemplateParam, index: number) => {
    if (param.kind === 'enum' && selectionModes.value[index] === 'folder') {
      const meta = folderMetas.value[index];
      if (meta && meta.folderPath) {
        param.options = buildOptionsWithFolderConfig(param.options || [], {
          path: meta.folderPath,
          deep: meta.loadDeep,
        });
      }
    }
  });

  if (isEdit.value && props.templateId) {
    await updateTemplate(props.templateId, payload);
  } else {
    await createTemplate(payload);
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
  max-width: 880px;
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

/* Options Cell & Selection Modes */
.options-cell {
  min-width: 200px;
}

.selection-cell-content {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.selection-mode-toggle {
  display: flex;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 2px;
}

.mode-btn {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 10px;
  font-weight: 500;
  padding: 2px 4px;
  border-radius: 2px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  transition: all 0.15s ease;
}

.mode-btn:hover {
  color: var(--text-primary);
  background-color: var(--bg-surface-hover);
}

.mode-btn.active {
  background-color: var(--primary-subtle);
  color: #e4b5ff;
  font-weight: 600;
}

.manual-input-box {
  width: 100%;
}

.folder-input-box {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.folder-control-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.hidden-file-input {
  display: none !important;
}

.btn-folder-pick {
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 10px;
  font-weight: 500;
  padding: 3px 6px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
  transition: all 0.15s ease;
}

.btn-folder-pick:hover {
  background-color: var(--primary-subtle);
  border-color: var(--primary);
  color: #e4b5ff;
}

.deep-checkbox-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
}

.deep-checkbox-label input {
  cursor: pointer;
  width: 12px;
  height: 12px;
}

.btn-folder-refresh {
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: 10px;
  padding: 3px 5px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}

.btn-folder-refresh:hover {
  background-color: var(--primary-subtle);
  border-color: var(--primary);
  color: #e4b5ff;
}

.folder-path-input-row {
  width: 100%;
}

.folder-path-input {
  font-size: 10px !important;
  padding: 3px 6px !important;
  color: var(--text-primary);
  background-color: var(--bg-app-base);
}

.folder-info-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 2px 6px;
  border-radius: 3px;
}

.folder-summary {
  font-size: 10px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 110px;
}

.folder-summary strong {
  color: #e4b5ff;
}

.btn-view-files {
  background: transparent;
  border: none;
  color: var(--primary);
  font-size: 10px;
  font-weight: 600;
  padding: 1px 4px;
  border-radius: 2px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 3px;
  white-space: nowrap;
}

.btn-view-files:hover {
  background-color: var(--primary-subtle);
  color: #e4b5ff;
}

.folder-empty-tip {
  font-size: 10px;
  color: var(--text-muted);
  font-style: italic;
  padding: 1px 2px;
}

/* Sub-modal Preview Files */
.sub-modal-backdrop {
  position: fixed;
  inset: 0;
  background-color: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(4px);
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}

.sub-modal-card {
  width: 100%;
  max-width: 580px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.8), 0 0 20px var(--primary-alpha-25);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  max-height: 80vh;
}

.sub-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background-color: #141721;
  border-bottom: 1px solid var(--border-subtle);
}

.sub-modal-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.sub-modal-icon {
  color: #cda8ee;
}

.badge-total {
  font-size: 10px;
  background-color: var(--primary-subtle);
  color: #e4b5ff;
  padding: 1px 6px;
  border-radius: 10px;
  border: 1px solid var(--primary);
  font-weight: 600;
}

.sub-modal-body {
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  min-height: 250px;
}

.sub-modal-toolbar {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: space-between;
}

.search-input-wrap {
  position: relative;
  flex: 1;
}

.search-icon {
  position: absolute;
  left: 9px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-muted);
}

.search-input {
  padding-left: 28px !important;
  font-size: 11px;
}

.format-toggle-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
}

.format-label {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.format-select {
  background-color: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 11px;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  outline: none;
}

.files-list-container {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  max-height: 320px;
  overflow-y: auto;
  padding: 10px;
  background-color: #10131c;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  align-content: flex-start;
}

.file-item-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background-color: var(--bg-surface-hover);
  border: 1px solid var(--border-subtle);
  padding: 3px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-primary);
  max-width: 100%;
}

.file-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.file-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 440px;
}

.btn-remove-file {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 1px;
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.btn-remove-file:hover {
  color: var(--status-failed);
  background-color: rgba(239, 68, 68, 0.15);
}

.empty-files-msg {
  width: 100%;
  text-align: center;
  color: var(--text-muted);
  font-size: 12px;
  padding: 24px;
}

.sub-modal-footer {
  padding: 10px 16px;
  background-color: #141721;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.files-hint {
  font-size: 11px;
  color: var(--text-secondary);
}

.files-hint code {
  color: #e4b5ff;
  font-family: var(--font-mono);
}
</style>
