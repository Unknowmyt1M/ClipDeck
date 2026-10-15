# ClipDeck — Architecture

## Overview

ClipDeck is a local-first clipboard manager for Windows built on **Tauri 2**:

- **Rust** owns all privileged work: clipboard monitoring, SQLite persistence, file storage, Win32 integration, AI orchestration.
- **React + TypeScript** is presentation only; it talks to Rust exclusively through typed Tauri commands (`src/lib/tauri.ts` is the single IPC boundary).

## Data flow

```
Windows clipboard
  → ClipboardMonitor (polling thread, 400ms)
  → pause check / source detection (foreground app) / exclusion check
  → SHA-256 hash (text bytes / image pixels)
  → duplicate check (existing hash → bump use_count)
  → type detection (text / url / image) + on-device sensitive detection
  → SQLite (WAL mode) [+ sources row]
  → retention + history-limit prune (orphan image sweep)
  → optional AI enqueue (never for sensitive / images / oversized)
  → "clipboard-changed" event → React refreshes list
```

AI analysis runs on a separate tokio worker fed by an unbounded channel; capture
never waits on the network.

## Layers

| Layer | Location | Responsibility |
|---|---|---|
| IPC commands | `src-tauri/src/commands/` | Validate input, call services, return serializable results |
| Services | `src-tauri/src/services/` | `database_service`, `clipboard_service`, `image_service`, `settings_service`, `ai_service`, `search_service`, `source_service`, `hotkey_service`, `windows_service`, `logger` |
| AI providers | `src-tauri/src/ai/` | `provider.rs` trait + `openrouter.rs` adapter |
| Privacy | `src-tauri/src/security/` | `sensitive_detector` (regex + Luhn), `secrets` (OS credential vault) |
| DB | `src-tauri/src/db/` | Schema migrations (`mod.rs`), SQL (`queries.rs`), settings/AI/FTS SQL (`settings_queries.rs`) |
| Models | `src-tauri/src/models/` | Domain types shared across layers |
| Frontend bridge | `src/lib/tauri.ts` | Typed wrappers for every command |
| UI state | `src/features/` | `useClipboard`, `useSettings` hooks |

## Key decisions

- **Polling, not Win32 listeners (MVP):** the monitor polls every 400ms via `arboard`. Simple, reliable, and fast enough. A native `AddClipboardFormatListener` path can replace it later without changing the service interface.
- **Self-write guard:** restoring an item writes to the system clipboard; an `AtomicBool` tells the monitor to skip the next capture so ClipDeck never re-records its own writes.
- **Images on disk, metadata in SQLite:** PNGs (full + 320px thumbnail) live under `<app_data>/images/<uuid>.png`. Only the preview/size string goes in the database; the UI fetches thumbnails through `get_item_image` as data URLs. Retention/wipe sweep orphaned files.
- **Close-to-tray is a setting:** the main window hides on close when enabled; the quick-pick window always hides. Tray menu: Show / Quick pick / Quit.
- **Content is never logged.** Errors log scope + error kind only, to `<app_data>/logs/clipdeck.log` (1 MB rotation) with a panic hook that records message + location.
- **Settings are cached** in an `RwLock` behind `SettingsService`; writes go through SQLite first so all threads observe updates immediately.
- **Secrets live in the OS vault:** the OpenRouter key is stored with the `keyring` crate (Windows Credential Manager). Only a boolean "configured" crosses IPC.
- **AI never sees sensitive content:** the detector runs at capture time (`is_sensitive` flag) *and* again inside the AI worker right before any network call; images and out-of-range content are skipped too.
- **Quick window is a second Tauri webview** (label `quick`, borderless, always-on-top) rendering `QuickPicker` — same bundle, compact UI.

## Database

SQLite with `PRAGMA user_version`-tracked migrations (`src-tauri/src/db/mod.rs`). WAL journal mode.

- v1: `clipboard_items`, `sources`, `tags`, `clipboard_tags`, `settings`
- v2: `ai_metadata` (1:1 with items, keyed by `clipboard_id`) + FTS5 external-content
  index `clipboard_fts` kept in sync by triggers. FTS5 creation is best-effort: if
  the SQLite build lacks it, the app degrades to `LIKE` search automatically
  (`settings_queries::fts_available`).

Schema changes always ship as a new migration — never by editing an applied one.
