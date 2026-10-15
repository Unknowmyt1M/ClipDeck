import { useCallback, useEffect, useRef, useState } from "react";
import type { ClipboardItem, ItemFilter } from "../../types";
import * as tauri from "../../lib/tauri";

interface UseClipboardOptions {
  filter: ItemFilter;
  query: string;
}

/** Load clipboard history, keep it live-synced with the Rust monitor. */
export function useClipboard({ filter, query }: UseClipboardOptions) {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Refs mirror the latest props so the event listener doesn't resubscribe.
  const filterRef = useRef(filter);
  const queryRef = useRef(query);
  filterRef.current = filter;
  queryRef.current = query;

  const refresh = useCallback(async () => {
    try {
      const current = filterRef.current;
      const result = await tauri.getItems({
        typeFilter: current === "all" ? null : current,
        query: queryRef.current,
      });
      setItems(result);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial load + refresh whenever filter/search changes.
  useEffect(() => {
    void refresh();
  }, [refresh, filter, query]);

  // Live updates from the clipboard monitor.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void tauri.onClipboardChanged(() => {
      if (!cancelled) void refresh();
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh]);

  return { items, loading, error, refresh };
}
