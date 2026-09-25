# Cấu trúc thư mục (MVP)

Nguồn: [plan.md](./plan.md) | [checklist.md](./checklist.md) | [screens.md](./screens.md).

**Chốt stack cho cây này:** Tauri v2, Rust (Tokio) ở `src-tauri/`, Vue + TypeScript + Vite ở `src/`. Dockview dùng bản Vue. Post-MVP (RJSF, systemd, Drive) không có thư mục riêng — thêm khi tới lúc.

Nếu đổi sang React: đổi `src/**/*.vue` → `*.tsx`, `composables/` → `hooks/`, `dockview-vue` → `dockview-react`. Phần `src-tauri/` giữ nguyên.

```
command-manager/
├── docs/
│   ├── plan.md
│   ├── checklist.md
│   ├── screens.md
│   └── structure.md
├── src/                          # frontend (WebView)
├── src-tauri/                    # Rust backend
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
└── README.md
```

---

## Frontend — `src/`

Bám App Shell và sitemap SCR-01…SCR-05. Một view = một route. Modal/overlay dùng chung nằm ở `components/`, không nhét sâu vào từng màn.

```
src/
├── main.ts
├── App.vue                       # shell: titlebar + nav + outlet + statusbar
├── env.d.ts
├── assets/
│   └── fonts/                    # JetBrains Mono / Fira Code (local, CSP 'self')
├── styles/
│   ├── tokens.css                # dark slate/zinc, WCAG AA
│   └── global.css
├── layouts/
│   └── AppShell.vue              # titlebar, activity bar 64px, side panel 260px, status 28px
├── router.ts                     # /workspace /commands /groups /history /settings
├── views/
│   ├── workspace/
│   │   └── WorkspaceView.vue     # SCR-01: tree nhóm + dockview PTY
│   ├── commands/
│   │   └── CommandLibraryView.vue  # SCR-02
│   ├── groups/
│   │   └── GroupsView.vue        # SCR-03: sequencer execution_order
│   ├── history/
│   │   └── HistoryView.vue       # SCR-04: run_session / run_event
│   └── settings/
│       └── SettingsView.vue      # SCR-05: autostart, buffer, backup
├── components/
│   ├── titlebar/
│   │   ├── Titlebar.vue          # data-tauri-drag-region, close → graceful shutdown
│   │   └── CommandPalette.vue    # Ctrl+K
│   ├── nav/
│   │   ├── ActivityBar.vue
│   │   └── SidePanel.vue
│   ├── statusbar/
│   │   └── StatusBar.vue         # running count, ring buffer, WAL, lock
│   ├── terminal/
│   │   ├── DockHost.vue          # dockview tabs; đóng tab ≠ stop
│   │   └── XtermPane.vue         # xterm.js, resize → PTY, reattach buffer
│   ├── explorer/
│   │   └── GroupTree.vue         # workspace sidebar
│   └── dialogs/
│       ├── ConfirmDialog.vue     # privileged: edit command, autostart, import
│       ├── StopProcessModal.vue  # MOD-01
│       ├── CommandEditorModal.vue
│       ├── RestoreWizard.vue     # MOD-08
│       └── ShutdownOverlay.vue   # MOD-09
├── composables/
│   ├── useCommands.ts
│   ├── useGroups.ts
│   ├── useRunSession.ts
│   ├── usePtyStream.ts           # IPC events → xterm; backpressure phía Rust
│   └── useAppLifecycle.ts        # close window, single-instance toast
├── ipc/
│   ├── client.ts                 # invoke wrappers, một chỗ duy nhất
│   └── events.ts                 # listen: pty-data, process-status, …
└── types/
    └── models.ts                 # Command, Group, RunSession, RunEvent, ProcessStatus
```

Không tách `services/` hay `store/` trừ khi một composable phình ra. Trạng thái chạy sống ở Rust; frontend subscribe event, không giữ PID làm source of truth.

---

## Backend — `src-tauri/`

Tách **DB tĩnh** / **process + PTY in-memory** / **backup** / **app lock**. Invoke handlers mỏng; logic nằm ở module tương ứng.

