import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath, URL } from 'node:url';

const tauriDebug = !!(process.env.TAURI_ENV_DEBUG || process.env.TAURI_DEBUG);

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_', 'TAURI_'],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: '0.0.0.0',
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: tauriDebug ? false : 'esbuild',
    sourcemap: tauriDebug,
  },
});
