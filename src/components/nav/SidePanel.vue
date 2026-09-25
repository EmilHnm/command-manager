<template>
  <aside v-if="isOpen" class="side-panel">
    <div class="panel-header">
      <h3 class="panel-title">{{ title }}</h3>
      <div class="panel-actions">
        <slot name="actions" />
      </div>
    </div>
    <div class="panel-body">
      <slot />
    </div>
  </aside>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string;
    isOpen?: boolean;
  }>(),
  {
    title: 'Danh mục',
    isOpen: true,
  }
);
</script>

<style scoped>
.side-panel {
  width: var(--side-panel-width);
  height: 100%;
  background-color: var(--bg-sidebar);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
}

.panel-header {
  padding: 10px 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  min-height: 40px;
}

.panel-title {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}
</style>
