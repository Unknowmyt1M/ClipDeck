import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ClipboardItem, ItemFilter } from "./types";
import { APP_NAME } from "./lib/constants";
import { clearHistory, deleteItem, restoreItem, setItemFavorite, setItemPinned } from "./lib/tauri";
import { useClipboard } from "./features/clipboard";
import { SearchBar } from "./components/search/SearchBar";
import { FilterChips } from "./components/search/FilterChips";
import { ClipboardList } from "./components/clipboard/ClipboardList";
import { EmptyState } from "./components/clipboard/EmptyState";

function App() {
  const [filter, setFilter] = useState<ItemFilter>("all");
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { items, loading, error } = useClipboard({ filter, query });

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 1800);
  }, []);

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

  // Global keyboard shortcuts (ignored while typing in the search box).
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const target = e.target as HTMLElement;
      const typing = target.tagName === "INPUT" || target.tagName === "TEXTAREA";

      if (e.key === "/" && !typing) {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }
      if (typing) return;

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
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [items.length, selected, handleRestore, handleDelete, handleTogglePin, handleToggleFavorite]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="flex flex-col gap-3 border-b border-border px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <h1 className="text-sm font-semibold tracking-tight">{APP_NAME}</h1>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              {items.length} clip{items.length === 1 ? "" : "s"}
            </span>
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
          />
        )}
      </main>

      {/* Footer: keyboard hints */}
      <footer className="border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
        <span className="mr-3">↑↓ navigate</span>
        <span className="mr-3">Enter copy</span>
        <span className="mr-3">P pin</span>
        <span className="mr-3">F favorite</span>
        <span className="mr-3">Del delete</span>
        <span>/ search</span>
      </footer>

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
