<template>
  <div
    ref="rootEl"
    class="files-pane"
    :class="{ 'is-drop-target': isDropTarget && !dropTargetDir, 'drop-invalid': dropInvalid }"
    :data-files-pane-id="paneId"
    tabindex="0"
    @keydown="handleKeydown"
    @contextmenu.prevent="openContextMenu($event, null)"
  >
    <!-- Toolbar -->
    <div class="files-toolbar">
      <button type="button" class="ft-btn" :title="'Lên thư mục cha ' + keyLabel('files.up')" :disabled="!parent" @click="goUp">
        <ArrowUp :size="13" />
      </button>
      <button type="button" class="ft-btn" :title="'Làm mới ' + keyLabel('files.refresh')" @click="reload">
        <RefreshCw :size="13" :class="{ spin: loading }" />
      </button>
      <button type="button" class="ft-btn" :title="'Thư mục mới ' + keyLabel('files.newFolder')" @click="promptNewFolder">
        <FolderPlus :size="13" />
      </button>

      <!-- Breadcrumb; click empty space to type a path -->
      <div v-if="!editingPath" class="ft-breadcrumb" @click.self="beginEditPath">
        <span class="ft-location" :style="{ borderColor: accent }">
          <component :is="location.kind === 'ssh' ? Globe : HardDrive" :size="11" />
          {{ locationLabel }}
        </span>
        <template v-for="(crumb, index) in crumbs" :key="crumb.path">
          <ChevronRight v-if="index > 0" :size="11" class="ft-crumb-sep" />
          <button
            type="button"
            class="ft-crumb"
            :data-drop-dir="crumb.path"
            @click="navigate(crumb.path)"
          >
            {{ crumb.label }}
          </button>
        </template>
      </div>
      <input
        v-else
        ref="pathInput"
        v-model="pathDraft"
        class="ft-path-input font-mono"
        spellcheck="false"
        @keydown.enter.prevent="commitPath"
        @keydown.esc.prevent="editingPath = false"
        @blur="editingPath = false"
      />

      <button
        type="button"
        class="ft-btn"
        :class="{ active: showHidden }"
        :title="(showHidden ? 'Ẩn tệp ẩn ' : 'Hiện tệp ẩn ') + keyLabel('files.toggleHidden')"
        @click="toggleHidden"
      >
        <Eye v-if="showHidden" :size="13" />
        <EyeOff v-else :size="13" />
      </button>
    </div>

    <!-- Column header -->
    <div class="files-header">
      <button type="button" class="fh-col fh-name" @click="setSort('name')">
        Tên <SortIcon v-if="sortKey === 'name'" :desc="sortDesc" />
      </button>
      <button type="button" class="fh-col fh-size" @click="setSort('size')">
        Kích thước <SortIcon v-if="sortKey === 'size'" :desc="sortDesc" />
      </button>
      <button type="button" class="fh-col fh-mtime" @click="setSort('mtime')">
        Sửa đổi <SortIcon v-if="sortKey === 'mtime'" :desc="sortDesc" />
      </button>
      <span v-if="location.kind === 'ssh'" class="fh-col fh-mode">Quyền</span>
    </div>

    <!-- Listing -->
    <div class="files-list custom-scrollbar" @pointerdown.self="clearSelection" @click.self="clearSelection">
      <div v-if="error" class="files-state files-error">
        <AlertTriangle :size="16" />
        <span>{{ error }}</span>
        <button type="button" class="btn btn-secondary btn-sm" @click="reload">Thử lại</button>
      </div>
      <div v-else-if="loading && entries.length === 0" class="files-state">
        <LoaderCircle :size="18" class="spin" />
        <span>Đang tải…</span>
      </div>
      <div v-else-if="sorted.length === 0" class="files-state">
        <span>Thư mục trống</span>
      </div>

      <div
        v-for="(entry, index) in sorted"
        :key="entry.path"
        class="files-row"
        :class="{
          selected: selected.has(entry.path),
          'drop-hover': dropTargetDir === entry.path && !dropInvalid,
          hidden: entry.hidden,
        }"
        :data-drop-dir="isDirLike(entry) ? entry.path : undefined"
        @pointerdown="handleRowPointerDown($event, entry, index)"
        @dblclick="openEntry(entry)"
        @contextmenu.prevent.stop="openContextMenu($event, entry)"
      >
        <span class="fr-name">
          <Folder v-if="entry.kind === 'dir'" :size="14" class="icon-dir" />
          <FolderSymlink v-else-if="entry.kind === 'symlink' && entry.link_is_dir" :size="14" class="icon-dir" />
          <FileSymlink v-else-if="entry.kind === 'symlink'" :size="14" class="icon-file" />
          <FileIcon v-else :size="14" class="icon-file" />
          <input
            v-if="renamingPath === entry.path"
            ref="renameInput"
            v-model="renameDraft"
            class="fr-rename-input"
            spellcheck="false"
            @pointerdown.stop
            @dblclick.stop
            @keydown.enter.prevent.stop="commitRename(entry)"
            @keydown.esc.prevent.stop="renamingPath = null"
            @blur="commitRename(entry)"
          />
          <span v-else class="fr-name-text" :title="entry.link_target ? `${entry.name} → ${entry.link_target}` : entry.name">
            {{ entry.name }}<span v-if="entry.link_target" class="fr-link"> → {{ entry.link_target }}</span>
          </span>
        </span>
        <span class="fr-size">{{ isDirLike(entry) ? '—' : formatBytes(entry.size) }}</span>
        <span class="fr-mtime">{{ formatTime(entry.mtime) }}</span>
        <span v-if="location.kind === 'ssh'" class="fr-mode font-mono">{{ formatMode(entry) }}</span>
      </div>
    </div>

    <!-- Footer -->
    <div class="files-footer">
      <span>{{ sorted.length }} mục</span>
      <span v-if="selected.size > 0">· {{ selected.size }} đã chọn ({{ formatBytes(selectedBytes) }})</span>
      <!-- A native (HTML5) drag, so the OS file manager receives real files. -->
      <span
        v-if="location.kind === 'local' && selected.size > 0"
        class="ff-drag-out"
        draggable="true"
        title="Kéo vào trình quản lý tệp hoặc Desktop để chép ra ngoài"
        @dragstart="handleDragOut"
      >
        <GripVertical :size="11" /> Kéo {{ selected.size }} mục ra ngoài
      </span>
      <span class="ff-spacer" />
      <span v-if="isDropTarget && dropInvalid" class="ff-drop-warn">{{ dropInvalid }}</span>
      <span v-else-if="isDropTarget" class="ff-drop-hint">Thả vào {{ dropTargetDir || path }}</span>
      <span v-else class="ff-hint">Kéo sang Files pane khác để chép · giữ Shift để di chuyển</span>
    </div>

    <!-- App context menu (the webview's own menu is blocked app-wide) -->
    <div
      v-if="menu"
      class="files-menu"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @pointerdown.stop
      @contextmenu.prevent.stop
    >
      <template v-if="menu.entry">
        <button v-if="isDirLike(menu.entry)" type="button" class="fm-item" @click="runMenu(() => openEntry(menu!.entry!))">
          <FolderOpen :size="12" /> Mở
        </button>
        <button
          v-else
          type="button"
          class="fm-item"
          @click="runMenu(() => openEntry(menu!.entry!))"
        >
          <ExternalLink :size="12" />
          {{ location.kind === 'local' ? 'Mở bằng ứng dụng mặc định' : 'Tải bản sao về & mở' }}
        </button>
        <button
          v-if="location.kind === 'local' && isDirLike(menu.entry)"
          type="button"
          class="fm-item"
          @click="runMenu(() => openAgentStudio(menu!.entry!.path))"
        >
          <Bot :size="12" /> Mở Agent Studio tại đây
        </button>
        <button type="button" class="fm-item" @click="runMenu(() => beginRename(menu!.entry!))">
          <Pencil :size="12" /> Đổi tên <kbd>{{ keyLabel('files.rename', false) }}</kbd>
        </button>
        <button type="button" class="fm-item" @click="runMenu(() => copyPaths())">
          <Copy :size="12" /> Sao chép đường dẫn <kbd>{{ keyLabel('files.copyPath', false) }}</kbd>
        </button>
        <div class="fm-divider" />
        <button type="button" class="fm-item fm-danger" @click="runMenu(() => askDelete())">
          <Trash2 :size="12" /> Xóa {{ selected.size > 1 ? `${selected.size} mục` : '' }} <kbd>{{ keyLabel('files.delete', false) }}</kbd>
        </button>
      </template>
      <template v-else>
        <button type="button" class="fm-item" @click="runMenu(promptNewFolder)">
          <FolderPlus :size="12" /> Thư mục mới
        </button>
        <button type="button" class="fm-item" @click="runMenu(reload)">
          <RefreshCw :size="12" /> Làm mới
        </button>
        <button type="button" class="fm-item" @click="runMenu(toggleHidden)">
          <Eye :size="12" /> {{ showHidden ? 'Ẩn tệp ẩn' : 'Hiện tệp ẩn' }}
        </button>
        <button v-if="location.kind === 'local'" type="button" class="fm-item" @click="runMenu(() => openAgentStudio(path))">
          <Bot :size="12" /> Mở Agent Studio tại đây
        </button>
      </template>
    </div>

    <PromptDialog
      :visible="newFolderVisible"
      title="Thư Mục Mới"
      label="Tên thư mục"
      default-value=""
      confirm-text="Tạo"
      icon="edit"
      @confirm="createFolder"
      @cancel="newFolderVisible = false"
    />
    <ConfirmDialog
      :visible="deleteTargets.length > 0"
      title="Xóa Tệp"
      :message="deleteMessage"
      :confirm-text="`Xóa ${deleteTargets.length > 1 ? deleteTargets.length + ' mục' : ''}`"
      danger
      @confirm="confirmDelete"
      @cancel="deleteTargets = []"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import {
  AlertTriangle,
  ArrowDown,
  Bot,
  ArrowUp,
  ChevronRight,
  Copy,
  ExternalLink,
  Eye,
  EyeOff,
  File as FileIcon,
  FileSymlink,
  Folder,
  FolderOpen,
  FolderPlus,
  FolderSymlink,
  Globe,
  GripVertical,
  HardDrive,
  LoaderCircle,
  Pencil,
  RefreshCw,
  Trash2,
} from 'lucide-vue-next';
import { ipcClient } from '@/ipc/client';
import { useRouter } from 'vue-router';
import type { FileLocation, FsEntry } from '@/types/files';
import { formatBytes, sameLocation } from '@/types/files';
import { useFileDrag } from '@/composables/useFileDrag';
import { useTransfers } from '@/composables/useTransfers';
import PromptDialog from '@/components/dialogs/PromptDialog.vue';
import { keyLabel, useKeyCommand } from '@/keybindings/dispatcher';
import ConfirmDialog from '@/components/dialogs/ConfirmDialog.vue';

const props = defineProps<{
  paneId: string;
  location: FileLocation;
  initialPath?: string;
  /** Host name or "Máy này". */
  locationLabel: string;
  accent?: string | null;
}>();

const emit = defineEmits<{
  (e: 'path-change', path: string): void;
}>();

const SortIcon = defineComponent({
  props: { desc: Boolean },
  setup: (p) => () => h(p.desc ? ArrowDown : ArrowUp, { size: 10 }),
});

const { drag, osDrag, registerPane, startDrag } = useFileDrag();
const { finishedTick, lastFinished, notify } = useTransfers();

const rootEl = ref<HTMLElement | null>(null);
const pathInput = ref<HTMLInputElement | null>(null);
const renameInput = ref<HTMLInputElement[] | HTMLInputElement | null>(null);

const path = ref(props.initialPath ?? '');
const parent = ref<string | null>(null);
const entries = ref<FsEntry[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const showHidden = ref(false);
const selected = ref(new Set<string>());
const anchorIndex = ref<number | null>(null);
const sortKey = ref<'name' | 'size' | 'mtime'>('name');
const sortDesc = ref(false);
const editingPath = ref(false);
const pathDraft = ref('');
const renamingPath = ref<string | null>(null);
const renameDraft = ref('');
const newFolderVisible = ref(false);
const deleteTargets = ref<FsEntry[]>([]);
const menu = ref<{ x: number; y: number; entry: FsEntry | null } | null>(null);

const accent = computed(() => props.accent || '#744791');
const isWindowsLocal = computed(
  () => props.location.kind === 'local' && navigator.userAgent.includes('Windows')
);

const isDirLike = (entry: FsEntry) => entry.kind === 'dir' || entry.link_is_dir;

const sorted = computed(() => {
  const list = [...entries.value];
  if (sortKey.value !== 'name') {
    const key = sortKey.value;
    list.sort((a, b) => {
      const dirOrder = Number(isDirLike(b)) - Number(isDirLike(a));
      if (dirOrder !== 0) return dirOrder;
      const va = key === 'size' ? a.size : a.mtime ?? 0;
      const vb = key === 'size' ? b.size : b.mtime ?? 0;
      return va - vb;
    });
  }
  // The backend already sorts by name (folders first, natural order).
  if (sortDesc.value) {
    const dirs = list.filter(isDirLike).reverse();
    const files = list.filter((entry) => !isDirLike(entry)).reverse();
    return [...dirs, ...files];
  }
  return list;
});

/** `file://` URI with every path segment percent-encoded. */
const fileUri = (path: string) => {
  const slashed = path.replace(/\\/g, '/');
  const encoded = slashed.split('/').map((segment, index) =>
    // Keep a Windows drive ("C:") as it is.
    index === 0 && /^[A-Za-z]:$/.test(segment) ? segment : encodeURIComponent(segment)
  );
  const joined = encoded.join('/');
  return joined.startsWith('/') ? `file://${joined}` : `file:///${joined}`;
};

const handleDragOut = (event: DragEvent) => {
  const paths = selectedEntries.value.map((entry) => entry.path);
  if (!event.dataTransfer || paths.length === 0) {
    event.preventDefault();
    return;
  }
  event.dataTransfer.effectAllowed = 'copy';
  event.dataTransfer.setData('text/uri-list', paths.map(fileUri).join('\r\n'));
  event.dataTransfer.setData('text/plain', paths.join('\n'));
};

const selectedEntries = computed(() => sorted.value.filter((entry) => selected.value.has(entry.path)));
const selectedBytes = computed(() =>
  selectedEntries.value.reduce((sum, entry) => sum + (isDirLike(entry) ? 0 : entry.size), 0)
);

const crumbs = computed(() => {
  const full = path.value;
  if (!full) return [];
  if (isWindowsLocal.value) {
    const parts = full.split('\\').filter(Boolean);
    return parts.map((part, index) => ({
      label: part,
      path: parts.slice(0, index + 1).join('\\') + (index === 0 ? '\\' : ''),
    }));
  }
  const parts = full.split('/').filter(Boolean);
  return [
    { label: '/', path: '/' },
    ...parts.map((part, index) => ({ label: part, path: '/' + parts.slice(0, index + 1).join('/') })),
  ];
});

const load = async (target: string) => {
  loading.value = true;
  error.value = null;
  try {
    const listing = await ipcClient.fsList(props.location, target, showHidden.value);
    const changed = listing.path !== path.value;
    path.value = listing.path;
    parent.value = listing.parent ?? null;
    entries.value = listing.entries;
    if (changed) {
      selected.value = new Set();
      anchorIndex.value = null;
      emit('path-change', listing.path);
    } else {
      // Keep the selection that still exists after a refresh.
      const present = new Set(listing.entries.map((entry) => entry.path));
      selected.value = new Set([...selected.value].filter((p) => present.has(p)));
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
};

const navigate = (target: string) => void load(target);
const reload = () => void load(path.value);
const goUp = () => {
  if (parent.value) navigate(parent.value);
};
const toggleHidden = () => {
  showHidden.value = !showHidden.value;
  reload();
};

const setSort = (key: 'name' | 'size' | 'mtime') => {
  if (sortKey.value === key) sortDesc.value = !sortDesc.value;
  else {
    sortKey.value = key;
    sortDesc.value = false;
  }
};

const beginEditPath = () => {
  pathDraft.value = path.value;
  editingPath.value = true;
  void nextTick(() => pathInput.value?.select());
};

const commitPath = () => {
  editingPath.value = false;
  const target = pathDraft.value.trim();
  if (target && target !== path.value) navigate(target);
};

const clearSelection = () => {
  selected.value = new Set();
  anchorIndex.value = null;
};

const handleRowPointerDown = (event: PointerEvent, entry: FsEntry, index: number) => {
  rootEl.value?.focus({ preventScroll: true });
  menu.value = null;
  if (event.button !== 0) return;
  const next = new Set(selected.value);
  if (event.shiftKey && anchorIndex.value !== null) {
    const [from, to] = [anchorIndex.value, index].sort((a, b) => a - b);
    if (!event.ctrlKey && !event.metaKey) next.clear();
    for (let i = from; i <= to; i += 1) next.add(sorted.value[i].path);
  } else if (event.ctrlKey || event.metaKey) {
    if (next.has(entry.path)) next.delete(entry.path);
    else next.add(entry.path);
    anchorIndex.value = index;
  } else if (!next.has(entry.path)) {
    next.clear();
    next.add(entry.path);
    anchorIndex.value = index;
  }
  selected.value = next;
  if (event.shiftKey || event.ctrlKey || event.metaKey || renamingPath.value) return;
  startDrag(
    event,
    props.location,
    selectedEntries.value.map((item) => ({
      path: item.path,
      name: item.name,
      isDir: isDirLike(item),
      size: item.size,
    }))
  );
};

const openEntry = (entry: FsEntry) => {
  if (isDirLike(entry)) {
    navigate(entry.path);
  } else if (props.location.kind === 'local') {
    void ipcClient.fsOpenLocal(entry.path).catch((e) => notify(String(e), 'error'));
  } else {
    notify(`Đang tải ${entry.name}…`);
    void ipcClient
      .fsOpenRemote(props.location, entry.path)
      .then(() =>
        notify(`Đã mở ${entry.name}; mỗi lần lưu sẽ tự tải lên máy chủ (theo dõi trong ⇅).`)
      )
      .catch((e) => notify(e instanceof Error ? e.message : String(e), 'error'));
  }
};

const beginRename = (entry: FsEntry) => {
  renamingPath.value = entry.path;
  renameDraft.value = entry.name;
  void nextTick(() => {
    const input = Array.isArray(renameInput.value) ? renameInput.value[0] : renameInput.value;
    if (!input) return;
    input.focus();
    const dot = entry.name.lastIndexOf('.');
    input.setSelectionRange(0, dot > 0 && !isDirLike(entry) ? dot : entry.name.length);
  });
};

const commitRename = async (entry: FsEntry) => {
  if (renamingPath.value !== entry.path) return;
  renamingPath.value = null;
  const name = renameDraft.value.trim();
  if (!name || name === entry.name) return;
  try {
    const newPath = await ipcClient.fsRename(props.location, entry.path, name);
    await load(path.value);
    selected.value = new Set([newPath]);
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), 'error');
  }
};

const promptNewFolder = () => {
  newFolderVisible.value = true;
};

const createFolder = async (name: string) => {
  newFolderVisible.value = false;
  if (!name.trim()) return;
  try {
    const created = await ipcClient.fsMkdir(props.location, path.value, name.trim());
    await load(path.value);
    selected.value = new Set([created]);
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), 'error');
  }
};

