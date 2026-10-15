# ClipDeck — Architecture

## Overview

ClipDeck is a local-first clipboard manager for Windows built on **Tauri 2**:

- **Rust** owns all privileged work: clipboard monitoring, SQLite persistence, file storage.
- **React + TypeScript** is presentation only; it talks to Rust exclusively through typed Tauri commands (`src/lib/tauri.ts` is the single IPC boundary).

## Data flow

```
Windows clipboard
  → ClipboardMonitor (polling thread, 400ms)
  → SHA-256 hash (text bytes / image pixels)
  → duplicate check (existing hash → bump use_count)
  → type detection (text / url / image)
  → SQLite (WAL mode)
  → "clipboard-changed" event → React refreshes list
```

AI processing is **not** part of this pipeline yet; when added, it must run asynchronously after the save step and never block capture.

## Layers

| Layer | Location | Responsibility |
|---|---|---|
| IPC commands | `src-tauri/src/commands/` | Validate input, call services, return serializable results |
| Services | `src-tauri/src/services/` | Business logic: `database_service`, `clipboard_service`, `image_service` |
| DB | `src-tauri/src/db/` | Schema migrations (`mod.rs`) and SQL (`queries.rs`) |
| Models | `src-tauri/src/models/` | Domain types shared across layers |
| Frontend bridge | `src/lib/tauri.ts` | Typed wrappers for every command |
| UI state | `src/features/clipboard/` | `useClipboard` hook: loads + live-syncs history |

## Key decisions

- **Polling, not Win32 listeners (MVP):** the monitor polls every 400ms via `arboard`. Simple, reliable, and fast enough. A native `AddClipboardFormatListener` path can replace it later without changing the service interface.
- **Self-write guard:** restoring an item writes to the system clipboard; an `AtomicBool` tells the monitor to skip the next capture so ClipDeck never re-records its own writes.
- **Images on disk, metadata in SQLite:** PNGs (full + 320px thumbnail) live under `<app_data>/images/<uuid>.png`. Only the preview/size string goes in the database; the UI fetches thumbnails through `get_item_image` as data URLs.
- **Close hides to tray:** the window close button hides the window; the tray menu (Show / Quit) controls the process lifetime.
- **Content is never logged.** Poll errors log the error kind only.

## Database

SQLite with `PRAGMA user_version`-tracked migrations (`src-tauri/src/db/mod.rs`). WAL journal mode. Schema v1: `clipboard_items`, `sources`, `tags`, `clipboard_tags`, `settings`. Source detection and tagging tables exist but populate in later phases.
