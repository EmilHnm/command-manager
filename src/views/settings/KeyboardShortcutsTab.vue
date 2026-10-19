<template>
  <div class="keybindings-tab">
    <!-- Hidden File Input for JSON Import -->
    <input
      ref="importFileInput"
      type="file"
      accept=".json,application/json"
      class="hidden-file-input"
      @change="handleImportFileSelected"
    />

    <!-- Sub-Toolbar -->
    <section class="shortcuts-toolbar">
      <!-- Left: Search Box + Record Mode Toggle + Scope Select -->
      <div class="toolbar-left">
        <!-- Search Input with Record Mode support -->
        <div
          class="search-box"
          :class="{ 'is-recording': isRecordingSearch, 'has-query': searchQuery.length > 0 }"
        >
          <Search :size="15" class="search-icon" />
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="text"
            :placeholder="
              isRecordingSearch
                ? 'Nhấn tổ hợp phím... (Esc để dừng)'
                : 'Tìm theo tên tác vụ, ID hoặc phím...'
            "
            class="search-input"
            @input="isSearchFromRecord = false"
            @keydown="handleSearchKeydown"
          />
          <button
            v-if="searchQuery"
            class="clear-search-btn"
            title="Xóa tìm kiếm (Esc)"
            @click="clearSearch"
          >
            <X :size="13" />
          </button>
          <span v-else class="search-hint">Esc để xóa</span>
        </div>

        <!-- Record Keystroke Button -->
        <button
          class="btn-record-key"
          :class="{ active: isRecordingSearch }"
          title="Bấm phím để lọc hoặc tìm tổ hợp phím trực tiếp"
          @click="toggleSearchRecord"
        >
          <Keyboard :size="14" />
          <span>[⌨ Ghi phím]</span>
        </button>

        <!-- Scope Selector Dropdown -->
        <div class="scope-dropdown-wrapper">
          <select v-model="filterScope" class="scope-filter-select">
            <option value="all">Tất cả phạm vi</option>
            <option value="global">Toàn ứng dụng</option>
            <option value="workspace">Workspace</option>
            <option value="terminal">Terminal đang gõ</option>
            <option value="explorer">Cây nhóm lệnh</option>
            <option value="files">Trình quản lý file</option>
            <option value="suggest">Terminal có gợi ý</option>
          </select>
        </div>
      </div>

      <!-- Right: Filter Chips & Action Menu -->
      <div class="toolbar-right">
        <!-- Filter Chips -->
        <div class="filter-chips-group">
          <button
            class="filter-chip"
            :class="{ active: activeFilterTab === 'all' }"
            @click="activeFilterTab = 'all'"
          >
            <span class="chip-dot dot-primary"></span>
            <span>Tất cả {{ totalCount }}</span>
          </button>

          <button
            class="filter-chip"
            :class="{ active: activeFilterTab === 'custom' }"
            @click="activeFilterTab = 'custom'"
          >
            <span class="chip-dot dot-secondary"></span>
            <span>Đã tùy chỉnh {{ customCount }}</span>
          </button>

          <button
            class="filter-chip chip-conflict"
            :class="{
              active: activeFilterTab === 'conflict',
              'has-conflicts': conflictCount > 0,
            }"
            :title="conflictCount > 0 ? `Có ${conflictCount} xung đột phím tắt` : 'Không có xung đột'"
            @click="activeFilterTab = 'conflict'"
          >
            <span class="chip-dot dot-warning" :class="{ pulse: conflictCount > 0 }"></span>
            <span>⚠ Xung đột {{ conflictCount }}</span>
          </button>
        </div>

        <div class="toolbar-divider"></div>

        <!-- More Actions Dropdown [...] -->
        <div class="dropdown-container">
          <button
            class="btn-more-actions"
            title="Tác vụ nâng cao"
            @click="showMoreMenu = !showMoreMenu"
          >
            <MoreVertical :size="15" />
          </button>

          <div v-if="showMoreMenu" class="dropdown-menu">
            <button class="dropdown-item" @click="handleExportJson">
              <Download :size="14" class="text-primary" />
              <span>Xuất ra JSON (keybindings.json)</span>
            </button>
            <button class="dropdown-item" @click="triggerImportFile">
              <Upload :size="14" class="text-accent" />
              <span>Nhập từ JSON...</span>
            </button>
            <div class="dropdown-sep"></div>
            <button class="dropdown-item item-danger" @click="promptResetAll">
              <RotateCcw :size="14" />
              <span>Khôi phục toàn bộ về mặc định</span>
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- Conflict Alert Banner (If Conflicts Exist) -->
    <div v-if="conflictCount > 0 && activeFilterTab !== 'conflict'" class="conflict-alert-banner">
      <div class="alert-content">
        <AlertTriangle :size="16" class="text-warning flex-shrink-0" />
        <div class="alert-text">
          <span class="alert-title">Phát hiện {{ conflictCount }} xung đột phím tắt!</span>
          <span class="alert-desc">
            Một số phím tắt đang được gán đồng thời cho các tác vụ trong cùng phạm vi.
          </span>
        </div>
      </div>
      <button class="btn-resolve-conflicts" @click="activeFilterTab = 'conflict'">
        Giải quyết ngay
      </button>
    </div>

    <!-- Main Table Container -->
    <div class="table-container" @click="showMoreMenu = false">
      <!-- Empty State -->
      <div v-if="groupedCategories.length === 0" class="empty-state">
        <Search :size="32" class="empty-icon" />
        <div class="empty-title">Không tìm thấy tác vụ nào</div>
        <p class="empty-desc">
          Không có tác vụ nào khớp với từ khóa "{{ searchQuery }}" hoặc bộ lọc đã chọn.
        </p>
        <button class="btn btn-secondary btn-sm" @click="clearAllFilters">
          Xóa bộ lọc
        </button>
      </div>

      <!-- High-Density Shortcuts Table -->
      <div v-else class="shortcuts-table">
        <!-- Sticky Table Header -->
        <div class="table-header-row">
          <div class="th-col th-action">TÁC VỤ (ACTION / COMMAND)</div>
          <div class="th-col th-key">PHÍM TẮT (KEYBINDING)</div>
          <div class="th-col th-scope">PHẠM VI (SCOPE)</div>
          <div class="th-col th-source">NGUỒN (SOURCE)</div>
        </div>

        <!-- Groups -->
        <div
          v-for="cat in groupedCategories"
          :key="cat.id"
          class="category-group"
        >
          <!-- Category Header Bar (Collapsible) -->
          <div class="category-header-bar" @click="toggleCategory(cat.id)">
            <div class="cat-title-wrap">
              <ChevronDown
                :size="15"
                class="cat-arrow"
                :class="{ collapsed: collapsedCategories.has(cat.id) }"
              />
              <span class="cat-title">{{ cat.title }}</span>
            </div>
            <span class="cat-count-badge">{{ cat.items.length }} tác vụ</span>
          </div>

          <!-- Group Items (Rows) -->
          <div v-show="!collapsedCategories.has(cat.id)" class="category-rows">
            <div
              v-for="row in cat.items"
              :key="`${row.commandId}-${row.key}-${row.when}`"
              :id="`row-${row.commandId}`"
              class="table-data-row"
              :class="{
                'is-highlighted': highlightedCommandId === row.commandId,
                'has-conflict': row.hasConflict && !row.isRemoved,
                'is-removed': row.isRemoved,
                'is-custom': row.source === 'user',
              }"
              @dblclick="openEditModal(row)"
            >
              <!-- Col 1: Action / Command -->
              <div class="td-col td-action">
                <span
                  class="row-status-dot"
                  :class="{
                    'dot-custom': row.source === 'user',
                    'dot-removed': row.isRemoved,
                    'dot-conflict': row.hasConflict && !row.isRemoved,
                  }"
                  :title="row.hasConflict ? row.conflictWith?.join(', ') : ''"
                ></span>

                <div class="action-info">
                  <div class="action-title-line">
                    <span class="action-title" :class="{ 'line-through': row.isRemoved }">
                      {{ row.title }}
                    </span>
                    <AlertTriangle
                      v-if="row.hasConflict && !row.isRemoved"
                      :size="13"
                      class="conflict-icon"
                      :title="'Xung đột với: ' + row.conflictWith?.join(', ')"
                    />
                  </div>
                  <code class="action-id-mono" :title="row.commandId">{{ row.commandId }}</code>
                </div>
              </div>

              <!-- Col 2: Keybinding -->
              <div class="td-col td-key">
                <span v-if="row.isRemoved" class="removed-key-label">—</span>
                <div v-else class="keycaps-display">
                  <template v-for="(kPart, kIdx) in formatChordForDisplay(row.key)" :key="kIdx">
                    <kbd class="keycap">{{ kPart }}</kbd>
                    <span
                      v-if="kIdx < formatChordForDisplay(row.key).length - 1"
                      class="keycap-plus"
                    >+</span>
                  </template>
                </div>
              </div>

              <!-- Col 3: Scope -->
              <div class="td-col td-scope">
                <span class="scope-pill">{{ getScopeLabel(row.when) }}</span>
              </div>

              <!-- Col 4: Source & Actions -->
              <div class="td-col td-source">
                <span
                  class="source-badge"
                  :class="{
                    'src-default': row.source === 'default',
                    'src-user': row.source === 'user',
                    'src-removed': row.source === 'removed',
                    'src-fixed': row.source === 'fixed',
                  }"
                >
                  {{ getSourceLabel(row.source) }}
                </span>

                <!-- Action Buttons (Hover / Focus) -->
                <div v-if="!row.locked" class="row-actions">
                  <button
                    class="row-action-btn add-btn"
                    title="Thêm phím tắt (+)"
                    @click.stop="openAddModal(row)"
                  >
                    <Plus :size="13" />
                  </button>
                  <button
                    class="row-action-btn edit-btn"
                    title="Chỉnh sửa phím tắt (✎)"
                    @click.stop="openEditModal(row)"
                  >
                    <Pencil :size="13" />
                  </button>
                  <button
                    v-if="row.isOverride || row.source === 'user' || row.isRemoved"
                    class="row-action-btn revert-btn"
                    title="Khôi phục về mặc định (↺)"
                    @click.stop="handleRevertRow(row)"
                  >
                    <RotateCcw :size="13" />
                  </button>
                  <button
                    v-if="!row.isRemoved"
                    class="row-action-btn remove-btn"
                    title="Gỡ phím tắt (✕)"
                    @click.stop="handleRemoveRow(row)"
                  >
                    <X :size="13" />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Telemetry Status Bar -->
    <footer class="tab-telemetry-bar">
      <div class="telemetry-left">
        <span class="info-pill">ⓘ Phím tắt áp dụng ngay khi thay đổi, không cần bấm "Lưu Cài Đặt".</span>
      </div>
      <div class="telemetry-right">
        <span>Tổng số: {{ effectiveBindings.length }}</span>
        <span>•</span>
        <span>Tùy chỉnh: {{ customCount }}</span>
        <span>•</span>
        <span :class="{ 'text-warning': conflictCount > 0 }">Xung đột: {{ conflictCount }}</span>
      </div>
    </footer>

    <!-- MOD-16 Modal -->
    <KeybindingModal
      :visible="showEditModal"
      :command-id="modalTarget?.commandId || ''"
      :title="modalTarget?.title || ''"
      :initial-key="modalTarget?.isRemoved ? '' : (modalIsNew ? '' : modalTarget?.key)"
      :initial-when="modalTarget?.when"
      :is-new-binding="modalIsNew"
      :existing-bindings="effectiveBindings"
      :allowed-scopes="modalTarget?.scopes"
      @save="onModalSave"
      @remove="onModalRemove"
      @close="showEditModal = false"
    />

    <!-- Reset All Confirmation Dialog -->
    <ConfirmDialog
      :visible="showResetConfirm"
      title="Khôi Phục Toàn Bộ Phím Tắt"
      subtitle="Thiết lập lại phím tắt về mặc định ban đầu"
      :message="`Bạn có chắc chắn muốn xóa tất cả ${customCount} tùy chỉnh phím tắt và quay về mặc định?`"
      confirm-text="Khôi Phục Mặc Định"
      danger
      @confirm="onResetAllConfirm"
      @cancel="showResetConfirm = false"
    />

    <!-- Import Preview Dialog -->
    <div v-if="showImportPreview" class="modal-backdrop" @click.self="showImportPreview = false">
      <div class="import-preview-container">
        <header class="modal-header">
          <div class="header-title-wrap">
            <Upload :size="16" class="text-primary" />
            <div>
              <div class="modal-title">Xem Trước Nhập Phím Tắt (Keybindings Import)</div>
              <div class="modal-subtitle">
                Tìm thấy {{ importParsed?.entries.length || 0 }} mục hợp lệ,
                {{ importParsed?.skipped.length || 0 }} mục bị bỏ qua
              </div>
            </div>
          </div>
          <button class="close-btn" @click="showImportPreview = false">
            <X :size="16" />
          </button>
        </header>

        <div class="preview-body">
          <div v-if="importParsed && importParsed.skipped.length > 0" class="skipped-warning">
            <AlertTriangle :size="15" class="text-warning flex-shrink-0" />
            <div>
              <strong>Các mục không hợp lệ bị bỏ qua:</strong>
              <ul class="skipped-list">
                <li v-for="(skip, sIdx) in importParsed.skipped" :key="sIdx">
                  Mục #{{ skip.index + 1 }}: {{ skip.reason }}
                </li>
              </ul>
            </div>
          </div>

          <div class="import-entries-list">
            <div class="entries-header">DANH SÁCH SẼ ÁP DỤNG:</div>
            <div
              v-for="(ent, eIdx) in importParsed?.entries || []"
              :key="eIdx"
              class="import-entry-row"
            >
              <span class="entry-cmd" :class="{ 'is-removal': ent.command.startsWith('-') }">
                {{ ent.command }}
              </span>
              <kbd class="keycap">{{ formatChordString(ent.key) }}</kbd>
              <span class="entry-when">{{ ent.when || 'global' }}</span>
            </div>
          </div>
        </div>

        <footer class="modal-footer">
          <button class="btn btn-secondary btn-sm" @click="showImportPreview = false">
            Hủy Bỏ
          </button>
          <button class="btn btn-primary btn-sm" @click="applyImport">
            Áp Dụng {{ importParsed?.entries.length || 0 }} Phím Tắt
          </button>
        </footer>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue';