const askDelete = () => {
  if (selectedEntries.value.length > 0) deleteTargets.value = [...selectedEntries.value];
};

const deleteMessage = computed(() => {
  const targets = deleteTargets.value;
  if (targets.length === 0) return '';
  const dirs = targets.filter(isDirLike).length;
  const what = targets.length === 1 ? `"${targets[0].name}"` : `${targets.length} mục`;
  const where = props.location.kind === 'ssh' ? ` trên ${props.locationLabel}` : '';
  const recursive = dirs > 0 ? ' Thư mục sẽ bị xóa cùng toàn bộ nội dung bên trong.' : '';
  return `Xóa vĩnh viễn ${what}${where}? Không có thùng rác, không thể hoàn tác.${recursive}`;
});

const confirmDelete = async () => {
  const targets = deleteTargets.value;
  deleteTargets.value = [];
  try {
    await ipcClient.fsDelete(
      props.location,
      targets.map((entry) => entry.path)
    );
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), 'error');
  }
  await load(path.value);
};

const copyPaths = async () => {
  const text = selectedEntries.value.map((entry) => entry.path).join('\n');
  if (!text) return;
  try {
    await ipcClient.writeClipboardText(text);
  } catch (e) {
    notify(String(e), 'error');
  }
};

const openContextMenu = (event: MouseEvent, entry: FsEntry | null) => {
  if (entry && !selected.value.has(entry.path)) {
    selected.value = new Set([entry.path]);
  }
  const bounds = rootEl.value?.getBoundingClientRect();
  if (!bounds) return;
  menu.value = {
    x: Math.min(event.clientX - bounds.left, bounds.width - 200),
    y: Math.min(event.clientY - bounds.top, bounds.height - 180),
    entry,
  };
};

