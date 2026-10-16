import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ClipboardItem, ItemFilter, Settings } from "./types";
import { APP_NAME } from "./lib/constants";
import {
  aiAnalyzeItem,
  clearHistory,
  deleteItem,
  restoreItem,
  setItemFavorite,
  setItemPinned,
} from "./lib/tauri";
import { useClipboard } from "./features/clipboard";
import { useSettings } from "./features/settings/useSettings";
import { applyTheme } from "./lib/theme";
import { SearchBar } from "./components/search/SearchBar";
import { FilterChips } from "./components/search/FilterChips";
import { ClipboardList } from "./components/clipboard/ClipboardList";
import { EmptyState } from "./components/clipboard/EmptyState";
import { SettingsPage } from "./components/settings/SettingsPage";
import { AiActionModal } from "./components/ai/AiActionModal";
import { QuickPicker } from "./components/quick/QuickPicker";

const IS_QUICK_WINDOW = getCurrentWindow().label === "quick";

function App() {
  // The quick-pick window renders a completely separate compact UI.
  if (IS_QUICK_WINDOW) return <QuickPicker />;
  return <MainWindow />;
}

function MainWindow() {
  const [filter, setFilter] = useState<ItemFilter>("all");
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [aiTarget, setAiTarget] = useState<ClipboardItem | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { items, loading, error, refresh } = useClipboard({ filter, query });
  const { settings, setSettings } = useSettings();

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 2200);
  }, []);

  // Apply the theme setting (system/light/dark) to the whole window.
  // The helper toggles the `dark` class + color-scheme and, for "system",
  // keeps a live prefers-color-scheme listener.
  useEffect(() => {
    if (!settings) return;
    applyTheme(settings.theme);
  }, [settings?.theme]);

  const selected = useMemo(
    () => items[selectedIndex] as ClipboardItem | undefined,
    [items, selectedIndex],
  );

  // Reset selection when the list changes shape.
  useEffect(() => {
    setSelectedIndex((prev) => (prev >= items.length ? 0 : prev));
  }, [items.length]);

  const handleRestore = useCallback(
    async (item: ClipboardItem) => {
      try {
        await restoreItem(item.id);
        showToast(item.type === "image" ? "Image copied" : "Copied to clipboard");
      } catch (e) {
        showToast(`Failed: ${String(e)}`);
      }
    },
    [showToast],
  );

  const handleTogglePin = useCallback(
    async (item: ClipboardItem) => {
      try {
        await setItemPinned(item.id, !item.is_pinned);
        showToast(item.is_pinned ? "Unpinned" : "Pinned");
      } catch (e) {
        showToast(`Failed: ${String(e)}`);
      }
    },
    [showToast],
  );

  const handleToggleFavorite = useCallback(
    async (item: ClipboardItem) => {
      try {
        await setItemFavorite(item.id, !item.is_favorite);
        showToast(item.is_favorite ? "Removed favorite" : "Added to favorites");
      } catch (e) {
        showToast(`Failed: ${String(e)}`);
      }
    },
    [showToast],
  );

  const handleDelete = useCallback(
    async (item: ClipboardItem) => {
      try {
        await deleteItem(item.id);
        showToast("Deleted");
      } catch (e) {
        showToast(`Failed: ${String(e)}`);
      }
    },
    [showToast],
  );

  const handleClearAll = useCallback(async () => {
    try {
      const { deleted } = await clearHistory();
      showToast(`Cleared ${deleted} clip${deleted === 1 ? "" : "s"}`);
    } catch (e) {
      showToast(`Failed: ${String(e)}`);
    }
  }, [showToast]);

  const handleAnalyze = useCallback(
    async (item: ClipboardItem) => {
      if (item.is_sensitive) {
        showToast("AI is blocked for sensitive clips");
        return;
      }
      try {
        await aiAnalyzeItem(item.id);
        showToast("Queued for AI analysis");
        void refresh();
      } catch (e) {
        showToast(`Failed: ${String(e)}`);
      }
    },
    [showToast, refresh],
  );

  const handleSettingsChange = useCallback((next: Settings) => {
    // SettingsPage persists via update_settings; here we only mirror state
    // so the header/theme react immediately (no double IPC write).
    setSettings(next);
  }, [setSettings]);

  // Global keyboard shortcuts (ignored while typing in the search box).
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const target = e.target as HTMLElement;
      const typing = target.tagName === "INPUT" || target.tagName === "TEXTAREA";

      if (e.key === "Escape" && !showSettings && !aiTarget) {
        return; // handled by child views
      }
      if (e.key === "/" && !typing && !showSettings) {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }
      if (typing || showSettings || aiTarget) return;

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) => Math.min(prev + 1, items.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) => Math.max(prev - 1, 0));
          break;
        case "Enter":
          if (selected) {
            e.preventDefault();
            void handleRestore(selected);
          }
          break;
        case "Delete":
          if (selected) {
            e.preventDefault();
            void handleDelete(selected);
          }
          break;
        case "p":
        case "P":
          if (selected) void handleTogglePin(selected);
          break;
        case "f":
        case "F":
          if (selected) void handleToggleFavorite(selected);
          break;
        case "a":
        case "A":
          if (selected && settings?.ai_enabled) void handleAnalyze(selected);
          break;
        case "s":
        case "S":
          setShowSettings(true);
          break;
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    items.length,
    selected,
    showSettings,
    aiTarget,
    settings?.ai_enabled,
    handleRestore,
    handleDelete,
    handleTogglePin,
    handleToggleFavorite,
    handleAnalyze,
  ]);

  if (showSettings && settings) {
    return (
      <SettingsPage
        settings={settings}
        onChange={(next) => void handleSettingsChange(next)}
        onClose={() => {
          setShowSettings(false);
          void refresh();
        }}
      />
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="flex flex-col gap-3 border-b border-border px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <h1 className="text-sm font-semibold tracking-tight">{APP_NAME}</h1>
          <div className="flex items-center gap-2">
            {settings?.pause_monitoring && (
              <span className="rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400">
                paused
              </span>
            )}
            <span className="text-xs text-muted-foreground">
              {items.length} clip{items.length === 1 ? "" : "s"}
            </span>
            <button
              type="button"
              onClick={() => setShowSettings(true)}
              title="Settings (S)"
              className="rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              ⚙ Settings
            </button>
            <button
              type="button"
              onClick={handleClearAll}
              className="rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
            >
              Clear all
            </button>
          </div>
        </div>
        <SearchBar ref={searchRef} value={query} onChange={setQuery} />
        <FilterChips active={filter} onChange={setFilter} />
      </header>

      {/* Body */}
      <main className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {error && (
          <div className="m-4 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </div>
        )}
        {loading ? (
          <div className="flex flex-1 items-center justify-center p-12 text-sm text-muted-foreground">
            Loading…
          </div>
        ) : items.length === 0 ? (
          <EmptyState hasQuery={query.trim().length > 0 || filter !== "all"} />
        ) : (
          <ClipboardList
            items={items}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
            onRestore={(item) => void handleRestore(item)}
            onTogglePin={(item) => void handleTogglePin(item)}
            onToggleFavorite={(item) => void handleToggleFavorite(item)}
            onDelete={(item) => void handleDelete(item)}
            onAi={settings?.ai_enabled ? (item) => setAiTarget(item) : undefined}
          />
        )}
      </main>

      {/* Footer: keyboard hints */}
      <footer className="border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
        <span className="mr-3">↑↓ navigate</span>
        <span className="mr-3">Enter copy</span>
        <span className="mr-3">P pin</span>
        <span className="mr-3">F favorite</span>
        {settings?.ai_enabled && <span className="mr-3">A analyze</span>}
        <span className="mr-3">Del delete</span>
        <span className="mr-3">/ search</span>
        <span>S settings</span>
      </footer>

      {/* AI actions modal */}
      {aiTarget && (
        <AiActionModal
          item={aiTarget}
          onClose={() => {
            setAiTarget(null);
            void refresh();
          }}
          onCopied={() => void refresh()}
        />
      )}

      {/* Toast */}
      {toast && (
        <div className="pointer-events-none fixed bottom-10 left-1/2 -translate-x-1/2 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}

export default App;