import {
  Search,
  Keyboard,
  MoreVertical,
  Download,
  Upload,
  RotateCcw,
  ChevronDown,
  Pencil,
  Plus,
  X,
  AlertTriangle,
} from 'lucide-vue-next';
import type {
  KeyScope,
  ResolvedKeybinding,
  ParsedKeybindings,
  KeybindingOverride,
  SkippedEntry,
} from '@/types/keybindings';
import { KEY_CATEGORIES, COMMAND_MAP } from '@/keybindings/registry';
import {
  formatChordForDisplay,
  formatChordString,
  getScopeLabel,
  eventToChord,
  normalizeKey,
  isImeComposing,
} from '@/keybindings/normalize';
import { pauseDispatcher, resumeDispatcher } from '@/keybindings/dispatcher';
import { useKeybindings } from '@/composables/useKeybindings';
import KeybindingModal from '@/components/dialogs/KeybindingModal.vue';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';

const emit = defineEmits<{
  (e: 'toast', payload: { message: string; type?: 'info' | 'success' | 'warning' | 'error' }): void;
}>();

const {
  effectiveBindings,
  conflictCount,
  customizedCount,
  fetchKeybindings,
  setBinding,
  removeBinding,
  resetBinding,
  resetAll,
  parseJson,
  exportJson,
  importOverrides,
} = useKeybindings();

