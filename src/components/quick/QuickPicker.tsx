import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ClipboardItem } from "../../types";
import * as tauri from "../../lib/tauri";
import { cn, truncate } from "../../lib/utils";

/**
 * Compact Ctrl+Shift+V picker (Phase 2): type → arrow → Enter to paste.
 * Rendered inside the borderless `quick` window.
 */
export function QuickPicker() {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const load = useCallback(async () => {
    try {
      setItems(await tauri.getItems({ query: query || null, limit: 50 }));
    } catch {
      setItems([]);
    }
  }, [query]);

  useEffect(() => {
    void load();
  }, [load]);

  // Live updates while the picker is open.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void tauri.onClipboardChanged(load).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const selected = useMemo(() => items[index], [items, index]);

  const pick = useCallback(async (item: ClipboardItem) => {
    try {
      await tauri.restoreItem(item.id);
      await tauri.hideQuickWindow();
    } catch {
      // keep the window open so the user can retry
    }
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setIndex((i) => Math.min(i + 1, items.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setIndex((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (selected) void pick(selected);
        break;
      case "Escape":
        e.preventDefault();
        void tauri.hideQuickWindow();
        break;
    }
  };

  return (
    <div className="flex h-full flex-col bg-background" onKeyDown={onKeyDown}>
      <div className="border-b border-border p-2.5">
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          placeholder="Search clips…  ↑↓ Enter Esc"
          className="w-full rounded-md border border-input bg-card px-3 py-2 text-sm outline-none focus:border-ring"
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {items.length === 0 ? (
          <p className="p-6 text-center text-xs text-muted-foreground">No clips yet</p>
        ) : (
          items.map((item, i) => (
            <button
              key={item.id}
              type="button"
              onClick={() => void pick(item)}
              onMouseEnter={() => setIndex(i)}
              className={cn(
                "mb-1 block w-full rounded-md px-2.5 py-2 text-left transition-colors",
                i === index ? "bg-accent" : "hover:bg-accent/50",
              )}
            >
              <span className="flex items-center gap-2">
                <span className="inline-flex h-4 shrink-0 items-center rounded bg-muted px-1 text-[9px] font-medium text-muted-foreground uppercase">
                  {item.type}
                </span>
                <span className="min-w-0 flex-1 truncate text-xs text-foreground selectable">
                  {item.preview || truncate(item.content, 120)}
                </span>
                {item.is_pinned && <span className="text-[10px] text-amber-500">pin</span>}
              </span>
            </button>
          ))
        )}
      </div>

      <div className="flex items-center justify-between border-t border-border px-3 py-1.5 text-[10px] text-muted-foreground">
        <span>↑↓ navigate · Enter copy · Esc close</span>
        <span>{items.length} clips</span>
      </div>
    </div>
  );
}
