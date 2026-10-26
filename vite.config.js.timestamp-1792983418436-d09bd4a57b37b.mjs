// vite.config.js
import { defineConfig } from "file:///data/Second/htdocs/projects/command-manager/node_modules/.pnpm/vite@5.4.21_@types+node@26.6.2/node_modules/vite/dist/node/index.js";
import vue from "file:///data/Second/htdocs/projects/command-manager/node_modules/.pnpm/@vitejs+plugin-vue@5.2.4_vite@5.4.21_@types+node@26.6.2__vue@3.5.43_typescript@5.9.3_/node_modules/@vitejs/plugin-vue/dist/index.mjs";
import { fileURLToPath, URL } from "node:url";

// package.json
var package_default = {
  name: "command-manager",
  private: true,
  version: "1.1.0",
  type: "module",
  scripts: {
    dev: "vite",
    build: "vue-tsc -b && vite build",
    preview: "vite preview",
    tauri: "tauri",
    "tauri:dev": "tauri dev --features desktop",
    "tauri:build": "node scripts/with-appindicator-pkgconfig.mjs tauri build --features desktop"
  },
  dependencies: {
    "@fontsource/fira-code": "^5.3.0",
    "@fontsource/inter": "^5.3.0",
    "@fontsource/jetbrains-mono": "^5.3.0",
    "@tauri-apps/api": "^2.11.1",
    "@xterm/addon-fit": "^0.11.0",
    "@xterm/addon-search": "^0.16.0",
    "@xterm/addon-unicode11": "^0.9.0",
    "@xterm/addon-web-links": "^0.12.0",
    "@xterm/xterm": "^6.0.0",
    "dockview-vue": "^8.3.1",
    "lucide-vue-next": "^1.0.0",
    vue: "^3.5.13",
    "vue-router": "^4.5.0"
  },
  devDependencies: {
    "@tauri-apps/cli": "^2.11.5",
    "@types/node": "^26.6.2",
    "@vitejs/plugin-vue": "^5.2.1",
    typescript: "^5.6.3",
    vite: "^5.4.11",
    "vue-tsc": "^2.1.10"
  }
};