const router = useRouter();

/** SCR-09 on this folder (its git repo). */
const openAgentStudio = (dir: string) => {
  void router.push({ path: '/agents', query: { path: dir } });
};

const runMenu = (action: () => void) => {
  menu.value = null;
  action();
};

const closeMenu = () => {
  menu.value = null;
};

// Keys that are bindings live in the registry (scope `files`, editable in
// Settings → Phím tắt). Every pane registers them; only the focused one acts.
const isFocused = () => Boolean(rootEl.value?.contains(document.activeElement));
const onKey = (id: string, action: () => void) =>
  useKeyCommand(id, () => {
    if (isFocused()) action();
  });

onKey('files.openSelected', () => {
  if (selectedEntries.value.length === 1) openEntry(selectedEntries.value[0]);
});
onKey('files.up', goUp);
onKey('files.rename', () => {
  if (selectedEntries.value.length === 1) beginRename(selectedEntries.value[0]);
});
onKey('files.refresh', reload);
onKey('files.delete', askDelete);
onKey('files.selectAll', () => {
  selected.value = new Set(sorted.value.map((entry) => entry.path));
});
onKey('files.toggleHidden', toggleHidden);
onKey('files.newFolder', promptNewFolder);
onKey('files.copyPath', () => void copyPaths());

