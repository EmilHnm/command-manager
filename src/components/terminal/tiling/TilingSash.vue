<template>
  <div
    class="tiling-sash"
    :class="[
      `sash-${orientation}`,
      { 'is-dragging': isDragging }
    ]"
    :title="orientation === 'horizontal' ? 'Kéo để đổi độ rộng (Nháy đúp về 50:50)' : 'Kéo để đổi độ cao (Nháy đúp về 50:50)'"
    @pointerdown="handlePointerDown"
    @dblclick="$emit('reset')"
  >
    <div class="sash-grip">
      <div class="sash-line" />
    </div>

    <!-- Fullscreen Sash Drag Guard (prevents pointer event loss when hovering over xterm canvas) -->
    <Teleport to="body">
      <div
        v-if="isDragging"
        class="sash-drag-guard"
        :class="`guard-${orientation}`"
      />
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import type { SplitOrientation } from '@/types/tiling';

const props = defineProps<{
  orientation: SplitOrientation;
  currentRatio: number;
}>();

const emit = defineEmits<{
  (e: 'update-ratio', ratio: number): void;
  (e: 'reset'): void;
}>();

const isDragging = ref(false);
let rafId: number | null = null;

const handlePointerDown = (e: PointerEvent) => {
  e.preventDefault();
  e.stopPropagation();

  const parent = (e.currentTarget as HTMLElement)?.parentElement;
  if (!parent) return;

  isDragging.value = true;
  const parentRect = parent.getBoundingClientRect();
  const isHorizontal = props.orientation === 'horizontal';

  const onPointerMove = (moveEvent: PointerEvent) => {
    const clientX = moveEvent.clientX;
    const clientY = moveEvent.clientY;

    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      rafId = null;
      let newRatio: number;
      if (isHorizontal) {
        const offsetX = clientX - parentRect.left;
        newRatio = offsetX / parentRect.width;
      } else {
        const offsetY = clientY - parentRect.top;
        newRatio = offsetY / parentRect.height;
      }
      const clamped = Math.max(0.15, Math.min(0.85, newRatio));
      emit('update-ratio', clamped);
    });
  };

  const onPointerUp = () => {
    isDragging.value = false;
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
  };

  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  window.addEventListener('pointercancel', onPointerUp);
};
</script>

<style scoped>
.tiling-sash {
  position: relative;
  flex-shrink: 0;
  background-color: var(--border-subtle);
  transition: background-color 0.15s ease, box-shadow 0.15s ease;
  user-select: none;
  z-index: 20;
}

.tiling-sash:hover,
.tiling-sash.is-dragging {
  background-color: var(--primary);
  box-shadow: 0 0 8px rgba(116, 71, 145, 0.4);
}

/* Horizontal split = divider is a vertical bar (side by side) */
.sash-horizontal {
  width: 5px;
  height: 100%;
  cursor: col-resize;
}

/* Vertical split = divider is a horizontal bar (stacked) */
.sash-vertical {
  width: 100%;
  height: 5px;
  cursor: row-resize;
}

.sash-grip {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.sash-horizontal .sash-line {
  width: 2px;
  height: 20px;
  border-radius: 1px;
  background-color: rgba(228, 181, 255, 0.6);
}

.sash-vertical .sash-line {
  width: 20px;
  height: 2px;
  border-radius: 1px;
  background-color: rgba(228, 181, 255, 0.6);
}

.tiling-sash.is-dragging .sash-line {
  background-color: #ffffff;
}

/* Fullscreen guard during sash resize to prevent lost mouse events */
.sash-drag-guard {
  position: fixed;
  inset: 0;
  z-index: 999999;
  user-select: none;
  background: transparent;
}

.sash-drag-guard.guard-horizontal {
  cursor: col-resize;
}

.sash-drag-guard.guard-vertical {
  cursor: row-resize;
}
</style>
