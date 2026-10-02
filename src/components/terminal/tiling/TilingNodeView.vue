<template>
  <!-- LEAF NODE: Renders a single terminal tile -->
  <template v-if="node.type === 'leaf'">
    <div
      v-if="currentTab"
      class="tiling-leaf-wrapper"
    >
      <TilingTile
        :tab="currentTab"
        :is-active="activeTabId === currentTab.id"
        :is-zoomed="zoomedTabId === currentTab.id"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting="restartingCmdId === currentTab.commandId"
        @focus="$emit('focus', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @close="$emit('close-tile', $event)"
        @stop="$emit('stop-process', $event)"
        @restart="$emit('restart-process', $event)"
        @rename="$emit('rename-tab', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @data="(id, d) => $emit('data', id, d)"
      />
    </div>
    <div v-else class="tiling-empty-leaf">
      <span class="empty-msg">Tab không khả dụng (đã đóng)</span>
    </div>
  </template>

  <!-- SPLIT NODE: Recursively renders 2 children with a resizable sash -->
  <div
    v-else-if="node.type === 'split'"
    class="tiling-split-wrapper"
    :class="`split-${node.orientation}`"
  >
    <!-- First Child -->
    <div
      class="tiling-child child-first"
      :style="firstDimensionStyle"
    >
      <TilingNodeView
        :node="node.firstChild"
        :tabs-map="tabsMap"
        :active-tab-id="activeTabId"
        :zoomed-tab-id="zoomedTabId"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting-cmd-id="restartingCmdId"
        @focus="$emit('focus', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @close-tile="$emit('close-tile', $event)"
        @stop-process="$emit('stop-process', $event)"
        @restart-process="$emit('restart-process', $event)"
        @rename-tab="$emit('rename-tab', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @update-ratio="(splitId, ratio) => $emit('update-ratio', splitId, ratio)"
        @data="(id, d) => $emit('data', id, d)"
      />
    </div>

    <!-- Resizable Sash Divider -->
    <TilingSash
      :orientation="node.orientation"
      :current-ratio="node.ratio"
      @update-ratio="handleRatioUpdate"
      @reset="handleRatioReset"
    />

    <!-- Second Child -->
    <div
      class="tiling-child child-second"
      :style="secondDimensionStyle"
    >
      <TilingNodeView
        :node="node.secondChild"
        :tabs-map="tabsMap"
        :active-tab-id="activeTabId"
        :zoomed-tab-id="zoomedTabId"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting-cmd-id="restartingCmdId"
        @focus="$emit('focus', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @close-tile="$emit('close-tile', $event)"
        @stop-process="$emit('stop-process', $event)"
        @restart-process="$emit('restart-process', $event)"
        @rename-tab="$emit('rename-tab', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @update-ratio="(splitId, ratio) => $emit('update-ratio', splitId, ratio)"
        @data="(id, d) => $emit('data', id, d)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { TilingNode, SyncInputMode } from '@/types/tiling';
import type { OpenTabItem } from '@/composables/useSplitLayout';
import TilingTile from './TilingTile.vue';
import TilingSash from './TilingSash.vue';

// Enable self-referencing recursive component in script setup
defineOptions({
  name: 'TilingNodeView',
});

const props = defineProps<{
  node: TilingNode;
  tabsMap: Map<string, OpenTabItem>;
  activeTabId: string;
  zoomedTabId: string | null;
  syncMode: SyncInputMode;
  ghostTextEnabled?: boolean;
  fontFamily?: string;
  fontSize?: number;
  restartingCmdId?: number | null;
}>();

const emit = defineEmits<{
  (e: 'focus', tabId: string): void;
  (e: 'split-right', tabId: string): void;
  (e: 'split-down', tabId: string): void;
  (e: 'toggle-zoom', tabId: string): void;
  (e: 'close-tile', tabId: string): void;
  (e: 'stop-process', commandId: number): void;
  (e: 'restart-process', commandId: number): void;
  (e: 'rename-tab', tab: OpenTabItem): void;
  (e: 'register-xterm', tabId: string, instance: unknown): void;
  (e: 'update-ratio', splitId: string, ratio: number): void;
  (e: 'data', tabId: string, data: string): void;
}>();

const currentTab = computed(() => {
  if (props.node.type === 'leaf') {
    return props.tabsMap.get(props.node.tabId);
  }
  return undefined;
});

const firstDimensionStyle = computed(() => {
  if (props.node.type !== 'split') return {};
  const pct = Math.round(props.node.ratio * 1000) / 10;
  if (props.node.orientation === 'horizontal') {
    return { width: `calc(${pct}% - 2.5px)`, minWidth: '160px' };
  }
  return { height: `calc(${pct}% - 2.5px)`, minHeight: '80px' };
});

const secondDimensionStyle = computed(() => {
  if (props.node.type !== 'split') return {};
  const pct = Math.round((1 - props.node.ratio) * 1000) / 10;
  if (props.node.orientation === 'horizontal') {
    return { width: `calc(${pct}% - 2.5px)`, minWidth: '160px' };
  }
  return { height: `calc(${pct}% - 2.5px)`, minHeight: '80px' };
});

const handleRatioUpdate = (ratio: number) => {
  if (props.node.type === 'split') {
    emit('update-ratio', props.node.id, ratio);
  }
};

const handleRatioReset = () => {
  if (props.node.type === 'split') {
    emit('update-ratio', props.node.id, 0.5);
  }
};
</script>

<style scoped>
.tiling-leaf-wrapper {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
}

.tiling-empty-leaf {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--bg-terminal);
  border: 1px dashed var(--border-subtle);
  color: var(--text-muted);
  font-size: 11px;
}

.tiling-split-wrapper {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.tiling-split-wrapper.split-horizontal {
  flex-direction: row;
}

.tiling-split-wrapper.split-vertical {
  flex-direction: column;
}

.tiling-child {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-shrink: 0;
  flex-grow: 1;
}
</style>