// Moving the selection and closing the menu are not rebindable.
const handleKeydown = (event: KeyboardEvent) => {
  const target = event.target as HTMLElement;
  if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || event.isComposing) return;
  if (event.key === 'Escape' && menu.value) {
    menu.value = null;
    event.preventDefault();
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    const list = sorted.value;
    if (list.length === 0) return;
    const current = anchorIndex.value ?? -1;
    const next = Math.max(0, Math.min(list.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)));
    anchorIndex.value = next;
    selected.value = new Set([list[next].path]);
    event.preventDefault();
  }
};

// Drop feedback from in-app drags and from the OS.
const activeTarget = computed(() => {
  const target = drag.value?.dragging ? drag.value.target : osDrag.value?.target ?? null;
  return target?.kind === 'files' && target.paneId === props.paneId ? target : null;
});
const isDropTarget = computed(() => activeTarget.value !== null);
const dropTargetDir = computed(() =>
  activeTarget.value && activeTarget.value.dir !== path.value ? activeTarget.value.dir : null
);
const dropInvalid = computed(() => {
  if (!isDropTarget.value) return null;
  return drag.value?.dragging ? drag.value.invalid : osDrag.value?.invalid ?? null;
});

// Refresh when a transfer into (or out of) this folder finishes.
watch(finishedTick, () => {
  const job = lastFinished.value;
  if (!job) return;
  const touchesDest = sameLocation(job.dest.location, props.location) && job.dest.path === path.value;
  const touchesSource =
    job.mode === 'move' &&
    job.sources.some(
      (source) => sameLocation(source.location, props.location) && source.path.startsWith(path.value)
    );
  if (touchesDest || touchesSource) reload();
});

