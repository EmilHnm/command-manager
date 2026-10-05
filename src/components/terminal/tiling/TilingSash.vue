<template>
  <div
    class="tiling-sash"
    :class="[
      `sash-${orientation}`,
      { 'is-dragging': isDragging }
    ]"
    :title="orientation === 'horizontal' ? 'Kéo để đổi độ rộng (Nháy đúp về 50:50)' : 'Kéo để đổi độ cao (Nháy đúp về 50:50)'"
    @pointerdown="handlePointerDown"
    @dblclick="handleReset"
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
let lastClickTime = 0;
let lastClickPos = { x: 0, y: 0 };

const handleReset = () => {
  lastClickTime = 0;
  isDragging.value = false;
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
  emit('reset');
};

const handlePointerDown = (e: PointerEvent) => {
  // Only primary mouse button (left-click)
  if (e.button !== 0) return;

  const now = Date.now();
  const dist = Math.hypot(e.clientX - lastClickPos.x, e.clientY - lastClickPos.y);

  // Detect double click: either browser native e.detail >= 2 or rapid successive clicks within 350ms & 15px
  if (e.detail === 2 || (now - lastClickTime < 350 && dist < 15)) {
    e.preventDefault();
    e.stopPropagation();
    handleReset();
    return;
  }

  lastClickTime = now;
  lastClickPos = { x: e.clientX, y: e.clientY };

  const currentTarget = e.currentTarget as HTMLElement | null;
  const parent = currentTarget?.parentElement;
  if (!parent) return;

  const startX = e.clientX;
  const startY = e.clientY;
  const parentRect = parent.getBoundingClientRect();
  const isHorizontal = props.orientation === 'horizontal';
  let hasMoved = false;

  const onPointerMove = (moveEvent: PointerEvent) => {
    const clientX = moveEvent.clientX;
    const clientY = moveEvent.clientY;

    if (!hasMoved) {
      if (Math.hypot(clientX - startX, clientY - startY) < 3) {
        return; // Small threshold to distinguish click/dblclick from drag
      }
      hasMoved = true;
      isDragging.value = true;
      lastClickTime = 0; // Mouse moved, cancel double click eligibility
    }

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
  box-shadow: 0 0 8px var(--primary-alpha-40, #74479166);
}

/* Horizontal split = divider is a vertical bar (side by side) */
.sash-horizontal {
  width: 5px;
  height: 100%;
  cursor: col-resize;
}

/* Expanded invisible hit target (13px wide) for easy grabbing and double-clicking */
.sash-horizontal::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: -4px;
  right: -4px;
  z-index: 1;
}

/* Vertical split = divider is a horizontal bar (stacked) */
.sash-vertical {
  width: 100%;
  height: 5px;
  cursor: row-resize;
}

/* Expanded invisible hit target (13px tall) for easy grabbing and double-clicking */
.sash-vertical::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: -4px;
  bottom: -4px;
  z-index: 1;
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
  z-index: 2;
}

.sash-horizontal .sash-line {
  width: 2px;
  height: 20px;
  border-radius: 1px;
  background-color: var(--primary-accent-alpha-60, #e4b5ff99);
}

.sash-vertical .sash-line {
  width: 20px;
  height: 2px;
  border-radius: 1px;
  background-color: var(--primary-accent-alpha-60, #e4b5ff99);
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
