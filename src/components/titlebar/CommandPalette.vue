<template>
  <div v-if="visible" class="modal-backdrop" @click.self="$emit('close')">
    <div class="palette-modal">
      <div class="palette-input-wrap">
        <Search class="palette-icon" :size="16" />
        <input
          ref="inputRef"
          v-model="query"
          class="palette-input"
          placeholder="Nhập tên lệnh, template, nhóm hoặc hành động... (Mũi tên ↑↓ để chọn)"
          @keydown.esc="$emit('close')"
          @keydown.down.prevent="moveSelection(1)"
          @keydown.up.prevent="moveSelection(-1)"
          @keydown.enter.prevent="executeSelected"
        />
        <kbd class="shortcut-tag">Esc</kbd>
      </div>

      <div class="palette-results">
        <div v-if="filteredItems.length === 0" class="no-results">
          Không tìm thấy thao tác nào khớp với "{{ query }}"
        </div>

        <div
          v-for="(item, index) in filteredItems"
          :key="item.id"
          class="palette-item"
          :class="{ active: index === selectedIndex }"
          @click="selectItem(item)"
          @mouseenter="selectedIndex = index"
        >
          <div class="item-icon-wrap" :class="item.category">
            <component :is="item.icon" :size="15" />
          </div>
          <div class="item-info">
            <div class="item-title">{{ item.title }}</div>
            <div class="item-subtitle">{{ item.subtitle }}</div>
          </div>
          <span class="category-badge">{{ item.categoryLabel }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from 'vue';
import { useRouter } from 'vue-router';
import { Search, Terminal, Folder, Settings, History, Play, Puzzle } from 'lucide-vue-next';
import { useCommands } from '@/composables/useCommands';
import { useGroups } from '@/composables/useGroups';
import { useTemplates } from '@/composables/useTemplates';
import { useRunSession } from '@/composables/useRunSession';
import { useSuggestions } from '@/composables/useSuggestions';

const props = defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const router = useRouter();
const inputRef = ref<HTMLInputElement | null>(null);
const query = ref('');
const selectedIndex = ref(0);

const { commands, fetchCommands } = useCommands();
const { groups, fetchGroups } = useGroups();
const { templates, fetchTemplates } = useTemplates();
const { startGroupSession } = useRunSession();
const { loadHistory, findSuggestions } = useSuggestions();

onMounted(async () => {
  await Promise.all([fetchCommands(), fetchGroups(), fetchTemplates(), loadHistory()]);
});

watch(() => props.visible, (val) => {
  if (val) {
    query.value = '';
    selectedIndex.value = 0;
    nextTick(() => inputRef.value?.focus());
  }
});

const paletteItems = computed(() => {
  const items: any[] = [
    {
      id: 'nav-workspace',
      title: 'Đi đến Workspace Terminal (Ctrl+1)',
      subtitle: 'Xem các tab PTY và bảng điều khiển phiên chạy',
      icon: Terminal,
      category: 'nav',
      categoryLabel: 'Chuyển màn',
      action: () => router.push('/workspace'),
    },
    {
      id: 'nav-commands',
      title: 'Mở Thư Viện Lệnh (Ctrl+2)',
      subtitle: 'Quản trị và định nghĩa các lệnh CLI',
      icon: Terminal,
      category: 'nav',
      categoryLabel: 'Chuyển màn',
      action: () => router.push('/commands'),
    },
    {
      id: 'nav-templates',
      title: 'Mở Thư Viện Mẫu Lệnh (Ctrl+6)',
      subtitle: 'Định nghĩa template {{param}} và preset tham số',
      icon: Puzzle,
      category: 'nav',
      categoryLabel: 'Chuyển màn',
      action: () => router.push('/templates'),
    },
    {
      id: 'nav-groups',
      title: 'Quản Lý Nhóm Lệnh & Sequencer (Ctrl+3)',
      subtitle: 'Sắp xếp thứ tự chạy và cờ autostart',
      icon: Folder,
      category: 'nav',
      categoryLabel: 'Chuyển màn',
      action: () => router.push('/groups'),
    },
    {
      id: 'nav-history',
      title: 'Lịch Sử Phiên Chạy (Ctrl+4)',
      subtitle: 'Xem lại các run_session và mã thoát exit code',
      icon: History,
      category: 'nav',
      categoryLabel: 'Chuyển màn',
      action: () => router.push('/history'),
    },
    {
      id: 'nav-settings',
      title: 'Cài Đặt & Sao Lưu DB (Ctrl+5)',
      subtitle: 'Xuất/nhập SQLite, cấu hình timeout dừng mềm',
      icon: Settings,
      category: 'nav',
      categoryLabel: 'Cài đặt',
      action: () => router.push('/settings'),
    },
  ];

  // Reuse the terminal suggestion engine so the palette can find recent
  // commands using the same ranking and privacy-filtered source.
  if (query.value.trim()) {
    findSuggestions(query.value).slice(0, 8).forEach((historyItem) => {
      items.push({
        id: `history-${historyItem.id}`,
        title: `Lịch sử: ${historyItem.command_line}`,
        subtitle: `${historyItem.shell_kind} · chạy ${historyItem.run_count} lần`,
        icon: History,
        category: 'history',
        categoryLabel: 'Lịch sử',
        action: () => {
          void navigator.clipboard?.writeText(historyItem.command_line);
          router.push('/history');
        },
      });
    });
  }

  // Thêm các Template để mở giao diện chạy trực tiếp
  templates.value.forEach(tpl => {
    items.push({
      id: `template-run-${tpl.id}`,
      title: `Thực thi Template: ${tpl.name}`,
      subtitle: `Cấu trúc: ${tpl.template_string}`,
      icon: Puzzle,
      category: 'template',
      categoryLabel: 'Template',
      action: () => {
        router.push({ path: '/templates', query: { openTemplateId: String(tpl.id) } });
      },
    });
  });

  // Thêm các nhóm lệnh để có thể Play trực tiếp
  groups.value.forEach(g => {
    items.push({
      id: `group-run-${g.id}`,
      title: `Chạy Nhóm: ${g.group_name}`,
      subtitle: `Khởi chạy ${g.commands.length} lệnh theo thứ tự tuần tự`,
      icon: Play,
      category: 'action',
      categoryLabel: 'Chạy nhóm',
      action: async () => {
        await startGroupSession(g.id, g.group_name, g.commands);
        router.push('/workspace');
      },
    });
  });

  return items;
});

const filteredItems = computed(() => {
  if (!query.value.trim()) return paletteItems.value;
  const q = query.value.toLowerCase();
  return paletteItems.value.filter(
    i => i.title.toLowerCase().includes(q) || i.subtitle.toLowerCase().includes(q)
  );
});

const moveSelection = (direction: number) => {
  const max = filteredItems.value.length;
  if (max === 0) return;
  selectedIndex.value = (selectedIndex.value + direction + max) % max;
};

const executeSelected = () => {
  if (filteredItems.value[selectedIndex.value]) {
    selectItem(filteredItems.value[selectedIndex.value]);
  }
};

const selectItem = (item: any) => {
  item.action();
  emit('close');
};
</script>

<style scoped>
.palette-modal {
  width: 90%;
  max-width: 580px;
  background-color: var(--bg-surface);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.6), 0 10px 10px -5px rgba(0, 0, 0, 0.4);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: scaleIn 0.15s ease-out;
}

.palette-input-wrap {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle);
  background-color: var(--bg-app-base);
  gap: 10px;
}

.palette-icon {
  color: var(--text-muted);
}

.palette-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: 14px;
  color: var(--text-primary);
  font-family: var(--font-sans);
}

.palette-input::placeholder {
  color: var(--text-muted);
  font-size: 13px;
}

.palette-results {
  max-height: 380px;
  overflow-y: auto;
  padding: 6px;
}

.palette-item {
  display: flex;
  align-items: center;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  cursor: pointer;
  gap: 12px;
  transition: all 0.1s ease;
}

.palette-item.active {
  background-color: var(--primary-subtle);
  border-left: 3px solid var(--primary);
}

.item-icon-wrap {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-surface-hover);
  color: var(--text-secondary);
}

.palette-item.active .item-icon-wrap {
  background: var(--primary);
  color: #ffffff;
}

.item-info {
  flex: 1;
}

.item-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.item-subtitle {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 1px;
}

.category-badge {
  font-size: 10px;
  color: var(--text-muted);
  background: var(--bg-app-base);
  border: 1px solid var(--border-subtle);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.no-results {
  padding: 24px;
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}
</style>
