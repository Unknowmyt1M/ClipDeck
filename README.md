# ClipDeck

A fast, local-first clipboard manager for Windows.

Built with **Tauri 2** + **React** + **TypeScript** + **Rust** + **SQLite**. Private by default, AI-ready.

## Why ClipDeck?

- ⚡ **Instant capture** — clipboard history saved locally the moment you copy
- 🔍 **Fast search** — find anything you've copied, instantly
- 🔒 **Privacy first** — your clipboard never leaves your device unless you explicitly enable AI features
- 🪶 **Native & light** — small footprint, system-tray resident, keyboard-first UX
- 🧠 **AI-ready** — optional AI tagging, categorization, and semantic search (coming soon)

## Status

🚧 **Phase 1 (MVP Clipboard) delivered.** See [docs/roadmap.md](docs/roadmap.md).

Works today:
- Captures text, URLs, and images as you copy (400ms monitor)
- Persistent SQLite history with duplicate detection (SHA-256)
- One-click / Enter restore back to the clipboard
- Pin & favorites, per-item delete, clear-all
- Search + filter chips (All / Pinned / Favorites / Text / URLs / Images)
- System tray with close-to-tray behavior
- Keyboard-first: `↑↓` navigate, `Enter` copy, `P` pin, `F` favorite, `Del` delete, `/` search

## Tech Stack

| Layer     | Technology            |
| --------- | --------------------- |
| Shell     | Tauri 2               |
| Frontend  | React 19 + TypeScript |
| UI        | Tailwind CSS 4 (system-follow light/dark) |
| Core      | Rust                  |
| Database  | SQLite (rusqlite, WAL) |
| AI        | Provider-agnostic abstraction (planned) |

## Development

Prerequisites: [Rust](https://rust-lang.org), [Node.js](https://nodejs.org) ≥ 22, [pnpm](https://pnpm.io), WebView2 (bundled with Windows 11).

```bash
pnpm install
pnpm tauri dev        # run the app in dev mode
pnpm tauri build      # produce installer bundles
```

Other useful commands:

```bash
pnpm build            # typecheck + build frontend only
cd src-tauri && cargo test   # Rust unit tests
```

## Project layout

See [docs/architecture.md](docs/architecture.md) for the full picture. Short version:

- `src/` — React frontend; `src/lib/tauri.ts` is the only place that calls `invoke()`
- `src-tauri/src/commands/` — thin IPC handlers
- `src-tauri/src/services/` — clipboard monitor, database, image storage
- `src-tauri/src/db/` — migrations and SQL
- `.github/workflows/` — CI (typecheck, clippy, tests, Windows build) and manual release builds

## License

MIT
