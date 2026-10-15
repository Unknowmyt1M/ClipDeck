import { useCallback, useEffect, useState } from "react";
import type { Settings } from "../../types";
import * as tauri from "../../lib/tauri";

/** Load settings, expose a save handler, and keep state live-synced. */
export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSettings(await tauri.getSettings());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = useCallback(async (next: Settings) => {
    const saved = await tauri.updateSettings(next);
    setSettings(saved);
    return saved;
  }, []);

  return { settings, loading, error, refresh, save, setSettings };
}
