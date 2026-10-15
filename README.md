# ClipDeck

A fast, local-first clipboard manager for Windows.

Built with **Tauri 2** + **React** + **TypeScript** + **Rust** + **SQLite**. Private by default, AI-ready.

## Why ClipDeck?

- ⚡ **Instant capture** — clipboard history saved locally the moment you copy
- ⌨️ **Quick pick anywhere** — `Ctrl+Shift+V` opens a compact searchable picker
- 🔍 **Fast search** — SQLite FTS5 full-text search + filters (Today / This week / type / pinned / favorites)
- 🔒 **Privacy first** — on-device secret detection (cards, OTPs, API keys…), app exclusions, pause, retention, export & wipe
- 🧠 **Optional AI (OpenRouter)** — auto summaries, categories, keywords/tags, and per-clip rewrite / translate / explain / fix-grammar
- 🪟 **Native & light** — tray-resident, keyboard-first, small footprint

## Status

✅ **Phases 0–8 delivered** (Phase 8 core; auto-update & code signing deferred). See [docs/roadmap.md](docs/roadmap.md).

Works today:
- Captures text, URLs, and images as you copy (400ms monitor)
- Source detection: which app/title the clip came from
- Persistent SQLite history with duplicate detection (SHA-256)
- Restore back to clipboard, pin & favorites, delete, clear-all
- FTS5 full-text search + filter chips (All / Pinned / Favorites / Today / This week / Text / URLs / Images)
- Global hotkey → borderless always-on-top quick picker; start with Windows (opt-in)
- Privacy: sensitive-content detection (never stored/sent when configured), app & title exclusions, pause monitoring, retention, JSON export, full wipe
- AI via **OpenRouter**: bring your own API key (stored in the Windows credential vault), pick any model from the live catalog — clearly marked **FREE** vs **Paid** with per-1M-token prices; background analysis queue; AI actions per clip
- Statistics view (totals, today, most-used, AI-processed)
- File logging (no content ever) + panic hook, close-to-tray setting

Keyboard-first: `↑↓` navigate · `Enter` copy · `P` pin · `F` favorite · `A` analyze (AI) · `Del` delete · `/` search · `S` settings

## Install

Prebuilt MSI/NSIS installers come from GitHub Actions: **Actions → CI → latest run → Artifacts → `clipdeck-windows-…`** (or run the manual `Release` workflow to publish a GitHub Release).

## Tech Stack

| Layer     | Technology            |
| --------- | --------------------- |
| Shell     | Tauri 2               |
| Frontend  | React 19 + TypeScript |
| UI        | Tailwind CSS 4 (system-follow light/dark) |
| Core      | Rust                  |
| Database  | SQLite (rusqlite, WAL, FTS5) |
| AI        | OpenRouter (provider abstraction, bring-your-own key) |
| Secrets   | Windows Credential Manager (`keyring`) |

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