let unregister: (() => void) | null = null;

onMounted(async () => {
  unregister = registerPane(props.paneId, {
    location: () => props.location,
    currentDir: () => path.value,
  });
  window.addEventListener('pointerdown', closeMenu);
  // A host's default folder may be "~/app" or relative: resolve it
  // against the home folder.
  let start = props.initialPath?.trim();
  const absolute = (p: string) => p.startsWith('/') || /^[A-Za-z]:[\\/]/.test(p);
  if (!start || !absolute(start)) {
    try {
      const home = await ipcClient.fsHome(props.location);
      const sep = props.location.kind === 'ssh' || home.startsWith('/') ? '/' : '\\';
      const rest = (start ?? '').replace(/^~[\\/]?/, '');
      start = rest ? `${home.replace(/[\\/]$/, '')}${sep}${rest}` : home;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      return;
    }
  }
  await load(start);
});

onUnmounted(() => {
  unregister?.();
  window.removeEventListener('pointerdown', closeMenu);
});

const formatTime = (secs?: number | null) => {
  if (!secs) return '—';
  const date = new Date(secs * 1000);
  const today = new Date();
  const sameDay = date.toDateString() === today.toDateString();
  return sameDay
    ? date.toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' })
    : date.toLocaleDateString('vi-VN', { day: '2-digit', month: '2-digit', year: 'numeric' });
};

