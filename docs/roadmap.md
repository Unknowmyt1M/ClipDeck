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

## Phase 2 — Windows Integration ✅
- Global hotkey (default `Ctrl+Shift+V`, user-editable) → compact quick-pick window
- Start with Windows (opt-in setting, HKCU Run key — no admin)
- Tray menu gains "Quick pick"; close-to-tray is now a setting
- MSI + NSIS installers built by CI (unsigned — see Phase 8 notes)
- ⏳ Notifications (toast/balloon) — deferred, tray + in-app toasts cover feedback today

## Phase 3 — Smart Metadata ✅
- Source detection at capture time: foreground process + window title (Win32:
  `GetForegroundWindow` → `QueryFullProcessImageNameW`), stored on the `sources` table
- Exclusion lists use the detected executable name / window title
- Automatic categories via AI (`ai_metadata.category`) — code/link/text/…
- ⏳ Native content-type maturity (code blocks, rich text, file lists) — deferred; AI
  categories cover the classification need

## Phase 4 — AI (OpenRouter) ✅
- Provider abstraction `src-tauri/src/ai/{provider,openrouter}.rs` (future providers
  plug into the `AiProvider` trait; `services/ai_service.rs` never sees OpenRouter types)
- User-supplied OpenRouter API key, stored in the **Windows credential vault**
  (`keyring`) — never in SQLite, logs, or frontend state
- Background analysis queue (tokio channel + worker): summaries, categories,
  keywords, tags; queue status events for the UI
- Model picker fetched live from OpenRouter, grouped **Free** vs **Paid** with
  per-1M-token prices; manual model-id fallback when the catalog can't load
- Sensitive content is blocked twice (detector + `is_sensitive` flag) before any
  network call — enforced in `ai_service::analyze_one`
- ⏳ Semantic search with embeddings — deferred (keyword FTS covers Phase 6)

## Phase 5 — Privacy ✅
- On-device sensitive-content detection: OTP/verification codes, payment cards
  (Luhn-verified), private keys, JWTs, AWS/GitHub/Google/Slack/OpenAI tokens,
  `password:`-style assignments, Authorization headers
- "Never store detected secrets" option; sensitive clips are always excluded from AI
- App exclusions (executable) + window-title exclusions
- Pause monitoring (tracks clipboard silently so resume doesn't import stale data)
- Retention days + history limit enforced after every capture (orphan image sweep)
- Export full history to local JSON; irreversible wipe of rows + image files

## Phase 6 — Advanced Search ✅
- FTS5 external-content index with insert/update/delete triggers, automatic
  graceful fallback to `LIKE` when FTS5 is unavailable
- Filters: type, pinned, favorites, **Today**, **This week** (local-timezone aware)
- ⏳ Semantic / natural-language search — deferred with embeddings

## Phase 7 — Power Features ✅
- AI actions per clip: summarize, rewrite, translate (20 languages), explain,
  fix grammar — via the configured OpenRouter model, blocked for sensitive clips
- Statistics view: totals, today, pinned/favorites, images, sensitive, AI-processed,
  top-5 most-used
- ⏳ Collections, saved snippets, OCR, smart paste — deferred beyond v1 scope

## Phase 8 — V1 Polish ✅ (core)
- Errors are `Result`-typed end-to-end and surfaced as toasts; hotkey/registry
  failures are reported without breaking the app
- File logger (`<app_data>/logs/clipdeck.log`, 1 MB rotation) + panic hook —
  **never logs clipboard content or secrets**, only scope + error kind
- Close-to-tray is user-controlled; quick window always hides
- Settings UI (General · Privacy · AI · Statistics), keyboard-first flows kept
- ⏳ Deferred: auto-update (needs signing keys + update endpoint), code signing
  (needs a certificate), formal accessibility audit

## Post-v1 hardening — Bug-fix batch ✅
- Quick picker: window-level key handling (Esc / ↑↓ / Enter work even when DOM
  focus sits on `body`), input re-focused with a fresh query on every focus
  gain, window auto-hides on focus loss (native-popup behaviour)
- AI action modal now closes on Esc (own window listener; the main window
  shortcut handler bails while the modal is open)
- Global hotkey rebind registers the new combo first and unregisters the
  previous one — no lingering duplicate shortcuts
- `ai_transform` runs the on-device sensitive detector again before any
  network call, matching the background queue's double gate
- Restore self-write guard is two-phase (`writing → armed → idle`), removing
  the 400 ms poll race that could re-capture ClipDeck's own writes
- Exclusion lists are edited as plain lines (no double-encoded JSON) and the
  Rust parser heals values corrupted by older builds
- FTS result hydration batched: one SELECT + one `attach_ai` pass instead of
  three queries per hit
