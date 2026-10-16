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

## Post-v1 hardening — Bug-fix batch 2 ✅
- **Migrations are transactional** (`BEGIN IMMEDIATE` → apply → stamp
  `user_version` → `COMMIT`); a failed migration rolls back instead of
  bricking start-up, and a half-created legacy v1 schema is detected and
  rebuilt. FTS is verified table+triggers (`fts_available == 4 objects`) and
  partial states are dropped to the LIKE fallback — including on already
  stamped databases
- **`PRAGMA foreign_keys = ON`** — the schema's `ON DELETE CASCADE` clauses
  actually work now (clear/delete used to leave `ai_metadata` /
  `clipboard_tags` rows behind)
- **Provenance GC**: `sources` rows (app name / window title) are purged
  whenever their item is cleared, deleted, pruned or wiped — "clear
  history" no longer leaves window titles on disk
- **Image files**: Clear history now deletes the PNGs of removed items,
  a failed image save drops the freshly inserted row instead of leaving a
  broken card, duplicate captures repair missing files, delete runs DB-first
  then files, and a start-up sweep removes orphaned image files
- **Settings**: read-modify-write holds the write lock across persist (no
  lost updates), `save_settings` runs in one transaction, dead
  `ai_allow_general` setting removed
- **Capture**: content is truncated *before* hashing (stored hash always
  matches stored content — no phantom duplicates after a restore), prune
  failures no longer abort a capture
- **Autostart**: Run-key value is quoted (`C:\Program Files\...` used to
  break) and the registry handle can no longer leak on the error path
- **Logger**: rotates during long sessions (not only at init), mutex
  poisoning recovers instead of panicking the panic hook, UTC timestamps
  are marked `Z`
- **OpenRouter**: missing/unparsable pricing is never classified as *free*
  (the free/paid promise stays honest), AI output is normalized —
  confidence clamped to 0–1, unknown categories mapped to `Other`
- **Frontend**: a focused card's Enter no longer double-fires with the
  global handler (use_count +2), wipe reports the real number of deleted
  image files, DB mutex poisoning recovers instead of failing forever

## Post-v1 features — Quick-pick paste + import ✅
- **Auto-paste (opt-out, default ON)**: picking a clip in the quick window
  copies it, hides the picker, focuses the window that was active before it
  opened (captured on every show, never ClipDeck's own windows) and sends
  Ctrl+V — only after verifying the target really owns the foreground, so a
  clip can never land in the wrong window. Toggle: *Settings → General →
  Quick picker*
- **Import history**: *Settings → Privacy → Your data → Import from JSON*
  merges an export back in — file is read in the webview, parsed with a
  20 MB / 10,000-entry cap, re-hashed and de-duplicated on-device, images
  skipped (exports carry no PNG bytes), sensitive items re-flagged by the
  local detector and skipped when *Never store detected secrets* is on,
  pins/favorites/usage/tags/AI summaries restored — all in one transaction

## Post-v1 hardening — Bug-fix batch 3 ✅
- **Quick-pick paste actually lands now**: Windows foreground-lock denies
  `SetForegroundWindow` from a background process (our picker just hid),
  so focus never returned to the app the user was typing in and the paste
  silently never happened. `paste_into` now uses the documented
  `AttachThreadInput` workaround (attach input queue → focus → detach),
  requires the target to *hold* the foreground for a 300 ms settle window
  before sending (browsers/Electron restore their caret asynchronously —
  keys sent in that gap were swallowed), spaces the Ctrl+V key events like
  real typing, and the paste target is cleared (not left stale) when
  ClipDeck itself was focused before the picker opened
- **Theme setting works**: replaced CSS `light-dark()` tokens (whose
  resolution silently depends on per-element `color-scheme` handling in
  each WebView2 build) with explicit light/dark variables toggled by a
  `dark` class on `<html>` — `src/lib/theme.ts` applies Light/Dark
  directly and follows *system* live via `prefers-color-scheme`. The
  quick-picker window now applies the saved theme too (it is a separate
  webview document and previously always rendered in the system default)
- **Quick picker shows image thumbnails**: image clips render an 8×8
  `ImageThumb` (lazy-loaded, same IPC path as the main list) instead of a
  plain type badge
- **Global hotkey field captures combos**: the settings input is now a
  key-capture field (`keydown` + `preventDefault`) — pressing e.g.
  Ctrl+Shift+V records the combo instead of triggering the webview's
  native paste into the field; Backspace clears, only parser-accepted
  keys (letters/digits/F-keys/Space/Enter + modifiers) are recorded