// State
const searchQuery = ref('');
const isRecordingSearch = ref(false);
const isSearchFromRecord = ref(false);
const searchInputRef = ref<HTMLInputElement | null>(null);
const filterScope = ref<string>('all');
const activeFilterTab = ref<'all' | 'custom' | 'conflict'>('all');
const showMoreMenu = ref(false);
const collapsedCategories = ref<Set<string>>(new Set());
const highlightedCommandId = ref<string | null>(null);

// Modal state
const showEditModal = ref(false);
const modalTarget = ref<ResolvedKeybinding | null>(null);
const modalIsNew = ref(false);

// Reset confirm state
const showResetConfirm = ref(false);

// Import preview state
const importFileInput = ref<HTMLInputElement | null>(null);
const showImportPreview = ref(false);
const importParsed = ref<ParsedKeybindings | null>(null);

onMounted(async () => {
  await fetchKeybindings();
});

onBeforeUnmount(() => {
  if (isRecordingSearch.value) {
    resumeDispatcher();
  }
});

const totalCount = computed(() => {
  return effectiveBindings.value.filter((b) => !b.isRemoved).length;
});

const customCount = computed(() => customizedCount.value);

// Search & Filtering
const filteredBindings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();

  return effectiveBindings.value.filter((b) => {
    // 1. Filter Tab
    if (activeFilterTab.value === 'custom') {
      if (!b.isOverride && b.source !== 'user' && !b.isRemoved) return false;
    } else if (activeFilterTab.value === 'conflict') {
      if (!b.hasConflict || b.isRemoved) return false;
    }

    // 2. Scope filter
    if (filterScope.value !== 'all' && b.when !== filterScope.value) {
      return false;
    }

    // 3. Search query filter
    if (query) {
      if (isSearchFromRecord.value) {
        if (b.key.toLowerCase() !== query) {
          return false;
        }
      } else {
        const matchTitle = b.title.toLowerCase().includes(query);
        const matchId = b.commandId.toLowerCase().includes(query);
        const matchKey = b.key.toLowerCase().includes(query);
        const matchFormattedKey = formatChordString(b.key).toLowerCase().includes(query);
        const matchScope = getScopeLabel(b.when).toLowerCase().includes(query);
        if (!matchTitle && !matchId && !matchKey && !matchFormattedKey && !matchScope) {
          return false;
        }
      }
    }

    return true;
  });
});