const formatMode = (entry: FsEntry) => {
  if (entry.mode == null) return '';
  const type = entry.kind === 'dir' ? 'd' : entry.kind === 'symlink' ? 'l' : '-';
  const bits = 'rwxrwxrwx';
  let out = type;
  for (let i = 0; i < 9; i += 1) {
    out += entry.mode & (1 << (8 - i)) ? bits[i] : '-';
  }
  return out;
};

defineExpose({ reload, focus: () => rootEl.value?.focus() });
</script>

<style scoped>
.files-pane {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--bg-app-base, #0c0e14);
  color: var(--text-primary);
  font-size: 12.5px;
  outline: none;
}

.files-pane.is-drop-target {
  box-shadow: inset 0 0 0 2px var(--primary, #744791);
}

.files-pane.drop-invalid {
  box-shadow: inset 0 0 0 2px var(--status-failed, #ef4444);
}

.files-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-surface);
  flex-shrink: 0;
}

.ft-btn {
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  flex-shrink: 0;
}

.ft-btn:hover:not(:disabled),
.ft-btn.active {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.ft-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.ft-breadcrumb {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  height: 26px;
  padding: 0 6px;
  overflow-x: auto;
  white-space: nowrap;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background-color: var(--bg-app-base);
  cursor: text;
  scrollbar-width: none;
}

.ft-location {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 6px;
  margin-right: 4px;
  border-left: 3px solid;
  border-radius: 3px;
  background-color: var(--bg-surface-hover);
  color: var(--text-secondary);
  font-size: 11px;
}

.ft-crumb {
  background: transparent;
  border: none;
  padding: 2px 4px;
  border-radius: 3px;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.ft-crumb:hover {
  background-color: var(--bg-surface-hover);
  color: var(--text-primary);
}

.ft-crumb-sep {
  color: var(--text-muted);
  flex-shrink: 0;
}

.ft-path-input {
  flex: 1;
  min-width: 0;
  height: 26px;
  padding: 0 8px;
  border: 1px solid var(--border-focus);
  border-radius: var(--radius-sm);
  background-color: var(--bg-app-base);
  color: var(--text-primary);
  font-size: 12px;
  outline: none;
}

.files-header,
.files-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 90px 110px;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
}

.files-pane:has(.fh-mode) .files-header,
.files-pane:has(.fh-mode) .files-row {
  grid-template-columns: minmax(0, 1fr) 90px 110px 90px;
}

.files-header {
  height: 26px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.fh-col {
  display: flex;
  align-items: center;
  gap: 3px;
  background: transparent;
  border: none;
  padding: 0;
  color: var(--text-muted);
  font-size: 10.5px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  cursor: pointer;
  text-align: left;
}

.fh-size,
.fh-mtime {
  justify-content: flex-end;
}

.files-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 0;
}

.files-row {
  height: 26px;
  cursor: default;
  user-select: none;
  border: 1px solid transparent;
}

.files-row:hover {
  background-color: var(--bg-surface-hover);
}

.files-row.selected {
  background-color: var(--primary-subtle, rgba(116, 71, 145, 0.25));
}

.files-row.drop-hover {
  border-color: var(--primary, #744791);
  background-color: var(--primary-subtle, rgba(116, 71, 145, 0.25));
}

.files-row.hidden {
  opacity: 0.6;
}

.fr-name {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.fr-name-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fr-link {
  color: var(--text-muted);
}

.icon-dir {
  color: #e4b5ff;
  flex-shrink: 0;
}

.icon-file {
  color: var(--text-muted);
  flex-shrink: 0;
}

.fr-rename-input {
  flex: 1;
  min-width: 0;
  height: 20px;
  padding: 0 4px;
  border: 1px solid var(--border-focus);
  border-radius: 3px;
  background-color: var(--bg-app-base);
  color: var(--text-primary);
  font-size: 12.5px;
  outline: none;
}

.fr-size,
.fr-mtime {
  text-align: right;
  color: var(--text-secondary);
  font-size: 11.5px;
  white-space: nowrap;
}

.fr-mode {
  color: var(--text-muted);
  font-size: 11px;
}

.files-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 32px 16px;
  color: var(--text-muted);
  text-align: center;
}

.files-error {
  color: var(--status-failed);
}

.files-footer {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  padding: 0 10px;
  border-top: 1px solid var(--border-subtle);
  background-color: var(--bg-surface);
  color: var(--text-muted);
  font-size: 11px;
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
}

.ff-spacer {
  flex: 1;
}

.ff-drag-out {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0 6px;
  border: 1px dashed var(--border-subtle);
  border-radius: 4px;
  color: var(--text-secondary, inherit);
  cursor: grab;
}

.ff-drag-out:hover {
  border-color: #e4b5ff;
  color: #e4b5ff;
}

.ff-drop-hint {
  color: #e4b5ff;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ff-drop-warn {
  color: var(--status-failed);
}

.files-menu {
  position: absolute;
  z-index: 50;
  min-width: 200px;
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 1px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.fm-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  padding: 0 8px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--text-primary);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

.fm-item:hover {
  background-color: var(--bg-surface-hover);
}

.fm-item kbd {
  margin-left: auto;
  font-size: 10px;
  color: var(--text-muted);
}

.fm-danger {
  color: var(--status-failed);
}

.fm-divider {
  height: 1px;
  margin: 3px 0;
  background-color: var(--border-subtle);
}

.spin {
  animation: files-spin 0.9s linear infinite;
}

@keyframes files-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
