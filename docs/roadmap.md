# ClipDeck — Roadmap

Phase plan follows the Hermes master prompt. Each phase is shippable on its own.

## Phase 0 — Foundation ✅
- Tauri 2 + React + TypeScript + Tailwind setup
- SQLite + migrations, CI/Release GitHub workflows

## Phase 1 — MVP Clipboard ✅
- Clipboard monitoring (text, URLs, images)
- Persistent history with duplicate detection (SHA-256)
- Restore to clipboard, delete, clear-all
- Pin / favorites
- Local search + filter chips
- System tray (show / quit, close-to-tray)
- Keyboard-first UI (↑↓, Enter, P, F, Del, `/`)

**Acceptance goal:** ClipDeck works as a reliable daily clipboard manager with no AI and no internet.

## Phase 2 — Windows Integration
- Global hotkey (default `Ctrl+Shift+V`) → quick clipboard picker
- Start with Windows (opt-in setting)
- Notifications
- First signed-ish installer pass

## Phase 3 — Smart Metadata
- Source detection (application name, window title, URL)
- Content type detection maturity (code, rich text, files)
- Automatic categories

## Phase 4 — AI
- Provider abstraction (`ai/provider.rs`) — OpenAI / Gemini / local
- Background queue, tags, summaries, keywords, categories
- AI settings UI
- Sensitive content is never auto-sent to external providers

## Phase 5 — Privacy
- Sensitive-content detection (patterns: OTP, API keys, tokens)
- App exclusions, AI exclusions, pause monitoring
- Retention policies, export, full local delete

## Phase 6 — Advanced Search
- SQLite FTS5
- Filters: tag / source / date
- Semantic search (embeddings), natural-language queries

## Phase 7 — Power Features
- Collections, saved snippets
- AI rewrite / translate / explain
- OCR, smart paste, statistics

## Phase 8 — V1 Polish
- Performance, accessibility, crash handling
- Auto-update, uninstaller, code signing, release docs
