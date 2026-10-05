<template>
  <!-- PANEL NODE: Renders a multi-tab terminal panel -->
  <template v-if="node.type === 'panel'">
    <div class="tiling-panel-leaf-wrapper">
      <TilingPanel
        :panel="node"
        :tabs-map="tabsMap"
        :is-active="activePanelId === node.id"
        :is-zoomed="zoomedPanelId === node.id"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting-cmd-id="restartingCmdId"
        @focus="$emit('focus', $event)"
        @select-tab="(pId, tId) => $emit('select-tab', pId, tId)"
        @new-tab="$emit('new-tab', $event)"
        @close-tab="$emit('close-tab', $event)"
        @close-panel="$emit('close-panel', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @rename-tab="$emit('rename-tab', $event)"
        @stop-process="$emit('stop-process', $event)"
        @restart-process="$emit('restart-process', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @data="(id, d) => $emit('data', id, d)"
        @cwd-change="(id, cwd) => $emit('cwd-change', id, cwd)"
        @title-change="(id, title) => $emit('title-change', id, title)"
        @phase-change="(id, phase) => $emit('phase-change', id, phase)"
        @transfer-tab="$emit('transfer-tab', $event)"
      />
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
        :active-panel-id="activePanelId"
        :zoomed-panel-id="zoomedPanelId"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting-cmd-id="restartingCmdId"
        @focus="$emit('focus', $event)"
        @select-tab="(pId, tId) => $emit('select-tab', pId, tId)"
        @new-tab="$emit('new-tab', $event)"
        @close-tab="$emit('close-tab', $event)"
        @close-panel="$emit('close-panel', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @rename-tab="$emit('rename-tab', $event)"
        @stop-process="$emit('stop-process', $event)"
        @restart-process="$emit('restart-process', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @update-ratio="(splitId, ratio) => $emit('update-ratio', splitId, ratio)"
        @data="(id, d) => $emit('data', id, d)"
        @cwd-change="(id, cwd) => $emit('cwd-change', id, cwd)"
        @title-change="(id, title) => $emit('title-change', id, title)"
        @phase-change="(id, phase) => $emit('phase-change', id, phase)"
        @transfer-tab="$emit('transfer-tab', $event)"
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
        :active-panel-id="activePanelId"
        :zoomed-panel-id="zoomedPanelId"
        :sync-mode="syncMode"
        :ghost-text-enabled="ghostTextEnabled"
        :font-family="fontFamily"
        :font-size="fontSize"
        :restarting-cmd-id="restartingCmdId"
        @focus="$emit('focus', $event)"
        @select-tab="(pId, tId) => $emit('select-tab', pId, tId)"
        @new-tab="$emit('new-tab', $event)"
        @close-tab="$emit('close-tab', $event)"
        @close-panel="$emit('close-panel', $event)"
        @split-right="$emit('split-right', $event)"
        @split-down="$emit('split-down', $event)"
        @toggle-zoom="$emit('toggle-zoom', $event)"
        @rename-tab="$emit('rename-tab', $event)"
        @stop-process="$emit('stop-process', $event)"
        @restart-process="$emit('restart-process', $event)"
        @register-xterm="(id, inst) => $emit('register-xterm', id, inst)"
        @update-ratio="(splitId, ratio) => $emit('update-ratio', splitId, ratio)"
        @data="(id, d) => $emit('data', id, d)"
        @cwd-change="(id, cwd) => $emit('cwd-change', id, cwd)"
        @title-change="(id, title) => $emit('title-change', id, title)"
        @phase-change="(id, phase) => $emit('phase-change', id, phase)"
        @transfer-tab="$emit('transfer-tab', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { TilingNode, SyncInputMode } from '@/types/tiling';
import type { OpenTabItem } from './TilingPanel.vue';
import TilingPanel from './TilingPanel.vue';
import TilingSash from './TilingSash.vue';

defineOptions({
  name: 'TilingNodeView',
});

const props = defineProps<{
  node: TilingNode;
  tabsMap: Map<string, OpenTabItem>;
  activePanelId: string;
  zoomedPanelId: string | null;
  syncMode: SyncInputMode;
  ghostTextEnabled?: boolean;
  fontFamily?: string;
  fontSize?: number;
  restartingCmdId?: number | null;
}>();

const emit = defineEmits<{
  (e: 'focus', panelId: string): void;
  (e: 'select-tab', panelId: string, tabId: string): void;
  (e: 'new-tab', panelId: string): void;
  (e: 'close-tab', tabId: string): void;
  (e: 'close-panel', panelId: string): void;
  (e: 'split-right', panelId: string): void;
  (e: 'split-down', panelId: string): void;
  (e: 'toggle-zoom', panelId: string): void;
  (e: 'rename-tab', tab: OpenTabItem): void;
  (e: 'stop-process', commandId: number): void;
  (e: 'restart-process', commandId: number): void;
  (e: 'register-xterm', tabId: string, instance: unknown): void;
  (e: 'update-ratio', splitId: string, ratio: number): void;
  (e: 'data', tabId: string, data: string): void;
  (e: 'cwd-change', tabId: string, cwd: string): void;
  (e: 'title-change', tabId: string, title: string): void;
  (e: 'phase-change', tabId: string, phase: 'prompt' | 'input' | 'running'): void;
  (e: 'transfer-tab', payload: {
    sourcePanelId: string;
    targetPanelId: string;
    tabId: string;
    targetIndex?: number;
  }): void;
}>();

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
.tiling-panel-leaf-wrapper {
  width: 100%;
  height: 100%;
  overflow: hidden;
  display: flex;
}

.tiling-split-wrapper {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
}

.tiling-split-wrapper.split-horizontal {
  flex-direction: row;
}

.tiling-split-wrapper.split-vertical {
  flex-direction: column;
}

.tiling-child {
  overflow: hidden;
  position: relative;
  display: flex;
  flex: 1 1 auto;
}
</style>