// vite.config.js
var __vite_injected_original_import_meta_url = "file:///data/Second/htdocs/projects/command-manager/vite.config.js";
var tauriDebug = !!(process.env.TAURI_ENV_DEBUG || process.env.TAURI_DEBUG);
var vite_config_default = defineConfig({
  plugins: [vue()],
  define: {
    __APP_VERSION__: JSON.stringify(package_default.version)
  },
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", __vite_injected_original_import_meta_url))
    }
  },
  envPrefix: ["VITE_", "TAURI_ENV_", "TAURI_"],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: "0.0.0.0",
    watch: { ignored: ["**/src-tauri/**"] }
  },
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: tauriDebug ? false : "esbuild",
    sourcemap: tauriDebug
  }
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiLCAicGFja2FnZS5qc29uIl0sCiAgInNvdXJjZXNDb250ZW50IjogWyJjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfZGlybmFtZSA9IFwiL2RhdGEvU2Vjb25kL2h0ZG9jcy9wcm9qZWN0cy9jb21tYW5kLW1hbmFnZXJcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfZmlsZW5hbWUgPSBcIi9kYXRhL1NlY29uZC9odGRvY3MvcHJvamVjdHMvY29tbWFuZC1tYW5hZ2VyL3ZpdGUuY29uZmlnLmpzXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ltcG9ydF9tZXRhX3VybCA9IFwiZmlsZTovLy9kYXRhL1NlY29uZC9odGRvY3MvcHJvamVjdHMvY29tbWFuZC1tYW5hZ2VyL3ZpdGUuY29uZmlnLmpzXCI7aW1wb3J0IHsgZGVmaW5lQ29uZmlnIH0gZnJvbSAndml0ZSc7XG5pbXBvcnQgdnVlIGZyb20gJ0B2aXRlanMvcGx1Z2luLXZ1ZSc7XG5pbXBvcnQgeyBmaWxlVVJMVG9QYXRoLCBVUkwgfSBmcm9tICdub2RlOnVybCc7XG5pbXBvcnQgcGtnIGZyb20gJy4vcGFja2FnZS5qc29uJztcbmNvbnN0IHRhdXJpRGVidWcgPSAhIShwcm9jZXNzLmVudi5UQVVSSV9FTlZfREVCVUcgfHwgcHJvY2Vzcy5lbnYuVEFVUklfREVCVUcpO1xuZXhwb3J0IGRlZmF1bHQgZGVmaW5lQ29uZmlnKHtcbiAgICBwbHVnaW5zOiBbdnVlKCldLFxuICAgIGRlZmluZToge1xuICAgICAgICBfX0FQUF9WRVJTSU9OX186IEpTT04uc3RyaW5naWZ5KHBrZy52ZXJzaW9uKSxcbiAgICB9LFxuICAgIHJlc29sdmU6IHtcbiAgICAgICAgYWxpYXM6IHtcbiAgICAgICAgICAgICdAJzogZmlsZVVSTFRvUGF0aChuZXcgVVJMKCcuL3NyYycsIGltcG9ydC5tZXRhLnVybCkpLFxuICAgICAgICB9LFxuICAgIH0sXG4gICAgZW52UHJlZml4OiBbJ1ZJVEVfJywgJ1RBVVJJX0VOVl8nLCAnVEFVUklfJ10sXG4gICAgY2xlYXJTY3JlZW46IGZhbHNlLFxuICAgIHNlcnZlcjoge1xuICAgICAgICBwb3J0OiA1MTczLFxuICAgICAgICBzdHJpY3RQb3J0OiB0cnVlLFxuICAgICAgICBob3N0OiAnMC4wLjAuMCcsXG4gICAgICAgIHdhdGNoOiB7IGlnbm9yZWQ6IFsnKiovc3JjLXRhdXJpLyoqJ10gfSxcbiAgICB9LFxuICAgIGJ1aWxkOiB7XG4gICAgICAgIHRhcmdldDogWydlczIwMjEnLCAnY2hyb21lMTAwJywgJ3NhZmFyaTEzJ10sXG4gICAgICAgIG1pbmlmeTogdGF1cmlEZWJ1ZyA/IGZhbHNlIDogJ2VzYnVpbGQnLFxuICAgICAgICBzb3VyY2VtYXA6IHRhdXJpRGVidWcsXG4gICAgfSxcbn0pO1xuIiwgIntcbiAgXCJuYW1lXCI6IFwiY29tbWFuZC1tYW5hZ2VyXCIsXG4gIFwicHJpdmF0ZVwiOiB0cnVlLFxuICBcInZlcnNpb25cIjogXCIxLjEuMFwiLFxuICBcInR5cGVcIjogXCJtb2R1bGVcIixcbiAgXCJzY3JpcHRzXCI6IHtcbiAgICBcImRldlwiOiBcInZpdGVcIixcbiAgICBcImJ1aWxkXCI6IFwidnVlLXRzYyAtYiAmJiB2aXRlIGJ1aWxkXCIsXG4gICAgXCJwcmV2aWV3XCI6IFwidml0ZSBwcmV2aWV3XCIsXG4gICAgXCJ0YXVyaVwiOiBcInRhdXJpXCIsXG4gICAgXCJ0YXVyaTpkZXZcIjogXCJ0YXVyaSBkZXYgLS1mZWF0dXJlcyBkZXNrdG9wXCIsXG4gICAgXCJ0YXVyaTpidWlsZFwiOiBcIm5vZGUgc2NyaXB0cy93aXRoLWFwcGluZGljYXRvci1wa2djb25maWcubWpzIHRhdXJpIGJ1aWxkIC0tZmVhdHVyZXMgZGVza3RvcFwiXG4gIH0sXG4gIFwiZGVwZW5kZW5jaWVzXCI6IHtcbiAgICBcIkBmb250c291cmNlL2ZpcmEtY29kZVwiOiBcIl41LjMuMFwiLFxuICAgIFwiQGZvbnRzb3VyY2UvaW50ZXJcIjogXCJeNS4zLjBcIixcbiAgICBcIkBmb250c291cmNlL2pldGJyYWlucy1tb25vXCI6IFwiXjUuMy4wXCIsXG4gICAgXCJAdGF1cmktYXBwcy9hcGlcIjogXCJeMi4xMS4xXCIsXG4gICAgXCJAeHRlcm0vYWRkb24tZml0XCI6IFwiXjAuMTEuMFwiLFxuICAgIFwiQHh0ZXJtL2FkZG9uLXNlYXJjaFwiOiBcIl4wLjE2LjBcIixcbiAgICBcIkB4dGVybS9hZGRvbi11bmljb2RlMTFcIjogXCJeMC45LjBcIixcbiAgICBcIkB4dGVybS9hZGRvbi13ZWItbGlua3NcIjogXCJeMC4xMi4wXCIsXG4gICAgXCJAeHRlcm0veHRlcm1cIjogXCJeNi4wLjBcIixcbiAgICBcImRvY2t2aWV3LXZ1ZVwiOiBcIl44LjMuMVwiLFxuICAgIFwibHVjaWRlLXZ1ZS1uZXh0XCI6IFwiXjEuMC4wXCIsXG4gICAgXCJ2dWVcIjogXCJeMy41LjEzXCIsXG4gICAgXCJ2dWUtcm91dGVyXCI6IFwiXjQuNS4wXCJcbiAgfSxcbiAgXCJkZXZEZXBlbmRlbmNpZXNcIjoge1xuICAgIFwiQHRhdXJpLWFwcHMvY2xpXCI6IFwiXjIuMTEuNVwiLFxuICAgIFwiQHR5cGVzL25vZGVcIjogXCJeMjYuNi4yXCIsXG4gICAgXCJAdml0ZWpzL3BsdWdpbi12dWVcIjogXCJeNS4yLjFcIixcbiAgICBcInR5cGVzY3JpcHRcIjogXCJeNS42LjNcIixcbiAgICBcInZpdGVcIjogXCJeNS40LjExXCIsXG4gICAgXCJ2dWUtdHNjXCI6IFwiXjIuMS4xMFwiXG4gIH1cbn1cbiJdLAogICJtYXBwaW5ncyI6ICI7QUFBc1QsU0FBUyxvQkFBb0I7QUFDblYsT0FBTyxTQUFTO0FBQ2hCLFNBQVMsZUFBZSxXQUFXOzs7QUNGbkM7QUFBQSxFQUNFLE1BQVE7QUFBQSxFQUNSLFNBQVc7QUFBQSxFQUNYLFNBQVc7QUFBQSxFQUNYLE1BQVE7QUFBQSxFQUNSLFNBQVc7QUFBQSxJQUNULEtBQU87QUFBQSxJQUNQLE9BQVM7QUFBQSxJQUNULFNBQVc7QUFBQSxJQUNYLE9BQVM7QUFBQSxJQUNULGFBQWE7QUFBQSxJQUNiLGVBQWU7QUFBQSxFQUNqQjtBQUFBLEVBQ0EsY0FBZ0I7QUFBQSxJQUNkLHlCQUF5QjtBQUFBLElBQ3pCLHFCQUFxQjtBQUFBLElBQ3JCLDhCQUE4QjtBQUFBLElBQzlCLG1CQUFtQjtBQUFBLElBQ25CLG9CQUFvQjtBQUFBLElBQ3BCLHVCQUF1QjtBQUFBLElBQ3ZCLDBCQUEwQjtBQUFBLElBQzFCLDBCQUEwQjtBQUFBLElBQzFCLGdCQUFnQjtBQUFBLElBQ2hCLGdCQUFnQjtBQUFBLElBQ2hCLG1CQUFtQjtBQUFBLElBQ25CLEtBQU87QUFBQSxJQUNQLGNBQWM7QUFBQSxFQUNoQjtBQUFBLEVBQ0EsaUJBQW1CO0FBQUEsSUFDakIsbUJBQW1CO0FBQUEsSUFDbkIsZUFBZTtBQUFBLElBQ2Ysc0JBQXNCO0FBQUEsSUFDdEIsWUFBYztBQUFBLElBQ2QsTUFBUTtBQUFBLElBQ1IsV0FBVztBQUFBLEVBQ2I7QUFDRjs7O0FEcENnTSxJQUFNLDJDQUEyQztBQUlqUCxJQUFNLGFBQWEsQ0FBQyxFQUFFLFFBQVEsSUFBSSxtQkFBbUIsUUFBUSxJQUFJO0FBQ2pFLElBQU8sc0JBQVEsYUFBYTtBQUFBLEVBQ3hCLFNBQVMsQ0FBQyxJQUFJLENBQUM7QUFBQSxFQUNmLFFBQVE7QUFBQSxJQUNKLGlCQUFpQixLQUFLLFVBQVUsZ0JBQUksT0FBTztBQUFBLEVBQy9DO0FBQUEsRUFDQSxTQUFTO0FBQUEsSUFDTCxPQUFPO0FBQUEsTUFDSCxLQUFLLGNBQWMsSUFBSSxJQUFJLFNBQVMsd0NBQWUsQ0FBQztBQUFBLElBQ3hEO0FBQUEsRUFDSjtBQUFBLEVBQ0EsV0FBVyxDQUFDLFNBQVMsY0FBYyxRQUFRO0FBQUEsRUFDM0MsYUFBYTtBQUFBLEVBQ2IsUUFBUTtBQUFBLElBQ0osTUFBTTtBQUFBLElBQ04sWUFBWTtBQUFBLElBQ1osTUFBTTtBQUFBLElBQ04sT0FBTyxFQUFFLFNBQVMsQ0FBQyxpQkFBaUIsRUFBRTtBQUFBLEVBQzFDO0FBQUEsRUFDQSxPQUFPO0FBQUEsSUFDSCxRQUFRLENBQUMsVUFBVSxhQUFhLFVBQVU7QUFBQSxJQUMxQyxRQUFRLGFBQWEsUUFBUTtBQUFBLElBQzdCLFdBQVc7QUFBQSxFQUNmO0FBQ0osQ0FBQzsiLAogICJuYW1lcyI6IFtdCn0K