// Grouped by Category
const groupedCategories = computed(() => {
  const groups: { id: string; title: string; items: ResolvedKeybinding[] }[] = [];

  for (const cat of KEY_CATEGORIES) {
    const items = filteredBindings.value.filter((b) => b.category === cat.id);
    if (items.length > 0) {
      groups.push({
        id: cat.id,
        title: cat.title,
        items,
      });
    }
  }

  return groups;
});

const toggleCategory = (catId: string) => {
  if (collapsedCategories.value.has(catId)) {
    collapsedCategories.value.delete(catId);
  } else {
    collapsedCategories.value.add(catId);
  }
};

const clearSearch = () => {
  searchQuery.value = '';
  isSearchFromRecord.value = false;
  if (isRecordingSearch.value) {
    isRecordingSearch.value = false;
    resumeDispatcher();
  }
};

const clearAllFilters = () => {
  searchQuery.value = '';
  isSearchFromRecord.value = false;
  filterScope.value = 'all';
  activeFilterTab.value = 'all';
  if (isRecordingSearch.value) {
    isRecordingSearch.value = false;
    resumeDispatcher();
  }
};

// Record Key Filter
const toggleSearchRecord = () => {
  isRecordingSearch.value = !isRecordingSearch.value;
  if (isRecordingSearch.value) {
    pauseDispatcher();
    searchQuery.value = '';
    isSearchFromRecord.value = false;
    nextTick(() => searchInputRef.value?.focus());
  } else {
    resumeDispatcher();
  }
};