```
src-tauri/
├── Cargo.toml
├── build.rs
├── tauri.conf.json               # CSP default-src 'self'
├── capabilities/
│   └── default.json              # tối thiểu: cửa sổ, fs dialog backup, pty, autostart
├── icons/
├── migrations/
│   └── 001_init.sql              # 5 bảng + schema_version
└── src/
    ├── main.rs                   # lock single-instance TRƯỚC process manager
    ├── lib.rs                    # setup: db pool, manager, plugins, handlers
    ├── error.rs
    ├── db/
    │   ├── mod.rs
    │   ├── pool.rs               # 1 writer + 2–3 readers, WAL
    │   ├── schema.rs
    │   └── repos/
    │       ├── commands.rs
    │       ├── groups.rs
    │       └── runs.rs           # run_session / run_event (lifecycle only)
    ├── process/
    │   ├── mod.rs
    │   ├── manager.rs            # DashMap/RwLock: pid, pty handle, status
    │   ├── spawn.rs              # argv vs is_shell; PDEATHSIG trước exec (Linux)
    │   ├── shutdown.rs           # SIGTERM/CTRL_C → wait → SIGKILL/TerminateProcess
    │   └── platform/
    │       ├── mod.rs
    │       ├── unix.rs
    │       └── windows.rs        # adapter process group
    ├── pty/
    │   ├── mod.rs
    │   ├── session.rs            # PTY alloc, resize, byte stream
    │   ├── buffer.rs             # ring buffer theo byte (1–2MB)
    │   └── backpressure.rs       # pause / drop / chunk — một chính sách
    ├── backup/
    │   ├── mod.rs
    │   ├── export.rs             # VACUUM INTO + integrity_check
    │   └── import.rs             # temp → check → confirm → snapshot → swap wal/shm → rollback
    ├── app/
    │   ├── mod.rs
    │   ├── single_instance.rs
    │   └── autostart.rs          # plugin; sau lock, chỉ start nhóm autostart chưa active
    └── ipc/
        ├── mod.rs                # đăng ký invoke + emit
        ├── commands.rs           # CRUD + start/stop + backup
        └── events.rs             # tên event PTY/status (khớp src/ipc/events.ts)
```

Runtime data (không commit): `app.db`, `app.db-wal`, `app.db-shm` do Tauri `app_data_dir` quản lý — không đặt trong repo.

---

## IPC surface (ràng buộc hai phía)

Tên invoke/event sống ở `src-tauri/src/ipc/` và `src/ipc/`. Không `invoke('...')` rải trong view.

| Invoke (gợi ý) | Module Rust |
| --- | --- |
| `commands_*` / `groups_*` | `db/repos` |
| `session_start` / `session_stop` / `process_stop` | `process` |
| `pty_resize` / `pty_write` / `pty_reattach` | `pty` |
| `backup_export` / `backup_import` | `backup` |
| `settings_get` / `settings_set` / `autostart_*` | `app` |

Events: `pty://data`, `process://status`, `app://shutdown-progress`, `app://instance`.

---

## Cố ý không có (YAGNI)

- `src/stores/` (Pinia) — chưa cần; composable + event đủ
- `src-tauri/src/rjsf/` / `adapters/ffmpeg` — Post-MVP
- `src-tauri/src/drive/` / `keyring` — Post-MVP
- `src-tauri/src/systemd/` — Post-MVP
- layout grid dockview riêng — cùng `DockHost.vue` khi tới lúc
- test framework nặng — một self-check nhỏ cạnh `process/shutdown.rs` hoặc `backup/import.rs` khi logic đó có

---

## Thứ tự tạo lúc scaffold

1. `create-tauri-app` (Vue + TS) → `src/` + `src-tauri/` mặc định
2. Thêm `migrations/`, `db/`, `process/`, `pty/`, `backup/`, `app/`, `ipc/`
3. Thêm `src/views/*`, `layouts/AppShell.vue`, `components/terminal/`
4. Capabilities + CSP trước khi gắn PTY/fs