const handleSearchKeydown = (event: KeyboardEvent) => {
  if (isImeComposing(event)) return;

  if (isRecordingSearch.value) {
    if (event.key === 'Escape') {
      event.preventDefault();
      isRecordingSearch.value = false;
      resumeDispatcher();
      return;
    }

    event.preventDefault();
    const parsed = eventToChord(event);
    if (parsed && !parsed.isModifierOnly) {
      try {
        const normalized = normalizeKey(parsed.chord);
        searchQuery.value = normalized;
        isSearchFromRecord.value = true;
        isRecordingSearch.value = false;
        resumeDispatcher();
      } catch {
        // Ignored
      }
    }
  } else {
    if (event.key === 'Escape') {
      clearSearch();
    }
  }
};

// Row Actions
const openEditModal = (row: ResolvedKeybinding) => {
  if (row.locked) return;
  modalTarget.value = row;
  modalIsNew.value = false;
  showEditModal.value = true;
};

const openAddModal = (row: ResolvedKeybinding) => {
  if (row.locked) return;
  modalTarget.value = row;
  modalIsNew.value = true;
  showEditModal.value = true;
};

const onModalSave = async (payload: {
  commandId: string;
  newKey: string;
  when: KeyScope;
  oldKey?: string;
  oldWhen?: KeyScope;
}) => {
  try {
    await setBinding(
      payload.commandId,
      payload.newKey,
      payload.when,
      payload.oldKey,
      payload.oldWhen
    );
    showEditModal.value = false;
    highlightRow(payload.commandId);
    emit('toast', { message: 'Đã lưu phím tắt thành công', type: 'success' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Lưu phím tắt thất bại', type: 'error' });
  }
};

const onModalRemove = async (payload: {
  commandId: string;
  key: string;
  when?: KeyScope;
}) => {
  try {
    await removeBinding(payload.commandId, payload.key, payload.when);
    showEditModal.value = false;
    highlightRow(payload.commandId);
    emit('toast', { message: 'Đã gỡ phím tắt', type: 'info' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Gỡ phím tắt thất bại', type: 'error' });
  }
};

const handleRevertRow = async (row: ResolvedKeybinding) => {
  try {
    await resetBinding(row.commandId, row.key);
    highlightRow(row.commandId);
    emit('toast', { message: `Đã khôi phục mặc định cho "${row.title}"`, type: 'success' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Khôi phục thất bại', type: 'error' });
  }
};

const handleRemoveRow = async (row: ResolvedKeybinding) => {
  try {
    await removeBinding(row.commandId, row.key, row.when);
    highlightRow(row.commandId);
    emit('toast', { message: `Đã gỡ phím cho "${row.title}"`, type: 'info' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Gỡ phím thất bại', type: 'error' });
  }
};

const highlightRow = (cmdId: string) => {
  highlightedCommandId.value = cmdId;
  nextTick(() => {
    const el = document.getElementById(`row-${cmdId}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  });
  setTimeout(() => {
    if (highlightedCommandId.value === cmdId) {
      highlightedCommandId.value = null;
    }
  }, 1500);
};

// Reset All
const promptResetAll = () => {
  showMoreMenu.value = false;
  showResetConfirm.value = true;
};

const onResetAllConfirm = async () => {
  showResetConfirm.value = false;
  try {
    await resetAll(true);
    emit('toast', { message: 'Đã khôi phục toàn bộ phím tắt về mặc định', type: 'success' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Khôi phục mặc định thất bại', type: 'error' });
  }
};

// Export JSON
const handleExportJson = async () => {
  showMoreMenu.value = false;
  try {
    const dest = await exportJson();
    emit('toast', { message: `Đã xuất phím tắt: ${dest}`, type: 'success' });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Xuất phím tắt thất bại', type: 'error' });
  }
};

// Import JSON
const triggerImportFile = () => {
  showMoreMenu.value = false;
  importFileInput.value?.click();
};

const handleImportFileSelected = async (e: Event) => {
  const target = e.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  try {
    const text = await file.text();
    const result = await parseJson(text);
    const validEntries: KeybindingOverride[] = [];
    const extraSkipped: SkippedEntry[] = [];

    result.entries.forEach((entry, idx) => {
      const cmdId = entry.command.startsWith('-') ? entry.command.slice(1) : entry.command;
      if (!COMMAND_MAP.has(cmdId)) {
        extraSkipped.push({
          index: idx,
          reason: `Lệnh "${entry.command}" không tồn tại trong hệ thống`,
        });
      } else {
        validEntries.push(entry);
      }
    });

    importParsed.value = {
      entries: validEntries,
      skipped: [...result.skipped, ...extraSkipped],
    };
    showImportPreview.value = true;
  } catch (err: any) {
    emit('toast', { message: 'Định dạng JSON không hợp lệ: ' + err?.message, type: 'error' });
  } finally {
    target.value = '';
  }
};

const applyImport = async () => {
  if (!importParsed.value) return;
  try {
    await importOverrides(importParsed.value.entries);
    showImportPreview.value = false;
    emit('toast', {
      message: `Đã áp dụng thành công ${importParsed.value.entries.length} phím tắt`,
      type: 'success',
    });
  } catch (err: any) {
    emit('toast', { message: err?.message || 'Áp dụng phím tắt thất bại', type: 'error' });
  }
};

const getSourceLabel = (src: string) => {
  switch (src) {
    case 'default':
      return 'Mặc định';
    case 'user':
      return 'Người dùng';
    case 'removed':
      return 'Đã gỡ';
    case 'fixed':
      return 'Cố định 🔒';
    default:
      return src;
  }
};
</script>

<style scoped>
.keybindings-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  position: relative;
}

.hidden-file-input {
  display: none;
}

/* Toolbar */
.shortcuts-toolbar {
  padding: 10px 24px;
  background: var(--surface-container-low);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
  flex-wrap: wrap;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 320px;
  max-width: 620px;
}

.search-box {
  flex: 1;
  height: 32px;
  background: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  display: flex;
  align-items: center;
  padding: 0 10px;
  gap: 8px;
  transition: all 0.15s ease;
}

.search-box:focus-within {
  border-color: var(--primary);
}

.search-box.is-recording {
  border-color: var(--primary-accent);
  box-shadow: 0 0 12px var(--primary-accent-alpha-50);
}

.search-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: 12px;
  outline: none;
  padding: 0;
}

.search-input::placeholder {
  color: var(--text-muted);
}

.search-hint {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
  background: var(--bg-surface);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid var(--border-subtle);
  white-space: nowrap;
}

.clear-search-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 2px;
}

.clear-search-btn:hover {
  color: var(--text-inverse);
}

.btn-record-key {
  height: 32px;
  padding: 0 10px;
  border-radius: 6px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s ease;
}

.btn-record-key:hover {
  border-color: var(--primary-alpha-60);
  color: var(--text-inverse);
}

.btn-record-key.active {
  background: var(--primary-alpha-25);
  border-color: var(--primary-accent);
  color: var(--primary-accent);
}

.scope-dropdown-wrapper {
  position: relative;
}

.scope-filter-select {
  height: 32px;
  padding: 0 10px;
  border-radius: 6px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
  outline: none;
}

.scope-filter-select:focus {
  border-color: var(--primary);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.filter-chips-group {
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--bg-terminal);
  padding: 2px;
  border-radius: 6px;
  border: 1px solid var(--border-subtle);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
}

.filter-chip {
  padding: 4px 10px;
  border-radius: 4px;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.filter-chip:hover {
  color: var(--text-inverse);
  background: var(--bg-surface);
}

.filter-chip.active {
  background: var(--bg-surface);
  color: var(--text-primary);
  border-color: var(--border-subtle);
  font-weight: 600;
}

.chip-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.dot-primary {
  background: var(--primary-accent);
}

.dot-secondary {
  background: var(--status-running);
}

.dot-warning {
  background: var(--status-starting);
}

.dot-warning.pulse {
  animation: pulseWarning 1.5s infinite;
}

@keyframes pulseWarning {
  0%, 100% {
    transform: scale(1);
    opacity: 1;
  }
  50% {
    transform: scale(1.3);
    opacity: 0.5;
  }
}

.chip-conflict.has-conflicts {
  color: var(--status-starting);
}

.toolbar-divider {
  width: 1px;
  height: 16px;
  background: var(--border-subtle);
}

/* More Menu */
.dropdown-container {
  position: relative;
}

.btn-more-actions {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-more-actions:hover {
  color: var(--text-inverse);
  border-color: var(--primary-alpha-60);
}

.dropdown-menu {
  position: absolute;
  right: 0;
  top: calc(100% + 4px);
  width: 240px;
  background: var(--surface-container-low);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.6);
  padding: 4px;
  z-index: 50;
}

.dropdown-item {
  width: 100%;
  text-align: left;
  padding: 8px 10px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.dropdown-item:hover {
  background: var(--bg-surface-hover);
  color: var(--text-inverse);
}

.dropdown-item.item-danger {
  color: var(--status-failed);
}

.dropdown-item.item-danger:hover {
  background: var(--status-failed-bg);
}

.dropdown-sep {
  height: 1px;
  background: var(--border-subtle);
  margin: 4px 0;
}

/* Conflict Alert Banner */
.conflict-alert-banner {
  margin: 12px 24px 0 24px;
  padding: 10px 14px;
  background: var(--status-starting-bg);
  border: 1px solid var(--status-starting);
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.alert-content {
  display: flex;
  align-items: center;
  gap: 10px;
}

.alert-text {
  display: flex;
  flex-direction: column;
}

.alert-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--status-starting);
}

.alert-desc {
  font-size: 11px;
  color: var(--text-muted);
}

.btn-resolve-conflicts {
  padding: 4px 10px;
  border-radius: 4px;
  background: var(--status-starting-bg);
  border: 1px solid var(--status-starting);
  color: var(--status-starting);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s ease;
}

.btn-resolve-conflicts:hover {
  background: var(--status-starting);
  color: var(--text-inverse);
}

/* Table */
.table-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  text-align: center;
}

.empty-icon {
  color: var(--text-muted);
  opacity: 0.4;
  margin-bottom: 12px;
}

.empty-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.empty-desc {
  font-size: 12px;
  color: var(--text-muted);
  max-width: 360px;
  margin-bottom: 16px;
}

.shortcuts-table {
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--bg-surface);
  overflow: hidden;
}

.table-header-row {
  display: grid;
  grid-template-columns: 4fr 3fr 2.5fr 2.5fr;
  padding: 8px 16px;
  background: var(--surface-container-low);
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  font-weight: 600;
  color: var(--text-muted);
  letter-spacing: 0.5px;
}

.category-group {
  border-bottom: 1px solid var(--border-subtle);
}

.category-group:last-child {
  border-bottom: none;
}

.category-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: var(--surface-container-low);
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
  user-select: none;
  transition: background 0.15s ease;
}

.category-header-bar:hover {
  background: var(--surface-container-high);
}

.cat-title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cat-arrow {
  color: var(--primary-accent);
  transition: transform 0.2s ease;
}

.cat-arrow.collapsed {
  transform: rotate(-90deg);
}

.cat-title {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  font-weight: 600;
  color: var(--primary-accent);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.cat-count-badge {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
  background: var(--bg-terminal);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid var(--border-subtle);
}

.table-data-row {
  display: grid;
  grid-template-columns: 4fr 3fr 2.5fr 2.5fr;
  align-items: center;
  padding: 9px 16px;
  border-bottom: 1px solid var(--border-subtle);
  font-size: 12px;
  transition: background 0.15s ease;
  position: relative;
}

.table-data-row:last-child {
  border-bottom: none;
}

.table-data-row:hover {
  background: var(--bg-surface-hover);
}

.table-data-row.is-highlighted {
  background: var(--primary-alpha-25) !important;
  box-shadow: inset 2px 0 0 var(--primary-accent);
}

.table-data-row.has-conflict {
  background: var(--status-starting-bg);
}

.td-action {
  display: flex;
  align-items: center;
  gap: 10px;
  overflow: hidden;
}

.row-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: transparent;
  flex-shrink: 0;
}

.row-status-dot.dot-custom {
  background: var(--status-running);
}

.row-status-dot.dot-removed {
  background: var(--status-failed);
}

.row-status-dot.dot-conflict {
  background: var(--status-starting);
}

.action-info {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.action-title-line {
  display: flex;
  align-items: center;
  gap: 6px;
}

.action-title {
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.action-title.line-through {
  text-decoration: line-through;
  color: var(--text-muted);
}

.conflict-icon {
  color: var(--status-starting);
  flex-shrink: 0;
}

.action-id-mono {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.td-key {
  display: flex;
  align-items: center;
  gap: 4px;
}

.removed-key-label {
  color: var(--text-muted);
  font-weight: bold;
}

.keycaps-display {
  display: flex;
  align-items: center;
  gap: 4px;
}

.keycap {
  background: linear-gradient(180deg, var(--surface-container-high) 0%, var(--surface-container-low) 100%);
  box-shadow: 0 2px 0 0 var(--surface-container-lowest), 0 0 0 1px var(--outline), inset 0 1px 0 rgba(255, 255, 255, 0.15);
  border-radius: 4px;
  padding: 2px 7px;
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  font-weight: 600;
  color: var(--text-inverse);
}

.keycap-plus {
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  font-weight: bold;
}

.td-scope {
  display: flex;
  align-items: center;
}

.scope-pill {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--bg-terminal);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
}

.td-source {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.source-badge {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
}

.source-badge.src-user {
  color: var(--status-running);
  font-weight: 600;
}

.source-badge.src-removed {
  color: var(--status-failed);
}

.source-badge.src-fixed {
  color: var(--text-secondary);
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.table-data-row:hover .row-actions {
  opacity: 1;
}

.row-action-btn {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s ease;
}

.row-action-btn:hover {
  background: var(--bg-surface);
  color: var(--text-inverse);
}

.row-action-btn.add-btn:hover {
  color: var(--primary-accent);
}

.row-action-btn.edit-btn:hover {
  color: var(--primary-accent);
}

.row-action-btn.revert-btn:hover {
  color: var(--status-running);
}

.row-action-btn.remove-btn:hover {
  color: var(--status-failed);
}

/* Footer Telemetry */
.tab-telemetry-bar {
  padding: 8px 24px;
  background: var(--surface-container-low);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
  flex-shrink: 0;
}

.info-pill {
  color: var(--text-secondary);
}

.telemetry-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Import Preview Modal */
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background-color: rgba(11, 13, 19, 0.78);
  backdrop-filter: blur(8px);
}

.import-preview-container {
  width: 100%;
  max-width: 520px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.7);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.preview-body {
  padding: 16px 20px;
  max-height: 400px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.skipped-warning {
  padding: 10px 12px;
  border-radius: 6px;
  background: var(--status-starting-bg);
  border: 1px solid var(--status-starting);
  display: flex;
  gap: 10px;
  font-size: 11px;
  color: var(--text-primary);
}

.skipped-list {
  margin: 4px 0 0 0;
  padding-left: 16px;
}

.entries-header {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  margin-bottom: 6px;
}

.import-entries-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.import-entry-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: 4px;
  background: var(--bg-terminal);
  border: 1px solid var(--border-subtle);
  font-size: 11px;
}

.entry-cmd {
  font-family: var(--font-mono, monospace);
  color: var(--text-primary);
}

.entry-cmd.is-removal {
  color: var(--status-failed);
}

.entry-when {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-muted);
}

.modal-header {
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.header-title-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.modal-subtitle {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 2px;
}

.close-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
}

.close-btn:hover {
  color: var(--text-inverse);
}

.modal-footer {
  padding: 12px 20px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 6px 14px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid transparent;
}

.btn-sm {
  height: 30px;
}

.btn-secondary {
  background: transparent;
  border-color: var(--border-subtle);
  color: var(--text-secondary);
}

.btn-secondary:hover {
  background: var(--bg-surface-hover);
  color: var(--text-inverse);
}

.btn-primary {
  background: var(--primary);
  color: var(--text-inverse);
}

.btn-primary:hover {
  background: var(--primary-hover);
}
</style>
